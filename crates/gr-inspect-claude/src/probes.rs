use std::{io, path::PathBuf, str::Utf8Error};

use gr_inspect_core::{ProcessOutput, run_bounded};
use serde::Deserialize;

use crate::PROBE_TIMEOUT;

/// One installed plugin as reported by `claude plugin list --json`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstalledPlugin {
    pub(crate) id: String,
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) scope: Option<String>,
    #[serde(default)]
    pub(crate) install_path: Option<PathBuf>,
}

/// One configured marketplace as reported by
/// `claude plugin marketplace list --json`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub(crate) struct Marketplace {
    #[allow(dead_code)]
    pub(crate) name: String,
}

pub(crate) fn version_text(output: &ProcessOutput) -> Result<Option<&str>, Utf8Error> {
    if !succeeded(output) {
        return Ok(None);
    }

    let text = std::str::from_utf8(&output.stdout)?.trim();
    Ok((!text.is_empty()).then_some(text))
}

pub(crate) const fn succeeded(output: &ProcessOutput) -> bool {
    !output.timed_out && matches!(output.exit_code, Some(0))
}

pub(crate) fn parse_plugins(stdout: &[u8]) -> Result<Vec<InstalledPlugin>, serde_json::Error> {
    serde_json::from_slice(stdout)
}

pub(crate) fn parse_marketplaces(stdout: &[u8]) -> Result<Vec<Marketplace>, serde_json::Error> {
    serde_json::from_slice(stdout)
}

pub(crate) fn probe_version() -> io::Result<ProcessOutput> {
    run_bounded("claude", &["--version"], PROBE_TIMEOUT)
}

pub(crate) fn probe_plugins() -> io::Result<ProcessOutput> {
    run_bounded("claude", &["plugin", "list", "--json"], PROBE_TIMEOUT)
}

pub(crate) fn probe_marketplaces() -> io::Result<ProcessOutput> {
    run_bounded(
        "claude",
        &["plugin", "marketplace", "list", "--json"],
        PROBE_TIMEOUT,
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn output(stdout: &[u8], exit_code: Option<i32>, timed_out: bool) -> ProcessOutput {
        ProcessOutput {
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
            exit_code,
            duration: Duration::ZERO,
            timed_out,
        }
    }

    #[test]
    fn succeeds_only_for_zero_exit_without_timeout() {
        assert!(succeeded(&output(b"", Some(0), false)));
        assert!(!succeeded(&output(b"", Some(0), true)));
        assert!(!succeeded(&output(b"", Some(1), false)));
        assert!(!succeeded(&output(b"", None, false)));
    }

    #[test]
    fn successful_version_probe_exposes_trimmed_text() {
        let probe = output(b"2.1.226 (Claude Code)\n", Some(0), false);

        assert_eq!(
            version_text(&probe).expect("version output should be UTF-8"),
            Some("2.1.226 (Claude Code)")
        );
    }

    #[test]
    fn failed_or_empty_version_probe_exposes_no_text() {
        let failed = output(b"2.1.226 (Claude Code)", Some(1), false);
        assert_eq!(
            version_text(&failed).expect("failed output is not inspected"),
            None
        );

        let empty = output(b"\n", Some(0), false);
        assert_eq!(
            version_text(&empty).expect("empty output should be UTF-8"),
            None
        );
    }

    #[test]
    fn invalid_utf8_version_output_is_reported_as_an_error() {
        let probe = output(b"\xff", Some(0), false);

        assert!(version_text(&probe).is_err());
    }

    #[test]
    fn parses_the_installed_plugin_inventory() {
        let plugins = parse_plugins(
            br#"[
                {
                    "id": "alpha@market",
                    "version": "1.0.0",
                    "scope": "user",
                    "enabled": true,
                    "installPath": "/tmp/alpha",
                    "installedAt": "2026-01-01T00:00:00.000Z"
                },
                {
                    "id": "beta@market",
                    "enabled": false
                }
            ]"#,
        )
        .expect("plugin inventory should parse");

        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].id, "alpha@market");
        assert!(plugins[0].enabled);
        assert_eq!(plugins[0].scope.as_deref(), Some("user"));
        assert_eq!(plugins[0].install_path, Some(PathBuf::from("/tmp/alpha")));
        assert!(!plugins[1].enabled);
        assert_eq!(plugins[1].scope, None);
        assert_eq!(plugins[1].install_path, None);
    }

    #[test]
    fn rejects_a_plugin_inventory_without_an_enabled_state() {
        assert!(parse_plugins(br#"[{"id": "alpha@market"}]"#).is_err());
        assert!(parse_plugins(br#"{"installed": []}"#).is_err());
    }

    #[test]
    fn parses_configured_marketplaces() {
        let marketplaces = parse_marketplaces(
            br#"[
                {
                    "name": "openai-codex",
                    "source": "github",
                    "repo": "openai/codex-plugin-cc",
                    "installLocation": "/tmp/openai-codex"
                },
                { "name": "local-market", "source": "local" }
            ]"#,
        )
        .expect("marketplace list should parse");

        assert_eq!(marketplaces.len(), 2);
        assert_eq!(marketplaces[0].name, "openai-codex");
        assert_eq!(marketplaces[1].name, "local-market");
    }

    #[test]
    fn rejects_a_marketplace_list_without_names() {
        assert!(parse_marketplaces(br#"[{"source": "github"}]"#).is_err());
    }
}
