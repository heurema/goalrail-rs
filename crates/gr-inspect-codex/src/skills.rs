use std::{
    collections::{BTreeMap, HashSet},
    env,
    fmt::Write as _,
    fs::{self, File},
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use memchr::memmem;
use serde::{Deserialize, Serialize};
use serde_json::{json, value::RawValue};

use crate::{
    PROBE_TIMEOUT, Verdict,
    report::{
        CodexFailureReport, REPORT_SCHEMA_VERSION, ReportFinding, ReportKind, SkillOriginSummary,
    },
};

const RECENT_DAYS: u64 = 7;
const UNOBSERVED_HISTORY_DAYS: u64 = 30;
const SECONDS_PER_DAY: i64 = 86_400;
const HISTORY_SCAN_TIMEOUT: Duration = Duration::from_secs(15);
const EVIDENCE_BASIS: &str = "exact_skill_manifest_path_in_retained_rollout_tool_calls";
const COUNTING_BASIS: &str = "unique_thread_turns";
const EVIDENCE_LIMITATIONS: [&str; 2] = [
    "renamed_or_previous_version_manifest_paths_can_undercount",
    "inlined_skill_context_without_a_manifest_path_is_not_observed",
];

#[derive(Debug)]
pub struct SkillsInspectionOutcome {
    report: SkillsOutcomeReport,
}

#[derive(Debug)]
enum SkillsOutcomeReport {
    Complete(Box<SkillsInspectionReport>),
    Failure(CodexFailureReport),
}

impl SkillsInspectionOutcome {
    pub const fn verdict(&self) -> Verdict {
        match &self.report {
            SkillsOutcomeReport::Complete(report) => report.verdict,
            SkillsOutcomeReport::Failure(report) => report.verdict,
        }
    }

    pub const fn is_failure(&self) -> bool {
        matches!(self.report, SkillsOutcomeReport::Failure(_))
    }

    pub fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        match &self.report {
            SkillsOutcomeReport::Complete(report) => serde_json::to_string_pretty(report),
            SkillsOutcomeReport::Failure(report) => report.to_pretty_json(),
        }
    }

    pub fn to_human(&self) -> String {
        match &self.report {
            SkillsOutcomeReport::Complete(report) => format_human_report(report),
            SkillsOutcomeReport::Failure(report) => report
                .findings
                .first()
                .map(|finding| finding.message.clone())
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillsInspectionReport {
    schema_version: u32,
    kind: ReportKind,
    observed_at: String,
    verdict: Verdict,
    coverage: SkillCoverage,
    evidence_basis: &'static str,
    counting_basis: &'static str,
    evidence_limitations: [&'static str; 2],
    thresholds: SkillThresholds,
    summary: SkillUsageSummary,
    items: Vec<SkillUsageItem>,
    findings: Vec<ReportFinding>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum CoverageStatus {
    Complete,
    Partial,
    None,
}

impl CoverageStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillCoverage {
    status: CoverageStatus,
    history_from: Option<String>,
    history_through: Option<String>,
    rollouts_discovered: usize,
    rollouts_scanned: usize,
    rollouts_excluded_current: usize,
    rollouts_unreadable: usize,
    discovery_errors: usize,
    records_unreadable: usize,
    catalog_errors: usize,
    truncated: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillThresholds {
    recent_days: u64,
    unobserved_history_days: u64,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillUsageSummary {
    total: usize,
    recent: usize,
    aging: usize,
    unobserved: usize,
    insufficient_history: usize,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum SkillSignal {
    Recent,
    Aging,
    Unobserved,
    InsufficientHistory,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum SkillOrigin {
    System,
    Plugin,
    Personal,
    Project,
    Admin,
    Unknown,
}

struct SkillOriginClassifier {
    plugins_root: PathBuf,
    canonical_plugins_root: Option<PathBuf>,
}

impl SkillOriginClassifier {
    fn new(codex_home: &Path) -> Self {
        let plugins_root = codex_home.join("plugins");
        let canonical_plugins_root = fs::canonicalize(&plugins_root).ok();
        Self {
            plugins_root,
            canonical_plugins_root,
        }
    }

    fn classify(&self, path: &Path, scope: &str) -> SkillOrigin {
        let is_plugin = path.starts_with(&self.plugins_root)
            || self.canonical_plugins_root.as_ref().is_some_and(|root| {
                fs::canonicalize(path).is_ok_and(|canonical_path| canonical_path.starts_with(root))
            });
        if is_plugin {
            return SkillOrigin::Plugin;
        }

        match scope.to_ascii_lowercase().as_str() {
            "system" => SkillOrigin::System,
            "user" => SkillOrigin::Personal,
            "repo" | "project" => SkillOrigin::Project,
            "admin" => SkillOrigin::Admin,
            _ => SkillOrigin::Unknown,
        }
    }
}

impl SkillOrigin {
    const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Plugin => "plugin",
            Self::Personal => "personal",
            Self::Project => "project",
            Self::Admin => "admin",
            Self::Unknown => "unknown",
        }
    }
}

impl SkillSignal {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Recent => "recent",
            Self::Aging => "aging",
            Self::Unobserved => "unobserved",
            Self::InsufficientHistory => "insufficient_history",
        }
    }

    const fn sort_rank(self) -> u8 {
        match self {
            Self::Unobserved => 0,
            Self::Aging => 1,
            Self::Recent => 2,
            Self::InsufficientHistory => 3,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillUsageItem {
    name: String,
    scope: String,
    manifest_path: PathBuf,
    origin: SkillOrigin,
    signal: SkillSignal,
    last_observed_use_at: Option<String>,
    age_days: Option<u64>,
    observed_uses_in_window: usize,
    last_evidence: Option<SkillEvidenceRef>,
    #[serde(skip)]
    last_observed_epoch: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillEvidenceRef {
    thread_id: String,
    turn_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeResponse {
    codex_home: PathBuf,
}

#[derive(Debug, Deserialize)]
struct SkillsListResponse {
    data: Vec<SkillsListEntry>,
}

#[derive(Debug, Deserialize)]
struct SkillsListEntry {
    cwd: PathBuf,
    skills: Vec<CatalogSkill>,
    errors: Vec<SkillCatalogError>,
}

#[derive(Debug, Deserialize)]
struct CatalogSkill {
    name: String,
    path: PathBuf,
    scope: String,
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct SkillCatalogError {
    #[allow(dead_code)]
    path: PathBuf,
    #[allow(dead_code)]
    message: String,
}

struct SkillCatalog {
    codex_home: PathBuf,
    skills: Vec<CatalogSkill>,
    error_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ActiveSkillSummary {
    pub(crate) active: usize,
    pub(crate) by_origin: SkillOriginSummary,
    pub(crate) catalog_errors: usize,
}

#[derive(Debug, Deserialize)]
struct RpcEnvelope {
    id: Option<u64>,
    result: Option<Box<RawValue>>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct RolloutEnvelope {
    timestamp: Option<String>,
    #[serde(rename = "type")]
    kind: String,
    payload: Box<RawValue>,
}

#[derive(Debug, Deserialize)]
struct SessionMetadata {
    id: Option<String>,
    session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TurnMetadata {
    turn_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ToolCallPayload {
    #[serde(rename = "type")]
    kind: String,
    input: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct ObservedUseKey {
    thread_id: String,
    turn_or_event: String,
}

#[derive(Debug, Clone)]
struct ObservedUse {
    key: ObservedUseKey,
    timestamp: String,
    epoch: i64,
    evidence: SkillEvidenceRef,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SkillIdentity {
    name: String,
    manifest_path: PathBuf,
}

impl From<&CatalogSkill> for SkillIdentity {
    fn from(skill: &CatalogSkill) -> Self {
        Self {
            name: skill.name.clone(),
            manifest_path: skill.path.clone(),
        }
    }
}

#[derive(Default)]
struct UsageEvidence {
    by_skill: BTreeMap<SkillIdentity, Vec<ObservedUse>>,
    history_from: Option<(String, i64)>,
    history_through: Option<(String, i64)>,
    rollouts_discovered: usize,
    rollouts_scanned: usize,
    rollouts_excluded_current: usize,
    rollouts_unreadable: usize,
    discovery_errors: usize,
    records_unreadable: usize,
    truncated: bool,
}

struct RolloutScan {
    complete: bool,
    excluded_current: bool,
    uses: Vec<(SkillIdentity, ObservedUse)>,
    history_from: Option<(String, i64)>,
    history_through: Option<(String, i64)>,
    records_unreadable: usize,
}

#[derive(Default)]
struct RolloutDiscovery {
    paths: Vec<PathBuf>,
    errors: usize,
}

impl RolloutDiscovery {
    fn record_error(&mut self) {
        self.errors += 1;
    }
}

impl UsageEvidence {
    fn record_excluded_current(&mut self) {
        self.rollouts_excluded_current += 1;
    }

    fn record_unreadable_rollout(&mut self) {
        self.rollouts_unreadable += 1;
    }

    fn record_scan(&mut self, scan: RolloutScan) {
        if scan.excluded_current {
            self.record_excluded_current();
            return;
        }
        if scan.complete {
            self.rollouts_scanned += 1;
        } else {
            self.truncated = true;
        }
        self.records_unreadable += scan.records_unreadable;
        merge_time_range(&mut self.history_from, scan.history_from, true);
        merge_time_range(&mut self.history_through, scan.history_through, false);
        for (skill, observed_use) in scan.uses {
            self.by_skill.entry(skill).or_default().push(observed_use);
        }
    }

    fn merge_partial(&mut self, partial: Self) {
        self.rollouts_scanned += partial.rollouts_scanned;
        self.rollouts_unreadable += partial.rollouts_unreadable;
        self.rollouts_excluded_current += partial.rollouts_excluded_current;
        self.records_unreadable += partial.records_unreadable;
        self.truncated |= partial.truncated;
        merge_time_range(&mut self.history_from, partial.history_from, true);
        merge_time_range(&mut self.history_through, partial.history_through, false);
        for (skill, uses) in partial.by_skill {
            self.by_skill.entry(skill).or_default().extend(uses);
        }
    }

    fn finalize_truncation(&mut self) {
        let accounted =
            self.rollouts_scanned + self.rollouts_unreadable + self.rollouts_excluded_current;
        self.truncated |= accounted < self.rollouts_discovered;
    }
}

pub fn inspect_codex_skills() -> SkillsInspectionOutcome {
    match inspect_codex_skills_inner() {
        Ok(report) => SkillsInspectionOutcome {
            report: SkillsOutcomeReport::Complete(Box::new(report)),
        },
        Err(error) => skill_failure(error),
    }
}

pub(crate) fn inspect_active_skills() -> io::Result<ActiveSkillSummary> {
    let current_dir = env::current_dir()?;
    let SkillCatalog {
        codex_home,
        skills,
        error_count: catalog_errors,
    } = probe_skill_catalog(&current_dir)?;
    let skills = unique_active_skills(skills);
    let active = skills.len();
    let by_origin = summarize_skill_origins(&skills, &codex_home);

    Ok(ActiveSkillSummary {
        active,
        by_origin,
        catalog_errors,
    })
}

fn inspect_codex_skills_inner() -> io::Result<SkillsInspectionReport> {
    let observed_epoch = current_epoch_seconds()?;
    let observed_at = format_rfc3339(observed_epoch);
    let current_dir = env::current_dir()?;
    let SkillCatalog {
        codex_home,
        skills,
        error_count: catalog_errors,
    } = probe_skill_catalog(&current_dir)?;
    let current_thread_id = nonempty_thread_id(env::var("CODEX_THREAD_ID").ok());
    let skills = unique_active_skills(skills);

    let discovery = discover_rollouts(&codex_home);
    let mut evidence = scan_rollouts(&discovery.paths, current_thread_id.as_deref(), &skills);
    evidence.discovery_errors = discovery.errors;
    Ok(build_report(
        observed_at,
        observed_epoch,
        &codex_home,
        skills,
        evidence,
        catalog_errors,
    ))
}

fn nonempty_thread_id(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

fn unique_active_skills(mut skills: Vec<CatalogSkill>) -> Vec<CatalogSkill> {
    skills.retain(|skill| skill.enabled);
    skills.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.path.cmp(&right.path))
    });
    skills.dedup_by(|left, right| left.name == right.name && left.path == right.path);
    skills
}

fn summarize_skill_origins(skills: &[CatalogSkill], codex_home: &Path) -> SkillOriginSummary {
    let mut summary = SkillOriginSummary::default();
    let classifier = SkillOriginClassifier::new(codex_home);
    for skill in skills {
        match classifier.classify(&skill.path, &skill.scope) {
            SkillOrigin::System => summary.system += 1,
            SkillOrigin::Plugin => summary.plugin += 1,
            SkillOrigin::Personal => summary.personal += 1,
            SkillOrigin::Project => summary.project += 1,
            SkillOrigin::Admin => summary.admin += 1,
            SkillOrigin::Unknown => summary.unknown += 1,
        }
    }
    summary
}

fn skill_failure(error: io::Error) -> SkillsInspectionOutcome {
    let (verdict, code, status) = if error.kind() == io::ErrorKind::NotFound {
        (Verdict::Blocked, "skills.catalog.unavailable", "blocked")
    } else if error.kind() == io::ErrorKind::TimedOut {
        (Verdict::Incomplete, "skills.catalog.timeout", "timeout")
    } else {
        (Verdict::Incomplete, "skills.inspection.failed", "failed")
    };

    SkillsInspectionOutcome {
        report: SkillsOutcomeReport::Failure(CodexFailureReport::new(
            ReportKind::Skills,
            verdict,
            ReportFinding::new(code, status, error.to_string()),
        )),
    }
}

fn probe_skill_catalog(cwd: &Path) -> io::Result<SkillCatalog> {
    let deadline = probe_deadline(Instant::now());
    let mut child = Command::new("codex")
        .args(["app-server", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("Codex app-server stdin was not captured"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("Codex app-server stdout was not captured"))?;
    let responses = spawn_line_reader(stdout);

    let result = (|| {
        send_rpc_request(
            &mut stdin,
            &json!({
                "id": 1,
                "method": "initialize",
                "params": {
                    "clientInfo": {
                        "name": "goalrail",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "capabilities": {
                        "experimentalApi": true
                    }
                }
            }),
        )?;
        let initialize_raw = receive_rpc_result(&responses, 1, deadline)?;
        let initialize: InitializeResponse = serde_json::from_str(initialize_raw.get())
            .map_err(|error| invalid_data("invalid initialize response", error))?;

        send_rpc_request(
            &mut stdin,
            &json!({
                "id": 2,
                "method": "skills/list",
                "params": {
                    "cwds": [cwd],
                    "forceReload": true
                }
            }),
        )?;
        let skills_raw = receive_rpc_result(&responses, 2, deadline)?;
        let response: SkillsListResponse = serde_json::from_str(skills_raw.get())
            .map_err(|error| invalid_data("invalid skills/list response", error))?;
        let entry = select_skills_entry(response.data, cwd)?;

        Ok(SkillCatalog {
            codex_home: initialize.codex_home,
            skills: entry.skills,
            error_count: entry.errors.len(),
        })
    })();

    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn probe_deadline(now: Instant) -> Instant {
    now + PROBE_TIMEOUT
}

fn select_skills_entry(
    mut entries: Vec<SkillsListEntry>,
    requested_cwd: &Path,
) -> io::Result<SkillsListEntry> {
    if entries.len() == 1 {
        return entries
            .pop()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "skills/list omitted cwd"));
    }

    let canonical_requested = fs::canonicalize(requested_cwd).ok();
    entries
        .into_iter()
        .find(|entry| {
            entry.cwd == requested_cwd
                || canonical_requested.as_ref().is_some_and(|canonical| {
                    fs::canonicalize(&entry.cwd).is_ok_and(|entry| entry == *canonical)
                })
        })
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "skills/list omitted cwd"))
}

fn send_rpc_request(stdin: &mut impl io::Write, request: &serde_json::Value) -> io::Result<()> {
    serde_json::to_writer(&mut *stdin, request)
        .map_err(|error| invalid_data("failed to encode app-server request", error))?;
    stdin.write_all(b"\n")?;
    stdin.flush()
}

fn spawn_line_reader(stream: impl io::Read + Send + 'static) -> Receiver<io::Result<String>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stream).lines() {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    receiver
}

fn receive_rpc_result(
    responses: &Receiver<io::Result<String>>,
    expected_id: u64,
    deadline: Instant,
) -> io::Result<Box<RawValue>> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Codex app-server request timed out after 15 seconds",
            ));
        }

        let line = match responses.recv_timeout(remaining) {
            Ok(line) => line?,
            Err(RecvTimeoutError::Timeout) => {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Codex app-server request timed out after 15 seconds",
                ));
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Codex app-server exited before returning a response",
                ));
            }
        };
        let response: RpcEnvelope = serde_json::from_str(&line)
            .map_err(|error| invalid_data("invalid Codex app-server JSON", error))?;
        if response.id != Some(expected_id) {
            continue;
        }
        if let Some(error) = response.error {
            return Err(io::Error::new(io::ErrorKind::InvalidData, error.message));
        }
        return response.result.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Codex app-server response omitted result",
            )
        });
    }
}

fn invalid_data(context: &str, error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("{context}: {error}"))
}

fn discover_rollouts(codex_home: &Path) -> RolloutDiscovery {
    let mut discovery = RolloutDiscovery::default();
    for root in [
        codex_home.join("sessions"),
        codex_home.join("archived_sessions"),
    ] {
        discover_jsonl_files(&root, &mut discovery);
    }
    discovery.paths.sort_by(|left, right| {
        right
            .file_name()
            .cmp(&left.file_name())
            .then_with(|| right.cmp(left))
    });
    discovery
}

fn discover_jsonl_files(root: &Path, discovery: &mut RolloutDiscovery) {
    let metadata = match fs::metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return,
        Err(_) => {
            discovery.record_error();
            return;
        }
    };
    if !metadata.is_dir() {
        return;
    }

    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => {
            discovery.record_error();
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                discovery.record_error();
                continue;
            }
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => {
                discovery.record_error();
                continue;
            }
        };
        if file_type.is_dir() {
            discover_jsonl_files(&entry.path(), discovery);
        } else if file_type.is_file() && entry.path().extension().is_some_and(|ext| ext == "jsonl")
        {
            discovery.paths.push(entry.path());
        }
    }
}

