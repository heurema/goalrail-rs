use std::{fmt::Write as _, io};

use crate::{
    Verdict, VersionProbe,
    agents::InstructionScope,
    doctor::DoctorReport,
    inspection::{InspectionProbes, probe_inspection},
    probe_version,
    report::{
        CodexFailureReport, CodexInspectionFacts, CodexInspectionReport, ReportFinding,
        synthesize_report,
    },
};

#[derive(Debug)]
pub struct InspectionOutcome {
    report: OutcomeReport,
}

#[derive(Debug)]
enum OutcomeReport {
    Complete(CodexInspectionReport),
    Failure(CodexFailureReport),
}

impl InspectionOutcome {
    pub const fn verdict(&self) -> Verdict {
        match &self.report {
            OutcomeReport::Complete(report) => report.verdict,
            OutcomeReport::Failure(report) => report.verdict,
        }
    }

    pub const fn is_failure(&self) -> bool {
        matches!(self.report, OutcomeReport::Failure(_))
    }

    pub fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        match &self.report {
            OutcomeReport::Complete(report) => report.to_pretty_json(),
            OutcomeReport::Failure(report) => report.to_pretty_json(),
        }
    }

    pub fn to_human(&self) -> String {
        match &self.report {
            OutcomeReport::Complete(report) => format_human_report(report),
            OutcomeReport::Failure(report) => report
                .findings
                .first()
                .map(|finding| finding.message.clone())
                .unwrap_or_default(),
        }
    }
}

struct InspectionSummary<'a> {
    version: &'a str,
    doctor: &'a DoctorReport,
    feature_count: usize,
    installed_plugin_count: usize,
    enabled_plugin_count: usize,
}

pub fn inspect_codex() -> InspectionOutcome {
    inspect_version_probe(probe_version())
}

fn inspect_version_probe(probe: io::Result<VersionProbe>) -> InspectionOutcome {
    inspect_version_probe_with(probe, inspect_remaining)
}

fn inspect_version_probe_with(
    probe: io::Result<VersionProbe>,
    inspect_remaining: impl FnOnce(&str) -> InspectionOutcome,
) -> InspectionOutcome {
    match probe {
        Ok(probe) if probe.timed_out => failure(
            Verdict::Incomplete,
            "probe.version.timeout",
            "timeout",
            "codex --version timed out after 15 seconds",
        ),
        Ok(probe) if probe.succeeded() => match probe.version_text() {
            Ok(Some(version)) => inspect_remaining(version),
            Ok(None) => failure(
                Verdict::Incomplete,
                "probe.version.empty_output",
                "invalid",
                "codex --version returned no version text",
            ),
            Err(error) => failure(
                Verdict::Incomplete,
                "probe.version.invalid_utf8",
                "invalid",
                format!("codex --version returned invalid UTF-8: {error}"),
            ),
        },
        Ok(probe) => failure(
            Verdict::Incomplete,
            "probe.version.failed",
            "failed",
            format!(
                "codex --version failed with {}",
                format_exit_code(probe.exit_code)
            ),
        ),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
            ) =>
        {
            failure(
                Verdict::Blocked,
                "codex.unavailable",
                "blocked",
                format!("codex executable is unavailable: {error}"),
            )
        }
        Err(error) => failure(
            Verdict::Incomplete,
            "probe.version.spawn_failed",
            "failed",
            format!("failed to run codex --version: {error}"),
        ),
    }
}

fn inspect_remaining(version: &str) -> InspectionOutcome {
    let probes = probe_inspection();

    inspect_doctor(version, &probes)
}

fn inspect_doctor(version: &str, probes: &InspectionProbes) -> InspectionOutcome {
    match &probes.doctor {
        Ok(probe) if probe.timed_out => failure(
            Verdict::Incomplete,
            "probe.doctor.timeout",
            "timeout",
            "codex doctor --json timed out after 15 seconds",
        ),
        Ok(probe) => match probe.report() {
            Ok(report) if doctor_exit_matches_report(probe.exit_code, &report) => {
                inspect_features(version, &report, probes)
            }
            Ok(_) => failure(
                Verdict::Incomplete,
                "probe.doctor.failed",
                "failed",
                format!(
                    "codex doctor --json returned valid JSON but failed with {}",
                    format_exit_code(probe.exit_code)
                ),
            ),
            Err(error) => failure(
                Verdict::Incomplete,
                "probe.doctor.invalid_json",
                "invalid",
                format!(
                    "failed to parse codex doctor JSON after {}: {error}",
                    format_exit_code(probe.exit_code)
                ),
            ),
        },
        Err(error) => failure(
            Verdict::Incomplete,
            "probe.doctor.spawn_failed",
            "failed",
            format!("failed to run codex doctor --json: {error}"),
        ),
    }
}

