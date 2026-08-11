use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(super) enum SkillOrigin {
    System,
    Plugin,
    Personal,
    Project,
    Admin,
    Unknown,
}

impl SkillOrigin {
    pub(super) const fn as_str(self) -> &'static str {
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SkillEvidenceRef {
    pub(super) thread_id: String,
    pub(super) turn_id: Option<String>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub(super) struct ObservedUseKey {
    pub(super) thread_id: String,
    pub(super) turn_or_event: String,
}

#[derive(Debug, Clone)]
pub(super) struct ObservedUse {
    pub(super) key: ObservedUseKey,
    pub(super) timestamp: String,
    pub(super) epoch: i64,
    pub(super) evidence: SkillEvidenceRef,
}

pub(super) struct AssessmentSkillEvidence {
    pub(super) name: String,
    pub(super) scope: String,
    pub(super) manifest_path: PathBuf,
    pub(super) origin: SkillOrigin,
    pub(super) observed: Vec<ObservedUse>,
}

pub(super) struct AssessmentInput {
    pub(super) observed_epoch: i64,
    pub(super) history_from_epoch: Option<i64>,
    pub(super) history_through_epoch: Option<i64>,
    pub(super) rollouts_discovered: usize,
    pub(super) rollouts_excluded_current: usize,
    pub(super) rollouts_unreadable: usize,
    pub(super) discovery_errors: usize,
    pub(super) records_unreadable: usize,
    pub(super) rollouts_scanned: usize,
    pub(super) catalog_errors: usize,
    pub(super) truncated: bool,
    pub(super) skills: Vec<AssessmentSkillEvidence>,
}