fn scan_rollouts(
    paths: &[PathBuf],
    current_thread_id: Option<&str>,
    skills: &[CatalogSkill],
) -> UsageEvidence {
    let mut evidence = UsageEvidence {
        rollouts_discovered: paths.len(),
        ..UsageEvidence::default()
    };
    let candidates = paths
        .iter()
        .filter(|path| {
            let excluded = current_thread_id.is_some_and(|thread_id| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().contains(thread_id))
            });
            if excluded {
                evidence.record_excluded_current();
            }
            !excluded
        })
        .collect::<Vec<_>>();
    let next_index = AtomicUsize::new(0);
    let deadline = Instant::now() + HISTORY_SCAN_TIMEOUT;
    let worker_count = thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .min(8)
        .min(candidates.len().max(1));
    let partials = thread::scope(|scope| {
        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            workers.push(scope.spawn(|| {
                let mut partial = UsageEvidence::default();
                loop {
                    if Instant::now() >= deadline {
                        break;
                    }
                    let index = next_index.fetch_add(1, Ordering::Relaxed);
                    let Some(path) = candidates.get(index) else {
                        break;
                    };
                    match scan_rollout(path, skills, deadline, current_thread_id) {
                        Ok(scan) => partial.record_scan(scan),
                        Err(_) => partial.record_unreadable_rollout(),
                    }
                }
                partial
            }));
        }
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap_or_default())
            .collect::<Vec<_>>()
    });
    for partial in partials {
        evidence.merge_partial(partial);
    }
    evidence.finalize_truncation();
    evidence
}

