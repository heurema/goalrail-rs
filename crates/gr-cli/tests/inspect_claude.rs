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
            "goalrail-cli-claude-test-{}-{sequence}",
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
fn emits_json_and_distinct_exit_codes_for_every_verdict() {
    let tree = TestDirectory::new();
    let empty_bin = tree.directory("empty-bin");
    let fake_bin = tree.directory("fake-bin");
    let claude_home = tree.directory("claude-home");
    let project = tree.directory("project");
    tree.directory("project/.git");
    write_fake_claude(&fake_bin);

    let blocked = run_gr(&empty_bin, &claude_home, &project, None);
    let stdout = assert_report(blocked, 4, "BLOCKED");
    assert!(stdout.contains(r#""code": "claude.unavailable""#));

    let incomplete = run_gr(&fake_bin, &claude_home, &project, Some("version-fail"));
    let stdout = assert_report(incomplete, 3, "INCOMPLETE");
    assert!(stdout.contains(r#""code": "probe.version.failed""#));

    let review = run_gr(
        &fake_bin,
        &claude_home,
        &project,
        Some("plugins-missing-path"),
    );
    let stdout = assert_report(review, 1, "REVIEW");
    assert!(stdout.contains(r#""code": "plugins.install_path_missing""#));

    let baseline = run_gr(&fake_bin, &claude_home, &project, None);
    assert_report(baseline, 0, "BASELINE_OK");
}

#[test]
fn reports_observed_local_evidence_with_explicit_limitations() {
    let tree = TestDirectory::new();
    let fake_bin = tree.directory("fake-bin");
    let claude_home = tree.directory("claude-home");
    let project = tree.directory("project");
    tree.directory("project/.git");
    let project = fs::canonicalize(project).expect("fixture project should canonicalize");
    let instructions = tree.file("project/CLAUDE.md", "project instructions");
    let instructions =
        fs::canonicalize(instructions).expect("fixture CLAUDE.md should canonicalize");
    tree.file("claude-home/CLAUDE.md", "global instructions");
    tree.file("claude-home/skills/personal-one/SKILL.md", "personal");
    tree.file("claude-home/skills/personal-two/SKILL.md", "personal");
    tree.file("project/.claude/skills/project-one/SKILL.md", "project");
    tree.file(
        "claude-home/.claude.json",
        &format!(
            r#"{{
                "mcpServers": {{ "user-one": {{}} }},
                "projects": {{ "{}": {{ "mcpServers": {{ "local-one": {{}}, "local-two": {{}} }} }} }}
            }}"#,
            project.display()
        ),
    );
    tree.file("project/.mcp.json", r#"{"mcpServers": {"shared": {}}}"#);
    write_fake_claude(&fake_bin);

    let output = run_gr(&fake_bin, &claude_home, &project, None);
    let stdout = assert_report(output, 0, "BASELINE_OK");
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("summary output should be JSON");

    assert_eq!(report["kind"], "summary");
    assert_eq!(report["claudeVersion"], "2.9.9 (Claude Code test)");
    assert_eq!(report["plugins"]["installed"], 2);
    assert_eq!(report["plugins"]["enabled"], 1);
    assert_eq!(report["plugins"]["byScope"]["user"], 1);
    assert_eq!(report["plugins"]["byScope"]["project"], 1);
    assert_eq!(report["marketplaces"]["configured"], 2);
    assert_eq!(report["mcp"]["configured"], 4);
    assert_eq!(report["mcp"]["byScope"]["user"], 1);
    assert_eq!(report["mcp"]["byScope"]["project"], 1);
    assert_eq!(report["mcp"]["byScope"]["local"], 2);
    assert_eq!(
        report["mcp"]["basis"],
        "configuration_files_without_health_checks"
    );
    assert_eq!(report["skills"]["personal"], 2);
    assert_eq!(report["skills"]["project"], 1);
    assert_eq!(
        report["skills"]["basis"],
        "skill_manifest_directories_on_disk"
    );
    assert_eq!(report["project"]["root"], project.display().to_string());
    assert_eq!(
        report["project"]["rootBasis"],
        "nearest_ancestor_containing_dot_git"
    );
    assert_eq!(report["project"]["stateEntry"], "present");
    assert_eq!(
        report["project"]["stateEntryPath"],
        project.display().to_string()
    );
    assert_eq!(
        report["project"]["currentDir"],
        project.display().to_string()
    );
    assert_eq!(
        report["project"]["instructionSources"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        report["project"]["instructionSources"][0]["scope"],
        "global"
    );
    assert_eq!(
        report["project"]["instructionSources"][1]["path"],
        instructions.display().to_string()
    );
    assert_eq!(
        report["project"]["instructionSources"][1]["sizeBytes"],
        "project instructions".len()
    );
    assert_eq!(
        report["evidenceLimitations"].as_array().map(Vec::len),
        Some(4)
    );
    assert_eq!(report["findings"].as_array().map(Vec::len), Some(0));
    assert!(report.get("doctor").is_none());
}

#[test]
fn fails_closed_at_every_probe_and_configuration_boundary() {
    let tree = TestDirectory::new();
    let fake_bin = tree.directory("fake-bin");
    let claude_home = tree.directory("claude-home");
    let project = tree.directory("project");
    tree.directory("project/.git");
    write_fake_claude(&fake_bin);

    let cases = [
        ("version-empty", "probe.version.empty_output"),
        ("version-invalid-utf8", "probe.version.invalid_utf8"),
        ("plugins-fail", "probe.plugins.failed"),
        ("plugins-malformed", "probe.plugins.invalid_json"),
        ("marketplaces-fail", "probe.marketplaces.failed"),
        ("marketplaces-malformed", "probe.marketplaces.invalid_json"),
    ];

    for (fixture, expected_code) in cases {
        let output = run_gr(&fake_bin, &claude_home, &project, Some(fixture));
        let stdout = assert_report(output, 3, "INCOMPLETE");
        assert!(
            stdout.contains(&format!(r#""code": "{expected_code}""#)),
            "fixture {fixture}: {stdout}"
        );
    }

    tree.file("claude-home/.claude.json", "{not json");
    let output = run_gr(&fake_bin, &claude_home, &project, None);
    let stdout = assert_report(output, 3, "INCOMPLETE");
    assert!(stdout.contains(r#""code": "probe.config.invalid""#));
}

#[test]
fn fails_closed_when_no_claude_home_can_be_resolved() {
    let tree = TestDirectory::new();
    let fake_bin = tree.directory("fake-bin");
    let project = tree.directory("project");
    tree.directory("project/.git");
    write_fake_claude(&fake_bin);

    let output = Command::new(env!("CARGO_BIN_EXE_gr"))
        .args(["inspect", "claude", "--json"])
        .current_dir(&project)
        .env("PATH", &fake_bin)
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .output()
        .expect("gr should run");

    let stdout = assert_report(output, 3, "INCOMPLETE");
    assert!(stdout.contains(r#""code": "probe.home.unresolved""#));
}

#[test]
fn renders_human_output_and_routes_failures_to_stderr() {
    let tree = TestDirectory::new();
    let fake_bin = tree.directory("fake-bin");
    let claude_home = tree.directory("claude-home");
    let project = tree.directory("project");
    tree.directory("project/.git");
    write_fake_claude(&fake_bin);

    let human = Command::new(env!("CARGO_BIN_EXE_gr"))
        .args(["inspect", "claude"])
        .current_dir(&project)
        .env("PATH", &fake_bin)
        .env("CLAUDE_CONFIG_DIR", &claude_home)
        .output()
        .expect("gr should render the Claude summary");
    let stdout = String::from_utf8(human.stdout).expect("stdout should be UTF-8");
    let stderr = String::from_utf8(human.stderr).expect("stderr should be UTF-8");

    assert_eq!(human.status.code(), Some(0), "{stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("Claude version: 2.9.9 (Claude Code test)"));
    assert!(stdout.contains("Claude plugins: 2 installed, 1 enabled"));
    assert!(stdout.contains("Claude marketplaces: 2 configured"));
    assert!(stdout.contains("no health state observed"));
    assert!(stdout.contains("Verdict: BASELINE_OK"));
    assert!(stdout.contains("Evidence limitations:"));

    let failed = Command::new(env!("CARGO_BIN_EXE_gr"))
        .args(["inspect", "claude"])
        .current_dir(&project)
        .env("PATH", &fake_bin)
        .env("CLAUDE_CONFIG_DIR", &claude_home)
        .env("FAKE_CASE", "version-fail")
        .output()
        .expect("gr should report a failed probe");
    let stdout = String::from_utf8(failed.stdout).expect("stdout should be UTF-8");
    let stderr = String::from_utf8(failed.stderr).expect("stderr should be UTF-8");

    assert_eq!(failed.status.code(), Some(3));
    assert!(stdout.is_empty());
    assert!(stderr.contains("claude --version failed with exit code 7"));
}

#[test]
fn inspecting_claude_writes_nothing_to_the_claude_home() {
    let tree = TestDirectory::new();
    let fake_bin = tree.directory("fake-bin");
    let claude_home = tree.directory("claude-home");
    let project = tree.directory("project");
    tree.directory("project/.git");
    tree.file("claude-home/CLAUDE.md", "global instructions");
    write_fake_claude(&fake_bin);

    let before = directory_snapshot(&claude_home);
    let output = run_gr(&fake_bin, &claude_home, &project, None);
    assert_report(output, 0, "BASELINE_OK");
    let after = directory_snapshot(&claude_home);

    assert_eq!(before, after);
}

#[test]
fn runs_only_the_three_documented_claude_commands() {
    let tree = TestDirectory::new();
    let fake_bin = tree.directory("fake-bin");
    let claude_home = tree.directory("claude-home");
    let project = tree.directory("project");
    tree.directory("project/.git");
    write_fake_claude(&fake_bin);
    let argv_log = tree.path.join("argv.log");

    let output = Command::new(env!("CARGO_BIN_EXE_gr"))
        .args(["inspect", "claude", "--json"])
        .current_dir(&project)
        .env("PATH", &fake_bin)
        .env("CLAUDE_CONFIG_DIR", &claude_home)
        .env("FAKE_ARGV_LOG", &argv_log)
        .output()
        .expect("gr should run");
    assert_report(output, 0, "BASELINE_OK");

    let mut invocations: Vec<String> = fs::read_to_string(&argv_log)
        .expect("the fake executable should have recorded its argv")
        .lines()
        .map(str::to_owned)
        .collect();
    invocations.sort();

    assert_eq!(
        invocations,
        vec![
            "--version".to_owned(),
            "plugin list --json".to_owned(),
            "plugin marketplace list --json".to_owned(),
        ]
    );
}

fn directory_snapshot(root: &Path) -> Vec<(PathBuf, u64)> {
    let mut entries = Vec::new();
    let mut pending = vec![root.to_path_buf()];

    while let Some(directory) = pending.pop() {
        let Ok(children) = fs::read_dir(&directory) else {
            continue;
        };
        for child in children {
            let path = child.expect("fixture entry should be readable").path();
            let metadata = fs::metadata(&path).expect("fixture metadata should be readable");
            if metadata.is_dir() {
                pending.push(path.clone());
                entries.push((path, 0));
            } else {
                entries.push((path, metadata.len()));
            }
        }
    }
    entries.sort();

    entries
}

fn run_gr(
    bin_path: &Path,
    claude_home: &Path,
    current_dir: &Path,
    fixture: Option<&str>,
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gr"));
    command
        .args(["inspect", "claude", "--json"])
        .current_dir(current_dir)
        .env("PATH", bin_path)
        .env("CLAUDE_CONFIG_DIR", claude_home);

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

fn write_fake_claude(bin_path: &Path) {
    let path = bin_path.join("claude");
    fs::write(
        &path,
        r#"#!/bin/sh
# Record the exact argv of every invocation, then accept only the three
# documented commands. Anything else fails, so a probe that drifts off the
# AD-8 evidence surface cannot pass these tests.
if [ -n "$FAKE_ARGV_LOG" ]; then
  printf '%s\n' "$*" >>"$FAKE_ARGV_LOG"
fi

case "$*" in
  '--version')
    case "$FAKE_CASE" in
      version-fail) exit 7 ;;
      version-empty) printf '\n' ;;
      version-invalid-utf8) printf '\377' ;;
      *) printf '%s\n' '2.9.9 (Claude Code test)' ;;
    esac
    ;;
  'plugin marketplace list --json')
    case "$FAKE_CASE" in
      marketplaces-fail) exit 7 ;;
      marketplaces-malformed) printf '%s\n' '{}' ;;
      *) printf '%s\n' '[{"name":"first","source":"github"},{"name":"second","source":"local"}]' ;;
    esac
    ;;
  'plugin list --json')
    case "$FAKE_CASE" in
      plugins-fail) exit 7 ;;
      plugins-malformed) printf '%s\n' '{}' ;;
      plugins-missing-path)
        printf '%s\n' '[{"id":"alpha@market","version":"1.0.0","scope":"user","enabled":true,"installPath":"/goalrail/fixture/absent"}]'
        ;;
      *)
        printf '%s\n' '[{"id":"alpha@market","version":"1.0.0","scope":"user","enabled":true},{"id":"beta@market","version":"2.0.0","scope":"project","enabled":false}]'
        ;;
    esac
    ;;
  *)
    printf 'undocumented claude invocation: %s\n' "$*" >&2
    exit 9
    ;;
esac
"#,
    )
    .expect("fake claude should be written");

    let mut permissions = fs::metadata(&path)
        .expect("fake claude metadata should exist")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("fake claude should be executable");
}
