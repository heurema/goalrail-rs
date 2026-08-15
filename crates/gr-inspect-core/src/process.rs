use std::{
    io::{self, Read},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread,
    time::{Duration, Instant},
};

const OUTPUT_DRAIN_GRACE: Duration = Duration::from_millis(100);
const POLL_INTERVAL: Duration = Duration::from_millis(10);
const READ_BUFFER_SIZE: usize = 8_192;

/// Bytes observed from one output stream and whether the reader reached EOF.
#[derive(Debug, PartialEq, Eq)]
struct DrainedStream {
    bytes: Vec<u8>,
    complete: bool,
}

enum ReaderEvent {
    Chunk(Vec<u8>),
    Finished(io::Result<()>),
}

type StreamReader = Receiver<ReaderEvent>;

#[derive(Debug)]
pub struct ProcessOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: Option<i32>,
    pub duration: Duration,
    pub timed_out: bool,
}

pub fn run_bounded(program: &str, args: &[&str], timeout: Duration) -> io::Result<ProcessOutput> {
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

    let (stdout, stderr) = drain_readers(
        &stdout_reader,
        &stderr_reader,
        started_at,
        timed_out,
        timeout,
    )?;
    timed_out |= output_drain_incomplete(&stdout, &stderr);

    Ok(ProcessOutput {
        stdout: stdout.bytes,
        stderr: stderr.bytes,
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
    let grace_deadline = drain_started_at + OUTPUT_DRAIN_GRACE;
    if timed_out {
        grace_deadline
    } else {
        (started_at + timeout).max(grace_deadline)
    }
}

/// Collect both output streams, giving each reader its own drain window.
///
/// A single shared deadline lets a slow first read consume the whole budget and
/// starve the second stream. Each wait therefore gets its own window, which
/// keeps the total bounded at the timeout plus one grace period per stream.
fn drain_readers(
    stdout_reader: &StreamReader,
    stderr_reader: &StreamReader,
    started_at: Instant,
    timed_out: bool,
    timeout: Duration,
) -> io::Result<(DrainedStream, DrainedStream)> {
    let stdout = receive_reader(
        stdout_reader,
        output_drain_deadline(started_at, timed_out, timeout, Instant::now()),
    )?;
    let stderr = receive_reader(
        stderr_reader,
        output_drain_deadline(started_at, timed_out, timeout, Instant::now()),
    )?;

    Ok((stdout, stderr))
}

fn output_drain_incomplete(stdout: &DrainedStream, stderr: &DrainedStream) -> bool {
    !stdout.complete || !stderr.complete
}

fn read_stream(mut stream: impl Read, sender: Sender<ReaderEvent>) {
    let mut buffer = [0; READ_BUFFER_SIZE];

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                let _ = sender.send(ReaderEvent::Finished(Ok(())));
                return;
            }
            Ok(read) => {
                if sender
                    .send(ReaderEvent::Chunk(buffer[..read].to_vec()))
                    .is_err()
                {
                    return;
                }
            }
            Err(error) => {
                let _ = sender.send(ReaderEvent::Finished(Err(error)));
                return;
            }
        }
    }
}

fn spawn_reader(stream: impl Read + Send + 'static) -> StreamReader {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        read_stream(stream, sender);
    });
    receiver
}

fn receive_reader(receiver: &StreamReader, deadline: Instant) -> io::Result<DrainedStream> {
    let mut bytes = Vec::new();

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(DrainedStream {
                bytes,
                complete: false,
            });
        }
        match receiver.recv_timeout(remaining) {
            Ok(ReaderEvent::Chunk(chunk)) => bytes.extend(chunk),
            Ok(ReaderEvent::Finished(Ok(()))) => {
                return Ok(DrainedStream {
                    bytes,
                    complete: true,
                });
            }
            Ok(ReaderEvent::Finished(Err(error))) => return Err(error),
            Err(RecvTimeoutError::Timeout) => {
                return Ok(DrainedStream {
                    bytes,
                    complete: false,
                });
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err(io::Error::other("process output reader panicked"));
            }
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
        let timeout = Duration::from_secs(1);
        let early_drain = started_at + Duration::from_millis(10);
        let late_drain = started_at + timeout;

        assert_eq!(
            output_drain_deadline(started_at, false, timeout, early_drain),
            started_at + timeout
        );
        assert_eq!(
            output_drain_deadline(started_at, false, timeout, late_drain),
            late_drain + OUTPUT_DRAIN_GRACE
        );
        assert_eq!(
            output_drain_deadline(started_at, true, timeout, late_drain),
            late_drain + OUTPUT_DRAIN_GRACE
        );
    }

    #[test]
    fn a_slow_first_stream_does_not_starve_the_second() {
        let (slow_sender, slow_reader) = mpsc::channel();
        let (prompt_sender, prompt_reader) = mpsc::channel();
        thread::spawn(move || {
            thread::sleep(OUTPUT_DRAIN_GRACE * 3);
            let _ = slow_sender.send(ReaderEvent::Finished(Ok(())));
        });
        prompt_sender
            .send(ReaderEvent::Chunk(b"stderr".to_vec()))
            .expect("the second stream is ready before the drain starts");
        prompt_sender
            .send(ReaderEvent::Finished(Ok(())))
            .expect("the second stream should finish before the drain starts");

        let started_at = Instant::now();
        let (stdout, stderr) = drain_readers(
            &slow_reader,
            &prompt_reader,
            started_at,
            false,
            Duration::ZERO,
        )
        .expect("draining should not fail");

        assert_eq!(
            stdout,
            DrainedStream {
                bytes: Vec::new(),
                complete: false,
            }
        );
        assert_eq!(
            stderr,
            DrainedStream {
                bytes: b"stderr".to_vec(),
                complete: true,
            }
        );
    }

    #[test]
    fn treats_either_missing_output_reader_as_incomplete() {
        let present = DrainedStream {
            bytes: Vec::new(),
            complete: true,
        };
        let incomplete = DrainedStream {
            bytes: Vec::new(),
            complete: false,
        };

        assert!(!output_drain_incomplete(&present, &present));
        assert!(output_drain_incomplete(&incomplete, &present));
        assert!(output_drain_incomplete(&present, &incomplete));
        assert!(output_drain_incomplete(&incomplete, &incomplete));
    }

    #[test]
    fn retains_partial_output_when_the_reader_misses_the_deadline() {
        let (sender, receiver) = mpsc::channel();
        sender
            .send(ReaderEvent::Chunk(b"ready".to_vec()))
            .expect("the reader should receive output before its deadline");
        thread::spawn(move || {
            thread::sleep(OUTPUT_DRAIN_GRACE * 3);
            let _ = sender.send(ReaderEvent::Finished(Ok(())));
        });

        let output = receive_reader(&receiver, Instant::now() + OUTPUT_DRAIN_GRACE)
            .expect("partial output should remain available");

        assert_eq!(output.bytes, b"ready");
        assert!(!output.complete);
    }

    #[test]
    fn does_not_read_queued_output_after_the_deadline() {
        let (sender, receiver) = mpsc::channel();
        sender
            .send(ReaderEvent::Chunk(b"late".to_vec()))
            .expect("the test should queue the late chunk");

        let output = receive_reader(&receiver, Instant::now())
            .expect("an expired reader deadline should remain bounded");

        assert!(output.bytes.is_empty());
        assert!(!output.complete);
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