fn scan_rollout(
    path: &Path,
    skills: &[CatalogSkill],
    deadline: Instant,
    current_thread_id: Option<&str>,
) -> io::Result<RolloutScan> {
    let file = File::open(path)?;
    let mut session_id = None;
    let mut turn_id = None;
    let mut turn_ordinal = 0_u64;
    let mut task_started_pending_context = false;
    let mut uses = Vec::new();
    let mut history_from = None;
    let mut history_through = None;
    let mut records_unreadable = 0;
    let mut reader = BufReader::new(file);
    let mut line = Vec::new();
    let response_finder = memmem::Finder::new(b"response_item");
    let turn_finder = memmem::Finder::new(b"turn_context");
    let task_started_finder = memmem::Finder::new(b"task_started");
    let skill_finder = memmem::Finder::new(b"SKILL.md");
    let mut first_record = true;
    let mut complete = true;

    loop {
        if Instant::now() >= deadline {
            complete = false;
            break;
        }
        line.clear();
        if reader.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        let interesting = first_record
            || turn_finder.find(&line).is_some()
            || task_started_finder.find(&line).is_some()
            || (response_finder.find(&line).is_some() && skill_finder.find(&line).is_some());
        if !interesting {
            first_record = false;
            continue;
        }
        let line = match std::str::from_utf8(&line) {
            Ok(line) => line,
            Err(_) => {
                records_unreadable += 1;
                first_record = false;
                continue;
            }
        };
        let envelope: RolloutEnvelope = match serde_json::from_str(line) {
            Ok(envelope) => envelope,
            Err(_) => {
                records_unreadable += 1;
                first_record = false;
                continue;
            }
        };
        let timestamp = envelope
            .timestamp
            .as_deref()
            .and_then(|value| parse_rfc3339_epoch(value).map(|epoch| (value.to_owned(), epoch)));
        if let Some(timestamp) = timestamp.clone() {
            if first_record {
                merge_time_range(&mut history_from, Some(timestamp.clone()), true);
            }
            merge_time_range(&mut history_through, Some(timestamp), false);
        }
        first_record = false;

        match envelope.kind.as_str() {
            "session_meta" => match serde_json::from_str::<SessionMetadata>(envelope.payload.get())
            {
                Ok(metadata) => {
                    let observed_session_id = metadata.id.or(metadata.session_id);
                    if current_thread_id
                        .is_some_and(|current| observed_session_id.as_deref() == Some(current))
                    {
                        return Ok(RolloutScan {
                            complete: true,
                            excluded_current: true,
                            uses: Vec::new(),
                            history_from: None,
                            history_through: None,
                            records_unreadable,
                        });
                    }
                    session_id = observed_session_id;
                }
                Err(_) => records_unreadable += 1,
            },
            "turn_context" => {
                let paired_with_task_started = task_started_pending_context;
                task_started_pending_context = false;
                if !paired_with_task_started {
                    turn_ordinal += 1;
                    turn_id = None;
                }
                match serde_json::from_str::<TurnMetadata>(envelope.payload.get()) {
                    Ok(metadata) if metadata.turn_id.is_some() => turn_id = metadata.turn_id,
                    Ok(_) => {}
                    Err(_) => records_unreadable += 1,
                }
            }
            "event_msg" => match serde_json::from_str::<ToolCallPayload>(envelope.payload.get()) {
                Ok(metadata) if metadata.kind == "task_started" => {
                    turn_ordinal += 1;
                    task_started_pending_context = true;
                    turn_id = serde_json::from_str::<TurnMetadata>(envelope.payload.get())
                        .ok()
                        .and_then(|metadata| metadata.turn_id)
                }
                Ok(_) => {}
                Err(_) => records_unreadable += 1,
            },
            "response_item" => {
                let call = match serde_json::from_str::<ToolCallPayload>(envelope.payload.get()) {
                    Ok(call) => call,
                    Err(_) => {
                        records_unreadable += 1;
                        continue;
                    }
                };
                if !matches!(call.kind.as_str(), "function_call" | "custom_tool_call") {
                    continue;
                }
                let Some(input) = call.input.or(call.arguments) else {
                    continue;
                };
                let Some((timestamp, epoch)) = timestamp else {
                    records_unreadable += 1;
                    continue;
                };
                let thread_id = session_id
                    .clone()
                    .or_else(|| rollout_id_from_path(path))
                    .unwrap_or_else(|| "unknown".to_owned());
                for skill in skills {
                    if !contains_exact_manifest_path(&input, &skill.path) {
                        continue;
                    }
                    let turn_or_event = turn_id.clone().unwrap_or_else(|| {
                        if turn_ordinal == 0 {
                            timestamp.clone()
                        } else {
                            format!("turn-ordinal:{turn_ordinal}")
                        }
                    });
                    uses.push((
                        SkillIdentity::from(skill),
                        ObservedUse {
                            key: ObservedUseKey {
                                thread_id: thread_id.clone(),
                                turn_or_event,
                            },
                            timestamp: timestamp.clone(),
                            epoch,
                            evidence: SkillEvidenceRef {
                                thread_id: thread_id.clone(),
                                turn_id: turn_id.clone(),
                            },
                        },
                    ));
                }
            }
            _ => {}
        }
    }

    Ok(RolloutScan {
        complete,
        excluded_current: false,
        uses,
        history_from,
        history_through,
        records_unreadable,
    })
}

fn contains_exact_manifest_path(input: &str, path: &Path) -> bool {
    let path = path.to_string_lossy();
    let needle = path.as_bytes();
    if needle.is_empty() {
        return false;
    }

    memmem::find_iter(input.as_bytes(), needle).any(|start| {
        let end = start + needle.len();
        let preceding_is_path = start
            .checked_sub(1)
            .and_then(|index| input.as_bytes().get(index))
            .is_some_and(|byte| is_path_continuation(*byte));
        let following_is_path = input
            .as_bytes()
            .get(end)
            .is_some_and(|byte| is_path_continuation(*byte));
        !preceding_is_path && !following_is_path
    })
}

const fn is_path_continuation(byte: u8) -> bool {
    !byte.is_ascii()
        || byte.is_ascii_alphanumeric()
        || matches!(byte, b'/' | b'\\' | b'.' | b'_' | b'-' | b'~')
}

fn rollout_id_from_path(path: &Path) -> Option<String> {
    let filename = path.file_name()?.to_string_lossy();
    let stem = filename.strip_suffix(".jsonl")?;
    (stem.len() >= 36).then(|| stem[stem.len() - 36..].to_owned())
}

fn merge_time_range(
    target: &mut Option<(String, i64)>,
    candidate: Option<(String, i64)>,
    choose_earliest: bool,
) {
    let Some(candidate) = candidate else {
        return;
    };
    let replace = target.as_ref().is_none_or(|current| {
        if choose_earliest {
            candidate.1 < current.1
        } else {
            candidate.1 > current.1
        }
    });
    if replace {
        *target = Some(candidate);
    }
}

fn build_report(
    observed_at: String,
    observed_epoch: i64,
    codex_home: &Path,
    skills: Vec<CatalogSkill>,
    mut evidence: UsageEvidence,
    catalog_errors: usize,
) -> SkillsInspectionReport {
    let status = coverage_status(&evidence, catalog_errors);
    let history_sufficient = status == CoverageStatus::Complete
        && history_is_sufficient(
            observed_epoch,
            evidence.history_from.as_ref().map(|value| value.1),
            evidence.history_through.as_ref().map(|value| value.1),
        );
    let mut summary = SkillUsageSummary::default();
    let mut items = Vec::with_capacity(skills.len());
    let origin_classifier = SkillOriginClassifier::new(codex_home);

    for skill in skills {
        let mut unique = HashSet::new();
        let identity = SkillIdentity::from(&skill);
        let mut observed = evidence.by_skill.remove(&identity).unwrap_or_default();
        observed.retain(|use_| unique.insert(use_.key.clone()));
        let last = observed.iter().max_by_key(|use_| use_.epoch);
        let age_days = last.map(|use_| age_days(observed_epoch, use_.epoch));
        let signal = match age_days {
            Some(days) if days <= RECENT_DAYS => SkillSignal::Recent,
            Some(_) => SkillSignal::Aging,
            None if history_sufficient => SkillSignal::Unobserved,
            None => SkillSignal::InsufficientHistory,
        };
        summary.total += 1;
        match signal {
            SkillSignal::Recent => summary.recent += 1,
            SkillSignal::Aging => summary.aging += 1,
            SkillSignal::Unobserved => summary.unobserved += 1,
            SkillSignal::InsufficientHistory => summary.insufficient_history += 1,
        }
        let origin = origin_classifier.classify(&skill.path, &skill.scope);
        items.push(SkillUsageItem {
            name: skill.name,
            scope: skill.scope,
            origin,
            manifest_path: skill.path,
            signal,
            last_observed_use_at: last.map(|use_| use_.timestamp.clone()),
            age_days,
            observed_uses_in_window: observed.len(),
            last_evidence: last.map(|use_| use_.evidence.clone()),
            last_observed_epoch: last.map(|use_| use_.epoch),
        });
    }

    items.sort_by(|left, right| {
        left.signal
            .sort_rank()
            .cmp(&right.signal.sort_rank())
            .then_with(|| left.last_observed_epoch.cmp(&right.last_observed_epoch))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.manifest_path.cmp(&right.manifest_path))
    });

    let mut findings = Vec::new();
    if status == CoverageStatus::None {
        findings.push(ReportFinding::new(
            "skills.history.unavailable",
            "unknown",
            "no retained Codex rollouts were available for usage evidence",
        ));
    } else if status == CoverageStatus::Partial {
        findings.push(ReportFinding::new(
            "skills.history.partial",
            "partial",
            "the bounded scan was truncated or some catalog, directory, rollout, or record evidence could not be read",
        ));
    }
    if summary.aging > 0 || summary.unobserved > 0 {
        findings.push(ReportFinding::new(
            "skills.usage.review",
            "review",
            format!(
                "{} aging and {} unobserved skills need review",
                summary.aging, summary.unobserved
            ),
        ));
    }
    let verdict = if status == CoverageStatus::Complete
        && summary.aging == 0
        && summary.unobserved == 0
        && summary.insufficient_history == 0
    {
        Verdict::BaselineOk
    } else {
        Verdict::Review
    };

    SkillsInspectionReport {
        schema_version: REPORT_SCHEMA_VERSION,
        kind: ReportKind::Skills,
        observed_at,
        verdict,
        coverage: SkillCoverage {
            status,
            history_from: evidence.history_from.map(|value| value.0),
            history_through: evidence.history_through.map(|value| value.0),
            rollouts_discovered: evidence.rollouts_discovered,
            rollouts_scanned: evidence.rollouts_scanned,
            rollouts_excluded_current: evidence.rollouts_excluded_current,
            rollouts_unreadable: evidence.rollouts_unreadable,
            discovery_errors: evidence.discovery_errors,
            records_unreadable: evidence.records_unreadable,
            catalog_errors,
            truncated: evidence.truncated,
        },
        evidence_basis: EVIDENCE_BASIS,
        counting_basis: COUNTING_BASIS,
        evidence_limitations: EVIDENCE_LIMITATIONS,
        thresholds: SkillThresholds {
            recent_days: RECENT_DAYS,
            unobserved_history_days: UNOBSERVED_HISTORY_DAYS,
        },
        summary,
        items,
        findings,
    }
}

