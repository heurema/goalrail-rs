use std::{io, path::Path, time::Duration};

use gr_inspect_core::{ProcessOutput, format_exit_code, run_bounded};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::{
    Verdict, VersionProbe,
    app::{CODEX_APP_INFO_PLIST, probe_app_version},
    probe_version,
    report::{REPORT_SCHEMA_VERSION, ReportFinding, ReportKind},
};

const CLI_LATEST_SOURCE: &str = "https://registry.npmjs.org/@openai%2Fcodex/latest";
const UPDATE_PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const CURL_NETWORK_TIMEOUT_SECONDS: &str = "5";
const APP_SOURCE_LIMITATION: &str =
    "no verified public machine-readable latest-version source is available for Codex.app";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum UpdateFreshness {
    Fresh,
    Unverified,
}

impl UpdateFreshness {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "FRESH",
            Self::Unverified => "UNVERIFIED",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum UpdateAvailability {
    NoChange,
    UpdateAvailable,
    InstalledNewer,
    NotInstalled,
    Unknown,
}

impl UpdateAvailability {
    const fn as_str(self) -> &'static str {
        match self {
            Self::NoChange => "NO_CHANGE",
            Self::UpdateAvailable => "UPDATE_AVAILABLE",
            Self::InstalledNewer => "INSTALLED_NEWER",
            Self::NotInstalled => "NOT_INSTALLED",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateChannel {
    pub(crate) installed_version: Option<String>,
    pub(crate) available_version: Option<String>,
    pub(crate) source: Option<String>,
    pub(crate) freshness: UpdateFreshness,
    pub(crate) availability: UpdateAvailability,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexUpdateReport {
    pub(crate) schema_version: u32,
    pub(crate) kind: ReportKind,
    pub(crate) verdict: Verdict,
    pub(crate) cli: UpdateChannel,
    pub(crate) app: UpdateChannel,
    pub(crate) evidence_limitations: Vec<String>,
    pub(crate) findings: Vec<ReportFinding>,
}

impl CodexUpdateReport {
    pub(crate) fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub(crate) fn to_human(&self) -> String {
        let mut output = String::new();
        append_channel(&mut output, "Codex CLI", &self.cli);
        append_channel(&mut output, "Codex app", &self.app);

        for limitation in &self.evidence_limitations {
            output.push_str("Limitation: ");
            output.push_str(limitation);
            output.push('\n');
        }
        for finding in &self.findings {
            output.push_str("Finding [");
            output.push_str(&finding.code);
            output.push_str("]: ");
            output.push_str(&finding.message);
            output.push('\n');
        }

        output.push_str("Verdict: ");
        output.push_str(self.verdict.as_str());
        output.push('\n');
        output
    }
}

fn append_channel(output: &mut String, label: &str, channel: &UpdateChannel) {
    output.push_str(label);
    output.push_str(" updates: installed ");
    output.push_str(
        channel
            .installed_version
            .as_deref()
            .unwrap_or("unavailable"),
    );
    output.push_str(", available ");
    output.push_str(channel.available_version.as_deref().unwrap_or("unknown"));
    output.push_str(" (");
    output.push_str(channel.availability.as_str());
    output.push_str(", ");
    output.push_str(channel.freshness.as_str());
    output.push_str(")\n");
}

pub(crate) fn inspect_codex_updates() -> CodexUpdateReport {
    inspect_updates_with(
        probe_version(),
        probe_app_version(Path::new(CODEX_APP_INFO_PLIST)),
        probe_latest_cli_version,
    )
}

fn inspect_updates_with(
    version_probe: io::Result<VersionProbe>,
    app_version: Option<String>,
    latest_probe: impl FnOnce() -> io::Result<ProcessOutput>,
) -> CodexUpdateReport {
    let app = app_channel(app_version);
    let mut findings = Vec::new();

    let installed = match installed_cli_version(version_probe, &mut findings) {
        Ok(version) => version,
        Err(verdict) => {
            return report(verdict, unknown_cli_channel(None, None), app, findings);
        }
    };

    let latest_output = match latest_probe() {
        Ok(output) => output,
        Err(error) => {
            findings.push(ReportFinding::new(
                "updates.cli.source_unavailable",
                "unavailable",
                format!("failed to query the Codex CLI registry source: {error}"),
            ));
            return report(
                Verdict::Incomplete,
                unknown_cli_channel(Some(installed), None),
                app,
                findings,
            );
        }
    };

    let available = match available_cli_version(&latest_output, &mut findings) {
        Ok(version) => version,
        Err(()) => {
            return report(
                Verdict::Incomplete,
                unknown_cli_channel(Some(installed), Some(CLI_LATEST_SOURCE)),
                app,
                findings,
            );
        }
    };

    let (availability, verdict) = match installed.cmp(&available) {
        std::cmp::Ordering::Less => (UpdateAvailability::UpdateAvailable, Verdict::BaselineOk),
        std::cmp::Ordering::Equal => (UpdateAvailability::NoChange, Verdict::BaselineOk),
        std::cmp::Ordering::Greater => {
            findings.push(ReportFinding::new(
                "updates.cli.installed_newer",
                "review",
                format!(
                    "installed Codex CLI version {installed} is newer than registry latest {available}"
                ),
            ));
            (UpdateAvailability::InstalledNewer, Verdict::Review)
        }
    };

    report(
        verdict,
        UpdateChannel {
            installed_version: Some(installed.to_string()),
            available_version: Some(available.to_string()),
            source: Some(CLI_LATEST_SOURCE.to_owned()),
            freshness: UpdateFreshness::Fresh,
            availability,
        },
        app,
        findings,
    )
}

fn installed_cli_version(
    probe: io::Result<VersionProbe>,
    findings: &mut Vec<ReportFinding>,
) -> Result<Version, Verdict> {
    let probe = match probe {
        Ok(probe) => probe,
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
            ) =>
        {
            findings.push(ReportFinding::new(
                "updates.cli.unavailable",
                "blocked",
                "codex executable is unavailable",
            ));
            return Err(Verdict::Blocked);
        }
        Err(error) => {
            findings.push(ReportFinding::new(
                "updates.cli.version_spawn_failed",
                "unavailable",
                format!("failed to run codex --version: {error}"),
            ));
            return Err(Verdict::Incomplete);
        }
    };

    if probe.timed_out {
        findings.push(ReportFinding::new(
            "updates.cli.version_timeout",
            "timeout",
            "codex --version timed out after 15 seconds",
        ));
        return Err(Verdict::Incomplete);
    }
    if !probe.succeeded() {
        findings.push(ReportFinding::new(
            "updates.cli.version_failed",
            "failed",
            format!(
                "codex --version failed with {}",
                format_exit_code(probe.exit_code)
            ),
        ));
        return Err(Verdict::Incomplete);
    }

    let text = match probe.version_text() {
        Ok(Some(text)) => text,
        Ok(None) => {
            findings.push(ReportFinding::new(
                "updates.cli.version_empty",
                "invalid",
                "codex --version returned no version text",
            ));
            return Err(Verdict::Incomplete);
        }
        Err(error) => {
            findings.push(ReportFinding::new(
                "updates.cli.version_invalid_utf8",
                "invalid",
                format!("codex --version returned invalid UTF-8: {error}"),
            ));
            return Err(Verdict::Incomplete);
        }
    };

    let Some(version) = text.strip_prefix("codex-cli ") else {
        findings.push(ReportFinding::new(
            "updates.cli.version_invalid",
            "invalid",
            format!("unsupported codex --version output: {text}"),
        ));
        return Err(Verdict::Incomplete);
    };

    Version::parse(version).map_err(|error| {
        findings.push(ReportFinding::new(
            "updates.cli.version_invalid",
            "invalid",
            format!("invalid Codex CLI semantic version {version}: {error}"),
        ));
        Verdict::Incomplete
    })
}

fn available_cli_version(
    output: &ProcessOutput,
    findings: &mut Vec<ReportFinding>,
) -> Result<Version, ()> {
    if output.timed_out {
        findings.push(ReportFinding::new(
            "updates.cli.source_timeout",
            "timeout",
            "Codex CLI registry query timed out after 10 seconds",
        ));
        return Err(());
    }
    if output.exit_code != Some(0) {
        findings.push(ReportFinding::new(
            "updates.cli.source_failed",
            "failed",
            format!(
                "Codex CLI registry query failed with {}",
                format_exit_code(output.exit_code)
            ),
        ));
        return Err(());
    }

    let metadata: RegistryMetadata = serde_json::from_slice(&output.stdout).map_err(|error| {
        findings.push(ReportFinding::new(
            "updates.cli.source_invalid_json",
            "invalid",
            format!("Codex CLI registry source returned invalid JSON: {error}"),
        ));
    })?;

    Version::parse(&metadata.version).map_err(|error| {
        findings.push(ReportFinding::new(
            "updates.cli.source_invalid_version",
            "invalid",
            format!(
                "Codex CLI registry source returned invalid semantic version {}: {error}",
                metadata.version
            ),
        ));
    })
}

#[derive(Deserialize)]
struct RegistryMetadata {
    version: String,
}

fn probe_latest_cli_version() -> io::Result<ProcessOutput> {
    run_bounded(
        "curl",
        &[
            "--disable",
            "--fail",
            "--silent",
            "--show-error",
            "--max-time",
            CURL_NETWORK_TIMEOUT_SECONDS,
            CLI_LATEST_SOURCE,
        ],
        UPDATE_PROBE_TIMEOUT,
    )
}

fn report(
    verdict: Verdict,
    cli: UpdateChannel,
    app: UpdateChannel,
    findings: Vec<ReportFinding>,
) -> CodexUpdateReport {
    CodexUpdateReport {
        schema_version: REPORT_SCHEMA_VERSION,
        kind: ReportKind::Updates,
        verdict,
        cli,
        app,
        evidence_limitations: vec![APP_SOURCE_LIMITATION.to_owned()],
        findings,
    }
}

fn unknown_cli_channel(installed: Option<Version>, source: Option<&str>) -> UpdateChannel {
    UpdateChannel {
        installed_version: installed.map(|version| version.to_string()),
        available_version: None,
        source: source.map(str::to_owned),
        freshness: UpdateFreshness::Unverified,
        availability: UpdateAvailability::Unknown,
    }
}

fn app_channel(installed: Option<String>) -> UpdateChannel {
    let availability = if installed.is_some() {
        UpdateAvailability::Unknown
    } else {
        UpdateAvailability::NotInstalled
    };

    UpdateChannel {
        installed_version: installed,
        available_version: None,
        source: None,
        freshness: UpdateFreshness::Unverified,
        availability,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version_probe(version: &[u8]) -> io::Result<VersionProbe> {
        Ok(VersionProbe {
            stdout: version.to_vec(),
            stderr: Vec::new(),
            exit_code: Some(0),
            duration: Duration::ZERO,
            timed_out: false,
        })
    }

    fn latest_probe(version: &str) -> impl FnOnce() -> io::Result<ProcessOutput> + '_ {
        move || {
            Ok(ProcessOutput {
                stdout: format!(r#"{{"version":"{version}"}}"#).into_bytes(),
                stderr: Vec::new(),
                exit_code: Some(0),
                duration: Duration::ZERO,
                timed_out: false,
            })
        }
    }

    #[test]
    fn reports_update_available_no_change_and_installed_newer() {
        let cases = [
            (
                "0.148.0",
                UpdateAvailability::UpdateAvailable,
                Verdict::BaselineOk,
            ),
            ("0.147.0", UpdateAvailability::NoChange, Verdict::BaselineOk),
            (
                "0.146.0",
                UpdateAvailability::InstalledNewer,
                Verdict::Review,
            ),
        ];

        for (available, availability, verdict) in cases {
            let report = inspect_updates_with(
                version_probe(b"codex-cli 0.147.0\n"),
                Some("26.715.61943".to_owned()),
                latest_probe(available),
            );

            assert_eq!(report.verdict, verdict);
            assert_eq!(report.cli.availability, availability);
            assert_eq!(report.cli.freshness, UpdateFreshness::Fresh);
            assert_eq!(report.cli.installed_version.as_deref(), Some("0.147.0"));
            assert_eq!(report.cli.available_version.as_deref(), Some(available));
            assert_eq!(report.app.availability, UpdateAvailability::Unknown);
        }
    }

    #[test]
    fn fails_closed_for_invalid_installed_and_available_versions() {
        let invalid_installed =
            inspect_updates_with(version_probe(b"codex-cli test\n"), None, || {
                panic!("registry should not be queried for an invalid installed version")
            });
        assert_eq!(invalid_installed.verdict, Verdict::Incomplete);
        assert_eq!(
            invalid_installed.cli.availability,
            UpdateAvailability::Unknown
        );
        assert_eq!(
            invalid_installed.findings[0].code,
            "updates.cli.version_invalid"
        );
        assert_eq!(invalid_installed.cli.source, None);

        let invalid_available = inspect_updates_with(
            version_probe(b"codex-cli 0.147.0\n"),
            None,
            latest_probe("latest"),
        );
        assert_eq!(invalid_available.verdict, Verdict::Incomplete);
        assert_eq!(invalid_available.cli.freshness, UpdateFreshness::Unverified);
        assert_eq!(
            invalid_available.cli.source.as_deref(),
            Some(CLI_LATEST_SOURCE)
        );
        assert_eq!(
            invalid_available.findings[0].code,
            "updates.cli.source_invalid_version"
        );
        assert_eq!(
            invalid_available.app.availability,
            UpdateAvailability::NotInstalled
        );
    }

    #[test]
    fn classifies_every_installed_version_probe_failure_without_querying_the_registry() {
        let unavailable_errors = [io::ErrorKind::NotFound, io::ErrorKind::PermissionDenied];
        for kind in unavailable_errors {
            let report =
                inspect_updates_with(Err(io::Error::new(kind, "codex unavailable")), None, || {
                    panic!("registry should not be queried when codex is unavailable")
                });
            assert_eq!(report.verdict, Verdict::Blocked);
            assert_eq!(report.findings[0].code, "updates.cli.unavailable");
        }

        let spawn_failure =
            inspect_updates_with(Err(io::Error::other("spawn failed")), None, || {
                panic!("registry should not be queried after a spawn failure")
            });
        assert_eq!(spawn_failure.verdict, Verdict::Incomplete);
        assert_eq!(
            spawn_failure.findings[0].code,
            "updates.cli.version_spawn_failed"
        );

        let cases = [
            (
                VersionProbe {
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                    exit_code: None,
                    duration: Duration::ZERO,
                    timed_out: true,
                },
                "updates.cli.version_timeout",
            ),
            (
                VersionProbe {
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                    exit_code: Some(1),
                    duration: Duration::ZERO,
                    timed_out: false,
                },
                "updates.cli.version_failed",
            ),
            (
                VersionProbe {
                    stdout: b"  \n".to_vec(),
                    stderr: Vec::new(),
                    exit_code: Some(0),
                    duration: Duration::ZERO,
                    timed_out: false,
                },
                "updates.cli.version_empty",
            ),
            (
                VersionProbe {
                    stdout: vec![0xff],
                    stderr: Vec::new(),
                    exit_code: Some(0),
                    duration: Duration::ZERO,
                    timed_out: false,
                },
                "updates.cli.version_invalid_utf8",
            ),
        ];

        for (probe, code) in cases {
            let report = inspect_updates_with(Ok(probe), None, || {
                panic!("registry should not be queried after a version probe failure")
            });
            assert_eq!(report.verdict, Verdict::Incomplete);
            assert_eq!(report.findings[0].code, code);
        }
    }

    #[test]
    fn classifies_registry_spawn_timeout_exit_and_json_failures() {
        let spawn_failure =
            inspect_updates_with(version_probe(b"codex-cli 0.147.0\n"), None, || {
                Err(io::Error::new(io::ErrorKind::NotFound, "curl missing"))
            });
        assert_eq!(spawn_failure.verdict, Verdict::Incomplete);
        assert_eq!(
            spawn_failure.findings[0].code,
            "updates.cli.source_unavailable"
        );
        assert_eq!(spawn_failure.cli.source, None);

        let cases = [
            (
                ProcessOutput {
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                    exit_code: Some(0),
                    duration: Duration::ZERO,
                    timed_out: true,
                },
                "updates.cli.source_timeout",
            ),
            (
                ProcessOutput {
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                    exit_code: Some(22),
                    duration: Duration::ZERO,
                    timed_out: false,
                },
                "updates.cli.source_failed",
            ),
            (
                ProcessOutput {
                    stdout: b"not-json".to_vec(),
                    stderr: Vec::new(),
                    exit_code: Some(0),
                    duration: Duration::ZERO,
                    timed_out: false,
                },
                "updates.cli.source_invalid_json",
            ),
        ];

        for (output, code) in cases {
            let report =
                inspect_updates_with(version_probe(b"codex-cli 0.147.0\n"), None, || Ok(output));
            assert_eq!(report.verdict, Verdict::Incomplete);
            assert_eq!(report.findings[0].code, code);
        }
    }

    #[test]
    fn renders_machine_and_human_contracts_without_an_app_update_claim() {
        let report = inspect_updates_with(
            version_probe(b"codex-cli 0.147.0\n"),
            Some("26.715.61943".to_owned()),
            latest_probe("0.148.0"),
        );

        let json = report.to_pretty_json().expect("report should serialize");
        assert!(json.contains(r#""kind": "updates""#));
        assert!(json.contains(r#""availability": "UPDATE_AVAILABLE""#));
        assert!(json.contains(r#""availableVersion": null"#));
        assert!(json.contains(APP_SOURCE_LIMITATION));

        let human = report.to_human();
        assert_eq!(
            human,
            format!(
                "Codex CLI updates: installed 0.147.0, available 0.148.0 \
                 (UPDATE_AVAILABLE, FRESH)\n\
                 Codex app updates: installed 26.715.61943, available unknown \
                 (UNKNOWN, UNVERIFIED)\n\
                 Limitation: {APP_SOURCE_LIMITATION}\n\
                 Verdict: BASELINE_OK\n"
            )
        );
    }
}
