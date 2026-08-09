use std::{
    io::{self, Read},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    thread,
    time::{Duration, Instant},
};

const OUTPUT_DRAIN_GRACE: Duration = Duration::from_millis(100);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug)]
pub(crate) struct ProcessOutput {
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) exit_code: Option<i32>,
    pub(crate) duration: Duration,
    pub(crate) timed_out: bool,
}

pub(crate) fn run_bounded(
    program: &str,
    args: &[&str],
    timeout: Duration,
) -> io::Result<ProcessOutput> {
    let started_at = Instant::now();
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("child stdout was not captured"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("child stderr was not captured"))?;

    let stdout_reader = spawn_reader(stdout);
    let stderr_reader = spawn_reader(stderr);

    let (status, mut timed_out) = loop {
        if let Some(status) = child.try_wait()? {
            break (status, false);
        }

        let elapsed = started_at.elapsed();
        if elapsed >= timeout {
            child.kill()?;
            break (child.wait()?, true);
        }

        thread::sleep(poll_delay(timeout, elapsed));
    };

    let drain_deadline = output_drain_deadline(started_at, timed_out, timeout, Instant::now());
    let stdout = receive_reader(&stdout_reader, drain_deadline)?;
    let stderr = receive_reader(&stderr_reader, drain_deadline)?;
    timed_out |= output_drain_incomplete(&stdout, &stderr);

    Ok(ProcessOutput {
        stdout: stdout.unwrap_or_default(),
        stderr: stderr.unwrap_or_default(),
        exit_code: status.code(),
        duration: started_at.elapsed(),
        timed_out,
    })
}

fn poll_delay(timeout: Duration, elapsed: Duration) -> Duration {
    POLL_INTERVAL.min(timeout.saturating_sub(elapsed))
}

fn output_drain_deadline(
    started_at: Instant,
    timed_out: bool,
    timeout: Duration,
    drain_started_at: Instant,
) -> Instant {
    if timed_out {
        drain_started_at + OUTPUT_DRAIN_GRACE
    } else {
        started_at + timeout
    }
}

fn output_drain_incomplete(stdout: &Option<Vec<u8>>, stderr: &Option<Vec<u8>>) -> bool {
    stdout.is_none() || stderr.is_none()
}

fn read_all(mut stream: impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn spawn_reader(stream: impl Read + Send + 'static) -> Receiver<io::Result<Vec<u8>>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(read_all(stream));
    });
    receiver
}

fn receive_reader(
    receiver: &Receiver<io::Result<Vec<u8>>>,
    deadline: Instant,
) -> io::Result<Option<Vec<u8>>> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    match receiver.recv_timeout(remaining) {
        Ok(result) => result.map(Some),
        Err(RecvTimeoutError::Timeout) => Ok(None),
        Err(RecvTimeoutError::Disconnected) => {
            Err(io::Error::other("process output reader panicked"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_polling_to_the_remaining_timeout() {
        assert_eq!(
            poll_delay(Duration::from_millis(8), Duration::from_millis(3)),
            Duration::from_millis(5)
        );
        assert_eq!(
            poll_delay(Duration::from_millis(20), Duration::from_millis(3)),
            POLL_INTERVAL
        );
    }

    #[test]
    fn selects_the_deadline_for_normal_and_timed_out_processes() {
        let started_at = Instant::now();
        let drain_started_at = started_at + Duration::from_secs(2);
        let timeout = Duration::from_secs(1);

        assert_eq!(
            output_drain_deadline(started_at, false, timeout, drain_started_at),
            started_at + timeout
        );
        assert_eq!(
            output_drain_deadline(started_at, true, timeout, drain_started_at),
            drain_started_at + OUTPUT_DRAIN_GRACE
        );
    }

    #[test]
    fn treats_either_missing_output_reader_as_incomplete() {
        let present = Some(Vec::new());
        let missing = None;

        assert!(!output_drain_incomplete(&present, &present));
        assert!(output_drain_incomplete(&missing, &present));
        assert!(output_drain_incomplete(&present, &missing));
        assert!(output_drain_incomplete(&missing, &missing));
    }

    #[test]
    fn captures_stdout_stderr_and_exit_code() {
        let output = run_bounded(
            "/bin/sh",
            &["-c", "printf stdout; printf stderr >&2; exit 7"],
            Duration::from_secs(1),
        )
        .expect("fixture command should run");

        assert_eq!(output.stdout, b"stdout");
        assert_eq!(output.stderr, b"stderr");
        assert_eq!(output.exit_code, Some(7));
        assert!(!output.timed_out);
    }

    #[test]
    fn kills_a_command_after_timeout() {
        let output = run_bounded(
            "/bin/sh",
            &["-c", "exec sleep 2"],
            Duration::from_millis(50),
        )
        .expect("fixture command should be killed");

        assert!(output.timed_out);
        assert!(output.duration < Duration::from_secs(1));
    }

    #[test]
    fn does_not_wait_for_a_descendant_holding_the_output_pipe() {
        let output = run_bounded(
            "/bin/sh",
            &["-c", "sleep 2 & exit 0"],
            Duration::from_millis(50),
        )
        .expect("fixture command should return within the bound");

        assert!(output.timed_out);
        assert!(output.duration < Duration::from_secs(1));
    }

    #[test]
    fn drains_stdout_and_stderr_larger_than_pipe_buffers() {
        let output = run_bounded(
            "/bin/sh",
            &[
                "-c",
                "i=0; while [ $i -lt 10000 ]; do printf 'stdout-line\n'; printf 'stderr-line\n' >&2; i=$((i + 1)); done",
            ],
            Duration::from_secs(5),
        )
        .expect("fixture output should be drained concurrently");

        assert_eq!(output.exit_code, Some(0));
        assert!(!output.timed_out);
        assert_eq!(output.stdout.len(), 120_000);
        assert_eq!(output.stderr.len(), 120_000);
    }

    #[test]
    fn preserves_output_written_before_timeout() {
        let output = run_bounded(
            "/bin/sh",
            &["-c", "printf ready; exec sleep 2"],
            Duration::from_millis(50),
        )
        .expect("fixture command should be killed after producing output");

        assert!(output.timed_out);
        assert_eq!(output.stdout, b"ready");
    }
}
