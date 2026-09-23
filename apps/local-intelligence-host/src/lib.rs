//! Application-facing Local Intelligence advisory boundary (M0.15.17).
//!
//! The real Local Intelligence host consumes deterministic hypothesis
//! generation and qualification through this typed, read-only adapter. The
//! adapter preserves qualification state, confidence, evidence references,
//! requested and actual evidence timing, gaps, freshness and uncertainty. It
//! neither writes the ledger nor controls, remediates or invokes any runtime.
#![forbid(unsafe_code)]

use maia_local_intelligence_diagnostics::DiagnosticPattern;
use maia_local_intelligence_hypothesis_generator::{
    GenerationError, GenerationOutcome, propose_repeated_timeout,
};
use maia_local_intelligence_hypothesis_ledger::{
    HypothesisConfidence, HypothesisId, Ledger, LedgerError, ObservationId,
};
use maia_local_intelligence_hypothesis_qualification::{
    EvidenceGapReason, EvidenceWindow, QualificationError, QualificationPolicy,
    QualificationReason, QualificationState, QualifiedDiagnosticHypothesis, qualify,
};
use std::time::SystemTime;

/// Caller-supplied bounds keep the adapter deterministic and free of a hidden
/// clock. The host supplies its real observation window and current time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplicationDiagnosticRequest {
    pub since: SystemTime,
    pub until: SystemTime,
    pub scan_limit: usize,
    pub evaluated_at: SystemTime,
    pub qualification_policy: QualificationPolicy,
}

/// Application vocabulary. It does not replace the underlying qualification
/// enums: each assessment retains the complete typed qualification result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationDiagnosticState {
    Qualified,
    NotQualified,
    InsufficientEvidence,
    StaleEvidence,
    IncompleteCoverage,
    CounterEvidencePresent,
}

