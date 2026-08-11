use std::collections::HashSet;

use serde::Serialize;

use crate::Verdict;

use super::model::{AssessmentInput, SkillEvidenceRef, SkillOrigin};

pub(super) const RECENT_DAYS: u64 = 7;
pub(super) const UNOBSERVED_HISTORY_DAYS: u64 = 30;
const SECONDS_PER_DAY: i64 = 86_400;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(super) enum CoverageStatus {
    Complete,
    Partial,
    None,
}

impl CoverageStatus {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SkillUsageSummary {
    pub(super) total: usize,
    pub(super) recent: usize,
    pub(super) aging: usize,
    pub(super) unobserved: usize,
    pub(super) insufficient_history: usize,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SkillCleanupSummary {
    pub(super) keep: usize,
    pub(super) manual_review: usize,
    pub(super) managed_no_manual_cleanup: usize,
    pub(super) defer: usize,
    pub(super) investigate_origin: usize,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum SkillSignal {
    Recent,
    Aging,
    Unobserved,
    InsufficientHistory,
}

impl SkillSignal {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Recent => "recent",
            Self::Aging => "aging",
            Self::Unobserved => "unobserved",
            Self::InsufficientHistory => "insufficient_history",
        }
    }

    pub(super) const fn sort_rank(self) -> u8 {
        match self {
            Self::Unobserved => 0,
            Self::Aging => 1,
            Self::Recent => 2,
            Self::InsufficientHistory => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum CleanupDisposition {
    Keep,
    ManualReview,
    ManagedNoManualCleanup,
    Defer,
    InvestigateOrigin,
}

impl CleanupDisposition {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::ManualReview => "manual_review",
            Self::ManagedNoManualCleanup => "managed_no_manual_cleanup",
            Self::Defer => "defer",
            Self::InvestigateOrigin => "investigate_origin",
        }
    }

    pub(super) const fn sort_rank(self) -> u8 {
        match self {
            Self::ManualReview => 0,
            Self::InvestigateOrigin => 1,
            Self::Defer => 2,
            Self::ManagedNoManualCleanup => 3,
            Self::Keep => 4,
        }
    }

    pub(super) const fn is_actionable(self) -> bool {
        matches!(
            self,
            Self::ManualReview | Self::Defer | Self::InvestigateOrigin
        )
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SkillUsageItem {
    pub(super) name: String,
    pub(super) scope: String,
    pub(super) manifest_path: std::path::PathBuf,
    pub(super) origin: SkillOrigin,
    pub(super) signal: SkillSignal,
    pub(super) cleanup_disposition: CleanupDisposition,
    pub(super) last_observed_use_at: Option<String>,
    pub(super) age_days: Option<u64>,
    pub(super) observed_uses_in_window: usize,
    pub(super) last_evidence: Option<SkillEvidenceRef>,
    #[serde(skip)]
    pub(super) last_observed_epoch: Option<i64>,
}

pub(super) struct Assessment {
    pub(super) coverage_status: CoverageStatus,
    pub(super) summary: SkillUsageSummary,
    pub(super) cleanup: SkillCleanupSummary,
    pub(super) items: Vec<SkillUsageItem>,
    pub(super) verdict: Verdict,
}

pub(super) fn assess(mut input: AssessmentInput) -> Assessment {
    let coverage_status = coverage_status(
        input.rollouts_discovered,
        input.rollouts_excluded_current,
        input.rollouts_unreadable,
        input.discovery_errors,
        input.records_unreadable,
        input.rollouts_scanned,
        input.catalog_errors,
        input.truncated,
    );
    let history_sufficient = coverage_status == CoverageStatus::Complete
        && history_is_sufficient(
            input.observed_epoch,
            input.history_from_epoch,
            input.history_through_epoch,
        );
    let mut summary = SkillUsageSummary::default();
    let mut cleanup = SkillCleanupSummary::default();
    let mut items = Vec::with_capacity(input.skills.len());

    for mut skill in input.skills.drain(..) {
        let mut unique = HashSet::new();
        skill
            .observed
            .retain(|observed| unique.insert(observed.key.clone()));
        let last = skill.observed.iter().max_by_key(|observed| observed.epoch);
        let last_observed_use_at = last.map(|observed| observed.timestamp.clone());
        let last_evidence = last.map(|observed| observed.evidence.clone());
        let last_observed_epoch = last.map(|observed| observed.epoch);
        let age_days = last.map(|observed| age_days(input.observed_epoch, observed.epoch));
        let signal = match age_days {
            Some(days) if days <= RECENT_DAYS => SkillSignal::Recent,
            Some(_) => SkillSignal::Aging,
            None if history_sufficient => SkillSignal::Unobserved,
            None => SkillSignal::InsufficientHistory,
        };
        let cleanup_disposition = cleanup_disposition(skill.origin, signal);

        summary.total += 1;
        match signal {
            SkillSignal::Recent => summary.recent += 1,
            SkillSignal::Aging => summary.aging += 1,
            SkillSignal::Unobserved => summary.unobserved += 1,
            SkillSignal::InsufficientHistory => summary.insufficient_history += 1,
        }
        match cleanup_disposition {
            CleanupDisposition::Keep => cleanup.keep += 1,
            CleanupDisposition::ManualReview => cleanup.manual_review += 1,
            CleanupDisposition::ManagedNoManualCleanup => cleanup.managed_no_manual_cleanup += 1,
            CleanupDisposition::Defer => cleanup.defer += 1,
            CleanupDisposition::InvestigateOrigin => cleanup.investigate_origin += 1,
        }

        items.push(SkillUsageItem {
            name: skill.name,
            scope: skill.scope,
            manifest_path: skill.manifest_path,
            origin: skill.origin,
            signal,
            cleanup_disposition,
            last_observed_use_at,
            age_days,
            observed_uses_in_window: skill.observed.len(),
            last_evidence,
            last_observed_epoch,
        });
    }

    items.sort_by(|left, right| {
        left.cleanup_disposition
            .sort_rank()
            .cmp(&right.cleanup_disposition.sort_rank())
            .then_with(|| left.signal.sort_rank().cmp(&right.signal.sort_rank()))
            .then_with(|| left.last_observed_epoch.cmp(&right.last_observed_epoch))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.manifest_path.cmp(&right.manifest_path))
    });

    let verdict = if coverage_status == CoverageStatus::Complete
        && cleanup.manual_review == 0
        && cleanup.defer == 0
        && cleanup.investigate_origin == 0
    {
        Verdict::BaselineOk
    } else {
        Verdict::Review
    };

    Assessment {
        coverage_status,
        summary,
        cleanup,
        items,
        verdict,
    }
}

pub(super) const fn cleanup_disposition(
    origin: SkillOrigin,
    signal: SkillSignal,
) -> CleanupDisposition {
    match origin {
        SkillOrigin::Plugin | SkillOrigin::System | SkillOrigin::Admin => {
            CleanupDisposition::ManagedNoManualCleanup
        }
        SkillOrigin::Unknown => CleanupDisposition::InvestigateOrigin,
        SkillOrigin::Personal | SkillOrigin::Project => match signal {
            SkillSignal::Recent => CleanupDisposition::Keep,
            SkillSignal::Aging | SkillSignal::Unobserved => CleanupDisposition::ManualReview,
            SkillSignal::InsufficientHistory => CleanupDisposition::Defer,
        },
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) const fn coverage_status(
    rollouts_discovered: usize,
    rollouts_excluded_current: usize,
    rollouts_unreadable: usize,
    discovery_errors: usize,
    records_unreadable: usize,
    rollouts_scanned: usize,
    catalog_errors: usize,
    truncated: bool,
) -> CoverageStatus {
    let candidate_rollouts = rollouts_discovered.saturating_sub(rollouts_excluded_current);
    let partial = rollouts_unreadable > 0
        || discovery_errors > 0
        || records_unreadable > 0
        || truncated
        || catalog_errors > 0
        || rollouts_scanned != candidate_rollouts;

    if partial {
        CoverageStatus::Partial
    } else if candidate_rollouts == 0 {
        CoverageStatus::None
    } else {
        CoverageStatus::Complete
    }
}

pub(super) const fn history_is_sufficient(
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

pub(super) fn age_days(observed_epoch: i64, evidence_epoch: i64) -> u64 {
    observed_epoch
        .saturating_sub(evidence_epoch)
        .max(0)
        .div_euclid(SECONDS_PER_DAY) as u64
}
