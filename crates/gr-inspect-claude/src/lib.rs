#![deny(unreachable_pub)]

//! Read-only inspection of a local Claude Code installation.
//!
//! The library observes evidence that Claude Code itself exposes: its version,
//! its native JSON plugin and marketplace surfaces, and the configuration files
//! documented under the resolved Claude home. It never installs, enables,
//! disables, or writes anything, and it reports missing evidence explicitly
//! instead of treating it as success.

mod local;
mod probes;
mod report;
mod use_case;

use std::time::Duration;

pub use use_case::{InspectionOutcome, inspect_claude};

const PROBE_TIMEOUT: Duration = Duration::from_secs(15);
