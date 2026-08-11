#![cfg(unix)]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_TEST_DIR: AtomicUsize = AtomicUsize::new(0);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "goalrail-cli-test-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("test directory should be created");
        Self { path }
    }

    fn directory(&self, relative: &str) -> PathBuf {
        let path = self.path.join(relative);
        fs::create_dir_all(&path).expect("fixture directory should be created");
        path
    }

    fn file(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture parent should be created");
        }
        fs::write(&path, contents).expect("fixture file should be written");
        path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn emits_json_and_distinct_exit_codes_for_all_verdicts() {
    let tree = TestDirectory::new();
    let empty_bin = tree.directory("empty-bin");
    let fake_bin = tree.directory("fake-bin");
    let codex_home = tree.directory("codex-home");
    let project = tree.directory("project");
    tree.directory("project/.git");
    write_fake_codex(&fake_bin);

    let blocked = run_gr(&empty_bin, &codex_home, &project, None);
    assert_report(blocked, 4, "BLOCKED");

    let incomplete = run_gr(&fake_bin, &codex_home, &project, Some("version-fail"));
    assert_report(incomplete, 3, "INCOMPLETE");

    let review = run_gr(&fake_bin, &codex_home, &project, Some("doctor-fail"));
    assert_report(review, 1, "REVIEW");

    let baseline = run_gr(&fake_bin, &codex_home, &project, None);
    assert_report(baseline, 0, "BASELINE_OK");
}

