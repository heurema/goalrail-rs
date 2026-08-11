#![no_std]

extern crate alloc;

mod assessment;
mod model;

pub use assessment::{
    Assessment, AssessmentVerdict, CleanupDisposition, CoverageStatus, RECENT_DAYS,
    SkillCleanupSummary, SkillSignal, SkillUsageItem, SkillUsageSummary, UNOBSERVED_HISTORY_DAYS,
    assess,
};
pub use model::{
    AssessmentInput, AssessmentSkillEvidence, ObservedUse, ObservedUseKey, SkillEvidenceRef,
    SkillOrigin,
};
