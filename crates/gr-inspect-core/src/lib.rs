#![deny(unreachable_pub)]

//! Agent-neutral inspection primitives shared by the Goalrail inspection
//! libraries. This crate owns the bounded process runner and the `Verdict`
//! contract only; probe vocabulary, report shapes, and policy stay in the
//! agent-specific crates.

mod process;

pub use process::{ProcessOutput, run_bounded};

use serde::Serialize;

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

/// Render an exit code the way every inspection failure message reports it.
pub fn format_exit_code(exit_code: Option<i32>) -> String {
    match exit_code {
        Some(code) => format!("exit code {code}"),
        None => "termination by signal".to_owned(),
    }
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
    fn formats_exit_codes_and_signal_termination() {
        assert_eq!(format_exit_code(Some(7)), "exit code 7");
        assert_eq!(format_exit_code(Some(0)), "exit code 0");
        assert_eq!(format_exit_code(None), "termination by signal");
    }
}