fn doctor_exit_matches_report(exit_code: Option<i32>, report: &DoctorReport) -> bool {
    exit_code == Some(0) || (exit_code == Some(1) && report.overall_status != "ok")
}

fn inspect_features(
    version: &str,
    doctor: &DoctorReport,
    probes: &InspectionProbes,
) -> InspectionOutcome {
    match &probes.features {
        Ok(probe) if probe.timed_out => failure(
            Verdict::Incomplete,
            "probe.features.timeout",
            "timeout",
            "codex features list timed out after 15 seconds",
        ),
        Ok(probe) if probe.succeeded() => {
            let Some(features) = probe.text() else {
                return failure(
                    Verdict::Incomplete,
                    "probe.features.invalid_output",
                    "invalid",
                    "codex features list returned invalid UTF-8",
                );
            };

            let feature_count = features
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count();

            inspect_plugins(version, doctor, feature_count, probes)
        }
        Ok(probe) => failure(
            Verdict::Incomplete,
            "probe.features.failed",
            "failed",
            format!(
                "codex features list failed with {}",
                format_exit_code(probe.exit_code)
            ),
        ),
        Err(error) => failure(
            Verdict::Incomplete,
            "probe.features.spawn_failed",
            "failed",
            format!("failed to run codex features list: {error}"),
        ),
    }
}

fn inspect_plugins(
    version: &str,
    doctor: &DoctorReport,
    feature_count: usize,
    probes: &InspectionProbes,
) -> InspectionOutcome {
    match &probes.plugins {
        Ok(probe) if probe.timed_out => failure(
            Verdict::Incomplete,
            "probe.plugins.timeout",
            "timeout",
            "codex plugin list --json timed out after 15 seconds",
        ),
        Ok(probe) if probe.succeeded() => match probe.report() {
            Ok(report) => {
                let installed_count = report.installed.len();
                let enabled_count = report
                    .installed
                    .iter()
                    .filter(|plugin| plugin.enabled)
                    .count();

                let summary = InspectionSummary {
                    version,
                    doctor,
                    feature_count,
                    installed_plugin_count: installed_count,
                    enabled_plugin_count: enabled_count,
                };

                inspect_marketplaces(summary, probes)
            }
            Err(error) => failure(
                Verdict::Incomplete,
                "probe.plugins.invalid_json",
                "invalid",
                format!("failed to parse codex plugin list JSON: {error}"),
            ),
        },
        Ok(probe) => failure(
            Verdict::Incomplete,
            "probe.plugins.failed",
            "failed",
            format!(
                "codex plugin list --json failed with {}",
                format_exit_code(probe.exit_code)
            ),
        ),
        Err(error) => failure(
            Verdict::Incomplete,
            "probe.plugins.spawn_failed",
            "failed",
            format!("failed to run codex plugin list --json: {error}"),
        ),
    }
}

fn inspect_marketplaces(
    summary: InspectionSummary<'_>,
    probes: &InspectionProbes,
) -> InspectionOutcome {
    match &probes.marketplaces {
        Ok(probe) if probe.timed_out => failure(
            Verdict::Incomplete,
            "probe.marketplaces.timeout",
            "timeout",
            "codex plugin marketplace list --json timed out after 15 seconds",
        ),
        Ok(probe) if probe.succeeded() => match probe.report() {
            Ok(report) => {
                let marketplace_count = report.marketplaces.len();
                inspect_mcp(summary, marketplace_count, probes)
            }
            Err(error) => failure(
                Verdict::Incomplete,
                "probe.marketplaces.invalid_json",
                "invalid",
                format!("failed to parse codex plugin marketplace list JSON: {error}"),
            ),
        },
        Ok(probe) => failure(
            Verdict::Incomplete,
            "probe.marketplaces.failed",
            "failed",
            format!(
                "codex plugin marketplace list --json failed with {}",
                format_exit_code(probe.exit_code)
            ),
        ),
        Err(error) => failure(
            Verdict::Incomplete,
            "probe.marketplaces.spawn_failed",
            "failed",
            format!("failed to run codex plugin marketplace list --json: {error}"),
        ),
    }
}

