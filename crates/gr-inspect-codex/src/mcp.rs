use std::{io, time::Duration};

use serde::Deserialize;

use gr_inspect_core::run_bounded;

use crate::PROBE_TIMEOUT;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub(crate) struct McpReport {
    pub(crate) servers: Vec<McpServer>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct McpServer {
    pub(crate) name: String,
    pub(crate) enabled: bool,
    pub(crate) transport: McpTransport,
    pub(crate) auth_status: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct McpTransport {
    #[serde(rename = "type")]
    pub(crate) kind: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct McpProbe {
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) exit_code: Option<i32>,
    pub(crate) duration: Duration,
    pub(crate) timed_out: bool,
}

impl McpProbe {
    pub(crate) fn succeeded(&self) -> bool {
        !self.timed_out && self.exit_code == Some(0)
    }

    pub(crate) fn report(&self) -> Result<McpReport, serde_json::Error> {
        serde_json::from_slice(&self.stdout)
    }
}

#[cfg(test)]
fn parse_mcp(json: &str) -> Result<McpReport, serde_json::Error> {
    serde_json::from_str(json)
}

pub(crate) fn probe_mcp() -> io::Result<McpProbe> {
    let output = run_bounded("codex", &["mcp", "list", "--json"], PROBE_TIMEOUT)?;

    Ok(McpProbe {
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
        let mut probe = McpProbe {
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
    fn parses_mcp_report_without_transport_details() {
        let json = r#"[
            {
                "name": "local-tools",
                "enabled": true,
                "disabled_reason": null,
                "transport": {
                    "type": "stdio",
                    "command": "/usr/local/bin/local-tools",
                    "args": []
                },
                "auth_status": "unsupported"
            },
            {
                "name": "remote-tools",
                "enabled": false,
                "transport": {
                    "type": "streamable_http",
                    "url": "https://example.invalid/mcp"
                },
                "auth_status": "not_authenticated"
            }
        ]"#;

        let report = parse_mcp(json).expect("MCP report should parse");

        assert_eq!(report.servers.len(), 2);
        assert_eq!(report.servers[0].name, "local-tools");
        assert!(report.servers[0].enabled);
        assert_eq!(report.servers[0].transport.kind, "stdio");
        assert_eq!(report.servers[0].auth_status, "unsupported");
        assert!(!report.servers[1].enabled);
        assert_eq!(report.servers[1].transport.kind, "streamable_http");
    }
}
