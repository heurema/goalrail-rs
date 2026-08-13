use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use gr_inspect_core::Verdict;
use serde::Serialize;

use crate::{
    local::{InstructionSource, LocalEvidence},
    probes::InstalledPlugin,
};

pub(crate) const REPORT_SCHEMA_VERSION: u32 = 1;
pub(crate) const MCP_BASIS: &str = "configuration_files_without_health_checks";
pub(crate) const SKILL_BASIS: &str = "skill_manifest_directories_on_disk";
pub(crate) const PROJECT_ROOT_BASIS: &str = "nearest_ancestor_containing_dot_git";

/// What this command cannot observe, stated with the report rather than left
/// for a reader to assume.
pub(crate) const EVIDENCE_LIMITATIONS: [&str; 4] = [
    "claude doctor exposes no machine-readable output, so installation diagnostics are not inspected",
    "claude mcp list exposes no machine-readable output, so MCP servers are counted from configuration files without connection, health, or authentication state",
    "skill counts cover personal and project skill manifests on disk; bundled skills and skills contributed by plugins are not counted by this basis",
    "no plugin or skill usage history is observed, so nothing in this report means a plugin or skill is unused",
];

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ReportKind {
    Summary,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ProjectStateEntry {
    Present,
    Absent,
}

impl ProjectStateEntry {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Absent => "absent",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PluginScopeSummary {
    pub(crate) user: usize,
    pub(crate) project: usize,
    pub(crate) local: usize,
    pub(crate) unknown: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PluginSummary {
    pub(crate) installed: usize,
    pub(crate) enabled: usize,
    pub(crate) by_scope: PluginScopeSummary,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CountSummary {
    pub(crate) configured: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpScopeSummary {
    pub(crate) user: usize,
    pub(crate) project: usize,
    pub(crate) local: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpSummary {
    pub(crate) configured: usize,
    pub(crate) by_scope: McpScopeSummary,
    pub(crate) basis: &'static str,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillSummary {
    pub(crate) personal: usize,
    pub(crate) project: usize,
    pub(crate) basis: &'static str,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectSummary {
    pub(crate) claude_home: PathBuf,
    pub(crate) current_dir: PathBuf,
    pub(crate) root: Option<PathBuf>,
    pub(crate) root_basis: &'static str,
    pub(crate) state_entry: ProjectStateEntry,
    /// The exact `projects` key the state entry was matched on, so a reader
    /// never has to infer which directory it describes.
    pub(crate) state_entry_path: Option<String>,
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
pub(crate) struct ClaudeFailureReport {
    pub(crate) schema_version: u32,
    pub(crate) kind: ReportKind,
    pub(crate) verdict: Verdict,
    pub(crate) findings: Vec<ReportFinding>,
}

impl ClaudeFailureReport {
    pub(crate) fn new(verdict: Verdict, finding: ReportFinding) -> Self {
        Self {
            schema_version: REPORT_SCHEMA_VERSION,
            kind: ReportKind::Summary,
            verdict,
            findings: vec![finding],
        }
    }

    pub(crate) fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClaudeInspectionReport {
    pub(crate) schema_version: u32,
    pub(crate) kind: ReportKind,
    pub(crate) verdict: Verdict,
    pub(crate) claude_version: String,
    pub(crate) plugins: PluginSummary,
    pub(crate) marketplaces: CountSummary,
    pub(crate) mcp: McpSummary,
    pub(crate) skills: SkillSummary,
    pub(crate) project: ProjectSummary,
    pub(crate) evidence_limitations: Vec<&'static str>,
    pub(crate) findings: Vec<ReportFinding>,
}

impl ClaudeInspectionReport {
    pub(crate) fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

pub(crate) struct ClaudeInspectionFacts<'a> {
    pub(crate) version: &'a str,
    pub(crate) plugins: &'a [InstalledPlugin],
    pub(crate) marketplace_count: usize,
    pub(crate) local: &'a LocalEvidence,
}

pub(crate) fn synthesize_report(facts: ClaudeInspectionFacts<'_>) -> ClaudeInspectionReport {
    let mut findings = Vec::new();
    let mut by_scope = PluginScopeSummary::default();
    let mut enabled = 0;
    let mut seen_ids: BTreeMap<&str, usize> = BTreeMap::new();
    let mut duplicate_ids = BTreeSet::new();

    for plugin in facts.plugins {
        if plugin.enabled {
            enabled += 1;
        }
        count_scope(&mut by_scope, plugin.scope.as_deref());

        *seen_ids.entry(plugin.id.as_str()).or_default() += 1;
        if seen_ids[plugin.id.as_str()] > 1 {
            duplicate_ids.insert(plugin.id.as_str());
        }

        if let Some(install_path) = plugin.install_path.as_deref() {
            match install_path.try_exists() {
                Ok(true) => {}
                Ok(false) => findings.push(ReportFinding::new(
                    "plugins.install_path_missing",
                    "missing",
                    format!(
                        "installed plugin {} records install path {} that does not exist",
                        plugin.id,
                        install_path.display()
                    ),
                )),
                Err(error) => findings.push(ReportFinding::new(
                    "plugins.install_path_unreadable",
                    "unreadable",
                    format!(
                        "installed plugin {} records install path {} that could not be examined: {error}",
                        plugin.id,
                        install_path.display()
                    ),
                )),
            }
        }
    }

    for duplicate in duplicate_ids {
        findings.push(ReportFinding::new(
            "plugins.duplicate_id",
            "duplicate",
            format!("plugin id {duplicate} is reported more than once"),
        ));
    }

    let verdict = if findings.is_empty() {
        Verdict::BaselineOk
    } else {
        Verdict::Review
    };

    ClaudeInspectionReport {
        schema_version: REPORT_SCHEMA_VERSION,
        kind: ReportKind::Summary,
        verdict,
        claude_version: facts.version.to_owned(),
        plugins: PluginSummary {
            installed: facts.plugins.len(),
            enabled,
            by_scope,
        },
        marketplaces: CountSummary {
            configured: facts.marketplace_count,
        },
        mcp: McpSummary {
            configured: facts.local.mcp.configured(),
            by_scope: McpScopeSummary {
                user: facts.local.mcp.user,
                project: facts.local.mcp.project,
                local: facts.local.mcp.local,
            },
            basis: MCP_BASIS,
        },
        skills: SkillSummary {
            personal: facts.local.skills.personal,
            project: facts.local.skills.project,
            basis: SKILL_BASIS,
        },
        project: ProjectSummary {
            claude_home: facts.local.paths.claude_home.clone(),
            current_dir: facts.local.paths.current_dir.clone(),
            root: facts.local.paths.project_root.clone(),
            root_basis: PROJECT_ROOT_BASIS,
            state_entry: if facts.local.project_state_key.is_some() {
                ProjectStateEntry::Present
            } else {
                ProjectStateEntry::Absent
            },
            state_entry_path: facts.local.project_state_key.clone(),
            instruction_sources: facts.local.instruction_sources.clone(),
        },
        evidence_limitations: EVIDENCE_LIMITATIONS.to_vec(),
        findings,
    }
}

fn count_scope(summary: &mut PluginScopeSummary, scope: Option<&str>) {
    match scope {
        Some("user") => summary.user += 1,
        Some("project") => summary.project += 1,
        Some("local") => summary.local += 1,
        _ => summary.unknown += 1,
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::local::{LocalPaths, McpEvidence, SkillEvidence};

    use super::*;

    fn plugin(
        id: &str,
        enabled: bool,
        scope: Option<&str>,
        install_path: Option<&str>,
    ) -> InstalledPlugin {
        InstalledPlugin {
            id: id.to_owned(),
            enabled,
            scope: scope.map(str::to_owned),
            install_path: install_path.map(PathBuf::from),
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
            mcp: McpEvidence {
                user: 2,
                project: 1,
                local: 0,
            },
            skills: SkillEvidence {
                personal: 3,
                project: 1,
            },
            instruction_sources: Vec::new(),
            project_state_key: Some("/work/repo".to_owned()),
        }
    }

    #[test]
    fn complete_evidence_without_findings_is_baseline_ok() {
        let existing = Path::new(env!("CARGO_MANIFEST_DIR"))
            .to_string_lossy()
            .into_owned();
        let plugins = [
            plugin("alpha@market", true, Some("user"), Some(&existing)),
            plugin("beta@market", false, Some("project"), None),
            plugin("gamma@market", true, None, None),
            plugin("delta@market", false, Some("local"), None),
            plugin("epsilon@market", true, Some("session"), None),
        ];
        let local = local_evidence();

        let report = synthesize_report(ClaudeInspectionFacts {
            version: "2.1.226 (Claude Code)",
            plugins: &plugins,
            marketplace_count: 4,
            local: &local,
        });

        assert_eq!(report.verdict, Verdict::BaselineOk);
        assert!(report.findings.is_empty());
        assert_eq!(report.plugins.installed, 5);
        assert_eq!(report.plugins.enabled, 3);
        assert_eq!(
            report.plugins.by_scope,
            PluginScopeSummary {
                user: 1,
                project: 1,
                local: 1,
                unknown: 2,
            }
        );
        assert_eq!(report.marketplaces.configured, 4);
        assert_eq!(report.mcp.configured, 3);
        assert_eq!(report.mcp.by_scope.user, 2);
        assert_eq!(report.mcp.by_scope.project, 1);
        assert_eq!(report.mcp.by_scope.local, 0);
        assert_eq!(report.skills.personal, 3);
        assert_eq!(report.skills.project, 1);
        assert_eq!(report.project.state_entry, ProjectStateEntry::Present);
        assert_eq!(
            report.project.state_entry_path.as_deref(),
            Some("/work/repo")
        );
        assert_eq!(report.project.current_dir, PathBuf::from("/work/repo"));
        assert_eq!(report.evidence_limitations.len(), 4);
    }

    #[test]
    fn a_missing_install_path_produces_one_review_finding() {
        let plugins = [plugin(
            "alpha@market",
            true,
            Some("user"),
            Some("/definitely/not/here"),
        )];
        let local = local_evidence();

        let report = synthesize_report(ClaudeInspectionFacts {
            version: "test",
            plugins: &plugins,
            marketplace_count: 0,
            local: &local,
        });

        assert_eq!(report.verdict, Verdict::Review);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].code, "plugins.install_path_missing");
        assert_eq!(report.findings[0].status, "missing");
        assert!(report.findings[0].message.contains("/definitely/not/here"));
    }

    #[test]
    fn an_install_path_that_cannot_be_examined_is_reported_as_unreadable() {
        let blocker = std::env::temp_dir().join(format!(
            "goalrail-inspect-claude-report-{}",
            std::process::id()
        ));
        std::fs::write(&blocker, "not a directory").expect("fixture file should be written");
        let unreadable = blocker.join("plugin-root");
        let plugins = [plugin(
            "alpha@market",
            true,
            Some("user"),
            Some(&unreadable.to_string_lossy()),
        )];
        let local = local_evidence();

        let report = synthesize_report(ClaudeInspectionFacts {
            version: "test",
            plugins: &plugins,
            marketplace_count: 0,
            local: &local,
        });
        let _ = std::fs::remove_file(&blocker);

        assert_eq!(report.verdict, Verdict::Review);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].code, "plugins.install_path_unreadable");
        assert_eq!(report.findings[0].status, "unreadable");
    }

    #[test]
    fn a_repeated_plugin_id_is_reported_once() {
        let plugins = [
            plugin("alpha@market", true, Some("user"), None),
            plugin("alpha@market", true, Some("user"), None),
            plugin("alpha@market", false, Some("user"), None),
        ];
        let local = local_evidence();

        let report = synthesize_report(ClaudeInspectionFacts {
            version: "test",
            plugins: &plugins,
            marketplace_count: 0,
            local: &local,
        });

        assert_eq!(report.verdict, Verdict::Review);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].code, "plugins.duplicate_id");
        assert_eq!(report.plugins.installed, 3);
        assert_eq!(report.plugins.enabled, 2);
    }

    #[test]
    fn an_empty_installation_stays_baseline_ok() {
        let local = local_evidence();

        let report = synthesize_report(ClaudeInspectionFacts {
            version: "test",
            plugins: &[],
            marketplace_count: 0,
            local: &local,
        });

        assert_eq!(report.verdict, Verdict::BaselineOk);
        assert_eq!(report.plugins.installed, 0);
        assert_eq!(report.plugins.enabled, 0);
        assert_eq!(report.plugins.by_scope, PluginScopeSummary::default());
    }

    #[test]
    fn project_state_entries_have_stable_names() {
        assert_eq!(ProjectStateEntry::Present.as_str(), "present");
        assert_eq!(ProjectStateEntry::Absent.as_str(), "absent");
    }
}
