//! Deterministic, advisory qualification of diagnostic hypotheses (M0.15.16).
//!
//! The only rule in v1 qualifies the `RepeatedTimeout` proposals generated in
//! M0.15.15. It checks public observation records, not runtime internals. A
//! supported timeout *pattern* can strengthen a proposal, but this rule never
//! confirms which competing causal explanation is correct. Results are pure
//! recomputations and are not persisted or used to control the runtime.
//!
//! The assessment covers only supplied IDs. Non-overlapping windows do not
//! establish statistical independence, target identity or causal truth. Reads
//! resolve references at qualification time; retention can remove them later.
//! Qualification is pattern-global: DiagnosticHypothesis.subject is a
//! DiagnosticPattern, not a runtime/model identity. Target-specific diagnosis
//! is outside this contract. Reference vectors preserve input ordering.
#![forbid(unsafe_code)]

use maia_local_intelligence_diagnostics::DiagnosticPattern;
use maia_local_intelligence_hypothesis_ledger::{
    DiagnosticHypothesis, EvidenceState, HypothesisConfidence, Ledger, LedgerError, ObservationId,
    pattern_state,
};
use std::{
    collections::BTreeSet,
    time::{Duration, SystemTime},
};

pub const MAX_QUALIFICATION_REFERENCES: usize = 128;
pub const QUALIFICATION_RULE: &str = "repeated-timeout-qualification-v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualificationState {
    Hypothesis,
    SupportedHypothesis,
    WeakenedHypothesis,
    /// Reserved for a future rule with a genuine causal falsifier. This
    /// timeout pattern cannot disprove either competing historical cause.
    DisprovenHypothesis,
    InsufficientEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualificationReason {
    NonOverlappingRepeatedSupport,
    LimitedOrOverlappingSupport,
    ContradictoryObservation,
    NoSupportingObservation,
    EvidenceGap,
    StaleEvidence,
}