fn inspect_mcp(
    summary: InspectionSummary<'_>,
    marketplace_count: usize,
    probes: &InspectionProbes,
) -> InspectionOutcome {
    match &probes.mcp {
        Ok(probe) if probe.timed_out => failure(
            Verdict::Incomplete,
            "probe.mcp.timeout",
            "timeout",
            "codex mcp list --json timed out after 15 seconds",
        ),
        Ok(probe) if probe.succeeded() => match probe.report() {
            Ok(report) => {
                let mcp_count = report.servers.len();
                let enabled_mcp_count = report
                    .servers
                    .iter()
                    .filter(|server| server.enabled)
                    .count();

                inspect_agents(
                    summary,
                    marketplace_count,
                    mcp_count,
                    enabled_mcp_count,
                    probes,
                )
            }
            Err(error) => failure(
                Verdict::Incomplete,
                "probe.mcp.invalid_json",
                "invalid",
                format!("failed to parse codex mcp list JSON: {error}"),
            ),
        },
        Ok(probe) => failure(
            Verdict::Incomplete,
            "probe.mcp.failed",
            "failed",
            format!(
                "codex mcp list --json failed with {}",
                format_exit_code(probe.exit_code)
            ),
        ),
        Err(error) => failure(
            Verdict::Incomplete,
            "probe.mcp.spawn_failed",
            "failed",
            format!("failed to run codex mcp list --json: {error}"),
        ),
    }
}

fn inspect_agents(
    summary: InspectionSummary<'_>,
    marketplace_count: usize,
    mcp_count: usize,
    enabled_mcp_count: usize,
    probes: &InspectionProbes,
) -> InspectionOutcome {
    match &probes.agents {
        Ok(agents) => complete(synthesize_report(CodexInspectionFacts {
            version: summary.version,
            doctor: summary.doctor,
            feature_count: summary.feature_count,
            installed_plugin_count: summary.installed_plugin_count,
            enabled_plugin_count: summary.enabled_plugin_count,
            marketplace_count,
            mcp_count,
            enabled_mcp_count,
            agents,
        })),
        Err(error) => failure(
            Verdict::Incomplete,
            "probe.instructions.failed",
            "failed",
            format!("failed to discover active instructions: {error}"),
        ),
    }
}

fn complete(report: CodexInspectionReport) -> InspectionOutcome {
    InspectionOutcome {
        report: OutcomeReport::Complete(report),
    }
}

fn failure(
    verdict: Verdict,
    code: &str,
    status: &str,
    message: impl Into<String>,
) -> InspectionOutcome {
    InspectionOutcome {
        report: OutcomeReport::Failure(CodexFailureReport::new(
            verdict,
            ReportFinding::new(code, status, message),
        )),
    }
}

fn format_exit_code(exit_code: Option<i32>) -> String {
    match exit_code {
        Some(code) => format!("exit code {code}"),
        None => "termination by signal".to_owned(),
    }
}

