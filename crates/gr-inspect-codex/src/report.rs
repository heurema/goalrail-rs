use std::path::PathBuf;

use serde::Serialize;

use crate::{
    Verdict,
    agents::{AgentInspection, InstructionSource, ProjectTrust},
    doctor::DoctorReport,
};

pub(crate) const REPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ReportKind {
    Summary,
    Skills,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Drilldown {
    pub(crate) section: String,
    pub(crate) argv: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ProjectTrustState {
    Trusted,
    Untrusted,
    Unconfigured,
}

impl ProjectTrustState {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Trusted => "trusted",
            Self::Untrusted => "untrusted",
            Self::Unconfigured => "unconfigured",
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DoctorSummary {
    pub(crate) status: String,
    pub(crate) check_count: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FeatureSummary {
    pub(crate) observed_rows: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PluginSummary {
    pub(crate) installed: usize,
    pub(crate) enabled: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillSummary {
    pub(crate) active: usize,
    pub(crate) by_origin: SkillOriginSummary,
    pub(crate) catalog_errors: usize,
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillOriginSummary {
    pub(crate) personal: usize,
    pub(crate) plugin: usize,
    pub(crate) system: usize,
    pub(crate) project: usize,
    pub(crate) admin: usize,
    pub(crate) unknown: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CountSummary {
    pub(crate) configured: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpSummary {
    pub(crate) configured: usize,
    pub(crate) enabled: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectSummary {
    pub(crate) root: Option<PathBuf>,
    pub(crate) trust: ProjectTrustState,
    pub(crate) config_layers: Vec<PathBuf>,
    pub(crate) instruction_sources: Vec<InstructionSource>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct ReportFinding {
    pub(crate) code: String,
    pub(crate) status: String,
    pub(crate) message: String,
}

impl ReportFinding {
    pub(crate) fn new(
        code: impl Into<String>,
        status: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            status: status.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexFailureReport {
    pub(crate) schema_version: u32,
    pub(crate) kind: ReportKind,
    pub(crate) verdict: Verdict,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) drilldowns: Vec<Drilldown>,
    pub(crate) findings: Vec<ReportFinding>,
}

impl CodexFailureReport {
    pub(crate) fn new(kind: ReportKind, verdict: Verdict, finding: ReportFinding) -> Self {
        Self {
            schema_version: REPORT_SCHEMA_VERSION,
            kind,
            verdict,
            drilldowns: if kind == ReportKind::Summary {
                skill_drilldowns()
            } else {
                Vec::new()
            },
            findings: vec![finding],
        }
    }

    pub(crate) fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexInspectionReport {
    pub(crate) schema_version: u32,
    pub(crate) kind: ReportKind,
    pub(crate) verdict: Verdict,
    pub(crate) codex_version: String,
    pub(crate) doctor: DoctorSummary,
    pub(crate) features: FeatureSummary,
    pub(crate) plugins: PluginSummary,
    pub(crate) skills: SkillSummary,
    pub(crate) marketplaces: CountSummary,
    pub(crate) mcp: McpSummary,
    pub(crate) project: ProjectSummary,
    pub(crate) drilldowns: Vec<Drilldown>,
    pub(crate) findings: Vec<ReportFinding>,
}

impl CodexInspectionReport {
    pub(crate) fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

pub(crate) struct CodexInspectionFacts<'a> {
    pub(crate) version: &'a str,
    pub(crate) doctor: &'a DoctorReport,
    pub(crate) feature_count: usize,
    pub(crate) installed_plugin_count: usize,
    pub(crate) enabled_plugin_count: usize,
    pub(crate) active_skill_count: usize,
    pub(crate) skill_origins: SkillOriginSummary,
    pub(crate) skill_catalog_error_count: usize,
    pub(crate) marketplace_count: usize,
    pub(crate) mcp_count: usize,
    pub(crate) enabled_mcp_count: usize,
    pub(crate) agents: &'a AgentInspection,
}

pub(crate) fn synthesize_report(facts: CodexInspectionFacts<'_>) -> CodexInspectionReport {
    let mut findings = Vec::new();
    let schema_supported = facts.doctor.schema_version == REPORT_SCHEMA_VERSION;
    let checks_present = !facts.doctor.checks.is_empty();

    if !schema_supported {
        findings.push(ReportFinding::new(
            "doctor.schema_version",
            "unknown",
            format!(
                "unsupported Codex doctor schema version {}",
                facts.doctor.schema_version
            ),
        ));
    }

    if !checks_present {
        findings.push(ReportFinding::new(
            "doctor.checks",
            "missing",
            "Codex doctor returned no checks",
        ));
    }

    for (key, check) in &facts.doctor.checks {
        if check.status == "ok" {
            continue;
        }

        findings.push(ReportFinding::new(
            format!("doctor.{key}"),
            check.status.clone(),
            check.summary.clone().unwrap_or_else(|| {
                format!("Codex doctor check {} reported {}", check.id, check.status)
            }),
        ));
    }

    if facts.doctor.overall_status != "ok" && findings.is_empty() {
        findings.push(ReportFinding::new(
            "doctor.overall_status",
            facts.doctor.overall_status.clone(),
            format!(
                "Codex doctor reported overall status {}",
                facts.doctor.overall_status
            ),
        ));
    }

    if facts.skill_catalog_error_count > 0 {
        findings.push(ReportFinding::new(
            "skills.catalog",
            "partial",
            format!(
                "Codex skills catalog reported {} errors",
                facts.skill_catalog_error_count
            ),
        ));
    }

    let verdict = if !schema_supported || !checks_present {
        Verdict::Incomplete
    } else if facts.doctor.overall_status == "ok" && findings.is_empty() {
        Verdict::BaselineOk
    } else {
        Verdict::Review
    };

    let project_trust = match facts.agents.discovery.project_trust {
        Some(ProjectTrust::Trusted) => ProjectTrustState::Trusted,
        Some(ProjectTrust::Untrusted) => ProjectTrustState::Untrusted,
        None => ProjectTrustState::Unconfigured,
    };

    CodexInspectionReport {
        schema_version: REPORT_SCHEMA_VERSION,
        kind: ReportKind::Summary,
        verdict,
        codex_version: facts.version.to_owned(),
        doctor: DoctorSummary {
            status: facts.doctor.overall_status.clone(),
            check_count: facts.doctor.checks.len(),
        },
        features: FeatureSummary {
            observed_rows: facts.feature_count,
        },
        plugins: PluginSummary {
            installed: facts.installed_plugin_count,
            enabled: facts.enabled_plugin_count,
        },
        skills: SkillSummary {
            active: facts.active_skill_count,
            by_origin: facts.skill_origins,
            catalog_errors: facts.skill_catalog_error_count,
        },
        marketplaces: CountSummary {
            configured: facts.marketplace_count,
        },
        mcp: McpSummary {
            configured: facts.mcp_count,
            enabled: facts.enabled_mcp_count,
        },
        project: ProjectSummary {
            root: facts.agents.discovery.options.project_root.clone(),
            trust: project_trust,
            config_layers: facts.agents.discovery.project_config_paths.clone(),
            instruction_sources: facts.agents.sources.clone(),
        },
        drilldowns: skill_drilldowns(),
        findings,
    }
}

fn skill_drilldowns() -> Vec<Drilldown> {
    vec![Drilldown {
        section: "skills".to_owned(),
        argv: ["gr", "inspect", "codex", "skills", "--json"]
            .map(str::to_owned)
            .to_vec(),
    }]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::agents::{AgentDiscoveryOptions, ResolvedAgentDiscovery};

    use super::*;

    fn agents() -> AgentInspection {
        AgentInspection {
            sources: Vec::new(),
            discovery: ResolvedAgentDiscovery {
                options: AgentDiscoveryOptions {
                    codex_home: PathBuf::from("/codex-home"),
                    project_root: Some(PathBuf::from("/repo")),
                    current_dir: PathBuf::from("/repo"),
                    fallback_filenames: Vec::new(),
                    max_bytes: 32 * 1024,
                },
                project_root_markers: vec![".git".to_owned()],
                config_path: PathBuf::from("/codex-home/config.toml"),
                project_trust: None,
                project_config_paths: Vec::new(),
            },
        }
    }

    fn facts<'a>(
        doctor: &'a DoctorReport,
        agents: &'a AgentInspection,
    ) -> CodexInspectionFacts<'a> {
        CodexInspectionFacts {
            version: "codex-cli 0.147.0",
            doctor,
            feature_count: 10,
            installed_plugin_count: 2,
            enabled_plugin_count: 1,
            active_skill_count: 29,
            skill_origins: SkillOriginSummary {
                personal: 12,
                plugin: 11,
                system: 6,
                project: 0,
                admin: 0,
                unknown: 0,
            },
            skill_catalog_error_count: 0,
            marketplace_count: 1,
            mcp_count: 3,
            enabled_mcp_count: 2,
            agents,
        }
    }

    #[test]
    fn returns_baseline_ok_when_doctor_is_clean() {
        let doctor = DoctorReport {
            schema_version: 1,
            overall_status: "ok".to_owned(),
            codex_version: "0.147.0".to_owned(),
            checks: BTreeMap::from([(
                "config.load".to_owned(),
                crate::doctor::DoctorCheck {
                    id: "config.load".to_owned(),
                    status: "ok".to_owned(),
                    summary: Some("config loaded".to_owned()),
                },
            )]),
        };
        let agents = agents();

        let report = synthesize_report(facts(&doctor, &agents));

        assert_eq!(report.verdict, Verdict::BaselineOk);
        assert_eq!(report.skills.active, 29);
        assert_eq!(report.skills.by_origin.personal, 12);
        assert_eq!(report.skills.by_origin.plugin, 11);
        assert_eq!(report.skills.by_origin.system, 6);
        assert_eq!(report.skills.by_origin.project, 0);
        assert_eq!(report.skills.catalog_errors, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn returns_review_when_the_skill_catalog_is_partial() {
        let doctor = DoctorReport {
            schema_version: 1,
            overall_status: "ok".to_owned(),
            codex_version: "0.147.0".to_owned(),
            checks: BTreeMap::from([(
                "config.load".to_owned(),
                crate::doctor::DoctorCheck {
                    id: "config.load".to_owned(),
                    status: "ok".to_owned(),
                    summary: Some("config loaded".to_owned()),
                },
            )]),
        };
        let agents = agents();
        let mut facts = facts(&doctor, &agents);
        facts.skill_catalog_error_count = 2;

        let report = synthesize_report(facts);

        assert_eq!(report.verdict, Verdict::Review);
        assert_eq!(report.skills.catalog_errors, 2);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].code, "skills.catalog");
    }

    #[test]
    fn returns_review_with_deterministic_doctor_findings_and_json() {
        let doctor = DoctorReport {
            schema_version: 1,
            overall_status: "fail".to_owned(),
            codex_version: "0.147.0".to_owned(),
            checks: BTreeMap::from([(
                "network.provider_reachability".to_owned(),
                crate::doctor::DoctorCheck {
                    id: "network.provider_reachability".to_owned(),
                    status: "fail".to_owned(),
                    summary: Some("provider endpoint is unreachable".to_owned()),
                },
            )]),
        };
        let agents = agents();

        let report = synthesize_report(facts(&doctor, &agents));
        let json = report.to_pretty_json().expect("report should serialize");

        assert_eq!(report.verdict, Verdict::Review);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].code,
            "doctor.network.provider_reachability"
        );
        assert!(json.contains(r#""schemaVersion": 1"#));
        assert!(json.contains(r#""kind": "summary""#));
        assert!(json.contains(r#""verdict": "REVIEW""#));
        assert!(json.contains(r#""trust": "unconfigured""#));
        assert!(json.contains(r#""section": "skills""#));
        assert!(json.contains(r#""gr""#));
    }

    #[test]
    fn does_not_report_baseline_ok_when_an_ok_overall_status_has_a_failed_check() {
        let doctor = DoctorReport {
            schema_version: 1,
            overall_status: "ok".to_owned(),
            codex_version: "0.147.0".to_owned(),
            checks: BTreeMap::from([(
                "fixture".to_owned(),
                crate::doctor::DoctorCheck {
                    id: "fixture".to_owned(),
                    status: "fail".to_owned(),
                    summary: Some("fixture failed".to_owned()),
                },
            )]),
        };
        let agents = agents();

        let report = synthesize_report(facts(&doctor, &agents));

        assert_eq!(report.verdict, Verdict::Review);
        assert_eq!(report.findings.len(), 1);
    }

    #[test]
    fn returns_incomplete_for_unknown_schema_or_missing_checks() {
        let agents = agents();
        let cases = [
            DoctorReport {
                schema_version: 2,
                overall_status: "ok".to_owned(),
                codex_version: "0.147.0".to_owned(),
                checks: BTreeMap::from([(
                    "config.load".to_owned(),
                    crate::doctor::DoctorCheck {
                        id: "config.load".to_owned(),
                        status: "ok".to_owned(),
                        summary: Some("config loaded".to_owned()),
                    },
                )]),
            },
            DoctorReport {
                schema_version: 1,
                overall_status: "ok".to_owned(),
                codex_version: "0.147.0".to_owned(),
                checks: BTreeMap::new(),
            },
        ];

        for doctor in cases {
            let report = synthesize_report(facts(&doctor, &agents));

            assert_eq!(report.verdict, Verdict::Incomplete);
            assert!(!report.findings.is_empty());
        }
    }

    #[test]
    fn serializes_failure_report_with_common_contract_fields() {
        let report = CodexFailureReport::new(
            ReportKind::Summary,
            Verdict::Blocked,
            ReportFinding::new("codex.not_found", "blocked", "Codex was not found"),
        );
        let json = report.to_pretty_json().expect("report should serialize");

        assert!(json.contains(r#""schemaVersion": 1"#));
        assert!(json.contains(r#""kind": "summary""#));
        assert!(json.contains(r#""verdict": "BLOCKED""#));
        assert!(json.contains(r#""section": "skills""#));
        assert!(json.contains(r#""code": "codex.not_found""#));
    }
}