impl QualificationReason {
    pub fn explanation(self) -> &'static str {
        match self {
            Self::NonOverlappingRepeatedSupport => {
                "At least two fresh, fully covered, non-overlapping observations support the timeout pattern; the causal explanation remains unverified."
            }
            Self::LimitedOrOverlappingSupport => {
                "Fresh support exists, but repeated pattern evidence from non-overlapping windows is not established; independence is not guaranteed."
            }
            Self::ContradictoryObservation => {
                "A fresh fully covered observation does not support the timeout pattern; the proposed explanation is weakened."
            }
            Self::NoSupportingObservation => {
                "No fresh, fully covered observation supports the timeout pattern."
            }
            Self::EvidenceGap => {
                "A referenced observation is missing, indeterminate, duplicated, has invalid or incomplete evidence boundaries, or is inconsistent with its claimed role."
            }
            Self::StaleEvidence => {
                "Referenced evidence is older than the explicit freshness limit or lies in the future."
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceGapReason {
    MissingReference,
    IndeterminateObservation,
    StaleObservation,
    FutureObservation,
    DuplicateReference,
    ClaimMismatch,
    InvalidWindow,
    MissingEvidenceBoundaries,
    InvalidEvidenceBoundaries,
    IncompleteEvidenceCoverage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceGap {
    pub observation_id: ObservationId,
    pub reason: EvidenceGapReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FalsificationCondition {
    /// A fresh, fully covered observation without the repeated-timeout pattern
    /// weakens pattern support. It does not disprove a historical cause.
    FullyCoveredWindowWithoutRepeatedTimeout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextDiscriminatingObservation {
    /// A future read-only measurement, not an instruction to run a probe.
    SeparateTransportAndRuntimeLatencyForSameRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceWindow {
    pub since: SystemTime,
    pub until: SystemTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QualifiedDiagnosticHypothesis {
    /// The source proposal, including its caller-supplied fields and IDs.
    /// Qualification never rewrites or promotes its `Proposed` status.
    pub hypothesis: DiagnosticHypothesis,
    pub state: QualificationState,
    /// Deterministic pattern-evidence confidence, capped at Moderate because
    /// this rule cannot distinguish the two possible timeout causes.
    pub confidence: HypothesisConfidence,
    pub reason: QualificationReason,
    pub verified_supporting_references: Vec<ObservationId>,
    pub verified_contradicting_references: Vec<ObservationId>,
    pub evidence_gaps: Vec<EvidenceGap>,
    /// Envelope of all resolved referenced windows, including stale evidence.
    /// This is not a claim of continuous coverage between those windows.
    pub source_window: Option<EvidenceWindow>,
    pub qualified_at: SystemTime,
    pub rule: &'static str,
    pub policy: QualificationPolicy,
    pub evidence_is_stale: bool,
    pub falsification_condition: FalsificationCondition,
    pub next_discriminating_observation: NextDiscriminatingObservation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualificationError {
    UnsupportedSubject,
    InvalidInput,
    TooManyReferences,
    Ledger(LedgerError),
}

impl From<LedgerError> for QualificationError {
    fn from(value: LedgerError) -> Self {
        Self::Ledger(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualificationPolicy {
    /// Evidence older than this at `qualified_at` is explicitly stale.
    pub max_evidence_age: Duration,
}

impl Default for QualificationPolicy {
    fn default() -> Self {
        Self {
            max_evidence_age: Duration::from_secs(24 * 60 * 60),
        }
    }
}

/// Qualify one proposed repeated-timeout hypothesis using its references and
/// explicit additional persisted observation IDs (for later evidence).
/// Additional evidence is classified by `pattern_state`, never by the caller.
/// `qualified_at` and policy are explicit inputs: the
/// same database state and inputs give the same result. No hypothesis or
/// observation is written. Missing/partial/stale inputs fail closed into an
/// inspectable `InsufficientEvidence` result.
pub fn qualify(
    ledger: &Ledger,
    hypothesis: &DiagnosticHypothesis,
    additional_evidence: &[ObservationId],
    qualified_at: SystemTime,
    policy: QualificationPolicy,
) -> Result<QualifiedDiagnosticHypothesis, QualificationError> {
    if hypothesis.subject != DiagnosticPattern::RepeatedTimeout {
        return Err(QualificationError::UnsupportedSubject);
    }
    if policy.max_evidence_age.is_zero() || hypothesis.generated_at > qualified_at {
        return Err(QualificationError::InvalidInput);
    }
    let total = hypothesis
        .supporting_evidence
        .len()
        .checked_add(hypothesis.contradicting_evidence.len())
        .and_then(|count| count.checked_add(additional_evidence.len()))
        .ok_or(QualificationError::TooManyReferences)?;
    if total > MAX_QUALIFICATION_REFERENCES {
        return Err(QualificationError::TooManyReferences);
    }

    let mut seen = BTreeSet::new();
    let mut support = Vec::new();
    let mut counter = Vec::new();
    let mut support_windows = Vec::new();
    let mut gaps = Vec::new();
    let mut window: Option<EvidenceWindow> = None;
    let mut stale = false;
    for (claimed_support, ids) in [
        (Some(true), hypothesis.supporting_evidence.as_slice()),
        (Some(false), hypothesis.contradicting_evidence.as_slice()),
        (None, additional_evidence),
    ] {
        for id in ids {
            if !seen.insert(id.as_str()) {
                gaps.push(EvidenceGap {
                    observation_id: id.clone(),
                    reason: EvidenceGapReason::DuplicateReference,
                });
                continue;
            }
            let Some(observation) = ledger.get(id)? else {
                gaps.push(EvidenceGap {
                    observation_id: id.clone(),
                    reason: EvidenceGapReason::MissingReference,
                });
                continue;
            };
            extend_window(&mut window, &observation);
            let coverage = &observation.snapshot.coverage;
            if coverage.requested_until <= coverage.requested_since {
                gaps.push(EvidenceGap {
                    observation_id: id.clone(),
                    reason: EvidenceGapReason::InvalidWindow,
                });
                continue;
            }
            if observation.observed_at > qualified_at || coverage.requested_until > qualified_at {
                stale = true;
                gaps.push(EvidenceGap {
                    observation_id: id.clone(),
                    reason: EvidenceGapReason::FutureObservation,
                });
                continue;
            }
            let Some((evidence_start, evidence_end)) = coverage.status_evidence_span_in_window
            else {
                gaps.push(EvidenceGap {
                    observation_id: id.clone(),
                    reason: EvidenceGapReason::MissingEvidenceBoundaries,
                });
                continue;
            };
            // These are actual samples from the bounded in-window query.
            // Reject inconsistent DTO boundaries rather than using timestamps
            // from another window or the store-wide latest state.
            if evidence_start > evidence_end
                || evidence_start < coverage.requested_since
                || evidence_end > coverage.requested_until
            {
                gaps.push(EvidenceGap {
                    observation_id: id.clone(),
                    reason: EvidenceGapReason::InvalidEvidenceBoundaries,
                });
                continue;
            }
            let complete = evidence_start <= coverage.requested_since
                && evidence_end >= coverage.requested_until;
            if !complete {
                gaps.push(EvidenceGap {
                    observation_id: id.clone(),
                    reason: EvidenceGapReason::IncompleteEvidenceCoverage,
                });
            }
            // For an eligible window, an actual status sample reaches its
            // endpoint. Thus no other in-window evidence can be newer. Neither
            // recording time nor a caller's requested endpoint refreshes data.
            let age = qualified_at
                .duration_since(evidence_end)
                .expect("future evidence checked");
            if age > policy.max_evidence_age {
                stale = true;
                gaps.push(EvidenceGap {
                    observation_id: id.clone(),
                    reason: EvidenceGapReason::StaleObservation,
                });
                continue;
            }
            if !complete {
                continue;
            }
            match pattern_state(&observation, hypothesis.subject) {
                EvidenceState::Indeterminate => {
                    gaps.push(EvidenceGap {
                        observation_id: id.clone(),
                        reason: EvidenceGapReason::IndeterminateObservation,
                    });
                }
                state
                    if claimed_support
                        .is_some_and(|claimed| (state == EvidenceState::Supported) != claimed) =>
                {
                    gaps.push(EvidenceGap {
                        observation_id: id.clone(),
                        reason: EvidenceGapReason::ClaimMismatch,
                    });
                }
                EvidenceState::Supported => {
                    support.push(id.clone());
                    support_windows.push((
                        observation.snapshot.coverage.requested_since,
                        observation.snapshot.coverage.requested_until,
                    ));
                }
                EvidenceState::NotSupported => {
                    counter.push(id.clone());
                }
            }
        }
    }

    let (state, confidence, reason) = if stale {
        (
            QualificationState::InsufficientEvidence,
            HypothesisConfidence::Low,
            QualificationReason::StaleEvidence,
        )
    } else if !gaps.is_empty() {
        (
            QualificationState::InsufficientEvidence,
            HypothesisConfidence::Low,
            QualificationReason::EvidenceGap,
        )
    } else if support.is_empty() {
        (
            QualificationState::InsufficientEvidence,
            HypothesisConfidence::Low,
            QualificationReason::NoSupportingObservation,
        )
    } else if !counter.is_empty() {
        (
            QualificationState::WeakenedHypothesis,
            HypothesisConfidence::Low,
            QualificationReason::ContradictoryObservation,
        )
    } else if disjoint_support_count(&mut support_windows) >= 2 {
        (
            QualificationState::SupportedHypothesis,
            HypothesisConfidence::Moderate,
            QualificationReason::NonOverlappingRepeatedSupport,
        )
    } else {
        (
            QualificationState::Hypothesis,
            HypothesisConfidence::Low,
            QualificationReason::LimitedOrOverlappingSupport,
        )
    };
    Ok(QualifiedDiagnosticHypothesis {
        hypothesis: hypothesis.clone(),
        state,
        confidence,
        reason,
        verified_supporting_references: support,
        verified_contradicting_references: counter,
        evidence_gaps: gaps,
        source_window: window,
        qualified_at,
        rule: QUALIFICATION_RULE,
        policy,
        evidence_is_stale: stale,
        falsification_condition: FalsificationCondition::FullyCoveredWindowWithoutRepeatedTimeout,
        next_discriminating_observation:
            NextDiscriminatingObservation::SeparateTransportAndRuntimeLatencyForSameRequest,
    })
}

fn extend_window(
    window: &mut Option<EvidenceWindow>,
    observation: &maia_local_intelligence_hypothesis_ledger::DiagnosticObservation,
) {
    let since = observation.snapshot.coverage.requested_since;
    let until = observation.snapshot.coverage.requested_until;
    *window = Some(match *window {
        Some(current) => EvidenceWindow {
            since: current.since.min(since),
            until: current.until.max(until),
        },
        None => EvidenceWindow { since, until },
    });
}

fn disjoint_support_count(windows: &mut [(SystemTime, SystemTime)]) -> usize {
    windows.sort_by_key(|(since, until)| (*until, *since));
    let mut last_until = None;
    let mut count = 0;
    for &(since, until) in windows.iter() {
        // Diagnostic windows are inclusive: a shared endpoint may share data.
        if last_until.is_none_or(|last| since > last) {
            count += 1;
            last_until = Some(until);
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use maia_local_intelligence_diagnostics::DiagnosticSnapshot;
    use maia_local_intelligence_health::{
        HealthStore, OperationClass, RetentionPolicy as HealthRetentionPolicy,
    };
    use maia_local_intelligence_hypothesis_ledger::{
        DiagnosticObservation, HypothesisId, HypothesisStatus, RecordOutcome, RetentionPolicy,
    };
    use maia_local_model::{LocalIntelligenceFailure, LocalIntelligenceStatus};
    use std::time::UNIX_EPOCH;

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn snapshot(start: u64, with_timeouts: bool) -> DiagnosticSnapshot {
        snapshot_with_status_span(start, start + 20, start + 20, with_timeouts)
    }

    fn snapshot_with_status_span(
        start: u64,
        last_status: u64,
        until: u64,
        with_timeouts: bool,
    ) -> DiagnosticSnapshot {
        let health = HealthStore::open_in_memory(HealthRetentionPolicy::default()).unwrap();
        let status = LocalIntelligenceStatus {
            available: true,
            model: "synthetic".into(),
        };
        for second in start..=last_status {
            health
                .record_status_sample(&status, None, at(second))
                .unwrap();
        }
        if with_timeouts {
            for i in 10..13 {
                health
                    .record_inference_failure(
                        "synthetic",
                        OperationClass::CompleteDetailed,
                        LocalIntelligenceFailure::Timeout,
                        None,
                        at(start + i),
                    )
                    .unwrap();
            }
        }
        DiagnosticSnapshot::build(&health, at(start), at(until), 100).unwrap()
    }

    fn record(ledger: &Ledger, id: &str, start: u64, with_timeouts: bool) -> ObservationId {
        let observation_id = ObservationId::new(id).unwrap();
        assert_eq!(
            ledger.record(&DiagnosticObservation {
                observation_id: observation_id.clone(),
                observed_at: at(start + 20),
                snapshot: snapshot(start, with_timeouts),
            }),
            Ok(RecordOutcome::Recorded)
        );
        observation_id
    }

    fn hypothesis(
        support: Vec<ObservationId>,
        counter: Vec<ObservationId>,
    ) -> DiagnosticHypothesis {
        DiagnosticHypothesis {
            hypothesis_id: HypothesisId::new("timeout-candidate").unwrap(),
            subject: DiagnosticPattern::RepeatedTimeout,
            explanation: "Transport delay may explain repeated timeouts".into(),
            supporting_evidence: support,
            contradicting_evidence: counter,
            confidence: HypothesisConfidence::Low,
            uncertainty: "Transport and runtime latency are not yet separated".into(),
            evidence_sufficient: false,
            generated_at: at(1),
            generator: "deterministic-repeated-timeout-v1".into(),
            status: HypothesisStatus::Proposed,
        }
    }

    fn ledger() -> Ledger {
        Ledger::open_in_memory(RetentionPolicy::default()).unwrap()
    }
    fn policy() -> QualificationPolicy {
        QualificationPolicy {
            max_evidence_age: Duration::from_secs(100),
        }
    }

    #[test]
    fn nonoverlapping_repeated_support_strengthens_but_cannot_confirm_a_cause() {
        let ledger = ledger();
        let first = record(&ledger, "first", 0, true);
        let second = record(&ledger, "second", 21, true);
        let candidate = hypothesis(vec![first.clone(), second.clone()], vec![]);
        let before = ledger.coverage().unwrap();
        let result = qualify(&ledger, &candidate, &[], at(50), policy()).unwrap();
        assert_eq!(result.state, QualificationState::SupportedHypothesis);
        assert_eq!(result.confidence, HypothesisConfidence::Moderate);
        assert_eq!(
            result.reason,
            QualificationReason::NonOverlappingRepeatedSupport
        );
        assert_eq!(result.verified_supporting_references, vec![first, second]);
        assert!(result.evidence_gaps.is_empty());
        assert_eq!(
            result.source_window,
            Some(EvidenceWindow {
                since: at(0),
                until: at(41)
            })
        );
        assert_eq!(result.hypothesis.status, HypothesisStatus::Proposed);
        assert_eq!(
            result.next_discriminating_observation,
            NextDiscriminatingObservation::SeparateTransportAndRuntimeLatencyForSameRequest
        );
        assert_eq!(
            ledger.coverage().unwrap(),
            before,
            "qualification is read-only"
        );
        assert!(ledger.latest_hypotheses(10).unwrap().is_empty());
    }

    #[test]
    fn overlapping_support_does_not_masquerade_as_independent_repetition() {
        let ledger = ledger();
        let first = record(&ledger, "first", 0, true);
        let second = record(&ledger, "overlap", 10, true);
        let result = qualify(
            &ledger,
            &hypothesis(vec![first, second], vec![]),
            &[],
            at(50),
            policy(),
        )
        .unwrap();
        assert_eq!(result.state, QualificationState::Hypothesis);
        assert_eq!(result.confidence, HypothesisConfidence::Low);
    }

    #[test]
    fn later_recovery_observation_weakens_but_does_not_disprove_historical_cause() {
        let ledger = ledger();
        let support = record(&ledger, "support", 0, true);
        let counter = record(&ledger, "recovery", 40, false);
        let result = qualify(
            &ledger,
            &hypothesis(vec![support.clone()], vec![counter.clone()]),
            &[],
            at(70),
            policy(),
        )
        .unwrap();
        assert_eq!(result.state, QualificationState::WeakenedHypothesis);
        assert_eq!(result.confidence, HypothesisConfidence::Low);
        assert_eq!(result.verified_supporting_references, vec![support]);
        assert_eq!(result.verified_contradicting_references, vec![counter]);
        assert_eq!(result.reason, QualificationReason::ContradictoryObservation);
        assert_ne!(result.state, QualificationState::DisprovenHypothesis);
    }

    #[test]
    fn absent_or_missing_evidence_is_explicitly_insufficient() {
        let ledger = ledger();
        let empty = qualify(&ledger, &hypothesis(vec![], vec![]), &[], at(50), policy()).unwrap();
        assert_eq!(empty.state, QualificationState::InsufficientEvidence);
        assert_eq!(empty.reason, QualificationReason::NoSupportingObservation);
        let missing = ObservationId::new("missing").unwrap();
        let result = qualify(
            &ledger,
            &hypothesis(vec![missing.clone()], vec![]),
            &[],
            at(50),
            policy(),
        )
        .unwrap();
        assert_eq!(result.state, QualificationState::InsufficientEvidence);
        assert_eq!(
            result.evidence_gaps,
            vec![EvidenceGap {
                observation_id: missing,
                reason: EvidenceGapReason::MissingReference
            }]
        );
        assert!(result.verified_supporting_references.is_empty());
    }

    #[test]
    fn stale_evidence_is_visible_and_cannot_be_supported() {
        let ledger = ledger();
        let old = record(&ledger, "old", 0, true);
        let result = qualify(
            &ledger,
            &hypothesis(vec![old.clone()], vec![]),
            &[],
            at(200),
            policy(),
        )
        .unwrap();
        assert_eq!(result.state, QualificationState::InsufficientEvidence);
        assert_eq!(result.reason, QualificationReason::StaleEvidence);
        assert!(result.evidence_is_stale);
        assert_eq!(
            result.evidence_gaps,
            vec![EvidenceGap {
                observation_id: old,
                reason: EvidenceGapReason::StaleObservation
            }]
        );
    }

    #[test]
    fn identical_inputs_produce_identical_qualification_and_bad_policy_fails_closed() {
        let ledger = ledger();
        let support = record(&ledger, "one", 0, true);
        let candidate = hypothesis(vec![support], vec![]);
        let first = qualify(&ledger, &candidate, &[], at(50), policy()).unwrap();
        let second = qualify(&ledger, &candidate, &[], at(50), policy()).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            qualify(
                &ledger,
                &candidate,
                &[],
                at(50),
                QualificationPolicy {
                    max_evidence_age: Duration::ZERO
                }
            )
            .err(),
            Some(QualificationError::InvalidInput)
        );
    }

    #[test]
    fn generated_competing_proposals_accept_later_evidence_without_mutating_the_ledger() {
        use maia_local_intelligence_hypothesis_generator::{
            GenerationOutcome, propose_repeated_timeout,
        };
        let ledger = ledger();
        record(&ledger, "original", 0, true);
        let GenerationOutcome::Proposed(candidates) =
            propose_repeated_timeout(&ledger, at(0), at(21), 10, at(21)).unwrap()
        else {
            panic!("expected M0.15.15 proposals")
        };
        assert_eq!(candidates.len(), 2);
        for candidate in &candidates {
            ledger.record_hypothesis(candidate).unwrap();
        }
        let later = record(&ledger, "later-support", 21, true);
        for candidate in &candidates {
            let initial = qualify(&ledger, candidate, &[], at(50), policy()).unwrap();
            assert_eq!(initial.state, QualificationState::Hypothesis);
            let supported = qualify(
                &ledger,
                candidate,
                std::slice::from_ref(&later),
                at(50),
                policy(),
            )
            .unwrap();
            assert_eq!(supported.state, QualificationState::SupportedHypothesis);
            assert_eq!(supported.hypothesis, *candidate);
            assert_eq!(supported.rule, QUALIFICATION_RULE);
            assert_eq!(supported.policy, policy());
        }
        let recovery = record(&ledger, "later-recovery", 42, false);
        for candidate in &candidates {
            let result = qualify(
                &ledger,
                candidate,
                &[later.clone(), recovery.clone()],
                at(70),
                policy(),
            )
            .unwrap();
            assert_eq!(result.state, QualificationState::WeakenedHypothesis);
            assert!(result.verified_contradicting_references.contains(&recovery));
            assert_eq!(
                ledger
                    .get_hypothesis(&candidate.hypothesis_id)
                    .unwrap()
                    .as_ref(),
                Some(candidate)
            );
        }
        assert_eq!(ledger.latest_hypotheses(10).unwrap().len(), 2);
    }

    #[test]
    fn recently_recorded_old_snapshot_is_stale_and_preserves_its_window() {
        let ledger = ledger();
        let id = ObservationId::new("replayed-snapshot").unwrap();
        ledger
            .record(&DiagnosticObservation {
                observation_id: id.clone(),
                observed_at: at(195),
                snapshot: snapshot(0, true),
            })
            .unwrap();
        let result = qualify(
            &ledger,
            &hypothesis(vec![id], vec![]),
            &[],
            at(200),
            policy(),
        )
        .unwrap();
        assert_eq!(result.state, QualificationState::InsufficientEvidence);
        assert!(result.evidence_is_stale);
        assert_eq!(
            result.source_window,
            Some(EvidenceWindow {
                since: at(0),
                until: at(20)
            })
        );
    }

    #[test]
    fn inclusive_windows_sharing_an_endpoint_do_not_strengthen_support() {
        let ledger = ledger();
        let first = record(&ledger, "first", 0, true);
        let second = record(&ledger, "touching", 20, true);
        let result = qualify(
            &ledger,
            &hypothesis(vec![first, second], vec![]),
            &[],
            at(50),
            policy(),
        )
        .unwrap();
        assert_eq!(result.state, QualificationState::Hypothesis);
    }

    #[test]
    fn duplicate_and_mislabelled_references_cannot_inflate_confidence() {
        let ledger = ledger();
        let support = record(&ledger, "support", 0, true);
        let candidate = hypothesis(vec![support.clone()], vec![]);
        let result = qualify(
            &ledger,
            &candidate,
            std::slice::from_ref(&support),
            at(50),
            policy(),
        )
        .unwrap();
        assert_eq!(result.state, QualificationState::InsufficientEvidence);
        assert_eq!(result.verified_supporting_references, vec![support.clone()]);
        assert_eq!(
            result.evidence_gaps[0].reason,
            EvidenceGapReason::DuplicateReference
        );
        let result = qualify(
            &ledger,
            &hypothesis(vec![], vec![support]),
            &[],
            at(50),
            policy(),
        )
        .unwrap();
        assert_eq!(result.state, QualificationState::InsufficientEvidence);
        assert_eq!(
            result.evidence_gaps[0].reason,
            EvidenceGapReason::ClaimMismatch
        );
    }

    #[test]
    fn absent_partial_and_future_evidence_fail_closed_even_with_valid_support() {
        let ledger = ledger();
        let support = record(&ledger, "support", 0, true);
        let empty_health = HealthStore::open_in_memory(HealthRetentionPolicy::default()).unwrap();
        let absent = DiagnosticSnapshot::build(&empty_health, at(21), at(40), 100).unwrap();
        let mut partial = snapshot(21, true);
        partial.coverage.status_history_covers_requested_window = false;
        let future = snapshot(60, true);
        for (label, snapshot, reason) in [
            (
                "absent",
                absent,
                EvidenceGapReason::MissingEvidenceBoundaries,
            ),
            (
                "partial",
                partial,
                EvidenceGapReason::IndeterminateObservation,
            ),
            (
                "future-window",
                future,
                EvidenceGapReason::FutureObservation,
            ),
        ] {
            let id = ObservationId::new(label).unwrap();
            ledger
                .record(&DiagnosticObservation {
                    observation_id: id.clone(),
                    observed_at: at(40),
                    snapshot,
                })
                .unwrap();
            let result = qualify(
                &ledger,
                &hypothesis(vec![support.clone()], vec![]),
                &[id],
                at(50),
                policy(),
            )
            .unwrap();
            assert_eq!(result.state, QualificationState::InsufficientEvidence);
            assert_eq!(result.evidence_gaps[0].reason, reason);
        }
    }

    #[test]
    fn bounded_input_rejects_excess_references_before_reading_evidence() {
        let ledger = ledger();
        let ids = vec![ObservationId::new("unresolved").unwrap(); MAX_QUALIFICATION_REFERENCES + 1];
        let result = qualify(&ledger, &hypothesis(vec![], vec![]), &ids, at(50), policy());
        assert_eq!(result.err(), Some(QualificationError::TooManyReferences));
        assert_eq!(ledger.coverage().unwrap().observation_count, 0);
    }

    #[test]
    fn extended_window_regression_cannot_refresh_old_evidence_or_promote_support() {
        let ledger = ledger();
        let old = ObservationId::new("old-evidence-extended-window").unwrap();
        let recent = ObservationId::new("recent-complete-window").unwrap();
        // Exact review counterexample: status 0..9, timeouts 10..12,
        // requested [0,1000], recorded at 1000. Do not align the endpoint.
        let old_snapshot = snapshot_with_status_span(0, 9, 1000, true);
        assert_eq!(
            old_snapshot.coverage.status_evidence_span_in_window,
            Some((at(0), at(9)))
        );
        assert!(old_snapshot.coverage.status_history_covers_requested_window);
        ledger
            .record(&DiagnosticObservation {
                observation_id: old.clone(),
                observed_at: at(1000),
                snapshot: old_snapshot,
            })
            .unwrap();
        ledger
            .record(&DiagnosticObservation {
                observation_id: recent.clone(),
                observed_at: at(1020),
                snapshot: snapshot_with_status_span(1001, 1020, 1020, true),
            })
            .unwrap();
        let candidate = hypothesis(vec![old.clone()], vec![]);
        let result = qualify(
            &ledger,
            &candidate,
            std::slice::from_ref(&recent),
            at(1050),
            policy(),
        )
        .unwrap();
        assert_eq!(result.state, QualificationState::InsufficientEvidence);
        assert_eq!(result.confidence, HypothesisConfidence::Low);
        assert_eq!(result.reason, QualificationReason::StaleEvidence);
        assert!(result.evidence_is_stale);
        assert_eq!(result.verified_supporting_references, vec![recent.clone()]);
        for reason in [
            EvidenceGapReason::IncompleteEvidenceCoverage,
            EvidenceGapReason::StaleObservation,
        ] {
            assert!(result.evidence_gaps.contains(&EvidenceGap {
                observation_id: old.clone(),
                reason
            }));
        }
        // The fully covered recent observation remains usable independently.
        let valid = qualify(
            &ledger,
            &hypothesis(vec![recent.clone()], vec![]),
            &[],
            at(1050),
            policy(),
        )
        .unwrap();
        assert_eq!(valid.state, QualificationState::Hypothesis);
        assert_eq!(valid.verified_supporting_references, vec![recent]);
        assert!(valid.evidence_gaps.is_empty());
        assert!(!valid.evidence_is_stale);
    }

    #[test]
    fn changing_only_requested_endpoint_cannot_improve_stale_qualification() {
        let base = snapshot(0, true);
        let mut extended = base.clone();
        extended.coverage.requested_until = at(1000);
        assert_eq!(
            base.coverage.status_evidence_span_in_window,
            extended.coverage.status_evidence_span_in_window
        );
        let id = ObservationId::new("same-source").unwrap();
        let candidate = hypothesis(vec![id.clone()], vec![]);
        let results: Vec<_> = [base, extended]
            .into_iter()
            .map(|snapshot| {
                let ledger = ledger();
                ledger
                    .record(&DiagnosticObservation {
                        observation_id: id.clone(),
                        observed_at: at(1000),
                        snapshot,
                    })
                    .unwrap();
                qualify(&ledger, &candidate, &[], at(1050), policy()).unwrap()
            })
            .collect();
        for result in &results {
            assert_eq!(result.state, QualificationState::InsufficientEvidence);
            assert_eq!(result.confidence, HypothesisConfidence::Low);
            assert_eq!(result.reason, QualificationReason::StaleEvidence);
            assert!(result.evidence_is_stale);
            assert!(result.verified_supporting_references.is_empty());
        }
        assert!(
            results[1]
                .evidence_gaps
                .iter()
                .any(|gap| gap.reason == EvidenceGapReason::IncompleteEvidenceCoverage)
        );
    }

    #[test]
    fn incomplete_or_unproven_counter_evidence_cannot_weaken_a_candidate() {
        let ledger = ledger();
        let support = record(&ledger, "support", 0, true);
        let candidate = hypothesis(vec![support], vec![]);
        let incomplete = snapshot_with_status_span(40, 49, 60, false);
        let mut legacy = snapshot(40, false);
        legacy.coverage.status_evidence_span_in_window = None;
        for (label, snapshot, reason) in [
            (
                "incomplete-counter",
                incomplete,
                EvidenceGapReason::IncompleteEvidenceCoverage,
            ),
            (
                "legacy-counter",
                legacy,
                EvidenceGapReason::MissingEvidenceBoundaries,
            ),
        ] {
            let id = ObservationId::new(label).unwrap();
            ledger
                .record(&DiagnosticObservation {
                    observation_id: id.clone(),
                    observed_at: at(60),
                    snapshot,
                })
                .unwrap();
            let result = qualify(&ledger, &candidate, &[id], at(70), policy()).unwrap();
            assert_eq!(result.state, QualificationState::InsufficientEvidence);
            assert!(result.verified_contradicting_references.is_empty());
            assert_eq!(result.evidence_gaps[0].reason, reason);
        }
    }

    #[test]
    fn right_edge_covered_without_left_edge_is_an_explicit_gap() {
        let ledger = ledger();
        let health = HealthStore::open_in_memory(HealthRetentionPolicy::default()).unwrap();
        let status = LocalIntelligenceStatus {
            available: true,
            model: "synthetic".into(),
        };
        for second in 1..=20 {
            health
                .record_status_sample(&status, None, at(second))
                .unwrap();
        }
        for second in 10..13 {
            health
                .record_inference_failure(
                    "synthetic",
                    OperationClass::CompleteDetailed,
                    LocalIntelligenceFailure::Timeout,
                    None,
                    at(second),
                )
                .unwrap();
        }
        let snapshot = DiagnosticSnapshot::build(&health, at(0), at(20), 100).unwrap();
        assert_eq!(
            snapshot.coverage.status_evidence_span_in_window,
            Some((at(1), at(20)))
        );
        assert_eq!(snapshot.coverage.requested_until, at(20));
        let id = ObservationId::new("missing-left-edge").unwrap();
        ledger
            .record(&DiagnosticObservation {
                observation_id: id.clone(),
                observed_at: at(20),
                snapshot,
            })
            .unwrap();
        let result = qualify(
            &ledger,
            &hypothesis(vec![id.clone()], vec![]),
            &[],
            at(30),
            policy(),
        )
        .unwrap();
        assert_eq!(result.state, QualificationState::InsufficientEvidence);
        assert_eq!(result.confidence, HypothesisConfidence::Low);
        assert!(result.verified_supporting_references.is_empty());
        assert!(result.evidence_gaps.contains(&EvidenceGap {
            observation_id: id,
            reason: EvidenceGapReason::IncompleteEvidenceCoverage,
        }));
    }

    #[test]
    fn exact_right_edge_and_maximum_age_are_accepted_but_older_evidence_is_stale() {
        let ledger = ledger();
        let id = record(&ledger, "exact-age", 0, true);
        let observation = ledger.get(&id).unwrap().unwrap();
        assert_eq!(
            observation.snapshot.coverage.status_evidence_span_in_window,
            Some((at(0), at(20)))
        );
        assert_eq!(observation.snapshot.coverage.requested_until, at(20));
        let candidate = hypothesis(vec![id.clone()], vec![]);
        let exact = qualify(&ledger, &candidate, &[], at(120), policy()).unwrap();
        assert_eq!(exact.state, QualificationState::Hypothesis);
        assert_eq!(exact.verified_supporting_references, vec![id.clone()]);
        assert!(exact.evidence_gaps.is_empty());
        let old = qualify(&ledger, &candidate, &[], at(121), policy()).unwrap();
        assert_eq!(old.state, QualificationState::InsufficientEvidence);
        assert_eq!(old.reason, QualificationReason::StaleEvidence);
        assert_eq!(
            old.evidence_gaps,
            vec![EvidenceGap {
                observation_id: id,
                reason: EvidenceGapReason::StaleObservation,
            }]
        );
    }
}
