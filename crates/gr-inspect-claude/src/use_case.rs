use std::{fmt::Write as _, io, thread};

use gr_inspect_core::{ProcessOutput, Verdict, format_exit_code};

use crate::{
    local::{LocalEvidence, inspect_local_evidence},
    probes::{
        InstalledPlugin, parse_marketplaces, parse_plugins, probe_marketplaces, probe_plugins,
        probe_version, succeeded, version_text,
    },
    report::{
        ClaudeFailureReport, ClaudeInspectionFacts, ClaudeInspectionReport, ReportFinding,
        synthesize_report,
    },
};

#[derive(Debug)]
pub struct InspectionOutcome {
    report: OutcomeReport,
}

#[derive(Debug)]
enum OutcomeReport {
    Complete(Box<ClaudeInspectionReport>),
    Failure(ClaudeFailureReport),
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

#[derive(Debug)]
struct InspectionProbes {
    plugins: io::Result<ProcessOutput>,
    marketplaces: io::Result<ProcessOutput>,
}

/// Inspect the local Claude Code installation without changing any of it.
pub fn inspect_claude() -> InspectionOutcome {
    inspect_version_probe_with(probe_version(), inspect_remaining)
}

fn inspect_version_probe_with(
    probe: io::Result<ProcessOutput>,
    inspect_remaining: impl FnOnce(&str) -> InspectionOutcome,
) -> InspectionOutcome {
    match probe {
        Ok(probe) if probe.timed_out => failure(
            Verdict::Incomplete,
            "probe.version.timeout",
            "timeout",
            "claude --version timed out after 15 seconds",
        ),
        Ok(probe) if succeeded(&probe) => match version_text(&probe) {
            Ok(Some(version)) => inspect_remaining(version),
            Ok(None) => failure(
                Verdict::Incomplete,
                "probe.version.empty_output",
                "invalid",
                "claude --version returned no version text",
            ),
            Err(error) => failure(
                Verdict::Incomplete,
                "probe.version.invalid_utf8",
                "invalid",
                format!("claude --version returned invalid UTF-8: {error}"),
            ),
        },
        Ok(probe) => failure(
            Verdict::Incomplete,
            "probe.version.failed",
            "failed",
            format!(
                "claude --version failed with {}",
                format_exit_code(probe.exit_code)
            ),
        ),
        Err(error) if unavailable(&error) => failure(
            Verdict::Blocked,
            "claude.unavailable",
            "blocked",
            format!("claude executable is unavailable: {error}"),
        ),
        Err(error) => failure(
            Verdict::Incomplete,
            "probe.version.spawn_failed",
            "failed",
            format!("failed to run claude --version: {error}"),
        ),
    }
}

fn probe_inspection() -> InspectionProbes {
    let plugins_worker = thread::spawn(probe_plugins);
    let marketplaces_worker = thread::spawn(probe_marketplaces);

    InspectionProbes {
        plugins: join_worker(plugins_worker, "plugins"),
        marketplaces: join_worker(marketplaces_worker, "marketplaces"),
    }
}

fn join_worker<T>(worker: thread::JoinHandle<io::Result<T>>, name: &str) -> io::Result<T> {
    worker
        .join()
        .unwrap_or_else(|_| Err(io::Error::other(format!("{name} worker panicked"))))
}

fn inspect_remaining(version: &str) -> InspectionOutcome {
    inspect_plugins(version, &probe_inspection(), inspect_local_evidence)
}

fn inspect_plugins(
    version: &str,
    probes: &InspectionProbes,
    inspect_local: impl FnOnce() -> io::Result<LocalEvidence>,
) -> InspectionOutcome {
    match &probes.plugins {
        Ok(probe) if probe.timed_out => failure(
            Verdict::Incomplete,
            "probe.plugins.timeout",
            "timeout",
            "claude plugin list --json timed out after 15 seconds",
        ),
        Ok(probe) if succeeded(probe) => match parse_plugins(&probe.stdout) {
            Ok(plugins) => inspect_marketplaces(version, &plugins, probes, inspect_local),
            Err(error) => failure(
                Verdict::Incomplete,
                "probe.plugins.invalid_json",
                "invalid",
                format!("failed to parse claude plugin list JSON: {error}"),
            ),
        },
        Ok(probe) => failure(
            Verdict::Incomplete,
            "probe.plugins.failed",
            "failed",
            format!(
                "claude plugin list --json failed with {}",
                format_exit_code(probe.exit_code)
            ),
        ),
        Err(error) => failure(
            Verdict::Incomplete,
            "probe.plugins.spawn_failed",
            "failed",
            format!("failed to run claude plugin list --json: {error}"),
        ),
    }
}

fn inspect_marketplaces(
    version: &str,
    plugins: &[InstalledPlugin],
    probes: &InspectionProbes,
    inspect_local: impl FnOnce() -> io::Result<LocalEvidence>,
) -> InspectionOutcome {
    match &probes.marketplaces {
        Ok(probe) if probe.timed_out => failure(
            Verdict::Incomplete,
            "probe.marketplaces.timeout",
            "timeout",
            "claude plugin marketplace list --json timed out after 15 seconds",
        ),
        Ok(probe) if succeeded(probe) => match parse_marketplaces(&probe.stdout) {
            Ok(marketplaces) => {
                inspect_local_configuration(version, plugins, marketplaces.len(), inspect_local())
            }
            Err(error) => failure(
                Verdict::Incomplete,
                "probe.marketplaces.invalid_json",
                "invalid",
                format!("failed to parse claude plugin marketplace list JSON: {error}"),
            ),
        },
        Ok(probe) => failure(
            Verdict::Incomplete,
            "probe.marketplaces.failed",
            "failed",
            format!(
                "claude plugin marketplace list --json failed with {}",
                format_exit_code(probe.exit_code)
            ),
        ),
        Err(error) => failure(
            Verdict::Incomplete,
            "probe.marketplaces.spawn_failed",
            "failed",
            format!("failed to run claude plugin marketplace list --json: {error}"),
        ),
    }
}

fn inspect_local_configuration(
    version: &str,
    plugins: &[InstalledPlugin],
    marketplace_count: usize,
    local: io::Result<LocalEvidence>,
) -> InspectionOutcome {
    match local {
        Ok(local) => InspectionOutcome {
            report: OutcomeReport::Complete(Box::new(synthesize_report(ClaudeInspectionFacts {
                version,
                plugins,
                marketplace_count,
                local: &local,
            }))),
        },
        Err(error) => local_failure(&error),
    }
}

fn local_failure(error: &io::Error) -> InspectionOutcome {
    let (code, status) = match error.kind() {
        io::ErrorKind::InvalidData => ("probe.config.invalid", "invalid"),
        io::ErrorKind::NotFound => ("probe.home.unresolved", "missing"),
        io::ErrorKind::PermissionDenied => ("probe.local.denied", "denied"),
        _ => ("probe.local.failed", "failed"),
    };

    failure(
        Verdict::Incomplete,
        code,
        status,
        format!("failed to read local Claude configuration: {error}"),
    )
}

fn unavailable(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
    )
}

