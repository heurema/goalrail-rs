use std::{io, time::Duration};

use serde::Deserialize;

use crate::{PROBE_TIMEOUT, process::run_bounded};

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct MarketplaceReport {
    pub(crate) marketplaces: Vec<Marketplace>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Marketplace {
    pub(crate) name: String,
    pub(crate) root: String,
    pub(crate) marketplace_source: Option<MarketplaceSource>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MarketplaceSource {
    pub(crate) source_type: String,
    pub(crate) source: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct MarketplaceProbe {
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) exit_code: Option<i32>,
    pub(crate) duration: Duration,
    pub(crate) timed_out: bool,
}

impl MarketplaceProbe {
    pub(crate) fn succeeded(&self) -> bool {
        !self.timed_out && self.exit_code == Some(0)
    }

    pub(crate) fn report(&self) -> Result<MarketplaceReport, serde_json::Error> {
        serde_json::from_slice(&self.stdout)
    }
}

#[cfg(test)]
fn parse_marketplaces(json: &str) -> Result<MarketplaceReport, serde_json::Error> {
    serde_json::from_str(json)
}

pub(crate) fn probe_marketplaces() -> io::Result<MarketplaceProbe> {
    let output = run_bounded(
        "codex",
        &["plugin", "marketplace", "list", "--json"],
        PROBE_TIMEOUT,
    )?;

    Ok(MarketplaceProbe {
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
        let mut probe = MarketplaceProbe {
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
    fn parses_marketplace_report_with_optional_source() {
        let json = r#"{
            "marketplaces": [
                {
                    "name": "openai-bundled",
                    "root": "/tmp/openai-bundled",
                    "marketplaceSource": {
                        "sourceType": "local",
                        "source": "/tmp/openai-bundled"
                    }
                },
                {
                    "name": "openai-curated",
                    "root": "/tmp/openai-curated"
                }
            ]
        }"#;

        let report = parse_marketplaces(json).expect("marketplace report should parse");

        assert_eq!(report.marketplaces.len(), 2);
        assert_eq!(report.marketplaces[0].name, "openai-bundled");
        assert_eq!(report.marketplaces[0].root, "/tmp/openai-bundled");
        let source = report.marketplaces[0]
            .marketplace_source
            .as_ref()
            .expect("marketplace source should exist");
        assert_eq!(source.source_type, "local");
        assert_eq!(source.source, "/tmp/openai-bundled");
        assert!(report.marketplaces[1].marketplace_source.is_none());
    }
}
