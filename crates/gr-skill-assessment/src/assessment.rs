use alloc::{collections::BTreeSet, string::String, vec::Vec};

use serde::Serialize;

use crate::{AssessmentInput, SkillEvidenceRef, SkillOrigin};

pub const RECENT_DAYS: u64 = 7;
pub const UNOBSERVED_HISTORY_DAYS: u64 = 30;
const SECONDS_PER_DAY: i64 = 86_400;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CoverageStatus {
    Complete,
    Partial,
    None,
}

impl CoverageStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillUsageSummary {
    pub total: usize,
    pub recent: usize,
    pub aging: usize,
    pub unobserved: usize,
    pub insufficient_history: usize,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillCleanupSummary {
    pub keep: usize,
    pub manual_review: usize,
    pub managed_no_manual_cleanup: usize,
    pub defer: usize,
    pub investigate_origin: usize,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillSignal {
    Recent,
    Aging,
    Unobserved,
    InsufficientHistory,
}

impl SkillSignal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Recent => "recent",
            Self::Aging => "aging",
            Self::Unobserved => "unobserved",
            Self::InsufficientHistory => "insufficient_history",
        }
    }

    pub const fn sort_rank(self) -> u8 {
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
pub enum CleanupDisposition {
    Keep,
    ManualReview,
    ManagedNoManualCleanup,
    Defer,
    InvestigateOrigin,
}

impl CleanupDisposition {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::ManualReview => "manual_review",
            Self::ManagedNoManualCleanup => "managed_no_manual_cleanup",
            Self::Defer => "defer",
            Self::InvestigateOrigin => "investigate_origin",
        }
    }

    pub const fn sort_rank(self) -> u8 {
        match self {
            Self::ManualReview => 0,
            Self::InvestigateOrigin => 1,
            Self::Defer => 2,
            Self::ManagedNoManualCleanup => 3,
            Self::Keep => 4,
        }
    }

    pub const fn is_actionable(self) -> bool {
        matches!(
            self,
            Self::ManualReview | Self::Defer | Self::InvestigateOrigin
        )
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillUsageItem {
    pub name: String,
    pub scope: String,
    pub manifest_path: String,
    pub origin: SkillOrigin,
    pub signal: SkillSignal,
    pub cleanup_disposition: CleanupDisposition,
    pub last_observed_use_at: Option<String>,
    pub age_days: Option<u64>,
    pub observed_uses_in_window: usize,
    pub last_evidence: Option<SkillEvidenceRef>,
    #[serde(skip)]
    pub last_observed_epoch: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssessmentVerdict {
    BaselineOk,
    Review,
}

pub struct Assessment {
    pub coverage_status: CoverageStatus,
    pub summary: SkillUsageSummary,
    pub cleanup: SkillCleanupSummary,
    pub items: Vec<SkillUsageItem>,
    pub verdict: AssessmentVerdict,
}

pub fn assess(mut input: AssessmentInput) -> Assessment {
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
        let mut unique = BTreeSet::new();
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
        AssessmentVerdict::BaselineOk
    } else {
        AssessmentVerdict::Review
    };

    Assessment {
        coverage_status,
        summary,
        cleanup,
        items,
        verdict,
    }
}

const fn cleanup_disposition(origin: SkillOrigin, signal: SkillSignal) -> CleanupDisposition {
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
const fn coverage_status(
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

const fn history_is_sufficient(
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

#[cfg(test)]
mod tests {
    use alloc::{string::String, vec, vec::Vec};

    use super::*;
    use crate::{AssessmentSkillEvidence, ObservedUse, ObservedUseKey};

    const OBSERVED_EPOCH: i64 = 100 * SECONDS_PER_DAY;

    fn observed(turn: &str, epoch: i64) -> ObservedUse {
        ObservedUse {
            key: ObservedUseKey {
                thread_id: String::from("thread-1"),
                turn_or_event: String::from(turn),
            },
            timestamp: String::from(turn),
            epoch,
            evidence: SkillEvidenceRef {
                thread_id: String::from("thread-1"),
                turn_id: Some(String::from(turn)),
            },
        }
    }

    fn skill(
        name: &str,
        origin: SkillOrigin,
        observed: Vec<ObservedUse>,
    ) -> AssessmentSkillEvidence {
        AssessmentSkillEvidence {
            name: String::from(name),
            scope: String::from("user"),
            manifest_path: String::from(name),
            origin,
            observed,
        }
    }

    fn complete_input(skills: Vec<AssessmentSkillEvidence>) -> AssessmentInput {
        AssessmentInput {
            observed_epoch: OBSERVED_EPOCH,
            history_from_epoch: Some(
                OBSERVED_EPOCH - UNOBSERVED_HISTORY_DAYS as i64 * SECONDS_PER_DAY,
            ),
            history_through_epoch: Some(OBSERVED_EPOCH),
            rollouts_discovered: 1,
            rollouts_excluded_current: 0,
            rollouts_unreadable: 0,
            discovery_errors: 0,
            records_unreadable: 0,
            rollouts_scanned: 1,
            catalog_errors: 0,
            truncated: false,
            skills,
        }
    }

    #[test]
    fn labels_and_sort_ranks_are_stable() {
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
        for (signal, label, rank) in signals {
            assert_eq!(signal.as_str(), label);
            assert_eq!(signal.sort_rank(), rank);
        }

        let dispositions = [
            (CleanupDisposition::ManualReview, "manual_review", 0, true),
            (
                CleanupDisposition::InvestigateOrigin,
                "investigate_origin",
                1,
                true,
            ),
            (CleanupDisposition::Defer, "defer", 2, true),
            (
                CleanupDisposition::ManagedNoManualCleanup,
                "managed_no_manual_cleanup",
                3,
                false,
            ),
            (CleanupDisposition::Keep, "keep", 4, false),
        ];
        for (disposition, label, rank, actionable) in dispositions {
            assert_eq!(disposition.as_str(), label);
            assert_eq!(disposition.sort_rank(), rank);
            assert_eq!(disposition.is_actionable(), actionable);
        }
    }

    #[test]
    fn cleanup_disposition_prioritizes_ownership() {
        let cases = [
            (
                SkillOrigin::Plugin,
                SkillSignal::Unobserved,
                CleanupDisposition::ManagedNoManualCleanup,
            ),
            (
                SkillOrigin::System,
                SkillSignal::Aging,
                CleanupDisposition::ManagedNoManualCleanup,
            ),
            (
                SkillOrigin::Admin,
                SkillSignal::Recent,
                CleanupDisposition::ManagedNoManualCleanup,
            ),
            (
                SkillOrigin::Personal,
                SkillSignal::Recent,
                CleanupDisposition::Keep,
            ),
            (
                SkillOrigin::Project,
                SkillSignal::Aging,
                CleanupDisposition::ManualReview,
            ),
            (
                SkillOrigin::Personal,
                SkillSignal::Unobserved,
                CleanupDisposition::ManualReview,
            ),
            (
                SkillOrigin::Project,
                SkillSignal::InsufficientHistory,
                CleanupDisposition::Defer,
            ),
            (
                SkillOrigin::Unknown,
                SkillSignal::Recent,
                CleanupDisposition::InvestigateOrigin,
            ),
        ];

        for (origin, signal, expected) in cases {
            assert_eq!(cleanup_disposition(origin, signal), expected);
        }
    }

    #[test]
    fn coverage_rejects_every_incomplete_evidence_reason() {
        assert_eq!(
            coverage_status(1, 0, 0, 0, 0, 1, 0, false),
            CoverageStatus::Complete
        );
        assert_eq!(
            coverage_status(0, 0, 0, 0, 0, 0, 0, false),
            CoverageStatus::None
        );

        let partial_cases = [
            (1, 0, 1, 0, 0, 1, 0, false),
            (1, 0, 0, 1, 0, 1, 0, false),
            (1, 0, 0, 0, 1, 1, 0, false),
            (1, 0, 0, 0, 0, 1, 1, false),
            (1, 0, 0, 0, 0, 1, 0, true),
            (2, 0, 0, 0, 0, 1, 0, false),
            (2, 1, 0, 0, 0, 0, 0, false),
        ];
        for case in partial_cases {
            assert_eq!(
                coverage_status(
                    case.0, case.1, case.2, case.3, case.4, case.5, case.6, case.7,
                ),
                CoverageStatus::Partial
            );
        }
    }

    #[test]
    fn history_and_age_boundaries_are_inclusive_and_saturating() {
        let oldest = OBSERVED_EPOCH - UNOBSERVED_HISTORY_DAYS as i64 * SECONDS_PER_DAY;
        let recent = OBSERVED_EPOCH - RECENT_DAYS as i64 * SECONDS_PER_DAY;
        assert!(history_is_sufficient(
            OBSERVED_EPOCH,
            Some(oldest),
            Some(recent)
        ));
        assert!(!history_is_sufficient(
            OBSERVED_EPOCH,
            Some(oldest + 1),
            Some(recent)
        ));
        assert!(!history_is_sufficient(
            OBSERVED_EPOCH,
            Some(oldest),
            Some(recent - 1)
        ));
        assert!(!history_is_sufficient(OBSERVED_EPOCH, None, Some(recent)));
        assert!(!history_is_sufficient(OBSERVED_EPOCH, Some(oldest), None));
        assert_eq!(age_days(OBSERVED_EPOCH, OBSERVED_EPOCH), 0);
        assert_eq!(age_days(OBSERVED_EPOCH, OBSERVED_EPOCH + 1), 0);
        assert_eq!(
            age_days(OBSERVED_EPOCH, OBSERVED_EPOCH - SECONDS_PER_DAY),
            1
        );
    }

    #[test]
    fn assessment_deduplicates_counts_sorts_and_reports_review() {
        let recent_epoch = OBSERVED_EPOCH - RECENT_DAYS as i64 * SECONDS_PER_DAY;
        let aging_epoch = recent_epoch - SECONDS_PER_DAY;
        let duplicate = observed("recent-turn", recent_epoch);
        let assessment = assess(complete_input(vec![
            skill(
                "recent",
                SkillOrigin::Personal,
                vec![duplicate.clone(), duplicate],
            ),
            skill(
                "aging",
                SkillOrigin::Project,
                vec![observed("aging-turn", aging_epoch)],
            ),
            skill("managed", SkillOrigin::Plugin, Vec::new()),
            skill("unknown", SkillOrigin::Unknown, Vec::new()),
        ]));

        assert_eq!(assessment.coverage_status, CoverageStatus::Complete);
        assert_eq!(assessment.verdict, AssessmentVerdict::Review);
        assert_eq!(assessment.summary.total, 4);
        assert_eq!(assessment.summary.recent, 1);
        assert_eq!(assessment.summary.aging, 1);
        assert_eq!(assessment.summary.unobserved, 2);
        assert_eq!(assessment.summary.insufficient_history, 0);
        assert_eq!(assessment.cleanup.keep, 1);
        assert_eq!(assessment.cleanup.manual_review, 1);
        assert_eq!(assessment.cleanup.managed_no_manual_cleanup, 1);
        assert_eq!(assessment.cleanup.defer, 0);
        assert_eq!(assessment.cleanup.investigate_origin, 1);
        assert_eq!(
            assessment
                .items
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            vec!["aging", "unknown", "managed", "recent"]
        );

        let recent = assessment
            .items
            .iter()
            .find(|item| item.name == "recent")
            .expect("recent item");
        assert_eq!(recent.signal, SkillSignal::Recent);
        assert_eq!(recent.cleanup_disposition, CleanupDisposition::Keep);
        assert_eq!(recent.observed_uses_in_window, 1);
        assert_eq!(recent.age_days, Some(RECENT_DAYS));
        assert_eq!(recent.last_observed_epoch, Some(recent_epoch));
        assert_eq!(recent.last_observed_use_at.as_deref(), Some("recent-turn"));
        assert_eq!(
            recent
                .last_evidence
                .as_ref()
                .and_then(|evidence| evidence.turn_id.as_deref()),
            Some("recent-turn")
        );
    }

    #[test]
    fn assessment_returns_baseline_only_without_actionable_cleanup() {
        let assessment = assess(complete_input(vec![skill(
            "managed",
            SkillOrigin::System,
            Vec::new(),
        )]));

        assert_eq!(assessment.coverage_status, CoverageStatus::Complete);
        assert_eq!(assessment.summary.unobserved, 1);
        assert_eq!(assessment.cleanup.managed_no_manual_cleanup, 1);
        assert_eq!(assessment.cleanup.manual_review, 0);
        assert_eq!(assessment.cleanup.defer, 0);
        assert_eq!(assessment.cleanup.investigate_origin, 0);
        assert_eq!(assessment.verdict, AssessmentVerdict::BaselineOk);
    }

    #[test]
    fn partial_or_short_history_never_claims_unobserved() {
        let mut partial = complete_input(vec![skill("partial", SkillOrigin::Personal, Vec::new())]);
        partial.truncated = true;
        let partial = assess(partial);
        assert_eq!(partial.coverage_status, CoverageStatus::Partial);
        assert_eq!(partial.summary.unobserved, 0);
        assert_eq!(partial.summary.insufficient_history, 1);
        assert_eq!(partial.cleanup.defer, 1);
        assert_eq!(partial.verdict, AssessmentVerdict::Review);

        let mut short = complete_input(vec![skill("short", SkillOrigin::Project, Vec::new())]);
        short.history_from_epoch = Some(OBSERVED_EPOCH - SECONDS_PER_DAY);
        let short = assess(short);
        assert_eq!(short.coverage_status, CoverageStatus::Complete);
        assert_eq!(short.summary.unobserved, 0);
        assert_eq!(short.summary.insufficient_history, 1);
        assert_eq!(short.cleanup.defer, 1);
        assert_eq!(short.verdict, AssessmentVerdict::Review);
    }
}
