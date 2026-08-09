use std::{io, time::Duration};

use serde::Deserialize;

use crate::{PROBE_TIMEOUT, process::run_bounded};

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PluginReport {
    pub(crate) installed: Vec<Plugin>,
    pub(crate) available: Vec<Plugin>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Plugin {
    pub(crate) plugin_id: String,
    pub(crate) name: String,
    pub(crate) marketplace_name: String,
    pub(crate) version: String,
    pub(crate) installed: bool,
    pub(crate) enabled: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PluginProbe {
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) exit_code: Option<i32>,
    pub(crate) duration: Duration,
    pub(crate) timed_out: bool,
}

impl PluginProbe {
    pub(crate) fn succeeded(&self) -> bool {
        !self.timed_out && self.exit_code == Some(0)
    }

    pub(crate) fn report(&self) -> Result<PluginReport, serde_json::Error> {
        serde_json::from_slice(&self.stdout)
    }
}

#[cfg(test)]
fn parse_plugins(json: &str) -> Result<PluginReport, serde_json::Error> {
    serde_json::from_str(json)
}

pub(crate) fn probe_plugins() -> io::Result<PluginProbe> {
    let output = run_bounded("codex", &["plugin", "list", "--json"], PROBE_TIMEOUT)?;

    Ok(PluginProbe {
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
    fn succeeds_only_for_zero_exit_without_timeout() {
        let mut probe = PluginProbe {
            stdout: Vec::new(),
            stderr: Vec::new(),
            exit_code: Some(0),
            duration: Duration::ZERO,
            timed_out: false,
        };

        assert!(probe.succeeded());
        probe.timed_out = true;
        assert!(!probe.succeeded());
        probe.timed_out = false;
        probe.exit_code = Some(1);
        assert!(!probe.succeeded());
    }

    #[test]
    fn parses_plugin_report() {
        let json = r#"{
            "installed": [
                {
                    "pluginId": "github@openai-curated",
                    "name": "github",
                    "marketplaceName": "openai-curated",
                    "version": "0.1.8",
                    "installed": true,
                    "enabled": true,
                    "authPolicy": "ON_INSTALL"
                }
            ],
            "available": []
        }"#;

        let report = parse_plugins(json).expect("plugin report should parse");

        assert_eq!(report.installed.len(), 1);
        assert!(report.available.is_empty());
        assert_eq!(report.installed[0].plugin_id, "github@openai-curated");
        assert_eq!(report.installed[0].name, "github");
        assert_eq!(report.installed[0].marketplace_name, "openai-curated");
        assert_eq!(report.installed[0].version, "0.1.8");
        assert!(report.installed[0].installed);
        assert!(report.installed[0].enabled);
    }
}
