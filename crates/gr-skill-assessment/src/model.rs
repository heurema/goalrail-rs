use alloc::{string::String, vec::Vec};

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SkillOrigin {
    System,
    Plugin,
    Personal,
    Project,
    Admin,
    Unknown,
}

impl SkillOrigin {
    pub const fn as_str(self) -> &'static str {
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
pub struct SkillEvidenceRef {
    pub thread_id: String,
    pub turn_id: Option<String>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObservedUseKey {
    pub thread_id: String,
    pub turn_or_event: String,
}

#[derive(Debug, Clone)]
pub struct ObservedUse {
    pub key: ObservedUseKey,
    pub timestamp: String,
    pub epoch: i64,
    pub evidence: SkillEvidenceRef,
}

pub struct AssessmentSkillEvidence {
    pub name: String,
    pub scope: String,
    pub manifest_path: String,
    pub origin: SkillOrigin,
    pub observed: Vec<ObservedUse>,
}

pub struct AssessmentInput {
    pub observed_epoch: i64,
    pub history_from_epoch: Option<i64>,
    pub history_through_epoch: Option<i64>,
    pub rollouts_discovered: usize,
    pub rollouts_excluded_current: usize,
    pub rollouts_unreadable: usize,
    pub discovery_errors: usize,
    pub records_unreadable: usize,
    pub rollouts_scanned: usize,
    pub catalog_errors: usize,
    pub truncated: bool,
    pub skills: Vec<AssessmentSkillEvidence>,
}