fn failure(
    verdict: Verdict,
    code: &str,
    status: &str,
    message: impl Into<String>,
) -> InspectionOutcome {
    InspectionOutcome {
        report: OutcomeReport::Failure(ClaudeFailureReport::new(
            verdict,
            ReportFinding::new(code, status, message),
        )),
    }
}

fn format_human_report(report: &ClaudeInspectionReport) -> String {
    let mut output = String::new();

    writeln!(output, "Claude version: {}", report.claude_version).expect("writing to String");
    writeln!(
        output,
        "Claude plugins: {} installed, {} enabled",
        report.plugins.installed, report.plugins.enabled
    )
    .expect("writing to String");
    writeln!(
        output,
        "Claude marketplaces: {} configured",
        report.marketplaces.configured
    )
    .expect("writing to String");
    writeln!(
        output,
        "Claude MCP: {} configured (user {}, project {}, local {}); no health state observed",
        report.mcp.configured,
        report.mcp.by_scope.user,
        report.mcp.by_scope.project,
        report.mcp.by_scope.local
    )
    .expect("writing to String");
    writeln!(
        output,
        "Claude skills: {} personal, {} project (manifests on disk)",
        report.skills.personal, report.skills.project
    )
    .expect("writing to String");
    writeln!(
        output,
        "Claude home: {}",
        report.project.claude_home.display()
    )
    .expect("writing to String");
    match &report.project.root {
        Some(project_root) => writeln!(output, "Claude project: {}", project_root.display())
            .expect("writing to String"),
        None => writeln!(output, "Claude project: not detected").expect("writing to String"),
    }
    match &report.project.state_entry_path {
        Some(path) => writeln!(output, "Claude project state entry: present for {path}")
            .expect("writing to String"),
        None => writeln!(
            output,
            "Claude project state entry: {} for {}",
            report.project.state_entry.as_str(),
            report.project.current_dir.display()
        )
        .expect("writing to String"),
    }
    writeln!(
        output,
        "Claude instructions: {} discovered sources",
        report.project.instruction_sources.len()
    )
    .expect("writing to String");
    for source in &report.project.instruction_sources {
        writeln!(
            output,
            "  {}: {}",
            source.scope.as_str(),
            source.path.display()
        )
        .expect("writing to String");
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
    writeln!(output, "Evidence limitations:").expect("writing to String");
    for limitation in &report.evidence_limitations {
        writeln!(output, "  {limitation}").expect("writing to String");
    }

    output
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::Duration};

    use crate::local::{LocalPaths, McpEvidence, SkillEvidence};

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

    fn local_evidence() -> LocalEvidence {
        LocalEvidence {
            paths: LocalPaths {
                claude_home: PathBuf::from("/home/user/.claude"),
                global_config: PathBuf::from("/home/user/.claude.json"),
                current_dir: PathBuf::from("/work/repo"),
                project_root: Some(PathBuf::from("/work/repo")),
            },
            mcp: McpEvidence::default(),
            skills: SkillEvidence::default(),
            instruction_sources: Vec::new(),
            project_state_key: None,
        }
    }

    fn probes(
        plugins: io::Result<ProcessOutput>,
        marketplaces: io::Result<ProcessOutput>,
    ) -> InspectionProbes {
        InspectionProbes {
            plugins,
            marketplaces,
        }
    }

    fn failure_code(outcome: &InspectionOutcome) -> String {
        let json = outcome
            .to_pretty_json()
            .expect("failure report should serialize");
        let report: serde_json::Value =
            serde_json::from_str(&json).expect("failure report should be JSON");
        report["findings"][0]["code"]
            .as_str()
            .expect("a failure report carries one finding code")
            .to_owned()
    }

    #[test]
    fn a_missing_executable_is_blocked_and_a_spawn_error_is_incomplete() {
        let blocked = inspect_version_probe_with(
            Err(io::Error::new(io::ErrorKind::NotFound, "no claude")),
            |_| panic!("inspection must stop at the version probe"),
        );
        assert_eq!(blocked.verdict(), Verdict::Blocked);
        assert!(blocked.is_failure());
        assert_eq!(failure_code(&blocked), "claude.unavailable");
        assert!(
            blocked
                .to_human()
                .contains("claude executable is unavailable")
        );

        let denied = inspect_version_probe_with(
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied")),
            |_| panic!("inspection must stop at the version probe"),
        );
        assert_eq!(denied.verdict(), Verdict::Blocked);

        let spawn_failed = inspect_version_probe_with(Err(io::Error::other("broken pipe")), |_| {
            panic!("inspection must stop at the version probe")
        });
        assert_eq!(spawn_failed.verdict(), Verdict::Incomplete);
        assert_eq!(failure_code(&spawn_failed), "probe.version.spawn_failed");
    }

    #[test]
    fn every_version_probe_boundary_fails_closed() {
        let cases: [(ProcessOutput, &str); 4] = [
            (output(b"", Some(0), true), "probe.version.timeout"),
            (
                output(b"  \n", Some(0), false),
                "probe.version.empty_output",
            ),
            (
                output(b"\xff", Some(0), false),
                "probe.version.invalid_utf8",
            ),
            (output(b"2.1.226", Some(7), false), "probe.version.failed"),
        ];

        for (probe, expected) in cases {
            let outcome =
                inspect_version_probe_with(Ok(probe), |_| panic!("inspection must fail closed"));

            assert_eq!(outcome.verdict(), Verdict::Incomplete);
            assert_eq!(failure_code(&outcome), expected);
        }
    }

    #[test]
    fn a_signal_terminated_version_probe_is_reported_as_such() {
        let outcome = inspect_version_probe_with(Ok(output(b"2.1.226", None, false)), |_| {
            panic!("inspection must fail closed")
        });

        assert_eq!(failure_code(&outcome), "probe.version.failed");
        assert!(outcome.to_human().contains("termination by signal"));
    }

    #[test]
    fn every_plugin_probe_boundary_fails_closed() {
        let cases: [(io::Result<ProcessOutput>, &str); 4] = [
            (Ok(output(b"[]", Some(0), true)), "probe.plugins.timeout"),
            (
                Ok(output(b"{}", Some(0), false)),
                "probe.plugins.invalid_json",
            ),
            (Ok(output(b"[]", Some(7), false)), "probe.plugins.failed"),
            (Err(io::Error::other("spawn")), "probe.plugins.spawn_failed"),
        ];

        for (probe, expected) in cases {
            let outcome = inspect_plugins(
                "test",
                &probes(probe, Ok(output(b"[]", Some(0), false))),
                || panic!("local evidence must not be read after a failed probe"),
            );

            assert_eq!(outcome.verdict(), Verdict::Incomplete);
            assert_eq!(failure_code(&outcome), expected);
        }
    }

    #[test]
    fn every_marketplace_probe_boundary_fails_closed() {
        let cases: [(io::Result<ProcessOutput>, &str); 4] = [
            (
                Ok(output(b"[]", Some(0), true)),
                "probe.marketplaces.timeout",
            ),
            (
                Ok(output(b"{}", Some(0), false)),
                "probe.marketplaces.invalid_json",
            ),
            (
                Ok(output(b"[]", Some(7), false)),
                "probe.marketplaces.failed",
            ),
            (
                Err(io::Error::other("spawn")),
                "probe.marketplaces.spawn_failed",
            ),
        ];

        for (probe, expected) in cases {
            let outcome = inspect_plugins(
                "test",
                &probes(Ok(output(b"[]", Some(0), false)), probe),
                || panic!("local evidence must not be read after a failed probe"),
            );

            assert_eq!(outcome.verdict(), Verdict::Incomplete);
            assert_eq!(failure_code(&outcome), expected);
        }
    }

    #[test]
    fn local_configuration_errors_map_to_distinct_codes() {
        let cases = [
            (io::ErrorKind::InvalidData, "probe.config.invalid"),
            (io::ErrorKind::NotFound, "probe.home.unresolved"),
            (io::ErrorKind::PermissionDenied, "probe.local.denied"),
            (io::ErrorKind::WouldBlock, "probe.local.failed"),
        ];

        for (kind, expected) in cases {
            let outcome = inspect_plugins(
                "test",
                &probes(
                    Ok(output(b"[]", Some(0), false)),
                    Ok(output(b"[]", Some(0), false)),
                ),
                || Err(io::Error::new(kind, "fixture")),
            );

            assert_eq!(outcome.verdict(), Verdict::Incomplete);
            assert_eq!(failure_code(&outcome), expected);
        }
    }

    #[test]
    fn a_complete_inspection_renders_json_and_human_output() {
        let outcome = inspect_plugins(
            "2.1.226 (Claude Code)",
            &probes(
                Ok(output(
                    br#"[{"id":"alpha@market","enabled":true,"scope":"user"}]"#,
                    Some(0),
                    false,
                )),
                Ok(output(br#"[{"name":"market"}]"#, Some(0), false)),
            ),
            || Ok(local_evidence()),
        );

        assert_eq!(outcome.verdict(), Verdict::BaselineOk);
        assert!(!outcome.is_failure());

        let json = outcome.to_pretty_json().expect("report should serialize");
        let report: serde_json::Value = serde_json::from_str(&json).expect("report should be JSON");
        assert_eq!(report["schemaVersion"], 1);
        assert_eq!(report["kind"], "summary");
        assert_eq!(report["verdict"], "BASELINE_OK");
        assert_eq!(report["claudeVersion"], "2.1.226 (Claude Code)");
        assert_eq!(report["plugins"]["installed"], 1);
        assert_eq!(report["marketplaces"]["configured"], 1);
        assert_eq!(report["project"]["stateEntry"], "absent");

        let human = outcome.to_human();
        assert!(human.contains("Claude version: 2.1.226 (Claude Code)"));
        assert!(human.contains("Claude plugins: 1 installed, 1 enabled"));
        assert!(human.contains("Claude marketplaces: 1 configured"));
        assert!(human.contains("Verdict: BASELINE_OK"));
        assert!(human.contains("Evidence limitations:"));
        assert!(!human.contains("Findings:"));
    }

    #[test]
    fn review_findings_reach_both_renderings() {
        let outcome = inspect_plugins(
            "test",
            &probes(
                Ok(output(
                    br#"[{"id":"alpha@market","enabled":true,"installPath":"/definitely/not/here"}]"#,
                    Some(0),
                    false,
                )),
                Ok(output(b"[]", Some(0), false)),
            ),
            || Ok(local_evidence()),
        );

        assert_eq!(outcome.verdict(), Verdict::Review);

        let human = outcome.to_human();
        assert!(human.contains("Verdict: REVIEW"));
        assert!(human.contains("plugins.install_path_missing"));
    }

    #[test]
    fn converts_a_worker_panic_to_an_io_error() {
        let worker = thread::spawn(|| -> io::Result<()> { panic!("fixture panic") });

        let error = join_worker(worker, "fixture").expect_err("worker should fail");

        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(error.to_string(), "fixture worker panicked");
    }
}