impl ApplicationDiagnosticState {
    pub fn code(self) -> &'static str {
        match self {
            Self::Qualified => "QUALIFIED",
            Self::NotQualified => "NOT_QUALIFIED",
            Self::InsufficientEvidence => "INSUFFICIENT_EVIDENCE",
            Self::StaleEvidence => "STALE_EVIDENCE",
            Self::IncompleteCoverage => "INCOMPLETE_COVERAGE",
            Self::CounterEvidencePresent => "COUNTER_EVIDENCE_PRESENT",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationEvaluationStage {
    Generation,
    Qualification,
    Provenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationUnavailableReason {
    Ledger(LedgerError),
    EvidenceChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationEvaluationFailure {
    InvalidRequest,
    UnsupportedSubject,
    TooManyReferences,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceRole {
    Supporting,
    Contradicting,
}

/// Point-in-time evidence provenance visible to the application. Missing
/// observations remain visible as `available=false`; no timestamp is inferred
/// from request metadata or recording time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationEvidenceProvenance {
    pub observation_id: ObservationId,
    pub role: EvidenceRole,
    pub available: bool,
    pub observed_at: Option<SystemTime>,
    pub requested_window: Option<EvidenceWindow>,
    pub actual_status_span: Option<EvidenceWindow>,
}

/// One candidate's application classification plus the unchanged, detailed
/// qualification record and evidence provenance used to present uncertainty.
#[derive(Debug, Clone, PartialEq)]
pub struct ApplicationDiagnosticAssessment {
    pub state: ApplicationDiagnosticState,
    pub qualification: QualifiedDiagnosticHypothesis,
    pub evidence: Vec<ApplicationEvidenceProvenance>,
}

impl ApplicationDiagnosticAssessment {
    pub fn hypothesis_id(&self) -> &HypothesisId {
        &self.qualification.hypothesis.hypothesis_id
    }

    pub fn subject(&self) -> DiagnosticPattern {
        self.qualification.hypothesis.subject
    }

    pub fn confidence(&self) -> HypothesisConfidence {
        self.qualification.confidence
    }
}

/// Top-level application result. Unavailability and evaluation failure are
/// explicit and cannot be mistaken for a successful or qualified diagnosis.
#[derive(Debug, Clone, PartialEq)]
pub enum ApplicationDiagnosticResult {
    Evaluated {
        assessments: Vec<ApplicationDiagnosticAssessment>,
    },
    InsufficientEvidence {
        observations_scanned: usize,
        indeterminate_observations: usize,
    },
    Unavailable {
        stage: ApplicationEvaluationStage,
        reason: ApplicationUnavailableReason,
    },
    EvaluationFailed {
        stage: ApplicationEvaluationStage,
        reason: ApplicationEvaluationFailure,
    },
}

/// Read the bounded observation ledger, generate the existing competing
/// repeated-timeout proposals, qualify each, and translate only the
/// application-level disposition. The detailed qualification remains intact.
/// This function performs no write and has no action or provider capability.
pub fn evaluate_local_intelligence(
    ledger: &Ledger,
    request: ApplicationDiagnosticRequest,
) -> ApplicationDiagnosticResult {
    let generated = match propose_repeated_timeout(
        ledger,
        request.since,
        request.until,
        request.scan_limit,
        request.evaluated_at,
    ) {
        Ok(outcome) => outcome,
        Err(error) => return map_generation_error(error),
    };
    let GenerationOutcome::Proposed(hypotheses) = generated else {
        let GenerationOutcome::InsufficientEvidence {
            observations_scanned,
            indeterminate_observations,
        } = generated
        else {
            unreachable!("generation outcome exhaustively matched")
        };
        return ApplicationDiagnosticResult::InsufficientEvidence {
            observations_scanned,
            indeterminate_observations,
        };
    };

    let mut assessments = Vec::with_capacity(hypotheses.len());
    for hypothesis in hypotheses {
        let qualification = match qualify(
            ledger,
            &hypothesis,
            &[],
            request.evaluated_at,
            request.qualification_policy,
        ) {
            Ok(result) => result,
            Err(error) => return map_qualification_error(error),
        };
        let evidence = match collect_provenance(ledger, &hypothesis) {
            Ok(evidence) => evidence,
            Err(error) => {
                return ApplicationDiagnosticResult::Unavailable {
                    stage: ApplicationEvaluationStage::Provenance,
                    reason: ApplicationUnavailableReason::Ledger(error),
                };
            }
        };
        assessments.push(ApplicationDiagnosticAssessment {
            state: classify(&qualification),
            qualification,
            evidence,
        });
    }
    ApplicationDiagnosticResult::Evaluated { assessments }
}

fn classify(result: &QualifiedDiagnosticHypothesis) -> ApplicationDiagnosticState {
    match result.state {
        QualificationState::SupportedHypothesis => ApplicationDiagnosticState::Qualified,
        QualificationState::WeakenedHypothesis => {
            ApplicationDiagnosticState::CounterEvidencePresent
        }
        QualificationState::Hypothesis | QualificationState::DisprovenHypothesis => {
            ApplicationDiagnosticState::NotQualified
        }
        QualificationState::InsufficientEvidence => {
            if result.evidence_is_stale || result.reason == QualificationReason::StaleEvidence {
                ApplicationDiagnosticState::StaleEvidence
            } else if result.evidence_gaps.iter().any(|gap| {
                matches!(
                    gap.reason,
                    EvidenceGapReason::InvalidWindow
                        | EvidenceGapReason::MissingEvidenceBoundaries
                        | EvidenceGapReason::InvalidEvidenceBoundaries
                        | EvidenceGapReason::IncompleteEvidenceCoverage
                )
            }) {
                ApplicationDiagnosticState::IncompleteCoverage
            } else {
                ApplicationDiagnosticState::InsufficientEvidence
            }
        }
    }
}

fn collect_provenance(
    ledger: &Ledger,
    hypothesis: &maia_local_intelligence_hypothesis_ledger::DiagnosticHypothesis,
) -> Result<Vec<ApplicationEvidenceProvenance>, LedgerError> {
    let mut evidence = Vec::with_capacity(
        hypothesis.supporting_evidence.len() + hypothesis.contradicting_evidence.len(),
    );
    for (role, references) in [
        (EvidenceRole::Supporting, &hypothesis.supporting_evidence),
        (
            EvidenceRole::Contradicting,
            &hypothesis.contradicting_evidence,
        ),
    ] {
        for id in references {
            let observation = ledger.get(id)?;
            evidence.push(match observation {
                Some(observation) => {
                    let coverage = &observation.snapshot.coverage;
                    ApplicationEvidenceProvenance {
                        observation_id: id.clone(),
                        role,
                        available: true,
                        observed_at: Some(observation.observed_at),
                        requested_window: Some(EvidenceWindow {
                            since: coverage.requested_since,
                            until: coverage.requested_until,
                        }),
                        actual_status_span: coverage
                            .status_evidence_span_in_window
                            .map(|(since, until)| EvidenceWindow { since, until }),
                    }
                }
                None => ApplicationEvidenceProvenance {
                    observation_id: id.clone(),
                    role,
                    available: false,
                    observed_at: None,
                    requested_window: None,
                    actual_status_span: None,
                },
            });
        }
    }
    Ok(evidence)
}

fn map_generation_error(error: GenerationError) -> ApplicationDiagnosticResult {
    match error {
        GenerationError::InvalidInput => ApplicationDiagnosticResult::EvaluationFailed {
            stage: ApplicationEvaluationStage::Generation,
            reason: ApplicationEvaluationFailure::InvalidRequest,
        },
        GenerationError::Ledger(error) => ApplicationDiagnosticResult::Unavailable {
            stage: ApplicationEvaluationStage::Generation,
            reason: ApplicationUnavailableReason::Ledger(error),
        },
        GenerationError::EvidenceUnavailable => ApplicationDiagnosticResult::Unavailable {
            stage: ApplicationEvaluationStage::Generation,
            reason: ApplicationUnavailableReason::EvidenceChanged,
        },
    }
}

fn map_qualification_error(error: QualificationError) -> ApplicationDiagnosticResult {
    match error {
        QualificationError::Ledger(error) => ApplicationDiagnosticResult::Unavailable {
            stage: ApplicationEvaluationStage::Qualification,
            reason: ApplicationUnavailableReason::Ledger(error),
        },
        QualificationError::UnsupportedSubject => ApplicationDiagnosticResult::EvaluationFailed {
            stage: ApplicationEvaluationStage::Qualification,
            reason: ApplicationEvaluationFailure::UnsupportedSubject,
        },
        QualificationError::InvalidInput => ApplicationDiagnosticResult::EvaluationFailed {
            stage: ApplicationEvaluationStage::Qualification,
            reason: ApplicationEvaluationFailure::InvalidRequest,
        },
        QualificationError::TooManyReferences => ApplicationDiagnosticResult::EvaluationFailed {
            stage: ApplicationEvaluationStage::Qualification,
            reason: ApplicationEvaluationFailure::TooManyReferences,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maia_local_intelligence_diagnostics::DiagnosticSnapshot;
    use maia_local_intelligence_health::{
        HealthStore, OperationClass, RetentionPolicy as HealthRetentionPolicy,
    };
    use maia_local_intelligence_hypothesis_ledger::{
        DiagnosticObservation, RecordOutcome, RetentionPolicy,
    };
    use maia_local_model::{LocalIntelligenceFailure, LocalIntelligenceStatus};
    use std::time::{Duration, UNIX_EPOCH};

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn snapshot(start: u64, last_status: u64, until: u64, timeouts: bool) -> DiagnosticSnapshot {
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
        if timeouts {
            for offset in 10..13 {
                health
                    .record_inference_failure(
                        "synthetic",
                        OperationClass::CompleteDetailed,
                        LocalIntelligenceFailure::Timeout,
                        None,
                        at(start + offset),
                    )
                    .unwrap();
            }
        }
        DiagnosticSnapshot::build(&health, at(start), at(until), 100).unwrap()
    }

    fn record(
        ledger: &Ledger,
        id: &str,
        start: u64,
        last_status: u64,
        until: u64,
        observed_at: u64,
        timeouts: bool,
    ) {
        assert_eq!(
            ledger.record(&DiagnosticObservation {
                observation_id: ObservationId::new(id).unwrap(),
                observed_at: at(observed_at),
                snapshot: snapshot(start, last_status, until, timeouts),
            }),
            Ok(RecordOutcome::Recorded)
        );
    }

    fn request(evaluated_at: u64) -> ApplicationDiagnosticRequest {
        ApplicationDiagnosticRequest {
            since: at(0),
            until: at(evaluated_at),
            scan_limit: 100,
            evaluated_at: at(evaluated_at),
            qualification_policy: QualificationPolicy {
                max_evidence_age: Duration::from_secs(100),
            },
        }
    }

    #[test]
    fn empty_ledger_is_explicitly_insufficient() {
        let ledger = Ledger::open_in_memory(RetentionPolicy::default()).unwrap();
        assert_eq!(
            evaluate_local_intelligence(&ledger, request(50)),
            ApplicationDiagnosticResult::InsufficientEvidence {
                observations_scanned: 0,
                indeterminate_observations: 0,
            }
        );
    }

    #[test]
    fn complete_fresh_disjoint_evidence_is_qualified_with_provenance() {
        let ledger = Ledger::open_in_memory(RetentionPolicy::default()).unwrap();
        record(&ledger, "first", 0, 20, 20, 20, true);
        record(&ledger, "second", 21, 41, 41, 41, true);
        let ApplicationDiagnosticResult::Evaluated { assessments } =
            evaluate_local_intelligence(&ledger, request(50))
        else {
            panic!("expected evaluated result")
        };
        assert_eq!(
            assessments.len(),
            2,
            "both competing proposals remain visible"
        );
        for assessment in assessments {
            assert_eq!(assessment.state, ApplicationDiagnosticState::Qualified);
            assert_eq!(assessment.confidence(), HypothesisConfidence::Moderate);
            assert_eq!(
                assessment.qualification.state,
                QualificationState::SupportedHypothesis
            );
            assert_eq!(assessment.evidence.len(), 2);
            assert!(assessment.evidence.iter().all(|item| {
                item.available
                    && item.requested_window.is_some()
                    && item.actual_status_span == item.requested_window
            }));
            assert!(!assessment.qualification.hypothesis.uncertainty.is_empty());
        }
    }

    #[test]
    fn one_complete_support_is_not_presented_as_qualified() {
        let ledger = Ledger::open_in_memory(RetentionPolicy::default()).unwrap();
        record(&ledger, "single", 0, 20, 20, 20, true);
        let ApplicationDiagnosticResult::Evaluated { assessments } =
            evaluate_local_intelligence(&ledger, request(30))
        else {
            panic!("expected evaluated result")
        };
        for assessment in assessments {
            assert_eq!(assessment.state, ApplicationDiagnosticState::NotQualified);
            assert_eq!(
                assessment.qualification.state,
                QualificationState::Hypothesis
            );
            assert_eq!(assessment.confidence(), HypothesisConfidence::Low);
        }
    }

    #[test]
    fn valid_counter_evidence_keeps_precedence_and_provenance() {
        let ledger = Ledger::open_in_memory(RetentionPolicy::default()).unwrap();
        record(&ledger, "support-a", 0, 20, 20, 20, true);
        record(&ledger, "support-b", 21, 41, 41, 41, true);
        record(&ledger, "counter", 42, 62, 62, 62, false);
        let ApplicationDiagnosticResult::Evaluated { assessments } =
            evaluate_local_intelligence(&ledger, request(70))
        else {
            panic!("expected evaluated result")
        };
        for assessment in assessments {
            assert_eq!(
                assessment.state,
                ApplicationDiagnosticState::CounterEvidencePresent
            );
            assert_eq!(
                assessment.qualification.state,
                QualificationState::WeakenedHypothesis
            );
            assert_eq!(
                assessment.qualification.reason,
                QualificationReason::ContradictoryObservation
            );
            assert_eq!(
                assessment
                    .evidence
                    .iter()
                    .filter(|item| item.role == EvidenceRole::Contradicting)
                    .count(),
                1
            );
        }
    }

    #[test]
    fn missing_right_edge_is_visible_as_incomplete_and_cannot_qualify() {
        let ledger = Ledger::open_in_memory(RetentionPolicy::default()).unwrap();
        record(&ledger, "partial", 0, 9, 20, 20, true);
        let ApplicationDiagnosticResult::Evaluated { assessments } =
            evaluate_local_intelligence(&ledger, request(30))
        else {
            panic!("expected evaluated result")
        };
        for assessment in assessments {
            assert_eq!(
                assessment.state,
                ApplicationDiagnosticState::IncompleteCoverage
            );
            assert_eq!(
                assessment.qualification.state,
                QualificationState::InsufficientEvidence
            );
            assert!(
                assessment
                    .qualification
                    .evidence_gaps
                    .iter()
                    .any(|gap| { gap.reason == EvidenceGapReason::IncompleteEvidenceCoverage })
            );
            assert!(
                assessment
                    .qualification
                    .verified_supporting_references
                    .is_empty()
            );
        }
    }

    #[test]
    fn extended_requested_end_cannot_refresh_old_evidence() {
        let ledger = Ledger::open_in_memory(RetentionPolicy::default()).unwrap();
        record(&ledger, "old-extended", 0, 9, 1000, 1000, true);
        record(&ledger, "fresh", 1001, 1020, 1020, 1020, true);
        let mut query = request(1050);
        query.until = at(1050);
        let ApplicationDiagnosticResult::Evaluated { assessments } =
            evaluate_local_intelligence(&ledger, query)
        else {
            panic!("expected evaluated result")
        };
        for assessment in assessments {
            assert_eq!(assessment.state, ApplicationDiagnosticState::StaleEvidence);
            assert_eq!(
                assessment.qualification.reason,
                QualificationReason::StaleEvidence
            );
            assert!(
                assessment
                    .qualification
                    .evidence_gaps
                    .iter()
                    .any(|gap| { gap.reason == EvidenceGapReason::IncompleteEvidenceCoverage })
            );
            assert!(
                assessment
                    .qualification
                    .evidence_gaps
                    .iter()
                    .any(|gap| { gap.reason == EvidenceGapReason::StaleObservation })
            );
        }
    }

    #[test]
    fn invalid_application_request_fails_without_fabricating_a_result() {
        let ledger = Ledger::open_in_memory(RetentionPolicy::default()).unwrap();
        let mut invalid = request(50);
        invalid.scan_limit = 0;
        assert_eq!(
            evaluate_local_intelligence(&ledger, invalid),
            ApplicationDiagnosticResult::EvaluationFailed {
                stage: ApplicationEvaluationStage::Generation,
                reason: ApplicationEvaluationFailure::InvalidRequest,
            }
        );
        assert!(matches!(
            map_generation_error(GenerationError::Ledger(LedgerError::StorageUnavailable)),
            ApplicationDiagnosticResult::Unavailable {
                stage: ApplicationEvaluationStage::Generation,
                reason: ApplicationUnavailableReason::Ledger(LedgerError::StorageUnavailable),
            }
        ));
    }
}
