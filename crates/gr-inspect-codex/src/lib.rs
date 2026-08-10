#![deny(unreachable_pub)]

mod agents;
mod doctor;
mod features;
mod inspection;
mod marketplaces;
mod mcp;
mod plugins;
mod process;
mod report;
mod skills;
mod use_case;

use std::{io, str::Utf8Error, time::Duration};

use process::run_bounded;
use serde::Serialize;

pub use skills::{SkillsInspectionOutcome, inspect_codex_skills};
pub use use_case::{InspectionOutcome, inspect_codex};

const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Verdict {
    BaselineOk,
    Review,
    Blocked,
    Incomplete,
}

impl Verdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BaselineOk => "BASELINE_OK",
            Self::Review => "REVIEW",
            Self::Blocked => "BLOCKED",
            Self::Incomplete => "INCOMPLETE",
        }
    }

    pub const fn exit_code(self) -> u8 {
        match self {
            Self::BaselineOk => 0,
            Self::Review => 1,
            Self::Blocked => 4,
            Self::Incomplete => 3,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct VersionProbe {
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) exit_code: Option<i32>,
    pub(crate) duration: Duration,
    pub(crate) timed_out: bool,
}

impl VersionProbe {
    pub(crate) fn succeeded(&self) -> bool {
        !self.timed_out && self.exit_code == Some(0)
    }

    pub(crate) fn version_text(&self) -> Result<Option<&str>, Utf8Error> {
        if !self.succeeded() {
            return Ok(None);
        }

        let text = std::str::from_utf8(&self.stdout)?.trim();
        Ok((!text.is_empty()).then_some(text))
    }
}

pub(crate) fn probe_version() -> io::Result<VersionProbe> {
    let output = run_bounded("codex", &["--version"], PROBE_TIMEOUT)?;

    Ok(VersionProbe {
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
    fn verdicts_have_stable_names_and_exit_codes() {
        let cases = [
            (Verdict::BaselineOk, "BASELINE_OK", 0),
            (Verdict::Review, "REVIEW", 1),
            (Verdict::Blocked, "BLOCKED", 4),
            (Verdict::Incomplete, "INCOMPLETE", 3),
        ];

        for (verdict, name, exit_code) in cases {
            assert_eq!(verdict.as_str(), name);
            assert_eq!(verdict.exit_code(), exit_code);
        }
    }

    #[test]
    fn successful_version_probe_exposes_trimmed_utf8_text() {
        let probe = VersionProbe {
            stdout: b"codex-cli 0.147.0\n".to_vec(),
            stderr: Vec::new(),
            exit_code: Some(0),
            duration: Duration::ZERO,
            timed_out: false,
        };

        assert!(probe.succeeded());
        assert_eq!(
            probe
                .version_text()
                .expect("version output should be UTF-8"),
            Some("codex-cli 0.147.0")
        );
    }

    #[test]
    fn failed_version_probe_does_not_expose_version_text() {
        let probe = VersionProbe {
            stdout: b"unexpected output".to_vec(),
            stderr: b"failure".to_vec(),
            exit_code: Some(1),
            duration: Duration::ZERO,
            timed_out: false,
        };

        assert!(!probe.succeeded());
        assert_eq!(
            probe
                .version_text()
                .expect("failed output is not inspected"),
            None
        );
    }
}
