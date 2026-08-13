use std::{collections::BTreeMap, io, time::Duration};

use serde::Deserialize;

use gr_inspect_core::run_bounded;

use crate::PROBE_TIMEOUT;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DoctorReport {
    pub(crate) schema_version: u32,
    pub(crate) overall_status: String,
    pub(crate) codex_version: String,
    pub(crate) checks: BTreeMap<String, DoctorCheck>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DoctorCheck {
    pub(crate) id: String,
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) summary: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DoctorProbe {
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) exit_code: Option<i32>,
    pub(crate) duration: Duration,
    pub(crate) timed_out: bool,
}

impl DoctorProbe {
    pub(crate) fn report(&self) -> Result<DoctorReport, serde_json::Error> {
        serde_json::from_slice(&self.stdout)
    }
}

#[cfg(test)]
fn parse_doctor(json: &str) -> Result<DoctorReport, serde_json::Error> {
    serde_json::from_str(json)
}

pub(crate) fn probe_doctor() -> io::Result<DoctorProbe> {
    let output = run_bounded("codex", &["doctor", "--json"], PROBE_TIMEOUT)?;

    Ok(DoctorProbe {
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
    fn parses_doctor_report() {
        let json = r#"{
            "schemaVersion": 1,
            "overallStatus": "ok",
            "codexVersion": "0.147.0",
            "checks": {
                "config.load": {
                    "id": "config.load",
                    "status": "ok"
                }
            }
        }"#;

        let report = parse_doctor(json).expect("doctor report should parse");

        assert_eq!(report.schema_version, 1);
        assert_eq!(report.overall_status, "ok");
        assert_eq!(report.codex_version, "0.147.0");
        assert_eq!(report.checks.len(), 1);
        let check = report
            .checks
            .get("config.load")
            .expect("config.load check should exist");
        assert_eq!(check.id, "config.load");
        assert_eq!(check.status, "ok");
    }
}