#[test]
fn reports_live_skill_catalog_usage_and_excludes_the_current_thread() {
    let tree = TestDirectory::new();
    let fake_bin = tree.directory("fake-bin");
    let codex_home = tree.directory("codex-home");
    let project = tree.directory("project");
    tree.directory("project/.git");
    tree.file(
        "codex-home/sessions/2999/01/rollout-2999-01-01T00-00-00-old-thread.jsonl",
        concat!(
            "{\"timestamp\":\"2999-01-01T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"old-thread\"}}\n",
            "{\"timestamp\":\"2999-01-01T00:00:01Z\",\"type\":\"turn_context\",\"payload\":{\"turn_id\":\"turn-1\"}}\n",
            "{\"timestamp\":\"2999-01-01T00:00:02Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call\",\"input\":\"cat /fixture/example/SKILL.md\"}}\n"
        ),
    );
    tree.file(
        "codex-home/sessions/2999/01/rollout-2999-01-01T00-00-00-current-alias.jsonl",
        concat!(
            "{\"timestamp\":\"2999-01-01T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"current-thread\"}}\n",
            "{\"timestamp\":\"2999-01-01T00:00:03Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call\",\"input\":\"cat /fixture/example/SKILL.md\"}}\n"
        ),
    );
    write_fake_codex(&fake_bin);

    let output = Command::new(env!("CARGO_BIN_EXE_gr"))
        .args(["inspect", "codex", "--json", "skills"])
        .current_dir(&project)
        .env("PATH", &fake_bin)
        .env("CODEX_HOME", &codex_home)
        .env("CODEX_THREAD_ID", "current-thread")
        .output()
        .expect("gr should run");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");

    assert_eq!(output.status.code(), Some(0), "{stderr}");
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("skills output should be JSON");
    assert_eq!(report["schemaVersion"], 1);
    assert_eq!(report["kind"], "skills");
    assert_eq!(report["countingBasis"], "unique_thread_turns");
    assert_eq!(
        report["evidenceLimitations"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(report["summary"]["total"], 1);
    assert_eq!(report["summary"]["recent"], 1);
    assert_eq!(
        report["cleanupPolicy"],
        "plugin_system_and_admin_skills_are_managed_and_not_manual_cleanup_candidates"
    );
    assert_eq!(report["cleanup"]["keep"], 1);
    assert_eq!(report["cleanup"]["manualReview"], 0);
    assert_eq!(report["itemView"], "all");
    assert_eq!(report["itemsReturned"], 1);
    assert_eq!(report["itemsOmitted"], 0);
    assert_eq!(report["coverage"]["rolloutsDiscovered"], 2);
    assert_eq!(report["coverage"]["rolloutsScanned"], 1);
    assert_eq!(report["coverage"]["rolloutsExcludedCurrent"], 1);
    assert_eq!(report["coverage"]["discoveryErrors"], 0);
    assert_eq!(report["items"][0]["observedUsesInWindow"], 1);
    assert_eq!(report["items"][0]["origin"], "personal");
    assert_eq!(report["items"][0]["cleanupDisposition"], "keep");
    assert_eq!(
        report["items"][0]["manifestPath"],
        "/fixture/example/SKILL.md"
    );
    assert_eq!(
        report["items"][0]["lastObservedUseAt"],
        "2999-01-01T00:00:02Z"
    );
    assert_eq!(report["items"][0]["lastEvidence"]["threadId"], "old-thread");

    let output = Command::new(env!("CARGO_BIN_EXE_gr"))
        .args(["inspect", "codex", "skills", "--actionable", "--json"])
        .current_dir(&project)
        .env("PATH", &fake_bin)
        .env("CODEX_HOME", &codex_home)
        .env("CODEX_THREAD_ID", "current-thread")
        .output()
        .expect("gr should run the actionable skills view");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert_eq!(output.status.code(), Some(0), "{stderr}");
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("actionable skills output should be JSON");
    assert_eq!(report["summary"]["total"], 1);
    assert_eq!(report["cleanup"]["keep"], 1);
    assert_eq!(report["itemView"], "actionable");
    assert_eq!(report["itemsReturned"], 0);
    assert_eq!(report["itemsOmitted"], 1);
    assert_eq!(report["items"].as_array().map(Vec::len), Some(0));
}

#[test]
fn links_plugins_to_active_skills_and_observed_use_without_cleanup_claims() {
    let tree = TestDirectory::new();
    let fake_bin = tree.directory("fake-bin");
    let codex_home = tree.directory("codex-home");
    let project = tree.directory("project");
    tree.directory("project/.git");
    let skill_manifest = tree.file(
        "codex-home/plugins/cache/market/alpha/1.0.0/skills/alpha-skill/SKILL.md",
        "# Alpha skill\n",
    );
    let rollout = concat!(
        "{\"timestamp\":\"2999-01-01T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"plugin-thread\"}}\n",
        "{\"timestamp\":\"2999-01-01T00:00:01Z\",\"type\":\"turn_context\",\"payload\":{\"turn_id\":\"turn-1\"}}\n",
        "{\"timestamp\":\"2999-01-01T00:00:02Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call\",\"input\":\"cat __SKILL_MANIFEST__\"}}\n"
    )
    .replace("__SKILL_MANIFEST__", &skill_manifest.to_string_lossy());
    tree.file(
        "codex-home/sessions/2999/01/rollout-2999-01-01T00-00-00-plugin-use.jsonl",
        &rollout,
    );
    write_fake_codex(&fake_bin);

    let output = Command::new(env!("CARGO_BIN_EXE_gr"))
        .args(["inspect", "codex", "plugins", "--json"])
        .current_dir(&project)
        .env("PATH", &fake_bin)
        .env("CODEX_HOME", &codex_home)
        .env("FAKE_CASE", "plugins-linked-version-fail")
        .output()
        .expect("gr should run the plugins drilldown");
    let stdout = assert_report(output, 0, "BASELINE_OK");
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("plugins output should be JSON");

    assert_eq!(report["kind"], "plugins");
    assert_eq!(report["summary"]["installed"], 2);
    assert_eq!(report["summary"]["enabled"], 1);
    assert_eq!(report["summary"]["pluginSkills"], 1);
    assert_eq!(report["summary"]["linkedPluginSkills"], 1);
    assert_eq!(report["summary"]["pluginsWithSkills"], 1);
    assert_eq!(report["summary"]["pluginsWithObservedSkillUse"], 1);
    assert!(report["skillEvidence"]["observedAt"].is_string());
    assert_eq!(report["skillEvidence"]["thresholds"]["recentDays"], 7);
    assert_eq!(
        report["skillEvidence"]["thresholds"]["unobservedHistoryDays"],
        30
    );
    assert_eq!(
        report["skillEvidence"]["skillCountingBasis"],
        "unique_thread_turns"
    );
    assert_eq!(
        report["skillEvidence"]["pluginAggregationBasis"],
        "sum_of_per_skill_unique_thread_turn_counts"
    );
    assert_eq!(
        report["skillEvidence"]["invalidPluginIds"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        report["skillEvidence"]["duplicatePluginIds"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(report["skillEvidence"]["coverage"]["status"], "complete");
    assert_eq!(report["skillEvidence"]["linkCoverage"]["linked"], 1);
    assert_eq!(report["skillEvidence"]["linkCoverage"]["unlinked"], 0);
    assert_eq!(report["skillEvidence"]["linkCoverage"]["ambiguous"], 0);
    assert_eq!(report["items"].as_array().map(Vec::len), Some(2));
    assert_eq!(report["items"][0]["pluginId"], "alpha@market");
    assert_eq!(report["items"][0]["name"], "alpha");
    assert_eq!(report["items"][0]["marketplace"], "market");
    assert_eq!(report["items"][0]["version"], "1.0.0");
    assert_eq!(report["items"][0]["enabled"], true);
    assert_eq!(report["items"][0]["authPolicy"], "ON_USE");
    assert_eq!(report["items"][0]["skillSummary"]["active"], 1);
    assert_eq!(report["items"][0]["skillSummary"]["withObservedUse"], 1);
    assert_eq!(
        report["items"][0]["skillSummary"]["observedUsesInWindow"],
        1
    );
    assert_eq!(report["items"][0]["skills"][0]["name"], "alpha:skill");
    assert_eq!(
        report["items"][0]["skills"][0]["manifestPath"],
        skill_manifest.to_string_lossy().as_ref()
    );
    assert_eq!(report["items"][0]["skills"][0]["signal"], "recent");
    assert_eq!(
        report["items"][0]["skills"][0]["lastObservedUseAt"],
        "2999-01-01T00:00:02Z"
    );
    assert_eq!(report["items"][1]["pluginId"], "zeta@market");
    assert_eq!(report["items"][1]["enabled"], false);
    assert_eq!(report["items"][1]["authPolicy"], serde_json::Value::Null);
    assert_eq!(report["items"][1]["skillSummary"]["active"], 0);
    assert_eq!(report["findings"].as_array().map(Vec::len), Some(0));
    assert!(report.get("available").is_none());
    assert!(report.get("cleanup").is_none());
    assert!(report.get("usage").is_none());
    assert!(report["items"][0].get("source").is_none());
    assert!(!stdout.contains(r#""sourcePath""#));

    let human = Command::new(env!("CARGO_BIN_EXE_gr"))
        .args(["inspect", "codex", "plugins"])
        .current_dir(&project)
        .env("PATH", &fake_bin)
        .env("CODEX_HOME", &codex_home)
        .env("FAKE_CASE", "plugins-linked-version-fail")
        .output()
        .expect("gr should render the plugin inventory");
    let human_stdout = String::from_utf8(human.stdout).expect("stdout should be UTF-8");
    let human_stderr = String::from_utf8(human.stderr).expect("stderr should be UTF-8");
    assert_eq!(human.status.code(), Some(0), "{human_stderr}");
    assert!(human_stderr.is_empty());
    assert!(
        human_stdout.contains("Codex plugins: 2 installed, 1 enabled; 1/1 plugin skills linked")
    );
    assert!(human_stdout.contains("skills 1 active, 1 observed"));
    assert!(human_stdout.contains("Verdict: BASELINE_OK"));
}

#[test]
fn plugin_inventory_fails_closed_with_section_specific_reports() {
    let tree = TestDirectory::new();
    let empty_bin = tree.directory("empty-bin");
    let fake_bin = tree.directory("fake-bin");
    let codex_home = tree.directory("codex-home");
    let project = tree.directory("project");
    tree.directory("project/.git");
    write_fake_codex(&fake_bin);

    let blocked = run_gr_plugins(&empty_bin, &codex_home, &project, None);
    let stdout = assert_report(blocked, 4, "BLOCKED");
    assert!(stdout.contains(r#""kind": "plugins""#));
    assert!(stdout.contains(r#""code": "plugins.unavailable""#));
    assert!(!stdout.contains(r#""drilldowns""#));

    for (fixture, code) in [
        ("plugins-fail", "probe.plugins.failed"),
        ("plugins-malformed", "probe.plugins.invalid_json"),
        ("skills-fail", "plugins.skills.failed"),
    ] {
        let output = run_gr_plugins(&fake_bin, &codex_home, &project, Some(fixture));
        let stdout = assert_report(output, 3, "INCOMPLETE");
        assert!(stdout.contains(r#""kind": "plugins""#));
        assert!(stdout.contains(&format!(r#""code": "{code}""#)));
    }
}

#[test]
fn ignores_the_exact_term_dumb_doctor_limitation() {
    let tree = TestDirectory::new();
    let fake_bin = tree.directory("fake-bin");
    let codex_home = tree.directory("codex-home");
    let project = tree.directory("project");
    tree.directory("project/.git");
    write_fake_codex(&fake_bin);

    let output = run_gr(&fake_bin, &codex_home, &project, Some("doctor-term-dumb"));
    let stdout = assert_report(output, 0, "BASELINE_OK");
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("summary output should be JSON");

    assert_eq!(report["doctor"]["status"], "ok");
    assert_eq!(report["doctor"]["ignoredCheckCount"], 1);
    assert_eq!(report["findings"].as_array().map(Vec::len), Some(0));
}

#[test]
fn rejects_clean_doctor_json_from_abnormal_termination() {
    let tree = TestDirectory::new();
    let fake_bin = tree.directory("fake-bin");
    let codex_home = tree.directory("codex-home");
    let project = tree.directory("project");
    tree.directory("project/.git");
    write_fake_codex(&fake_bin);

    for fixture in ["doctor-ok-exit-2", "doctor-ok-signal"] {
        let output = run_gr(&fake_bin, &codex_home, &project, Some(fixture));
        let stdout = assert_report(output, 3, "INCOMPLETE");
        assert!(stdout.contains(r#""code": "probe.doctor.failed""#));
    }
}

#[test]
fn distinguishes_empty_and_invalid_utf8_version_output() {
    let tree = TestDirectory::new();
    let fake_bin = tree.directory("fake-bin");
    let codex_home = tree.directory("codex-home");
    let project = tree.directory("project");
    tree.directory("project/.git");
    write_fake_codex(&fake_bin);

    let empty = run_gr(&fake_bin, &codex_home, &project, Some("version-empty"));
    let stdout = assert_report(empty, 3, "INCOMPLETE");
    assert!(stdout.contains(r#""code": "probe.version.empty_output""#));

    let invalid = run_gr(
        &fake_bin,
        &codex_home,
        &project,
        Some("version-invalid-utf8"),
    );
    let stdout = assert_report(invalid, 3, "INCOMPLETE");
    assert!(stdout.contains(r#""code": "probe.version.invalid_utf8""#));

    let signal = run_gr(&fake_bin, &codex_home, &project, Some("version-signal"));
    let stdout = assert_report(signal, 3, "INCOMPLETE");
    assert!(stdout.contains(r#""code": "probe.version.failed""#));
    assert!(stdout.contains("termination by signal"));
}

#[test]
fn fails_closed_at_every_probe_boundary() {
    let tree = TestDirectory::new();
    let fake_bin = tree.directory("fake-bin");
    let codex_home = tree.directory("codex-home");
    let project = tree.directory("project");
    tree.directory("project/.git");
    write_fake_codex(&fake_bin);

    let cases = [
        ("doctor-malformed", "probe.doctor.invalid_json"),
        ("features-fail", "probe.features.failed"),
        ("features-invalid-utf8", "probe.features.invalid_output"),
        ("plugins-fail", "probe.plugins.failed"),
        ("plugins-malformed", "probe.plugins.invalid_json"),
        ("marketplaces-fail", "probe.marketplaces.failed"),
        ("marketplaces-malformed", "probe.marketplaces.invalid_json"),
        ("mcp-fail", "probe.mcp.failed"),
        ("mcp-malformed", "probe.mcp.invalid_json"),
        ("skills-fail", "probe.skills.failed"),
        (
            "doctor-malformed-and-plugins-fail",
            "probe.doctor.invalid_json",
        ),
    ];

    for (fixture, expected_code) in cases {
        let output = run_gr(&fake_bin, &codex_home, &project, Some(fixture));
        let stdout = assert_report(output, 3, "INCOMPLETE");
        assert!(
            stdout.contains(&format!(r#""code": "{expected_code}""#)),
            "fixture {fixture}: {stdout}"
        );
    }
}

#[test]
fn fails_closed_when_instruction_configuration_is_unavailable_or_invalid() {
    let tree = TestDirectory::new();
    let fake_bin = tree.directory("fake-bin");
    let project = tree.directory("project");
    tree.directory("project/.git");
    write_fake_codex(&fake_bin);

    let output = Command::new(env!("CARGO_BIN_EXE_gr"))
        .args(["inspect", "codex", "--json"])
        .current_dir(&project)
        .env("PATH", &fake_bin)
        .env_remove("CODEX_HOME")
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .output()
        .expect("gr should run");

    let stdout = assert_report(output, 3, "INCOMPLETE");
    assert!(stdout.contains(r#""code": "probe.instructions.failed""#));

    let codex_home = tree.directory("invalid-codex-home");
    tree.file("invalid-codex-home/config.toml", "not valid = [toml");
    let output = run_gr(&fake_bin, &codex_home, &project, None);
    let stdout = assert_report(output, 3, "INCOMPLETE");
    assert!(stdout.contains(r#""code": "probe.instructions.failed""#));
}

#[test]
fn reports_project_and_instruction_sources_from_the_fixture() {
    let tree = TestDirectory::new();
    let fake_bin = tree.directory("fake-bin");
    let codex_home = tree.directory("codex-home");
    let project = tree.directory("project");
    tree.directory("project/.git");
    let agents = tree.file("project/AGENTS.md", "fixture instructions");
    let project = fs::canonicalize(project).expect("fixture project should canonicalize");
    let agents = fs::canonicalize(agents).expect("fixture AGENTS.md should canonicalize");
    tree.file(
        "codex-home/config.toml",
        &format!(
            "[projects.\"{}\"]\ntrust_level = \"trusted\"\n",
            project.display()
        ),
    );
    write_fake_codex(&fake_bin);

    let output = run_gr(&fake_bin, &codex_home, &project, None);
    let stdout = assert_report(output, 0, "BASELINE_OK");

    assert!(stdout.contains(r#""trust": "trusted""#));
    assert!(stdout.contains(&project.display().to_string()));
    assert!(stdout.contains(&agents.display().to_string()));
    assert!(stdout.contains(r#""scope": "project""#));
    assert!(stdout.contains(r#""kind": "summary""#));
    assert!(stdout.contains(r#""skills": {"#));
    assert!(stdout.contains(r#""active": 1"#));
    assert!(stdout.contains(r#""byOrigin": {"#));
    assert!(stdout.contains(r#""personal": 1"#));
    assert!(stdout.contains(r#""project": 0"#));
    assert!(stdout.contains(r#""catalogErrors": 0"#));
    assert!(stdout.contains(r#""section": "skills""#));
    assert!(stdout.contains(r#""inspect""#));
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("summary output should be JSON");
    assert_eq!(
        report["drilldowns"][0]["argv"],
        serde_json::json!(["gr", "inspect", "codex", "skills", "--actionable", "--json"])
    );
    assert_eq!(report["drilldowns"][1]["section"], "plugins");
    assert_eq!(
        report["drilldowns"][1]["argv"],
        serde_json::json!(["gr", "inspect", "codex", "plugins", "--json"])
    );
}

#[test]
fn preserves_human_error_and_clap_usage_exit_contracts() {
    let tree = TestDirectory::new();
    let fake_bin = tree.directory("fake-bin");
    let codex_home = tree.directory("codex-home");
    let project = tree.directory("project");
    tree.directory("project/.git");
    write_fake_codex(&fake_bin);

    let human = Command::new(env!("CARGO_BIN_EXE_gr"))
        .args(["inspect", "codex"])
        .current_dir(&project)
        .env("PATH", &fake_bin)
        .env("CODEX_HOME", &codex_home)
        .env("FAKE_CASE", "doctor-ok-signal")
        .output()
        .expect("gr should run");
    let human_stdout = String::from_utf8(human.stdout).expect("stdout should be UTF-8");
    let human_stderr = String::from_utf8(human.stderr).expect("stderr should be UTF-8");

    assert_eq!(human.status.code(), Some(3));
    assert!(human_stdout.is_empty());
    assert!(human_stderr.contains("termination by signal"));

    let usage = Command::new(env!("CARGO_BIN_EXE_gr"))
        .arg("inspect")
        .output()
        .expect("gr should render usage");
    let usage_stderr = String::from_utf8(usage.stderr).expect("stderr should be UTF-8");

    assert_eq!(usage.status.code(), Some(2));
    assert!(usage_stderr.contains("Usage:"));
}

fn run_gr(bin_path: &Path, codex_home: &Path, current_dir: &Path, fixture: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gr"));
    command
        .args(["inspect", "codex", "--json"])
        .current_dir(current_dir)
        .env("PATH", bin_path)
        .env("CODEX_HOME", codex_home);

    if let Some(fixture) = fixture {
        command.env("FAKE_CASE", fixture);
    }

    command.output().expect("gr should run")
}

fn run_gr_plugins(
    bin_path: &Path,
    codex_home: &Path,
    current_dir: &Path,
    fixture: Option<&str>,
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gr"));
    command
        .args(["inspect", "codex", "plugins", "--json"])
        .current_dir(current_dir)
        .env("PATH", bin_path)
        .env("CODEX_HOME", codex_home);

    if let Some(fixture) = fixture {
        command.env("FAKE_CASE", fixture);
    }

    command.output().expect("gr should run")
}

fn assert_report(output: Output, expected_exit: i32, expected_verdict: &str) -> String {
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");

    assert_eq!(output.status.code(), Some(expected_exit), "{stderr}");
    assert!(stdout.trim_start().starts_with('{'), "{stdout}");
    assert!(stdout.contains(r#""schemaVersion": 1"#), "{stdout}");
    assert!(
        stdout.contains(&format!(r#""verdict": "{expected_verdict}""#)),
        "{stdout}"
    );

    stdout
}

fn write_fake_codex(bin_path: &Path) {
    let path = bin_path.join("codex");
    fs::write(
        &path,
        r#"#!/bin/sh
case "$1" in
  --version)
    case "$FAKE_CASE" in
      version-fail|plugins-linked-version-fail) exit 7 ;;
      version-empty) printf '\n' ;;
      version-invalid-utf8) printf '\377' ;;
      version-signal) kill -TERM "$$" ;;
      *) printf '%s\n' 'codex-cli test' ;;
    esac
    ;;
  doctor)
    case "$FAKE_CASE" in
      doctor-fail)
        printf '%s\n' '{"schemaVersion":1,"overallStatus":"fail","codexVersion":"test","checks":{"fixture":{"id":"fixture","status":"fail","summary":"fixture failure"}}}'
        exit 1
        ;;
      doctor-term-dumb)
        printf '%s\n' '{"schemaVersion":1,"overallStatus":"fail","codexVersion":"test","checks":{"terminal.env":{"id":"terminal.env","status":"fail","summary":"TERM=dumb - colors and cursor control are disabled"}}}'
        exit 1
        ;;
      doctor-malformed|doctor-malformed-and-plugins-fail)
        printf '%s\n' '{invalid'
        ;;
      doctor-ok-exit-2)
        printf '%s\n' '{"schemaVersion":1,"overallStatus":"ok","codexVersion":"test","checks":{"fixture":{"id":"fixture","status":"ok","summary":"fixture ok"}}}'
        exit 2
        ;;
      doctor-ok-signal)
        printf '%s\n' '{"schemaVersion":1,"overallStatus":"ok","codexVersion":"test","checks":{"fixture":{"id":"fixture","status":"ok","summary":"fixture ok"}}}'
        kill -TERM "$$"
        ;;
      *)
        printf '%s\n' '{"schemaVersion":1,"overallStatus":"ok","codexVersion":"test","checks":{"fixture":{"id":"fixture","status":"ok","summary":"fixture ok"}}}'
        ;;
    esac
    ;;
  features)
    case "$FAKE_CASE" in
      features-fail) exit 7 ;;
      features-invalid-utf8) printf '\377' ;;
      *) printf '%s\n' 'fixture stable true' ;;
    esac
    ;;
  plugin)
    if [ "$2" = "marketplace" ]; then
      case "$FAKE_CASE" in
        marketplaces-fail) exit 7 ;;
        marketplaces-malformed) printf '%s\n' '{}' ;;
        *) printf '%s\n' '{"marketplaces":[]}' ;;
      esac
    else
      case "$FAKE_CASE" in
        plugins-fail|doctor-malformed-and-plugins-fail) exit 7 ;;
        plugins-malformed) printf '%s\n' '{}' ;;
        plugins-linked-version-fail)
          printf '{"installed":[{"pluginId":"zeta@market","name":"zeta","marketplaceName":"market","version":"2.0.0","installed":true,"enabled":false,"source":{"path":"%s/native-sources/zeta","source":"local"}},{"pluginId":"alpha@market","name":"alpha","marketplaceName":"market","version":"1.0.0","installed":true,"enabled":true,"authPolicy":"ON_USE","source":{"path":"%s/native-sources/alpha","source":"local"}}],"available":[]}\n' "$CODEX_HOME" "$CODEX_HOME"
          ;;
        *) printf '%s\n' '{"installed":[{"pluginId":"zeta@market","name":"zeta","marketplaceName":"market","version":"2.0.0","installed":true,"enabled":false},{"pluginId":"alpha@market","name":"alpha","marketplaceName":"market","version":"1.0.0","installed":true,"enabled":true,"authPolicy":"ON_USE"}],"available":[{"pluginId":"available@market","name":"available","marketplaceName":"market","version":"3.0.0","installed":false,"enabled":false,"authPolicy":"ON_INSTALL"}]}' ;;
      esac
    fi
    ;;
  mcp)
    case "$FAKE_CASE" in
      mcp-fail) exit 7 ;;
      mcp-malformed) printf '%s\n' '{}' ;;
      *) printf '%s\n' '[]' ;;
    esac
    ;;
  app-server)
    case "$FAKE_CASE" in
      skills-fail) exit 7 ;;
    esac
    read -r initialize_request
    printf '{"id":1,"result":{"codexHome":"%s"}}\n' "$CODEX_HOME"
    read -r skills_request
    case "$FAKE_CASE" in
      plugins-linked-version-fail)
        printf '{"id":2,"result":{"data":[{"cwd":"%s","skills":[{"name":"alpha:skill","path":"%s/plugins/cache/market/alpha/1.0.0/skills/alpha-skill/SKILL.md","scope":"user","enabled":true},{"name":"personal","path":"/fixture/personal/SKILL.md","scope":"user","enabled":true}],"errors":[]}]}}\n' "$PWD" "$CODEX_HOME"
        ;;
      *)
        printf '{"id":2,"result":{"data":[{"cwd":"%s","skills":[{"name":"example","path":"/fixture/example/SKILL.md","scope":"user","enabled":true}],"errors":[]}]}}\n' "$PWD"
        ;;
    esac
    ;;
  *)
    exit 9
    ;;
esac
"#,
    )
    .expect("fake codex should be written");

    let mut permissions = fs::metadata(&path)
        .expect("fake codex metadata should exist")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("fake codex should be executable");
}