fn format_human_report(report: &CodexInspectionReport) -> String {
    let mut output = String::new();

    writeln!(output, "Codex version: {}", report.codex_version).expect("writing to String");
    writeln!(
        output,
        "Codex doctor: {} ({} checks)",
        report.doctor.status, report.doctor.check_count
    )
    .expect("writing to String");
    writeln!(
        output,
        "Codex features: observed ({} rows)",
        report.features.observed_rows
    )
    .expect("writing to String");
    writeln!(
        output,
        "Codex plugins: {} installed, {} enabled",
        report.plugins.installed, report.plugins.enabled
    )
    .expect("writing to String");
    writeln!(
        output,
        "Codex marketplaces: {} configured",
        report.marketplaces.configured
    )
    .expect("writing to String");
    writeln!(
        output,
        "Codex MCP: {} configured, {} enabled",
        report.mcp.configured, report.mcp.enabled
    )
    .expect("writing to String");
    match &report.project.root {
        Some(project_root) => writeln!(
            output,
            "Codex project: {} ({})",
            project_root.display(),
            report.project.trust.as_str()
        )
        .expect("writing to String"),
        None => writeln!(output, "Codex project: not detected").expect("writing to String"),
    }
    writeln!(
        output,
        "Codex project config: {} active layers",
        report.project.config_layers.len()
    )
    .expect("writing to String");
    for path in &report.project.config_layers {
        writeln!(output, "  project: {}", path.display()).expect("writing to String");
    }
    writeln!(
        output,
        "Codex instructions: {} active sources",
        report.project.instruction_sources.len()
    )
    .expect("writing to String");
    for source in &report.project.instruction_sources {
        let scope = match source.scope {
            InstructionScope::Global => "global",
            InstructionScope::Project => "project",
        };
        writeln!(output, "  {scope}: {}", source.path.display()).expect("writing to String");
    }
    writeln!(output, "Verdict: {}", report.verdict.as_str()).expect("writing to String");
    if !report.findings.is_empty() {
        writeln!(output, "Findings:").expect("writing to String");
        for finding in &report.findings {
            writeln!(
                output,
                "  {} [{}]: {}",
                finding.code, finding.status, finding.message
            )
            .expect("writing to String");
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf, time::Duration};

    use crate::{
        agents::{
            AgentDiscoveryOptions, AgentInspection, InstructionSource, ProjectTrust,
            ResolvedAgentDiscovery,
        },
        doctor::{DoctorCheck, DoctorProbe},
        features::FeaturesProbe,
        marketplaces::MarketplaceProbe,
        mcp::McpProbe,
        plugins::PluginProbe,
    };

    use super::*;

    fn successful_probes() -> InspectionProbes {
        InspectionProbes {
            doctor: Ok(DoctorProbe {
                stdout: br#"{"schemaVersion":1,"overallStatus":"ok","codexVersion":"0.147.0","checks":{"fixture":{"id":"fixture","status":"ok","summary":"fixture ok"}}}"#.to_vec(),
                stderr: Vec::new(),
                exit_code: Some(0),
                duration: Duration::ZERO,
                timed_out: false,
            }),
            features: Ok(FeaturesProbe {
                stdout: b"apps stable true\n\nplugins stable true\n".to_vec(),
                stderr: Vec::new(),
                exit_code: Some(0),
                duration: Duration::ZERO,
                timed_out: false,
            }),
            plugins: Ok(PluginProbe {
                stdout: br#"{"installed":[{"pluginId":"one@test","name":"one","marketplaceName":"test","version":"1","installed":true,"enabled":true},{"pluginId":"two@test","name":"two","marketplaceName":"test","version":"1","installed":true,"enabled":false}],"available":[]}"#.to_vec(),
                stderr: Vec::new(),
                exit_code: Some(0),
                duration: Duration::ZERO,
                timed_out: false,
            }),
            marketplaces: Ok(MarketplaceProbe {
                stdout: br#"{"marketplaces":[{"name":"test","root":"/marketplace"}]}"#.to_vec(),
                stderr: Vec::new(),
                exit_code: Some(0),
                duration: Duration::ZERO,
                timed_out: false,
            }),
            mcp: Ok(McpProbe {
                stdout: br#"[{"name":"one","enabled":true,"transport":{"type":"stdio"},"auth_status":"unsupported"},{"name":"two","enabled":false,"transport":{"type":"streamable_http"},"auth_status":"not_authenticated"}]"#.to_vec(),
                stderr: Vec::new(),
                exit_code: Some(0),
                duration: Duration::ZERO,
                timed_out: false,
            }),
            agents: Ok(AgentInspection {
                sources: vec![
                    InstructionSource {
                        path: PathBuf::from("/codex-home/AGENTS.md"),
                        scope: InstructionScope::Global,
                        size_bytes: 10,
                    },
                    InstructionSource {
                        path: PathBuf::from("/repo/AGENTS.md"),
                        scope: InstructionScope::Project,
                        size_bytes: 20,
                    },
                ],
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
                    project_trust: Some(ProjectTrust::Trusted),
                    project_config_paths: vec![PathBuf::from("/repo/.codex/config.toml")],
                },
            }),
        }
    }

    fn clean_doctor() -> DoctorReport {
        DoctorReport {
            schema_version: 1,
            overall_status: "ok".to_owned(),
            codex_version: "0.147.0".to_owned(),
            checks: BTreeMap::from([(
                "fixture".to_owned(),
                DoctorCheck {
                    id: "fixture".to_owned(),
                    status: "ok".to_owned(),
                    summary: Some("fixture ok".to_owned()),
                },
            )]),
        }
    }

    fn summary(doctor: &DoctorReport) -> InspectionSummary<'_> {
        InspectionSummary {
            version: "codex-cli 0.147.0",
            doctor,
            feature_count: 2,
            installed_plugin_count: 2,
            enabled_plugin_count: 1,
        }
    }

    fn assert_failure(outcome: InspectionOutcome, verdict: Verdict, code: &str) {
        let OutcomeReport::Failure(report) = outcome.report else {
            panic!("expected failure outcome");
        };

        assert_eq!(report.verdict, verdict);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].code, code);
    }

    #[test]
    fn exposes_complete_and_failure_outcomes_through_the_public_contract() {
        let doctor = clean_doctor();
        let complete = inspect_features("codex-cli 0.147.0", &doctor, &successful_probes());

        assert_eq!(complete.verdict(), Verdict::BaselineOk);
        assert!(!complete.is_failure());
        let complete_json = complete
            .to_pretty_json()
            .expect("complete outcome should serialize");
        let complete_value: serde_json::Value =
            serde_json::from_str(&complete_json).expect("complete JSON should be valid");
        assert_eq!(complete_value["verdict"], "BASELINE_OK");
        assert!(complete.to_human().contains("Verdict: BASELINE_OK"));

        let failed = failure(
            Verdict::Blocked,
            "fixture.blocked",
            "blocked",
            "fixture failure",
        );
        assert_eq!(failed.verdict(), Verdict::Blocked);
        assert!(failed.is_failure());
        let failed_json = failed
            .to_pretty_json()
            .expect("failure outcome should serialize");
        let failed_value: serde_json::Value =
            serde_json::from_str(&failed_json).expect("failure JSON should be valid");
        assert_eq!(failed_value["findings"][0]["code"], "fixture.blocked");
        assert_eq!(failed.to_human(), "fixture failure");
    }

    #[test]
    fn continues_only_after_a_successful_version_probe() {
        let success = inspect_version_probe_with(
            Ok(VersionProbe {
                stdout: b"codex-cli 0.147.0\n".to_vec(),
                stderr: Vec::new(),
                exit_code: Some(0),
                duration: Duration::ZERO,
                timed_out: false,
            }),
            |version| {
                assert_eq!(version, "codex-cli 0.147.0");
                failure(Verdict::Review, "fixture.continued", "fixture", "continued")
            },
        );
        assert_failure(success, Verdict::Review, "fixture.continued");

        let failed = inspect_version_probe_with(
            Ok(VersionProbe {
                stdout: b"ignored".to_vec(),
                stderr: Vec::new(),
                exit_code: Some(2),
                duration: Duration::ZERO,
                timed_out: false,
            }),
            |_| panic!("failed version probe must not continue"),
        );
        assert_failure(failed, Verdict::Incomplete, "probe.version.failed");
    }

    fn expect_complete(outcome: InspectionOutcome) -> CodexInspectionReport {
        let OutcomeReport::Complete(report) = outcome.report else {
            panic!("expected complete outcome");
        };
        report
    }

    #[test]
    fn reports_the_exact_timeout_boundary() {
        assert_failure(
            inspect_version_probe(Ok(VersionProbe {
                stdout: b"codex-cli 0.147.0".to_vec(),
                stderr: Vec::new(),
                exit_code: Some(0),
                duration: Duration::ZERO,
                timed_out: true,
            })),
            Verdict::Incomplete,
            "probe.version.timeout",
        );

        let mut probes = successful_probes();
        probes.doctor.as_mut().expect("doctor probe").timed_out = true;
        assert_failure(
            inspect_doctor("codex-cli 0.147.0", &probes),
            Verdict::Incomplete,
            "probe.doctor.timeout",
        );

        let doctor = clean_doctor();
        let mut probes = successful_probes();
        probes.features.as_mut().expect("features probe").timed_out = true;
        assert_failure(
            inspect_features("codex-cli 0.147.0", &doctor, &probes),
            Verdict::Incomplete,
            "probe.features.timeout",
        );

        let mut probes = successful_probes();
        probes.plugins.as_mut().expect("plugins probe").timed_out = true;
        assert_failure(
            inspect_plugins("codex-cli 0.147.0", &doctor, 2, &probes),
            Verdict::Incomplete,
            "probe.plugins.timeout",
        );

        let mut probes = successful_probes();
        probes
            .marketplaces
            .as_mut()
            .expect("marketplaces probe")
            .timed_out = true;
        assert_failure(
            inspect_marketplaces(summary(&doctor), &probes),
            Verdict::Incomplete,
            "probe.marketplaces.timeout",
        );

        let mut probes = successful_probes();
        probes.mcp.as_mut().expect("MCP probe").timed_out = true;
        assert_failure(
            inspect_mcp(summary(&doctor), 1, &probes),
            Verdict::Incomplete,
            "probe.mcp.timeout",
        );
    }

    #[test]
    fn classifies_version_spawn_errors_by_error_kind() {
        assert_failure(
            inspect_version_probe(Err(io::Error::new(
                io::ErrorKind::NotFound,
                "fixture missing executable",
            ))),
            Verdict::Blocked,
            "codex.unavailable",
        );
        assert_failure(
            inspect_version_probe(Err(io::Error::other("fixture spawn failure"))),
            Verdict::Incomplete,
            "probe.version.spawn_failed",
        );
    }

    #[test]
    fn accepts_only_doctor_exit_codes_that_match_the_report() {
        let mut doctor = clean_doctor();
        assert!(doctor_exit_matches_report(Some(0), &doctor));
        assert!(!doctor_exit_matches_report(Some(1), &doctor));

        doctor.overall_status = "fail".to_owned();
        assert!(doctor_exit_matches_report(Some(1), &doctor));
        assert!(!doctor_exit_matches_report(Some(2), &doctor));
    }

    #[test]
    fn rejects_a_doctor_exit_code_that_contradicts_its_report() {
        let mut probes = successful_probes();
        probes.doctor.as_mut().expect("doctor probe").exit_code = Some(1);

        assert_failure(
            inspect_doctor("codex-cli 0.147.0", &probes),
            Verdict::Incomplete,
            "probe.doctor.failed",
        );
    }

    #[test]
    fn rejects_nonzero_exit_codes_at_each_plain_probe_stage() {
        let doctor = clean_doctor();

        let mut probes = successful_probes();
        probes.features.as_mut().expect("features probe").exit_code = Some(2);
        assert_failure(
            inspect_features("codex-cli 0.147.0", &doctor, &probes),
            Verdict::Incomplete,
            "probe.features.failed",
        );

        let mut probes = successful_probes();
        probes.plugins.as_mut().expect("plugins probe").exit_code = Some(2);
        assert_failure(
            inspect_plugins("codex-cli 0.147.0", &doctor, 2, &probes),
            Verdict::Incomplete,
            "probe.plugins.failed",
        );

        let mut probes = successful_probes();
        probes
            .marketplaces
            .as_mut()
            .expect("marketplaces probe")
            .exit_code = Some(2);
        assert_failure(
            inspect_marketplaces(summary(&doctor), &probes),
            Verdict::Incomplete,
            "probe.marketplaces.failed",
        );

        let mut probes = successful_probes();
        probes.mcp.as_mut().expect("MCP probe").exit_code = Some(2);
        assert_failure(
            inspect_mcp(summary(&doctor), 1, &probes),
            Verdict::Incomplete,
            "probe.mcp.failed",
        );
    }

    #[test]
    fn formats_numeric_and_signal_exit_statuses() {
        assert_eq!(format_exit_code(Some(7)), "exit code 7");
        assert_eq!(format_exit_code(None), "termination by signal");
    }

    #[test]
    fn counts_only_non_empty_feature_rows() {
        let doctor = clean_doctor();
        let report = expect_complete(inspect_features(
            "codex-cli 0.147.0",
            &doctor,
            &successful_probes(),
        ));

        assert_eq!(report.features.observed_rows, 2);
    }

    #[test]
    fn formats_the_complete_human_report_including_findings() {
        let mut probes = successful_probes();
        probes.doctor = Ok(DoctorProbe {
            stdout: br#"{"schemaVersion":1,"overallStatus":"fail","codexVersion":"0.147.0","checks":{"fixture":{"id":"fixture","status":"fail","summary":"fixture failure"}}}"#.to_vec(),
            stderr: Vec::new(),
            exit_code: Some(1),
            duration: Duration::ZERO,
            timed_out: false,
        });
        let report = expect_complete(inspect_doctor("codex-cli 0.147.0", &probes));

        let output = format_human_report(&report);

        assert!(output.contains("Codex version: codex-cli 0.147.0"));
        assert!(output.contains("Codex features: observed (2 rows)"));
        assert!(output.contains("Codex plugins: 2 installed, 1 enabled"));
        assert!(output.contains("Codex marketplaces: 1 configured"));
        assert!(output.contains("Codex MCP: 2 configured, 1 enabled"));
        assert!(output.contains("Codex project: /repo (trusted)"));
        assert!(output.contains("project: /repo/.codex/config.toml"));
        assert!(output.contains("global: /codex-home/AGENTS.md"));
        assert!(output.contains("project: /repo/AGENTS.md"));
        assert!(output.contains("Verdict: REVIEW"));
        assert!(output.contains("Findings:"));
        assert!(output.contains("doctor.fixture [fail]: fixture failure"));
    }
}