fn coverage_status(evidence: &UsageEvidence, catalog_errors: usize) -> CoverageStatus {
    let candidate_rollouts = evidence
        .rollouts_discovered
        .saturating_sub(evidence.rollouts_excluded_current);
    let partial = evidence.rollouts_unreadable > 0
        || evidence.discovery_errors > 0
        || evidence.records_unreadable > 0
        || evidence.truncated
        || catalog_errors > 0
        || evidence.rollouts_scanned != candidate_rollouts;

    if partial {
        CoverageStatus::Partial
    } else if candidate_rollouts == 0 {
        CoverageStatus::None
    } else {
        CoverageStatus::Complete
    }
}

fn history_is_sufficient(
    observed_epoch: i64,
    history_from: Option<i64>,
    history_through: Option<i64>,
) -> bool {
    let Some(history_from) = history_from else {
        return false;
    };
    let Some(history_through) = history_through else {
        return false;
    };
    history_from <= observed_epoch - (UNOBSERVED_HISTORY_DAYS as i64 * SECONDS_PER_DAY)
        && history_through >= observed_epoch - (RECENT_DAYS as i64 * SECONDS_PER_DAY)
}

fn age_days(observed_epoch: i64, evidence_epoch: i64) -> u64 {
    observed_epoch
        .saturating_sub(evidence_epoch)
        .max(0)
        .div_euclid(SECONDS_PER_DAY) as u64
}

fn current_epoch_seconds() -> io::Result<i64> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| io::Error::other(format!("system clock is before Unix epoch: {error}")))?
        .as_secs();
    i64::try_from(seconds).map_err(|_| io::Error::other("system time exceeded i64 seconds"))
}

fn parse_rfc3339_epoch(value: &str) -> Option<i64> {
    let bytes = value.as_bytes();
    if bytes.len() < 20
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || bytes.get(10) != Some(&b'T')
        || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
    {
        return None;
    }
    let year = parse_digits(bytes, 0, 4)? as i64;
    let month = parse_digits(bytes, 5, 2)? as i64;
    let day = parse_digits(bytes, 8, 2)? as i64;
    let hour = parse_digits(bytes, 11, 2)? as i64;
    let minute = parse_digits(bytes, 14, 2)? as i64;
    let second = parse_digits(bytes, 17, 2)? as i64;
    if !(1..=12).contains(&month)
        || !(1..=days_in_month(year, month)).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let timezone_index = value[19..].find(['Z', '+', '-']).map(|index| index + 19)?;
    let offset = match bytes[timezone_index] {
        b'Z' if timezone_index + 1 == bytes.len() => 0,
        b'+' | b'-' if timezone_index + 6 == bytes.len() => {
            if bytes.get(timezone_index + 3) != Some(&b':') {
                return None;
            }
            let hours = parse_digits(bytes, timezone_index + 1, 2)? as i64;
            let minutes = parse_digits(bytes, timezone_index + 4, 2)? as i64;
            if hours > 23 || minutes > 59 {
                return None;
            }
            let seconds = hours * 3_600 + minutes * 60;
            if bytes[timezone_index] == b'+' {
                seconds
            } else {
                -seconds
            }
        }
        _ => return None,
    };
    Some(
        days_from_civil(year, month, day) * SECONDS_PER_DAY + hour * 3_600 + minute * 60 + second
            - offset,
    )
}

fn parse_digits(bytes: &[u8], start: usize, length: usize) -> Option<u32> {
    let mut value = 0_u32;
    for byte in bytes.get(start..start + length)? {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value * 10 + u32::from(*byte - b'0');
    }
    Some(value)
}

const fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) => 29,
        2 => 28,
        _ => 0,
    }
}

