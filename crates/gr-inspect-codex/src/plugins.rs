use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs, io,
    path::{Component, Path, PathBuf},
    time::Duration,
};

use gr_skill_assessment::{CoverageStatus, SkillSignal};
use serde::{Deserialize, Serialize};

use crate::{
    PROBE_TIMEOUT, Verdict,
    process::run_bounded,
    report::{REPORT_SCHEMA_VERSION, ReportFinding, ReportKind},
    skills::{
        AssessedPluginSkill, AssessedPluginSkills, AssessedSkillCoverage, COUNTING_BASIS,
        EVIDENCE_BASIS,
    },
};

const LINK_BASIS: &str = "exact_skill_manifest_path_under_native_source_or_identity_cache_root";
const PLUGIN_AGGREGATION_BASIS: &str = "sum_of_per_skill_unique_thread_turn_counts";
const EVIDENCE_LIMITATIONS: [&str; 3] = [
    "skill_use_is_observed_only_from_exact_manifest_paths_in_retained_rollouts",
    "plugin_use_through_mcp_apps_or_other_non_skill_capabilities_is_not_measured",
    "no_observed_skill_use_does_not_mean_the_plugin_is_unused",
];

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PluginReport {
    pub(crate) installed: Vec<Plugin>,
    pub(crate) available: Vec<Plugin>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Plugin {
    pub(crate) plugin_id: String,
    pub(crate) name: String,
    pub(crate) marketplace_name: String,
    pub(crate) version: String,
    pub(crate) installed: bool,
    pub(crate) enabled: bool,
    pub(crate) auth_policy: Option<String>,
    pub(crate) source: Option<PluginSource>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct PluginSource {
    #[serde(rename = "source")]
    pub(crate) source_type: Option<String>,
    pub(crate) path: Option<PathBuf>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PluginInspectionReport {
    schema_version: u32,
    kind: ReportKind,
    verdict: Verdict,
    summary: PluginInventorySummary,
    skill_evidence: PluginSkillEvidenceReport,
    items: Vec<PluginInventoryItem>,
    findings: Vec<ReportFinding>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct PluginInventorySummary {
    installed: usize,
    enabled: usize,
    plugin_skills: usize,
    linked_plugin_skills: usize,
    plugins_with_skills: usize,
    plugins_with_observed_skill_use: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct PluginInventoryItem {
    plugin_id: String,
    name: String,
    marketplace: String,
    version: String,
    enabled: bool,
    auth_policy: Option<String>,
    skill_summary: PluginItemSkillSummary,
    skills: Vec<PluginSkillItem>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct PluginItemSkillSummary {
    active: usize,
    with_observed_use: usize,
    observed_uses_in_window: usize,
    last_observed_use_at: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct PluginSkillItem {
    name: String,
    manifest_path: String,
    signal: SkillSignal,
    last_observed_use_at: Option<String>,
    observed_uses_in_window: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct PluginSkillEvidenceReport {
    observed_at: String,
    thresholds: PluginSkillThresholds,
    coverage: AssessedSkillCoverage,
    evidence_basis: &'static str,
    skill_counting_basis: &'static str,
    plugin_aggregation_basis: &'static str,
    link_basis: &'static str,
    evidence_limitations: [&'static str; 3],
    link_coverage: PluginSkillLinkCoverage,
    invalid_plugin_ids: Vec<String>,
    duplicate_plugin_ids: Vec<String>,
    unlinked_skills: Vec<PluginSkillIdentityItem>,
    ambiguous_skills: Vec<PluginSkillIdentityItem>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct PluginSkillThresholds {
    recent_days: u64,
    unobserved_history_days: u64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct PluginSkillLinkCoverage {
    plugin_skills: usize,
    linked: usize,
    unlinked: usize,
    ambiguous: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct PluginSkillIdentityItem {
    name: String,
    manifest_path: String,
}

#[derive(Debug)]
struct PluginRecord {
    plugin_id: String,
    name: String,
    marketplace: String,
    version: String,
    enabled: bool,
    auth_policy: Option<String>,
    link_roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SkillIdentity {
    name: String,
    manifest_path: PathBuf,
}

#[derive(Debug)]
struct SkillRecord {
    identity: SkillIdentity,
    signal: SkillSignal,
    last_observed_use_at: Option<String>,
    last_observed_epoch: Option<i64>,
    observed_uses_in_window: usize,
}

#[derive(Debug, PartialEq, Eq)]
struct PluginSkillLink {
    plugin_id: String,
    skill: SkillIdentity,
}

impl PluginInspectionReport {
    pub(crate) const fn verdict(&self) -> Verdict {
        self.verdict
    }

    pub(crate) fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub(crate) fn to_human(&self) -> String {
        let mut output = String::new();
        writeln!(
            output,
            "Codex plugins: {} installed, {} enabled; {}/{} plugin skills linked",
            self.summary.installed,
            self.summary.enabled,
            self.summary.linked_plugin_skills,
            self.summary.plugin_skills
        )
        .expect("writing to String");
        for item in &self.items {
            writeln!(
                output,
                "  {} ({}): {} {} [{}; auth {}; skills {} active, {} observed]",
                item.plugin_id,
                item.name,
                item.marketplace,
                item.version,
                if item.enabled { "enabled" } else { "disabled" },
                item.auth_policy.as_deref().unwrap_or("unknown"),
                item.skill_summary.active,
                item.skill_summary.with_observed_use
            )
            .expect("writing to String");
        }
        if !self.findings.is_empty() {
            writeln!(output, "Findings:").expect("writing to String");
            for finding in &self.findings {
                writeln!(
                    output,
                    "  {} [{}]: {}",
                    finding.code, finding.status, finding.message
                )
                .expect("writing to String");
            }
        }
        writeln!(output, "Verdict: {}", self.verdict.as_str()).expect("writing to String");
        output
    }
}

pub(crate) fn build_plugin_inspection_report(
    report: PluginReport,
    evidence: AssessedPluginSkills,
) -> PluginInspectionReport {
    let AssessedPluginSkills {
        plugin_cache_root,
        observed_at,
        recent_days,
        unobserved_history_days,
        coverage,
        skills,
    } = evidence;
    let duplicate_plugin_ids = duplicate_plugin_ids(&report.installed);
    let cache_root_valid = valid_absolute_root(&plugin_cache_root);
    let plugin_cache_root = comparable_path(&plugin_cache_root);
    let mut invalid_plugin_ids = BTreeSet::new();
    let mut plugins = report
        .installed
        .into_iter()
        .map(|plugin| {
            let identity_valid = cache_root_valid
                && valid_identity_component(&plugin.marketplace_name)
                && valid_identity_component(&plugin.name)
                && valid_identity_component(&plugin.version);
            let mut link_roots = Vec::with_capacity(2);
            if identity_valid {
                link_roots.push(comparable_path(
                    &plugin_cache_root
                        .join(&plugin.marketplace_name)
                        .join(&plugin.name)
                        .join(&plugin.version),
                ));
            } else {
                invalid_plugin_ids.insert(plugin.plugin_id.clone());
            }
            if let Some(source) = plugin.source {
                let PluginSource { source_type, path } = source;
                match path {
                    Some(path) if valid_absolute_root(&path) => {
                        link_roots.push(comparable_path(&path));
                    }
                    // Git sources expose a repository-relative subdirectory. It is
                    // valid metadata, but it must never become a filesystem root.
                    Some(path)
                        if source_type.as_deref() == Some("git-subdir")
                            && valid_relative_source_metadata_path(&path) => {}
                    _ => {
                        invalid_plugin_ids.insert(plugin.plugin_id.clone());
                    }
                }
            }
            link_roots.sort();
            link_roots.dedup();
            PluginRecord {
                plugin_id: plugin.plugin_id,
                name: plugin.name,
                marketplace: plugin.marketplace_name,
                version: plugin.version,
                enabled: plugin.enabled,
                auth_policy: plugin.auth_policy,
                link_roots,
            }
        })
        .collect::<Vec<_>>();
    plugins.sort_by(|left, right| {
        left.plugin_id
            .cmp(&right.plugin_id)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.marketplace.cmp(&right.marketplace))
            .then_with(|| left.version.cmp(&right.version))
    });

    let skills = skills
        .into_iter()
        .map(normalize_skill_record)
        .collect::<Vec<_>>();
    let (links, unlinked, ambiguous) = link_plugin_skills(&plugins, &skills, &duplicate_plugin_ids);
    let linked = links.len();
    let plugin_skills = skills.len();
    let links_by_plugin = links.into_iter().fold(
        BTreeMap::<String, BTreeSet<SkillIdentity>>::new(),
        |mut grouped, link| {
            grouped
                .entry(link.plugin_id)
                .or_default()
                .insert(link.skill);
            grouped
        },
    );
    let skills_by_identity = skills
        .into_iter()
        .map(|skill| (skill.identity.clone(), skill))
        .collect::<BTreeMap<_, _>>();
    let items = plugins
        .into_iter()
        .map(|plugin| {
            let mut linked_skills = links_by_plugin
                .get(&plugin.plugin_id)
                .into_iter()
                .flatten()
                .filter_map(|identity| skills_by_identity.get(identity))
                .map(skill_item)
                .collect::<Vec<_>>();
            linked_skills.sort_by(|left, right| {
                left.name
                    .cmp(&right.name)
                    .then_with(|| left.manifest_path.cmp(&right.manifest_path))
            });
            let last_observed_use_at = links_by_plugin
                .get(&plugin.plugin_id)
                .into_iter()
                .flatten()
                .filter_map(|identity| skills_by_identity.get(identity))
                .filter_map(|skill| {
                    skill
                        .last_observed_epoch
                        .zip(skill.last_observed_use_at.as_deref())
                })
                .max_by_key(|(epoch, _)| *epoch)
                .map(|(_, timestamp)| timestamp.to_owned());
            let with_observed_use = linked_skills
                .iter()
                .filter(|skill| skill.observed_uses_in_window > 0)
                .count();
            let observed_uses_in_window = linked_skills
                .iter()
                .map(|skill| skill.observed_uses_in_window)
                .sum();

            PluginInventoryItem {
                plugin_id: plugin.plugin_id,
                name: plugin.name,
                marketplace: plugin.marketplace,
                version: plugin.version,
                enabled: plugin.enabled,
                auth_policy: plugin.auth_policy,
                skill_summary: PluginItemSkillSummary {
                    active: linked_skills.len(),
                    with_observed_use,
                    observed_uses_in_window,
                    last_observed_use_at,
                },
                skills: linked_skills,
            }
        })
        .collect::<Vec<_>>();

    let link_coverage = PluginSkillLinkCoverage {
        plugin_skills,
        linked,
        unlinked: unlinked.len(),
        ambiguous: ambiguous.len(),
    };
    let mut findings = plugin_skill_findings(
        &coverage,
        &link_coverage,
        &invalid_plugin_ids,
        &duplicate_plugin_ids,
    );
    let verdict = if findings.is_empty() {
        Verdict::BaselineOk
    } else {
        Verdict::Review
    };
    findings.shrink_to_fit();

    PluginInspectionReport {
        schema_version: REPORT_SCHEMA_VERSION,
        kind: ReportKind::Plugins,
        verdict,
        summary: PluginInventorySummary {
            installed: items.len(),
            enabled: items.iter().filter(|item| item.enabled).count(),
            plugin_skills,
            linked_plugin_skills: linked,
            plugins_with_skills: items
                .iter()
                .filter(|item| item.skill_summary.active > 0)
                .count(),
            plugins_with_observed_skill_use: items
                .iter()
                .filter(|item| item.skill_summary.with_observed_use > 0)
                .count(),
        },
        skill_evidence: PluginSkillEvidenceReport {
            observed_at,
            thresholds: PluginSkillThresholds {
                recent_days,
                unobserved_history_days,
            },
            coverage,
            evidence_basis: EVIDENCE_BASIS,
            skill_counting_basis: COUNTING_BASIS,
            plugin_aggregation_basis: PLUGIN_AGGREGATION_BASIS,
            link_basis: LINK_BASIS,
            evidence_limitations: EVIDENCE_LIMITATIONS,
            link_coverage,
            invalid_plugin_ids: invalid_plugin_ids.into_iter().collect(),
            duplicate_plugin_ids: duplicate_plugin_ids.into_iter().collect(),
            unlinked_skills: unlinked.into_iter().map(identity_item).collect(),
            ambiguous_skills: ambiguous.into_iter().map(identity_item).collect(),
        },
        items,
        findings,
    }
}

fn normalize_skill_record(skill: AssessedPluginSkill) -> SkillRecord {
    SkillRecord {
        identity: SkillIdentity {
            name: skill.name,
            manifest_path: skill.manifest_path,
        },
        signal: skill.signal,
        last_observed_use_at: skill.last_observed_use_at,
        last_observed_epoch: skill.last_observed_epoch,
        observed_uses_in_window: skill.observed_uses_in_window,
    }
}

fn link_plugin_skills(
    plugins: &[PluginRecord],
    skills: &[SkillRecord],
    duplicate_plugin_ids: &BTreeSet<String>,
) -> (Vec<PluginSkillLink>, Vec<SkillIdentity>, Vec<SkillIdentity>) {
    let mut links = Vec::new();
    let mut unlinked = Vec::new();
    let mut ambiguous = Vec::new();

    for skill in skills {
        let comparable_manifest = comparable_path(&skill.identity.manifest_path);
        let matches = plugins
            .iter()
            .filter(|plugin| {
                plugin
                    .link_roots
                    .iter()
                    .any(|root| comparable_manifest.starts_with(root))
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => unlinked.push(skill.identity.clone()),
            [plugin] if !duplicate_plugin_ids.contains(&plugin.plugin_id) => {
                links.push(PluginSkillLink {
                    plugin_id: plugin.plugin_id.clone(),
                    skill: skill.identity.clone(),
                })
            }
            _ => ambiguous.push(skill.identity.clone()),
        }
    }

    (links, unlinked, ambiguous)
}

fn comparable_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn valid_absolute_root(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }
    let mut normal_components = 0;
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {}
            Component::Normal(_) => normal_components += 1,
            Component::CurDir | Component::ParentDir => return false,
        }
    }
    normal_components > 0
}

fn valid_relative_source_metadata_path(path: &Path) -> bool {
    if path.is_absolute() {
        return false;
    }
    let mut normal_components = 0;
    for component in path.components() {
        match component {
            Component::Normal(_) => normal_components += 1,
            Component::Prefix(_)
            | Component::RootDir
            | Component::CurDir
            | Component::ParentDir => return false,
        }
    }
    normal_components > 0
}

fn valid_identity_component(value: &str) -> bool {
    let mut components = Path::new(value).components();
    matches!(components.next(), Some(Component::Normal(part)) if !part.is_empty())
        && components.next().is_none()
}

fn duplicate_plugin_ids(plugins: &[Plugin]) -> BTreeSet<String> {
    let counts = plugins.iter().fold(BTreeMap::new(), |mut counts, plugin| {
        *counts.entry(plugin.plugin_id.clone()).or_insert(0_usize) += 1;
        counts
    });
    counts
        .into_iter()
        .filter_map(|(plugin_id, count)| (count > 1).then_some(plugin_id))
        .collect()
}

fn skill_item(skill: &SkillRecord) -> PluginSkillItem {
    PluginSkillItem {
        name: skill.identity.name.clone(),
        manifest_path: skill.identity.manifest_path.to_string_lossy().into_owned(),
        signal: skill.signal,
        last_observed_use_at: skill.last_observed_use_at.clone(),
        observed_uses_in_window: skill.observed_uses_in_window,
    }
}

fn identity_item(identity: SkillIdentity) -> PluginSkillIdentityItem {
    PluginSkillIdentityItem {
        name: identity.name,
        manifest_path: identity.manifest_path.to_string_lossy().into_owned(),
    }
}

fn plugin_skill_findings(
    coverage: &AssessedSkillCoverage,
    links: &PluginSkillLinkCoverage,
    invalid_plugin_ids: &BTreeSet<String>,
    duplicate_plugin_ids: &BTreeSet<String>,
) -> Vec<ReportFinding> {
    let mut findings = Vec::new();
    if !invalid_plugin_ids.is_empty() {
        findings.push(ReportFinding::new(
            "plugins.identity.invalid",
            "review",
            format!(
                "{} installed plugins have invalid source roots or identity path components",
                invalid_plugin_ids.len()
            ),
        ));
    }
    if !duplicate_plugin_ids.is_empty() {
        findings.push(ReportFinding::new(
            "plugins.identity.duplicate",
            "review",
            format!(
                "{} plugin IDs occur more than once in the installed inventory",
                duplicate_plugin_ids.len()
            ),
        ));
    }
    match coverage.status {
        CoverageStatus::Complete => {}
        CoverageStatus::Partial => findings.push(ReportFinding::new(
            "plugins.skills.evidence.partial",
            "partial",
            "plugin skill evidence coverage is partial; inspect coverage counters for the cause",
        )),
        CoverageStatus::None => findings.push(ReportFinding::new(
            "plugins.skills.history.unavailable",
            "unknown",
            "no retained Codex rollouts were available for plugin skill usage evidence",
        )),
    }
    if links.unlinked > 0 {
        findings.push(ReportFinding::new(
            "plugins.skills.unlinked",
            "review",
            format!(
                "{} plugin-origin skills could not be linked to an exact native source or identity cache root",
                links.unlinked
            ),
        ));
    }
    if links.ambiguous > 0 {
        findings.push(ReportFinding::new(
            "plugins.skills.ambiguous",
            "review",
            format!(
                "{} plugin-origin skills matched more than one exact plugin root",
                links.ambiguous
            ),
        ));
    }
    findings
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PluginProbe {
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) exit_code: Option<i32>,
    pub(crate) duration: Duration,
    pub(crate) timed_out: bool,
}

impl PluginProbe {
    pub(crate) fn succeeded(&self) -> bool {
        !self.timed_out && self.exit_code == Some(0)
    }

    pub(crate) fn report(&self) -> Result<PluginReport, serde_json::Error> {
        serde_json::from_slice(&self.stdout)
    }
}

#[cfg(test)]
fn parse_plugins(json: &str) -> Result<PluginReport, serde_json::Error> {
    serde_json::from_str(json)
}

pub(crate) fn probe_plugins() -> io::Result<PluginProbe> {
    let output = run_bounded("codex", &["plugin", "list", "--json"], PROBE_TIMEOUT)?;

    Ok(PluginProbe {
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
        let mut probe = PluginProbe {
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
    fn parses_plugin_report() {
        let json = r#"{
            "installed": [
                {
                    "pluginId": "github@openai-curated",
                    "name": "github",
                    "marketplaceName": "openai-curated",
                    "version": "0.1.8",
                    "installed": true,
                    "enabled": true,
                    "authPolicy": "ON_INSTALL",
                    "source": {
                        "path": "/plugins/github/0.1.8",
                        "source": "local"
                    }
                }
            ],
            "available": []
        }"#;

        let report = parse_plugins(json).expect("plugin report should parse");

        assert_eq!(report.installed.len(), 1);
        assert!(report.available.is_empty());
        assert_eq!(report.installed[0].plugin_id, "github@openai-curated");
        assert_eq!(report.installed[0].name, "github");
        assert_eq!(report.installed[0].marketplace_name, "openai-curated");
        assert_eq!(report.installed[0].version, "0.1.8");
        assert!(report.installed[0].installed);
        assert!(report.installed[0].enabled);
        assert_eq!(
            report.installed[0].auth_policy.as_deref(),
            Some("ON_INSTALL")
        );
        assert_eq!(
            report.installed[0]
                .source
                .as_ref()
                .and_then(|source| source.path.as_deref()),
            Some(Path::new("/plugins/github/0.1.8"))
        );
        assert_eq!(
            report.installed[0]
                .source
                .as_ref()
                .and_then(|source| source.source_type.as_deref()),
            Some("local")
        );
    }

    #[test]
    fn parses_git_subdir_source_path_as_relative_metadata() {
        let json = r#"{
            "installed": [{
                "pluginId": "goalrail@goalrail",
                "name": "goalrail",
                "marketplaceName": "goalrail",
                "version": "0.3.1",
                "installed": true,
                "enabled": true,
                "source": {
                    "source": "git-subdir",
                    "url": "https://github.com/heurema/goalrail-rs.git",
                    "path": "plugins/goalrail",
                    "ref": "v0.3.1"
                }
            }],
            "available": []
        }"#;

        let report = parse_plugins(json).expect("git-subdir plugin report should parse");

        assert_eq!(
            report.installed[0]
                .source
                .as_ref()
                .and_then(|source| source.path.as_deref()),
            Some(Path::new("plugins/goalrail"))
        );
    }

    #[test]
    fn links_skills_by_native_source_path_and_keeps_disabled_plugins_factual() {
        let report = build_plugin_inspection_report(
            PluginReport {
                installed: vec![
                    plugin("zeta@market", "2.0.0", false, None, Some("/plugins/zeta")),
                    plugin("theta@market", "1.0.0", true, None, Some("/plugins/theta")),
                    plugin(
                        "alpha@market",
                        "1.0.0",
                        true,
                        Some("ON_USE"),
                        Some("/plugins/alpha"),
                    ),
                ],
                available: vec![plugin(
                    "available@market",
                    "3.0.0",
                    false,
                    Some("ON_INSTALL"),
                    Some("/plugins/available"),
                )],
            },
            complete_evidence(vec![
                skill(
                    "alpha:b",
                    "/plugins/alpha/skills/b/SKILL.md",
                    SkillSignal::Recent,
                    Some(("2026-08-10T12:00:00Z", 100)),
                    2,
                ),
                skill(
                    "alpha:a",
                    "/plugins/alpha/skills/a/SKILL.md",
                    SkillSignal::Aging,
                    Some(("2026-08-09T12:00:00Z", 90)),
                    1,
                ),
                skill(
                    "alpha:c",
                    "/plugins/alpha/skills/c/SKILL.md",
                    SkillSignal::Unobserved,
                    None,
                    0,
                ),
            ]),
        );

        assert_eq!(report.verdict, Verdict::BaselineOk);
        assert_eq!(report.summary.installed, 3);
        assert_eq!(report.summary.enabled, 2);
        assert_eq!(report.summary.plugin_skills, 3);
        assert_eq!(report.summary.linked_plugin_skills, 3);
        assert_eq!(report.summary.plugins_with_skills, 1);
        assert_eq!(report.summary.plugins_with_observed_skill_use, 1);
        assert_eq!(report.skill_evidence.observed_at, "2026-08-11T00:00:00Z");
        assert_eq!(report.skill_evidence.thresholds.recent_days, 7);
        assert_eq!(report.skill_evidence.thresholds.unobserved_history_days, 30);
        assert_eq!(report.skill_evidence.skill_counting_basis, COUNTING_BASIS);
        assert_eq!(
            report.skill_evidence.plugin_aggregation_basis,
            PLUGIN_AGGREGATION_BASIS
        );
        assert!(report.skill_evidence.invalid_plugin_ids.is_empty());
        assert!(report.skill_evidence.duplicate_plugin_ids.is_empty());
        assert_eq!(report.items.len(), 3);
        assert_eq!(report.items[0].plugin_id, "alpha@market");
        assert_eq!(report.items[0].skills[0].name, "alpha:a");
        assert_eq!(report.items[0].skills[1].name, "alpha:b");
        assert_eq!(report.items[0].skills[2].name, "alpha:c");
        assert_eq!(report.items[0].skill_summary.active, 3);
        assert_eq!(report.items[0].skill_summary.with_observed_use, 2);
        assert_eq!(report.items[0].skill_summary.observed_uses_in_window, 3);
        assert_eq!(
            report.items[0]
                .skill_summary
                .last_observed_use_at
                .as_deref(),
            Some("2026-08-10T12:00:00Z")
        );
        assert_eq!(report.items[1].plugin_id, "theta@market");
        assert_eq!(report.items[1].skill_summary.active, 0);
        assert_eq!(report.items[2].plugin_id, "zeta@market");
        assert_eq!(report.items[2].skill_summary.active, 0);
        assert!(report.findings.is_empty());

        let json = report
            .to_pretty_json()
            .expect("plugin inventory should serialize");
        assert!(json.contains(r#""kind": "plugins""#));
        assert!(json.contains(r#""authPolicy": null"#));
        assert!(!json.contains("available@market"));
        assert!(!json.contains(r#""source""#));
        assert!(!json.contains("cleanupDisposition"));

        let human = report.to_human();
        assert!(human.contains("Codex plugins: 3 installed, 2 enabled; 3/3 plugin skills linked"));
        assert!(human.contains(
            "alpha@market (alpha): market 1.0.0 [enabled; auth ON_USE; skills 3 active, 2 observed]"
        ));
        assert!(human.contains(
            "zeta@market (zeta): market 2.0.0 [disabled; auth unknown; skills 0 active, 0 observed]"
        ));
        assert!(!human.contains("Findings:"));
    }

    #[test]
    fn exposes_unlinked_and_ambiguous_skills_instead_of_guessing() {
        let report = build_plugin_inspection_report(
            PluginReport {
                installed: vec![
                    plugin("root@market", "1", true, None, Some("/plugins/root")),
                    plugin(
                        "nested@market",
                        "1",
                        true,
                        None,
                        Some("/plugins/root/nested"),
                    ),
                ],
                available: Vec::new(),
            },
            complete_evidence(vec![
                skill(
                    "ambiguous",
                    "/plugins/root/nested/skills/a/SKILL.md",
                    SkillSignal::Unobserved,
                    None,
                    0,
                ),
                skill(
                    "unlinked",
                    "/plugins/other/skills/b/SKILL.md",
                    SkillSignal::Unobserved,
                    None,
                    0,
                ),
            ]),
        );

        assert_eq!(report.verdict, Verdict::Review);
        assert_eq!(report.skill_evidence.link_coverage.linked, 0);
        assert_eq!(report.skill_evidence.link_coverage.unlinked, 1);
        assert_eq!(report.skill_evidence.link_coverage.ambiguous, 1);
        assert_eq!(report.skill_evidence.unlinked_skills[0].name, "unlinked");
        assert_eq!(report.skill_evidence.ambiguous_skills[0].name, "ambiguous");
        assert_eq!(report.findings.len(), 2);
        assert_eq!(report.findings[0].code, "plugins.skills.unlinked");
        assert_eq!(report.findings[1].code, "plugins.skills.ambiguous");
    }

    #[test]
    fn rejects_unsafe_source_roots_and_identity_components() {
        for invalid in ["", ".", "..", "nested/value", "/absolute"] {
            assert!(!valid_identity_component(invalid), "{invalid}");
        }
        assert!(valid_identity_component("safe-value"));
        for invalid in ["", ".", "..", "/", "/safe/../escape"] {
            assert!(!valid_absolute_root(Path::new(invalid)), "{invalid}");
        }
        assert!(valid_absolute_root(Path::new("/safe/root")));
        for invalid in ["", ".", "..", "/absolute", "plugins/../victim"] {
            assert!(
                !valid_relative_source_metadata_path(Path::new(invalid)),
                "{invalid}"
            );
        }
        assert!(valid_relative_source_metadata_path(Path::new(
            "plugins/goalrail"
        )));

        let mut unsafe_marketplace = plugin("marketplace@market", "1", true, None, None);
        unsafe_marketplace.marketplace_name = "..".to_owned();
        let mut unsafe_name = plugin("name@market", "1", true, None, None);
        unsafe_name.name = "..".to_owned();
        let unsafe_version = plugin("version@market", "/victim", true, None, None);
        let mut missing_source_path = plugin("missing-source@market", "1", true, None, None);
        missing_source_path.source = Some(PluginSource {
            source_type: Some("local".to_owned()),
            path: None,
        });
        let mut relative_local = plugin(
            "relative-local@market",
            "1",
            true,
            None,
            Some("plugins/local"),
        );
        relative_local
            .source
            .as_mut()
            .expect("source should exist")
            .source_type = Some("local".to_owned());
        let report = build_plugin_inspection_report(
            PluginReport {
                installed: vec![
                    plugin("source@market", "1", true, None, Some("")),
                    unsafe_marketplace,
                    unsafe_name,
                    unsafe_version,
                    missing_source_path,
                    relative_local,
                ],
                available: Vec::new(),
            },
            complete_evidence(vec![skill(
                "victim",
                "/victim/skills/example/SKILL.md",
                SkillSignal::Unobserved,
                None,
                0,
            )]),
        );

        assert_eq!(report.verdict, Verdict::Review);
        assert_eq!(
            report.skill_evidence.invalid_plugin_ids,
            [
                "marketplace@market",
                "missing-source@market",
                "name@market",
                "relative-local@market",
                "source@market",
                "version@market"
            ]
        );
        assert_eq!(report.skill_evidence.link_coverage.linked, 0);
        assert_eq!(report.skill_evidence.link_coverage.unlinked, 1);
        assert_eq!(report.findings[0].code, "plugins.identity.invalid");

        let mut invalid_cache = complete_evidence(Vec::new());
        invalid_cache.plugin_cache_root = PathBuf::from("relative/cache");
        let invalid_cache_report = build_plugin_inspection_report(
            PluginReport {
                installed: vec![plugin("cache@market", "1", true, None, None)],
                available: Vec::new(),
            },
            invalid_cache,
        );
        assert_eq!(invalid_cache_report.verdict, Verdict::Review);
        assert_eq!(
            invalid_cache_report.skill_evidence.invalid_plugin_ids,
            ["cache@market"]
        );
    }

    #[test]
    fn accepts_repository_relative_source_metadata_without_using_it_as_a_root() {
        let report = build_plugin_inspection_report(
            PluginReport {
                installed: vec![plugin(
                    "goalrail@market",
                    "0.3.1",
                    true,
                    None,
                    Some("plugins/goalrail"),
                )],
                available: Vec::new(),
            },
            complete_evidence(vec![skill(
                "goalrail",
                "/plugins/cache/market/goalrail/0.3.1/skills/goalrail/SKILL.md",
                SkillSignal::Recent,
                Some(("2026-08-10T12:00:00Z", 1_786_363_200)),
                1,
            )]),
        );

        assert_eq!(report.verdict, Verdict::BaselineOk);
        assert!(report.skill_evidence.invalid_plugin_ids.is_empty());
        assert_eq!(report.skill_evidence.link_coverage.linked, 1);
        assert_eq!(report.items[0].skills[0].name, "goalrail");
    }

    #[test]
    fn duplicate_plugin_ids_never_collapse_to_a_unique_link() {
        let report = build_plugin_inspection_report(
            PluginReport {
                installed: vec![
                    plugin("duplicate@market", "1", true, None, Some("/plugins/one")),
                    plugin("duplicate@market", "2", true, None, Some("/plugins/two")),
                ],
                available: Vec::new(),
            },
            complete_evidence(vec![skill(
                "duplicate:skill",
                "/plugins/one/skills/example/SKILL.md",
                SkillSignal::Unobserved,
                None,
                0,
            )]),
        );

        assert_eq!(report.verdict, Verdict::Review);
        assert_eq!(
            report.skill_evidence.duplicate_plugin_ids,
            ["duplicate@market"]
        );
        assert_eq!(report.skill_evidence.link_coverage.linked, 0);
        assert_eq!(report.skill_evidence.link_coverage.ambiguous, 1);
        assert!(report.items.iter().all(|item| item.skills.is_empty()));
        assert_eq!(report.findings[0].code, "plugins.identity.duplicate");
        assert_eq!(report.findings[1].code, "plugins.skills.ambiguous");
    }

    #[test]
    fn partial_coverage_requires_review_without_guessing_the_cause_or_calling_a_plugin_unused() {
        let mut evidence = complete_evidence(Vec::new());
        evidence.coverage.status = CoverageStatus::Partial;
        evidence.coverage.catalog_errors = 1;

        let report = build_plugin_inspection_report(
            PluginReport {
                installed: vec![plugin(
                    "alpha@market",
                    "1",
                    true,
                    None,
                    Some("/plugins/alpha"),
                )],
                available: Vec::new(),
            },
            evidence,
        );

        assert_eq!(report.verdict, Verdict::Review);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].code, "plugins.skills.evidence.partial");
        assert_eq!(
            report.findings[0].message,
            "plugin skill evidence coverage is partial; inspect coverage counters for the cause"
        );
        assert!(!report.findings[0].message.contains("could not read"));
        let json = report.to_pretty_json().expect("report should serialize");
        assert!(json.contains("no_observed_skill_use_does_not_mean_the_plugin_is_unused"));
        assert!(!json.contains(r#""cleanup""#));
        let human = report.to_human();
        assert!(human.contains("Findings:"));
        assert!(human.contains("plugins.skills.evidence.partial [partial]"));
    }

    fn plugin(
        plugin_id: &str,
        version: &str,
        enabled: bool,
        auth_policy: Option<&str>,
        source_root: Option<&str>,
    ) -> Plugin {
        Plugin {
            plugin_id: plugin_id.to_owned(),
            name: plugin_id.split('@').next().unwrap_or(plugin_id).to_owned(),
            marketplace_name: "market".to_owned(),
            version: version.to_owned(),
            installed: true,
            enabled,
            auth_policy: auth_policy.map(str::to_owned),
            source: source_root.map(|path| PluginSource {
                source_type: Some(
                    if Path::new(path).is_absolute() {
                        "local"
                    } else {
                        "git-subdir"
                    }
                    .to_owned(),
                ),
                path: Some(PathBuf::from(path)),
            }),
        }
    }

    fn skill(
        name: &str,
        manifest_path: &str,
        signal: SkillSignal,
        observed: Option<(&str, i64)>,
        observed_uses_in_window: usize,
    ) -> AssessedPluginSkill {
        AssessedPluginSkill {
            name: name.to_owned(),
            manifest_path: PathBuf::from(manifest_path),
            signal,
            last_observed_use_at: observed.map(|(timestamp, _)| timestamp.to_owned()),
            last_observed_epoch: observed.map(|(_, epoch)| epoch),
            observed_uses_in_window,
        }
    }

    fn complete_evidence(skills: Vec<AssessedPluginSkill>) -> AssessedPluginSkills {
        AssessedPluginSkills {
            plugin_cache_root: PathBuf::from("/plugins/cache"),
            observed_at: "2026-08-11T00:00:00Z".to_owned(),
            recent_days: 7,
            unobserved_history_days: 30,
            coverage: AssessedSkillCoverage {
                status: CoverageStatus::Complete,
                history_from: Some("2026-07-01T00:00:00Z".to_owned()),
                history_through: Some("2026-08-10T12:00:00Z".to_owned()),
                rollouts_discovered: 2,
                rollouts_scanned: 2,
                rollouts_excluded_current: 0,
                rollouts_unreadable: 0,
                discovery_errors: 0,
                records_unreadable: 0,
                catalog_errors: 0,
                truncated: false,
            },
            skills,
        }
    }
}
