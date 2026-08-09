use std::{io, time::Duration};

use crate::{PROBE_TIMEOUT, process::run_bounded};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct FeaturesProbe {
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) exit_code: Option<i32>,
    pub(crate) duration: Duration,
    pub(crate) timed_out: bool,
}

impl FeaturesProbe {
    pub(crate) fn succeeded(&self) -> bool {
        !self.timed_out && self.exit_code == Some(0)
    }

    pub(crate) fn text(&self) -> Option<&str> {
        if !self.succeeded() {
            return None;
        }

        std::str::from_utf8(&self.stdout).ok().map(str::trim)
    }
}

pub(crate) fn probe_features() -> io::Result<FeaturesProbe> {
    let output = run_bounded("codex", &["features", "list"], PROBE_TIMEOUT)?;

    Ok(FeaturesProbe {
        stdout: output.stdout,
        stderr: output.stderr,
        exit_code: output.exit_code,
        duration: output.duration,
        timed_out: output.timed_out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_probe_exposes_trimmed_text() {
        let probe = FeaturesProbe {
            stdout: b"apps  stable  true\nplugins  stable  true\n".to_vec(),
            stderr: Vec::new(),
            exit_code: Some(0),
            duration: Duration::ZERO,
            timed_out: false,
        };

        assert!(probe.succeeded());
        assert_eq!(
            probe.text(),
            Some("apps  stable  true\nplugins  stable  true")
        );
    }

    #[test]
    fn failed_probe_does_not_expose_text() {
        let probe = FeaturesProbe {
            stdout: b"unexpected output".to_vec(),
            stderr: b"failure".to_vec(),
            exit_code: Some(1),
            duration: Duration::ZERO,
            timed_out: false,
        };

        assert!(!probe.succeeded());
        assert_eq!(probe.text(), None);
    }

    #[test]
    fn successful_empty_output_represents_zero_feature_rows() {
        let probe = FeaturesProbe {
            stdout: Vec::new(),
            stderr: Vec::new(),
            exit_code: Some(0),
            duration: Duration::ZERO,
            timed_out: false,
        };

        assert_eq!(probe.text(), Some(""));
    }
}