const fn days_from_civil(mut year: i64, month: i64, day: i64) -> i64 {
    year -= if month <= 2 { 1 } else { 0 };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn format_rfc3339(epoch: i64) -> String {
    let days = epoch.div_euclid(SECONDS_PER_DAY);
    let seconds = epoch.rem_euclid(SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds / 3_600;
    let minute = (seconds % 3_600) / 60;
    let second = seconds % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

const fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let zero_based = days + 719_468;
    let era = if zero_based >= 0 {
        zero_based
    } else {
        zero_based - 146_096
    } / 146_097;
    let day_of_era = zero_based - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

fn format_human_report(report: &SkillsInspectionReport) -> String {
    let mut output = String::new();
    writeln!(output, "Codex skills: {}", report.summary.total).expect("writing to String");
    writeln!(
        output,
        "Coverage: {} ({} scanned, {} excluded current, {} unreadable, {} discovery errors)",
        report.coverage.status.as_str(),
        report.coverage.rollouts_scanned,
        report.coverage.rollouts_excluded_current,
        report.coverage.rollouts_unreadable,
        report.coverage.discovery_errors
    )
    .expect("writing to String");
    writeln!(output, "SIGNAL\tUSES\tLAST OBSERVED\tORIGIN\tSCOPE\tSKILL")
        .expect("writing to String");
    for item in &report.items {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}",
            item.signal.as_str(),
            item.observed_uses_in_window,
            item.last_observed_use_at.as_deref().unwrap_or("-"),
            item.origin.as_str(),
            item.scope,
            item.name
        )
        .expect("writing to String");
    }
    writeln!(output, "Verdict: {}", report.verdict.as_str()).expect("writing to String");
    output
}

#[cfg(test)]
mod tests {
    use std::{
        process,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::*;

    static NEXT_TEST_DIR: AtomicUsize = AtomicUsize::new(0);

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!(
                "goalrail-skill-usage-test-{}-{sequence}",
                process::id()
            ));
            fs::create_dir_all(&path).expect("test directory should be created");
            Self { path }
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

    fn skill(name: &str, path: &str) -> CatalogSkill {
        CatalogSkill {
            name: name.to_owned(),
            path: PathBuf::from(path),
            scope: "user".to_owned(),
            enabled: true,
        }
    }

    fn skills_list_entry(cwd: impl Into<PathBuf>, name: &str) -> SkillsListEntry {
        SkillsListEntry {
            cwd: cwd.into(),
            skills: vec![skill(name, &format!("/skills/{name}/SKILL.md"))],
            errors: Vec::new(),
        }
    }

    fn empty_rollout_scan(
        complete: bool,
        excluded_current: bool,
        records_unreadable: usize,
    ) -> RolloutScan {
        RolloutScan {
            complete,
            excluded_current,
            uses: Vec::new(),
            history_from: None,
            history_through: None,
            records_unreadable,
        }
    }

    #[test]
    fn parses_and_formats_rfc3339_without_a_time_dependency() {
        let cases = [
            ("1970-01-01T00:00:00Z", 0, "1970-01-01T00:00:00Z"),
            ("1970-01-01T23:59:59Z", 86_399, "1970-01-01T23:59:59Z"),
            ("1970-01-01T23:59:60Z", 86_400, "1970-01-02T00:00:00Z"),
            ("1970-01-01T03:30:00+03:30", 0, "1970-01-01T00:00:00Z"),
            ("1970-01-01T23:59:00+23:59", 0, "1970-01-01T00:00:00Z"),
            ("1970-01-01T00:00:00-03:30", 12_600, "1970-01-01T03:30:00Z"),
            (
                "2026-08-09T17:53:50.120Z",
                1_786_298_030,
                "2026-08-09T17:53:50Z",
            ),
            (
                "2026-08-09T20:53:50+03:00",
                1_786_298_030,
                "2026-08-09T17:53:50Z",
            ),
        ];
        for (timestamp, epoch, formatted) in cases {
            assert_eq!(parse_rfc3339_epoch(timestamp), Some(epoch));
            assert_eq!(format_rfc3339(epoch), formatted);
        }
        let invalid = [
            "2026-02-30T00:00:00Z",
            "not-a-timestamp",
            "2026/08-09T00:00:00Z",
            "2026-08/09T00:00:00Z",
            "2026-08-09 00:00:00Z",
            "2026-08-09T00-00:00Z",
            "2026-08-09T00:00-00Z",
            "1970-01-01T24:00:00Z",
            "1970-01-01T00:60:00Z",
            "1970-01-01T00:00:61Z",
            "1970-01-01T00:00:00Zjunk",
            "1970-01-01T00:00:00+03:00junk",
            "1970-01-01T00:00:00+03-00",
            "1970-01-01T00:00:00+24:00",
            "1970-01-01T00:00:00+00:60",
        ];
        for timestamp in invalid {
            assert_eq!(
                parse_rfc3339_epoch(timestamp),
                None,
                "{timestamp} should be rejected"
            );
        }
    }

    #[test]
    fn reads_the_current_epoch_from_the_system_clock() {
        let system_epoch = || {
            i64::try_from(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("test clock should be after the Unix epoch")
                    .as_secs(),
            )
            .expect("test clock should fit in i64")
        };
        let before = system_epoch();
        let observed = current_epoch_seconds().expect("current epoch should be available");
        let after = system_epoch();

        assert!(observed >= before);
        assert!(observed <= after);
    }

    #[test]
    fn applies_gregorian_leap_year_rules_at_century_boundaries() {
        let cases = [
            (2023, 2, 28),
            (2024, 2, 29),
            (1900, 2, 28),
            (2000, 2, 29),
            (2100, 2, 28),
            (2026, 1, 31),
            (2026, 4, 30),
        ];

        for (year, month, expected) in cases {
            assert_eq!(
                days_in_month(year, month),
                expected,
                "unexpected days in {year:04}-{month:02}"
            );
        }
    }

    #[test]
    fn calendar_conversion_round_trips_era_and_month_boundaries() {
        let cases = [
            (-400, 2, 29),
            (-1, 12, 31),
            (0, 1, 1),
            (1, 2, 1),
            (1, 3, 1),
            (4, 2, 29),
            (100, 3, 1),
            (399, 12, 31),
            (400, 2, 29),
            (1600, 2, 29),
            (1700, 3, 1),
            (1900, 2, 28),
            (1969, 12, 31),
            (1970, 1, 1),
            (2000, 2, 29),
            (2024, 2, 29),
            (9999, 12, 31),
        ];

        for date @ (year, month, day) in cases {
            let days = days_from_civil(year, month, day);
            assert_eq!(
                civil_from_days(days),
                date,
                "calendar round trip failed for {year:04}-{month:02}-{day:02}"
            );
        }
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(1969, 12, 31), -1);
    }

    #[test]
    fn discovers_only_jsonl_rollouts() {
        let tree = TestDirectory::new();
        tree.file("sessions/2026/rollout-one.jsonl", "{}\n");
        tree.file("sessions/2026/rollout-one.jsonl.langfuse", "ignored\n");
        tree.file("archived_sessions/rollout-two.jsonl", "{}\n");

        let discovery = discover_rollouts(&tree.path);

        assert_eq!(discovery.errors, 0);
        assert_eq!(discovery.paths.len(), 2);
        assert!(
            discovery
                .paths
                .iter()
                .all(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        );
    }

    #[test]
    fn distinguishes_missing_rollout_roots_from_discovery_errors() {
        let tree = TestDirectory::new();
        let mut discovery = RolloutDiscovery::default();

        discover_jsonl_files(&tree.path.join("missing"), &mut discovery);
        assert_eq!(discovery.errors, 0);

        let file = tree.file("not-a-directory", "fixture");
        discover_jsonl_files(&file.join("child"), &mut discovery);
        assert_eq!(discovery.errors, 1);
    }

    #[test]
    fn increments_discovery_errors_without_resetting_previous_evidence() {
        let mut discovery = RolloutDiscovery::default();

        discovery.record_error();
        discovery.record_error();

        assert_eq!(discovery.errors, 2);
    }

    #[test]
    fn aggregates_rollout_scan_counters_without_losing_partial_evidence() {
        let mut evidence = UsageEvidence::default();
        evidence.record_scan(empty_rollout_scan(true, false, 2));
        evidence.record_scan(empty_rollout_scan(false, false, 3));
        evidence.record_scan(empty_rollout_scan(true, true, 0));
        evidence.record_unreadable_rollout();

        assert_eq!(evidence.rollouts_scanned, 1);
        assert_eq!(evidence.rollouts_excluded_current, 1);
        assert_eq!(evidence.rollouts_unreadable, 1);
        assert_eq!(evidence.records_unreadable, 5);
        assert!(evidence.truncated);

        evidence.merge_partial(UsageEvidence {
            rollouts_scanned: 2,
            rollouts_excluded_current: 3,
            rollouts_unreadable: 4,
            records_unreadable: 5,
            truncated: false,
            ..UsageEvidence::default()
        });

        assert_eq!(evidence.rollouts_scanned, 3);
        assert_eq!(evidence.rollouts_excluded_current, 4);
        assert_eq!(evidence.rollouts_unreadable, 5);
        assert_eq!(evidence.records_unreadable, 10);
        assert!(evidence.truncated);
    }

    #[test]
    fn marks_only_unaccounted_rollouts_as_truncated() {
        let mut complete = UsageEvidence {
            rollouts_discovered: 3,
            rollouts_scanned: 1,
            rollouts_excluded_current: 1,
            rollouts_unreadable: 1,
            ..UsageEvidence::default()
        };
        complete.finalize_truncation();
        assert!(!complete.truncated);

        let mut incomplete = UsageEvidence {
            rollouts_discovered: 4,
            rollouts_scanned: 1,
            rollouts_excluded_current: 1,
            rollouts_unreadable: 1,
            ..UsageEvidence::default()
        };
        incomplete.finalize_truncation();
        assert!(incomplete.truncated);
    }

    #[test]
    fn counts_unreadable_rollout_paths_without_marking_the_scan_truncated() {
        let tree = TestDirectory::new();
        let evidence = scan_rollouts(&[tree.path.join("missing.jsonl")], None, &[]);

        assert_eq!(evidence.rollouts_discovered, 1);
        assert_eq!(evidence.rollouts_scanned, 0);
        assert_eq!(evidence.rollouts_unreadable, 1);
        assert!(!evidence.truncated);
    }

    #[test]
    fn excludes_a_current_rollout_by_filename_before_reading_it() {
        let tree = TestDirectory::new();
        let current = tree.file("rollout-current-thread.jsonl", "not JSON\n");
        let evidence = scan_rollouts(&[current], Some("current-thread"), &[]);

        assert_eq!(evidence.rollouts_discovered, 1);
        assert_eq!(evidence.rollouts_excluded_current, 1);
        assert_eq!(evidence.rollouts_scanned, 0);
        assert_eq!(evidence.rollouts_unreadable, 0);
        assert_eq!(evidence.records_unreadable, 0);
        assert!(!evidence.truncated);
    }

    #[test]
    fn counts_one_use_per_turn_and_keeps_the_latest_evidence() {
        let tree = TestDirectory::new();
        let path = tree.file(
            "rollout.jsonl",
            concat!(
                "{\"timestamp\":\"2026-07-01T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"thread-1\"}}\n",
                "{\"timestamp\":\"2026-07-01T00:00:01Z\",\"type\":\"turn_context\",\"payload\":{\"turn_id\":\"turn-1\"}}\n",
                "{\"timestamp\":\"2026-07-01T00:00:02Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call\",\"input\":\"cat /skills/example/SKILL.md\"}}\n",
                "{\"timestamp\":\"2026-07-01T00:00:03Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"function_call\",\"arguments\":\"sed -n 1,20p /skills/example/SKILL.md\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:00Z\",\"type\":\"turn_context\",\"payload\":{\"turn_id\":\"turn-2\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:01Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call\",\"input\":\"cat /skills/example/SKILL.md\"}}\n"
            ),
        );
        let skills = vec![skill("example", "/skills/example/SKILL.md")];
        let scan = scan_rollout(
            &path,
            &skills,
            Instant::now() + Duration::from_secs(1),
            None,
        )
        .expect("rollout should scan");
        assert!(scan.complete);
        let mut evidence = UsageEvidence {
            rollouts_discovered: 1,
            rollouts_scanned: 1,
            history_from: scan.history_from,
            history_through: scan.history_through,
            records_unreadable: scan.records_unreadable,
            ..UsageEvidence::default()
        };
        for (identity, observed_use) in scan.uses {
            evidence
                .by_skill
                .entry(identity)
                .or_default()
                .push(observed_use);
        }

        let report = build_report(
            "2026-08-09T00:00:00Z".to_owned(),
            parse_rfc3339_epoch("2026-08-09T00:00:00Z").expect("timestamp"),
            Path::new("/codex-home"),
            skills,
            evidence,
            0,
        );

        assert_eq!(report.summary.recent, 1);
        assert_eq!(report.summary.total, 1);
        assert_eq!(report.items[0].observed_uses_in_window, 2);
        assert!(report.findings.is_empty());
        assert_eq!(
            report.items[0].last_observed_use_at.as_deref(),
            Some("2026-08-08T00:00:01Z")
        );
        assert_eq!(
            report.items[0]
                .last_evidence
                .as_ref()
                .and_then(|evidence| evidence.turn_id.as_deref()),
            Some("turn-2")
        );
        let outcome = SkillsInspectionOutcome {
            report: SkillsOutcomeReport::Complete(Box::new(report)),
        };
        assert_eq!(outcome.verdict(), Verdict::BaselineOk);
        assert!(!outcome.is_failure());
        let json = outcome
            .to_pretty_json()
            .expect("complete outcome should serialize");
        assert!(json.contains(r#""kind": "skills""#));
        let human = outcome.to_human();
        assert!(human.contains(
            "Coverage: complete (1 scanned, 0 excluded current, 0 unreadable, 0 discovery errors)"
        ));
        assert!(human.contains("SIGNAL\tUSES\tLAST OBSERVED\tORIGIN\tSCOPE\tSKILL"));
        assert!(human.contains("recent\t2\t2026-08-08T00:00:01Z\tpersonal\tuser\texample"));
    }

    #[test]
    fn human_labels_and_cleanup_sort_order_are_stable() {
        assert_eq!(CoverageStatus::Complete.as_str(), "complete");
        assert_eq!(CoverageStatus::Partial.as_str(), "partial");
        assert_eq!(CoverageStatus::None.as_str(), "none");

        let origins = [
            (SkillOrigin::System, "system"),
            (SkillOrigin::Plugin, "plugin"),
            (SkillOrigin::Personal, "personal"),
            (SkillOrigin::Project, "project"),
            (SkillOrigin::Admin, "admin"),
            (SkillOrigin::Unknown, "unknown"),
        ];
        for (origin, expected) in origins {
            assert_eq!(origin.as_str(), expected);
        }

        let signals = [
            (SkillSignal::Unobserved, "unobserved", 0),
            (SkillSignal::Aging, "aging", 1),
            (SkillSignal::Recent, "recent", 2),
            (SkillSignal::InsufficientHistory, "insufficient_history", 3),
        ];
        for (signal, expected_label, expected_rank) in signals {
            assert_eq!(signal.as_str(), expected_label);
            assert_eq!(signal.sort_rank(), expected_rank);
        }
    }

    #[test]
    fn keeps_only_nonempty_current_thread_ids() {
        assert_eq!(nonempty_thread_id(None), None);
        assert_eq!(nonempty_thread_id(Some(String::new())), None);
        assert_eq!(
            nonempty_thread_id(Some("current-thread".to_owned())).as_deref(),
            Some("current-thread")
        );
    }

    #[test]
    fn keeps_same_name_catalog_entries_when_manifest_paths_differ() {
        let skills = unique_active_skills(vec![
            skill("shared", "/home/user/.agents/skills/shared/SKILL.md"),
            skill("shared", "/repo/.agents/skills/shared/SKILL.md"),
            skill("shared", "/repo/.agents/skills/shared/SKILL.md"),
        ]);

        assert_eq!(skills.len(), 2);
        assert_ne!(skills[0].path, skills[1].path);
    }

    #[test]
    fn keeps_usage_separate_for_same_name_skills() {
        let tree = TestDirectory::new();
        let rollout = tree.file(
            "rollout.jsonl",
            concat!(
                "{\"timestamp\":\"2026-07-01T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"thread-1\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:00Z\",\"type\":\"turn_context\",\"payload\":{\"turn_id\":\"turn-1\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:01Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call\",\"input\":\"cat /repo/.agents/skills/shared/SKILL.md\"}}\n"
            ),
        );
        let personal = skill("shared", "/home/user/.agents/skills/shared/SKILL.md");
        let mut project = skill("shared", "/repo/.agents/skills/shared/SKILL.md");
        project.scope = "repo".to_owned();
        let skills = vec![personal, project];
        let scan = scan_rollout(
            &rollout,
            &skills,
            Instant::now() + Duration::from_secs(1),
            None,
        )
        .expect("rollout should scan");
        let mut evidence = UsageEvidence {
            rollouts_discovered: 1,
            rollouts_scanned: 1,
            history_from: scan.history_from,
            history_through: scan.history_through,
            records_unreadable: scan.records_unreadable,
            ..UsageEvidence::default()
        };
        for (identity, observed_use) in scan.uses {
            evidence
                .by_skill
                .entry(identity)
                .or_default()
                .push(observed_use);
        }

        let report = build_report(
            "2026-08-09T00:00:00Z".to_owned(),
            parse_rfc3339_epoch("2026-08-09T00:00:00Z").expect("timestamp"),
            Path::new("/codex-home"),
            skills,
            evidence,
            0,
        );
        let personal = report
            .items
            .iter()
            .find(|item| {
                item.manifest_path.as_path()
                    == Path::new("/home/user/.agents/skills/shared/SKILL.md")
            })
            .expect("personal skill should remain");
        let project = report
            .items
            .iter()
            .find(|item| {
                item.manifest_path.as_path() == Path::new("/repo/.agents/skills/shared/SKILL.md")
            })
            .expect("project skill should remain");

        assert_eq!(personal.signal, SkillSignal::Unobserved);
        assert_eq!(personal.observed_uses_in_window, 0);
        assert_eq!(project.signal, SkillSignal::Recent);
        assert_eq!(project.observed_uses_in_window, 1);
    }

    #[test]
    fn classifies_skill_origins_for_cleanup_ownership() {
        let codex_home = Path::new("/codex-home");
        let classifier = SkillOriginClassifier::new(codex_home);
        let cases = [
            (
                "/codex-home/plugins/cache/example/skills/one/SKILL.md",
                "user",
                SkillOrigin::Plugin,
            ),
            ("/virtual/system/SKILL.md", "system", SkillOrigin::System),
            (
                "/home/user/.agents/skills/one/SKILL.md",
                "user",
                SkillOrigin::Personal,
            ),
            (
                "/repo/.agents/skills/one/SKILL.md",
                "repo",
                SkillOrigin::Project,
            ),
            (
                "/etc/codex/skills/one/SKILL.md",
                "admin",
                SkillOrigin::Admin,
            ),
            ("/unknown/one/SKILL.md", "future", SkillOrigin::Unknown),
        ];

        for (path, scope, expected) in cases {
            assert_eq!(classifier.classify(Path::new(path), scope), expected);
        }
    }

    #[test]
    fn summarizes_each_skill_origin_for_the_agent_cleanup_view() {
        let codex_home = Path::new("/codex-home");
        let mut system = skill("system", "/virtual/system/SKILL.md");
        system.scope = "system".to_owned();
        let plugin = skill(
            "plugin",
            "/codex-home/plugins/cache/example/skills/plugin/SKILL.md",
        );
        let personal = skill("personal", "/home/user/.agents/skills/personal/SKILL.md");
        let mut project = skill("project", "/repo/.agents/skills/project/SKILL.md");
        project.scope = "repo".to_owned();
        let mut admin = skill("admin", "/etc/codex/skills/admin/SKILL.md");
        admin.scope = "admin".to_owned();
        let mut unknown = skill("unknown", "/unknown/skills/unknown/SKILL.md");
        unknown.scope = "future".to_owned();

        let summary = summarize_skill_origins(
            &[system, plugin, personal, project, admin, unknown],
            codex_home,
        );

        assert_eq!(
            summary,
            SkillOriginSummary {
                system: 1,
                plugin: 1,
                personal: 1,
                project: 1,
                admin: 1,
                unknown: 1,
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn classifies_plugin_skills_through_a_symlinked_codex_home() {
        use std::os::unix::fs::symlink;

        let tree = TestDirectory::new();
        let manifest = tree.file(
            "canonical-codex/plugins/cache/example/SKILL.md",
            "fixture manifest",
        );
        let codex_home = tree.path.join("codex-home");
        symlink(tree.path.join("canonical-codex"), &codex_home)
            .expect("Codex home symlink should be created");

        let classifier = SkillOriginClassifier::new(&codex_home);

        assert_eq!(classifier.classify(&manifest, "user"), SkillOrigin::Plugin);
    }

    #[test]
    fn matches_only_the_exact_manifest_path() {
        let manifest = Path::new("/repo/.agents/skills/example/SKILL.md");

        assert!(contains_exact_manifest_path(
            "cat /repo/.agents/skills/example/SKILL.md",
            manifest
        ));
        assert!(!contains_exact_manifest_path(
            "cat /backup/repo/.agents/skills/example/SKILL.md",
            manifest
        ));
        assert!(!contains_exact_manifest_path(
            "cat /repo/.agents/skills/example/SKILL.md.bak",
            manifest
        ));
    }

    #[test]
    fn extracts_only_the_trailing_rollout_thread_id() {
        let thread_id = "019fe059-dd62-7ef1-a310-17c6716bb837";
        let filename = format!("rollout-2026-08-09T00-00-00-{thread_id}.jsonl");

        assert_eq!(
            rollout_id_from_path(Path::new(&filename)).as_deref(),
            Some(thread_id)
        );
        assert_eq!(rollout_id_from_path(Path::new("short.jsonl")), None);
        assert_eq!(
            rollout_id_from_path(Path::new("rollout-without-jsonl-suffix")),
            None
        );
    }

    #[test]
    fn merges_history_bounds_without_replacing_equal_timestamps() {
        let mut earliest = Some(("middle".to_owned(), 20));
        merge_time_range(&mut earliest, Some(("earlier".to_owned(), 10)), true);
        assert_eq!(earliest, Some(("earlier".to_owned(), 10)));
        merge_time_range(
            &mut earliest,
            Some(("equal-but-different".to_owned(), 10)),
            true,
        );
        assert_eq!(earliest, Some(("earlier".to_owned(), 10)));

        let mut latest = Some(("middle".to_owned(), 20));
        merge_time_range(&mut latest, Some(("later".to_owned(), 30)), false);
        assert_eq!(latest, Some(("later".to_owned(), 30)));
        merge_time_range(
            &mut latest,
            Some(("equal-but-different".to_owned(), 30)),
            false,
        );
        assert_eq!(latest, Some(("later".to_owned(), 30)));
    }

    #[test]
    fn partial_coverage_never_labels_a_skill_unobserved() {
        let evidence = UsageEvidence {
            history_from: Some((
                "2026-06-01T00:00:00Z".to_owned(),
                parse_rfc3339_epoch("2026-06-01T00:00:00Z").expect("timestamp"),
            )),
            history_through: Some((
                "2026-08-08T00:00:00Z".to_owned(),
                parse_rfc3339_epoch("2026-08-08T00:00:00Z").expect("timestamp"),
            )),
            rollouts_discovered: 2,
            rollouts_scanned: 1,
            truncated: true,
            ..UsageEvidence::default()
        };

        let report = build_report(
            "2026-08-09T00:00:00Z".to_owned(),
            parse_rfc3339_epoch("2026-08-09T00:00:00Z").expect("timestamp"),
            Path::new("/codex-home"),
            vec![skill("example", "/skills/example/SKILL.md")],
            evidence,
            0,
        );

        assert_eq!(report.coverage.status, CoverageStatus::Partial);
        assert_eq!(report.summary.unobserved, 0);
        assert_eq!(report.summary.insufficient_history, 1);
        assert_eq!(report.items[0].signal, SkillSignal::InsufficientHistory);
    }

    #[test]
    fn classifies_the_recent_age_boundary_inclusively() {
        let observed_epoch = parse_rfc3339_epoch("2026-08-09T00:00:00Z").expect("timestamp");
        let recent = skill("recent-boundary", "/skills/recent-boundary/SKILL.md");
        let aging = skill("aging-boundary", "/skills/aging-boundary/SKILL.md");
        let mut evidence = UsageEvidence {
            rollouts_discovered: 1,
            rollouts_scanned: 1,
            ..UsageEvidence::default()
        };
        for (skill, days) in [(&recent, RECENT_DAYS), (&aging, RECENT_DAYS + 1)] {
            let epoch = observed_epoch - days as i64 * SECONDS_PER_DAY;
            evidence
                .by_skill
                .entry(SkillIdentity::from(skill))
                .or_default()
                .push(ObservedUse {
                    key: ObservedUseKey {
                        thread_id: "thread-1".to_owned(),
                        turn_or_event: skill.name.clone(),
                    },
                    timestamp: format_rfc3339(epoch),
                    epoch,
                    evidence: SkillEvidenceRef {
                        thread_id: "thread-1".to_owned(),
                        turn_id: None,
                    },
                });
        }

        let report = build_report(
            "2026-08-09T00:00:00Z".to_owned(),
            observed_epoch,
            Path::new("/codex-home"),
            vec![recent, aging],
            evidence,
            0,
        );

        assert_eq!(report.summary.recent, 1);
        assert_eq!(report.summary.aging, 1);
        assert_eq!(report.summary.total, 2);
        assert_eq!(report.verdict, Verdict::Review);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "skills.usage.review")
        );
        assert_eq!(
            report
                .items
                .iter()
                .find(|item| item.name == "recent-boundary")
                .map(|item| item.signal),
            Some(SkillSignal::Recent)
        );
        assert_eq!(
            report
                .items
                .iter()
                .find(|item| item.name == "aging-boundary")
                .map(|item| item.signal),
            Some(SkillSignal::Aging)
        );
    }

    #[test]
    fn discovery_errors_make_coverage_partial_instead_of_none() {
        let evidence = UsageEvidence {
            discovery_errors: 1,
            ..UsageEvidence::default()
        };

        assert_eq!(coverage_status(&evidence, 0), CoverageStatus::Partial);
    }

    #[test]
    fn treats_each_incomplete_evidence_reason_as_partial() {
        let complete = UsageEvidence {
            rollouts_discovered: 1,
            rollouts_scanned: 1,
            ..UsageEvidence::default()
        };
        assert_eq!(coverage_status(&complete, 0), CoverageStatus::Complete);
        assert_eq!(
            coverage_status(&UsageEvidence::default(), 0),
            CoverageStatus::None
        );

        let cases = [
            (
                "unreadable rollout",
                UsageEvidence {
                    rollouts_discovered: 1,
                    rollouts_scanned: 1,
                    rollouts_unreadable: 1,
                    ..UsageEvidence::default()
                },
                0,
            ),
            (
                "discovery error",
                UsageEvidence {
                    rollouts_discovered: 1,
                    rollouts_scanned: 1,
                    discovery_errors: 1,
                    ..UsageEvidence::default()
                },
                0,
            ),
            (
                "unreadable record",
                UsageEvidence {
                    rollouts_discovered: 1,
                    rollouts_scanned: 1,
                    records_unreadable: 1,
                    ..UsageEvidence::default()
                },
                0,
            ),
            (
                "truncated scan",
                UsageEvidence {
                    rollouts_discovered: 1,
                    rollouts_scanned: 1,
                    truncated: true,
                    ..UsageEvidence::default()
                },
                0,
            ),
            (
                "catalog error",
                UsageEvidence {
                    rollouts_discovered: 1,
                    rollouts_scanned: 1,
                    ..UsageEvidence::default()
                },
                1,
            ),
            (
                "unaccounted rollout",
                UsageEvidence {
                    rollouts_discovered: 2,
                    rollouts_scanned: 1,
                    ..UsageEvidence::default()
                },
                0,
            ),
        ];

        for (reason, evidence, catalog_errors) in cases {
            assert_eq!(
                coverage_status(&evidence, catalog_errors),
                CoverageStatus::Partial,
                "{reason} should make coverage partial"
            );
        }
    }

    #[test]
    fn reports_none_and_partial_coverage_with_distinct_findings() {
        let none = build_report(
            "2026-08-09T00:00:00Z".to_owned(),
            parse_rfc3339_epoch("2026-08-09T00:00:00Z").expect("timestamp"),
            Path::new("/codex-home"),
            Vec::new(),
            UsageEvidence::default(),
            0,
        );
        assert_eq!(none.verdict, Verdict::Review);
        assert_eq!(none.findings.len(), 1);
        assert_eq!(none.findings[0].code, "skills.history.unavailable");

        let partial = build_report(
            "2026-08-09T00:00:00Z".to_owned(),
            parse_rfc3339_epoch("2026-08-09T00:00:00Z").expect("timestamp"),
            Path::new("/codex-home"),
            Vec::new(),
            UsageEvidence {
                discovery_errors: 1,
                ..UsageEvidence::default()
            },
            0,
        );
        assert_eq!(partial.verdict, Verdict::Review);
        assert_eq!(partial.findings.len(), 1);
        assert_eq!(partial.findings[0].code, "skills.history.partial");
    }

    #[test]
    fn complete_history_reports_an_unobserved_skill_for_review() {
        let observed_epoch = parse_rfc3339_epoch("2026-08-09T00:00:00Z").expect("timestamp");
        let report = build_report(
            "2026-08-09T00:00:00Z".to_owned(),
            observed_epoch,
            Path::new("/codex-home"),
            vec![skill("unobserved", "/skills/unobserved/SKILL.md")],
            UsageEvidence {
                history_from: Some((
                    format_rfc3339(
                        observed_epoch - UNOBSERVED_HISTORY_DAYS as i64 * SECONDS_PER_DAY,
                    ),
                    observed_epoch - UNOBSERVED_HISTORY_DAYS as i64 * SECONDS_PER_DAY,
                )),
                history_through: Some((format_rfc3339(observed_epoch), observed_epoch)),
                rollouts_discovered: 1,
                rollouts_scanned: 1,
                ..UsageEvidence::default()
            },
            0,
        );

        assert_eq!(report.coverage.status, CoverageStatus::Complete);
        assert_eq!(report.summary.total, 1);
        assert_eq!(report.summary.unobserved, 1);
        assert_eq!(report.verdict, Verdict::Review);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "skills.usage.review")
        );
    }

    #[test]
    fn complete_but_short_history_remains_review() {
        let observed_epoch = parse_rfc3339_epoch("2026-08-09T00:00:00Z").expect("timestamp");
        let report = build_report(
            "2026-08-09T00:00:00Z".to_owned(),
            observed_epoch,
            Path::new("/codex-home"),
            vec![skill("unknown", "/skills/unknown/SKILL.md")],
            UsageEvidence {
                history_from: Some((
                    format_rfc3339(observed_epoch - SECONDS_PER_DAY),
                    observed_epoch - SECONDS_PER_DAY,
                )),
                history_through: Some((format_rfc3339(observed_epoch), observed_epoch)),
                rollouts_discovered: 1,
                rollouts_scanned: 1,
                ..UsageEvidence::default()
            },
            0,
        );

        assert_eq!(report.coverage.status, CoverageStatus::Complete);
        assert_eq!(report.summary.insufficient_history, 1);
        assert_eq!(report.verdict, Verdict::Review);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn missing_turn_ids_still_deduplicate_uses_within_one_turn() {
        let tree = TestDirectory::new();
        let rollout = tree.file(
            "rollout.jsonl",
            concat!(
                "{\"timestamp\":\"2026-06-01T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"thread-1\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:01Z\",\"type\":\"turn_context\",\"payload\":{}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:02Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call\",\"input\":\"cat /skills/example/SKILL.md\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:03Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"function_call\",\"arguments\":\"sed -n 1,20p /skills/example/SKILL.md\"}}\n"
            ),
        );
        let skills = vec![skill("example", "/skills/example/SKILL.md")];
        let scan = scan_rollout(
            &rollout,
            &skills,
            Instant::now() + Duration::from_secs(1),
            None,
        )
        .expect("rollout should scan");
        let mut evidence = UsageEvidence {
            rollouts_discovered: 1,
            rollouts_scanned: 1,
            history_from: scan.history_from,
            history_through: scan.history_through,
            records_unreadable: scan.records_unreadable,
            ..UsageEvidence::default()
        };
        for (identity, observed_use) in scan.uses {
            evidence
                .by_skill
                .entry(identity)
                .or_default()
                .push(observed_use);
        }

        let report = build_report(
            "2026-08-09T00:00:00Z".to_owned(),
            parse_rfc3339_epoch("2026-08-09T00:00:00Z").expect("timestamp"),
            Path::new("/codex-home"),
            skills,
            evidence,
            0,
        );

        assert_eq!(report.items[0].observed_uses_in_window, 1);
    }

    #[test]
    fn task_started_without_turn_context_still_groups_one_turn() {
        let tree = TestDirectory::new();
        let rollout = tree.file(
            "rollout.jsonl",
            concat!(
                "{\"timestamp\":\"2026-08-08T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"thread-1\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:01Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:02Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call\",\"input\":\"cat /skills/example/SKILL.md\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:03Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call\",\"input\":\"cat /skills/example/SKILL.md\"}}\n"
            ),
        );
        let scan = scan_rollout(
            &rollout,
            &[skill("example", "/skills/example/SKILL.md")],
            Instant::now() + Duration::from_secs(1),
            None,
        )
        .expect("rollout should scan");

        assert_eq!(scan.uses.len(), 2);
        assert_eq!(scan.uses[0].1.key, scan.uses[1].1.key);
    }

    #[test]
    fn unrelated_events_do_not_create_turn_boundaries() {
        let tree = TestDirectory::new();
        let rollout = tree.file(
            "rollout.jsonl",
            concat!(
                "{\"timestamp\":\"2026-08-08T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"thread-1\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:01Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"progress\",\"input\":\"task_started\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:02Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call\",\"input\":\"cat /skills/example/SKILL.md\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:03Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call\",\"input\":\"cat /skills/example/SKILL.md\"}}\n"
            ),
        );
        let scan = scan_rollout(
            &rollout,
            &[skill("example", "/skills/example/SKILL.md")],
            Instant::now() + Duration::from_secs(1),
            None,
        )
        .expect("rollout should scan");

        assert_eq!(scan.uses.len(), 2);
        assert_ne!(scan.uses[0].1.key, scan.uses[1].1.key);
    }

    #[test]
    fn paired_and_standalone_turn_contexts_create_distinct_turns() {
        let tree = TestDirectory::new();
        let rollout = tree.file(
            "rollout.jsonl",
            concat!(
                "{\"timestamp\":\"2026-08-08T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"thread-1\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:01Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:02Z\",\"type\":\"turn_context\",\"payload\":{}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:03Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call\",\"input\":\"cat /skills/example/SKILL.md\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:04Z\",\"type\":\"turn_context\",\"payload\":{}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:05Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call\",\"input\":\"cat /skills/example/SKILL.md\"}}\n"
            ),
        );
        let scan = scan_rollout(
            &rollout,
            &[skill("example", "/skills/example/SKILL.md")],
            Instant::now() + Duration::from_secs(1),
            None,
        )
        .expect("rollout should scan");

        assert_eq!(scan.uses.len(), 2);
        assert_ne!(scan.uses[0].1.key, scan.uses[1].1.key);
    }

    #[test]
    fn paired_context_without_an_id_preserves_the_task_started_turn_id() {
        let tree = TestDirectory::new();
        let rollout = tree.file(
            "rollout.jsonl",
            concat!(
                "{\"timestamp\":\"2026-08-08T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"thread-1\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:01Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\",\"turn_id\":\"turn-1\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:02Z\",\"type\":\"turn_context\",\"payload\":{}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:03Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call\",\"input\":\"cat /skills/example/SKILL.md\"}}\n"
            ),
        );
        let scan = scan_rollout(
            &rollout,
            &[skill("example", "/skills/example/SKILL.md")],
            Instant::now() + Duration::from_secs(1),
            None,
        )
        .expect("rollout should scan");

        assert_eq!(scan.uses.len(), 1);
        assert_eq!(scan.uses[0].1.evidence.turn_id.as_deref(), Some("turn-1"));
    }

    #[test]
    fn counts_interesting_malformed_records_without_aborting_the_rollout() {
        let tree = TestDirectory::new();
        let path = tree.path.join("rollout.jsonl");
        let mut contents = concat!(
            "{\"timestamp\":\"2026-08-08T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"thread-1\"}}\n",
            "{\"timestamp\":\"2026-08-08T00:00:01Z\",\"type\":\"turn_context\",\"payload\":"
        )
        .as_bytes()
        .to_vec();
        contents.push(0xff);
        contents.extend_from_slice(b"}\n");
        contents.extend_from_slice(
            b"{\"timestamp\":\"2026-08-08T00:00:02Z\",\"type\":\"turn_context\"\n",
        );
        contents.extend_from_slice(
            b"{\"timestamp\":\"2026-08-08T00:00:03Z\",\"type\":\"turn_context\",\"payload\":{\"turn_id\":7}}\n",
        );
        contents.extend_from_slice(
            b"{\"timestamp\":\"2026-08-08T00:00:04Z\",\"type\":\"event_msg\",\"payload\":{\"turn_id\":\"task_started\"}}\n",
        );
        fs::write(&path, contents).expect("malformed rollout fixture should be written");

        let scan = scan_rollout(&path, &[], Instant::now() + Duration::from_secs(1), None)
            .expect("malformed records should not abort the rollout");

        assert!(scan.complete);
        assert_eq!(scan.records_unreadable, 4);
    }

    #[test]
    fn counts_each_relevant_rollout_record_error() {
        let tree = TestDirectory::new();
        let rollout = tree.file(
            "rollout.jsonl",
            concat!(
                "{\"timestamp\":\"2026-08-08T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":7}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:01Z\",\"type\":\"response_item\",\"payload\":{\"type\":7,\"input\":\"cat /skills/example/SKILL.md\"}}\n",
                "{\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call\",\"input\":\"cat /skills/example/SKILL.md\"}}\n"
            ),
        );

        let scan = scan_rollout(
            &rollout,
            &[skill("example", "/skills/example/SKILL.md")],
            Instant::now() + Duration::from_secs(1),
            None,
        )
        .expect("record errors should not abort the rollout");

        assert!(scan.complete);
        assert_eq!(scan.records_unreadable, 3);
        assert!(scan.uses.is_empty());
    }

    #[test]
    fn ignores_unrelated_response_items_when_extending_history() {
        let tree = TestDirectory::new();
        let rollout = tree.file(
            "rollout.jsonl",
            concat!(
                "{\"timestamp\":\"2026-06-01T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"thread-1\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:00Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call\",\"input\":\"echo unrelated\"}}\n"
            ),
        );

        let scan = scan_rollout(&rollout, &[], Instant::now() + Duration::from_secs(1), None)
            .expect("rollout should scan");

        assert_eq!(
            scan.history_through.as_ref().map(|value| value.0.as_str()),
            Some("2026-06-01T00:00:00Z")
        );
    }

    #[test]
    fn distinguishes_unobserved_from_insufficient_history() {
        let observed_epoch = parse_rfc3339_epoch("2026-08-09T00:00:00Z").expect("timestamp");
        assert!(history_is_sufficient(
            observed_epoch,
            Some(observed_epoch - UNOBSERVED_HISTORY_DAYS as i64 * SECONDS_PER_DAY),
            Some(observed_epoch - RECENT_DAYS as i64 * SECONDS_PER_DAY)
        ));
        assert!(history_is_sufficient(
            observed_epoch,
            parse_rfc3339_epoch("2026-06-01T00:00:00Z"),
            parse_rfc3339_epoch("2026-08-08T00:00:00Z")
        ));
        assert!(!history_is_sufficient(
            observed_epoch,
            parse_rfc3339_epoch("2026-08-01T00:00:00Z"),
            parse_rfc3339_epoch("2026-08-08T00:00:00Z")
        ));
        assert!(!history_is_sufficient(
            observed_epoch,
            parse_rfc3339_epoch("2026-06-01T00:00:00Z"),
            parse_rfc3339_epoch("2026-07-01T00:00:00Z")
        ));
    }

    #[test]
    fn stops_inside_a_rollout_when_the_scan_deadline_is_reached() {
        let tree = TestDirectory::new();
        let path = tree.file(
            "rollout.jsonl",
            "{\"timestamp\":\"2026-08-09T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"thread-1\"}}\n",
        );

        let scan =
            scan_rollout(&path, &[], Instant::now(), None).expect("rollout should stop cleanly");

        assert!(!scan.complete);
        assert!(scan.history_from.is_none());
    }

    #[test]
    fn excludes_the_current_thread_before_aggregating_usage() {
        let tree = TestDirectory::new();
        let old = tree.file(
            "sessions/rollout-old-thread.jsonl",
            concat!(
                "{\"timestamp\":\"2026-08-08T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"old-thread\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:01Z\",\"type\":\"turn_context\",\"payload\":{\"turn_id\":\"turn-1\"}}\n",
                "{\"timestamp\":\"2026-08-08T00:00:02Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call\",\"input\":\"cat /skills/example/SKILL.md\"}}\n"
            ),
        );
        let current = tree.file(
            "sessions/rollout-current-alias.jsonl",
            concat!(
                "{\"timestamp\":\"2026-08-09T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"current-thread\"}}\n",
                "{\"timestamp\":\"2026-08-09T00:00:01Z\",\"type\":\"turn_context\",\"payload\":{\"turn_id\":\"turn-2\"}}\n",
                "{\"timestamp\":\"2026-08-09T00:00:02Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call\",\"input\":\"cat /skills/example/SKILL.md\"}}\n"
            ),
        );
        let skills = vec![skill("example", "/skills/example/SKILL.md")];

        let evidence = scan_rollouts(&[old, current], Some("current-thread"), &skills);

        assert_eq!(evidence.rollouts_discovered, 2);
        assert_eq!(evidence.rollouts_scanned, 1);
        assert_eq!(evidence.rollouts_excluded_current, 1);
        let uses = evidence
            .by_skill
            .get(&SkillIdentity::from(&skills[0]))
            .expect("old-thread evidence should remain");
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].evidence.thread_id, "old-thread");
    }

    #[test]
    fn accepts_the_only_skills_list_entry_when_cwd_spelling_differs() {
        let entry = SkillsListEntry {
            cwd: PathBuf::from("/reported/cwd"),
            skills: vec![skill("example", "/skills/example/SKILL.md")],
            errors: Vec::new(),
        };

        let selected = select_skills_entry(vec![entry], Path::new("/requested/cwd"))
            .expect("sole entry should be authoritative");

        assert_eq!(selected.cwd, Path::new("/reported/cwd"));
    }

    #[test]
    fn selects_the_exact_cwd_from_multiple_skills_list_entries() {
        let selected = select_skills_entry(
            vec![
                skills_list_entry("/wrong/cwd", "wrong"),
                skills_list_entry("/requested/cwd", "selected"),
            ],
            Path::new("/requested/cwd"),
        )
        .expect("exact cwd should be selected");

        assert_eq!(selected.skills[0].name, "selected");
    }

    #[cfg(unix)]
    #[test]
    fn selects_a_canonical_cwd_match_from_multiple_entries() {
        use std::os::unix::fs::symlink;

        let tree = TestDirectory::new();
        let canonical = tree.path.join("canonical-project");
        let wrong = tree.path.join("wrong-project");
        fs::create_dir_all(&canonical).expect("canonical project should be created");
        fs::create_dir_all(&wrong).expect("wrong project should be created");
        let alias = tree.path.join("project-alias");
        symlink(&canonical, &alias).expect("project symlink should be created");

        let selected = select_skills_entry(
            vec![
                skills_list_entry(wrong, "wrong"),
                skills_list_entry(canonical, "selected"),
            ],
            &alias,
        )
        .expect("canonical cwd should be selected");

        assert_eq!(selected.skills[0].name, "selected");
    }

    #[test]
    fn writes_each_rpc_request_as_one_json_line() {
        let mut output = Vec::new();

        send_rpc_request(&mut output, &json!({"id": 7, "method": "skills/list"}))
            .expect("RPC request should be written");

        assert_eq!(output.last(), Some(&b'\n'));
        let value: serde_json::Value =
            serde_json::from_slice(&output).expect("written request should be valid JSON");
        assert_eq!(value["id"], 7);
        assert_eq!(value["method"], "skills/list");
    }

    #[test]
    fn ignores_rpc_responses_for_other_request_ids() {
        let (sender, receiver) = mpsc::channel();
        sender
            .send(Ok(r#"{"id":1,"result":{"selected":false}}"#.to_owned()))
            .expect("unrelated response should be queued");
        sender
            .send(Ok(r#"{"id":2,"result":{"selected":true}}"#.to_owned()))
            .expect("expected response should be queued");

        let raw = receive_rpc_result(&receiver, 2, Instant::now() + Duration::from_secs(1))
            .expect("expected response should be returned");
        let value: serde_json::Value =
            serde_json::from_str(raw.get()).expect("result should be valid JSON");

        assert_eq!(value["selected"], true);
    }

    #[test]
    fn builds_the_catalog_deadline_from_the_full_probe_timeout() {
        let now = Instant::now();

        assert_eq!(probe_deadline(now).duration_since(now), PROBE_TIMEOUT);
    }

    #[test]
    fn classifies_an_unavailable_catalog_as_blocked() {
        let outcome = skill_failure(io::Error::new(
            io::ErrorKind::NotFound,
            "codex executable was not found",
        ));
        let json = outcome
            .to_pretty_json()
            .expect("failure report should serialize");

        assert_eq!(outcome.verdict(), Verdict::Blocked);
        assert!(outcome.is_failure());
        assert!(json.contains(r#""kind": "skills""#));
        assert!(json.contains(r#""code": "skills.catalog.unavailable""#));
        assert!(!json.contains("drilldowns"));
    }

    #[test]
    fn classifies_a_catalog_timeout_as_incomplete() {
        let outcome = skill_failure(io::Error::new(
            io::ErrorKind::TimedOut,
            "Codex app-server timed out",
        ));
        let json = outcome
            .to_pretty_json()
            .expect("failure report should serialize");

        assert_eq!(outcome.verdict(), Verdict::Incomplete);
        assert!(outcome.is_failure());
        assert!(json.contains(r#""code": "skills.catalog.timeout""#));
        assert!(json.contains(r#""status": "timeout""#));
    }
}
