//! Deterministic, read-only diagnostic hypothesis generation (M0.15.15).
//!
//! This first slice interprets the already-recorded `RepeatedTimeout` pattern.
//! It proposes two competing possible explanations without choosing a cause.
//! It reads only the ledger's public observation contract, verifies that its
//! referenced records still resolve, and leaves persistence to the caller via
//! `Ledger::record_hypothesis`. It cannot control the monitored runtime, call
//! a model, or authorize remediation.
#![forbid(unsafe_code)]

use maia_local_intelligence_diagnostics::DiagnosticPattern;
use maia_local_intelligence_hypothesis_ledger::{
    DiagnosticHypothesis, EvidenceState, HypothesisConfidence, HypothesisId, HypothesisStatus,
    Ledger, LedgerError, MAX_BOUNDED_SCAN, ObservationId, pattern_state,
};
use sha2::{Digest, Sha256};
use std::time::SystemTime;

const GENERATOR: &str = "deterministic-repeated-timeout-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationError {
    InvalidInput,
    Ledger(LedgerError),
    /// Retention or a concurrent writer removed a referenced observation
    /// between the bounded scan and reference verification.
    EvidenceUnavailable,
}

impl From<LedgerError> for GenerationError {
    fn from(value: LedgerError) -> Self {
        Self::Ledger(value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GenerationOutcome {
    /// No fully-covered observation supports the pattern. This is a real
    /// result, never a fabricated low-confidence causal explanation.
    InsufficientEvidence {
        observations_scanned: usize,
        indeterminate_observations: usize,
    },
    /// Competing proposals, not ranked or confirmed diagnoses. The caller
    /// may persist each through the separate hypothesis ledger API.
    Proposed(Vec<DiagnosticHypothesis>),
}

/// Reads at most `scan_limit` observations in `[since, until]` and proposes
/// two explanations for a fully-covered repeated-timeout observation.
/// `generated_at` is caller-supplied, so identical inputs have identical
/// results without a hidden clock.
///
/// Every evidence ID is obtained from a persisted observation and resolved
/// once more before returning. This proves reference existence at generation
/// time only. Retention may evict an observation later; the hypothesis record
/// remains auditable as a proposal with an ID, not as verified causal truth.
/// All generated confidence is deliberately `Low`: overlapping observation
/// windows and the observed timeout pattern do not distinguish the causes.
pub fn propose_repeated_timeout(
    ledger: &Ledger,
    since: SystemTime,
    until: SystemTime,
    scan_limit: usize,
    generated_at: SystemTime,
) -> Result<GenerationOutcome, GenerationError> {
    if scan_limit == 0 || scan_limit > MAX_BOUNDED_SCAN || until <= since {
        return Err(GenerationError::InvalidInput);
    }
    let observations = ledger.observations_in_window(since, until, scan_limit)?;
    let mut support = Vec::new();
    let mut counter = Vec::new();
    let mut indeterminate = 0;
    for observation in &observations {
        match pattern_state(observation, DiagnosticPattern::RepeatedTimeout) {
            EvidenceState::Supported => support.push(observation.observation_id.clone()),
            EvidenceState::NotSupported => counter.push(observation.observation_id.clone()),
            EvidenceState::Indeterminate => indeterminate += 1,
        }
    }
    if support.is_empty() {
        return Ok(GenerationOutcome::InsufficientEvidence {
            observations_scanned: observations.len(),
            indeterminate_observations: indeterminate,
        });
    }
    for id in support.iter().chain(counter.iter()) {
        if ledger.get(id)?.is_none() {
            return Err(GenerationError::EvidenceUnavailable);
        }
    }
    let candidates = [
        (
            "transport-delay",
            "A delay in the local transport path may explain repeated timeouts.",
        ),
        (
            "runtime-delay",
            "Slow local runtime processing may explain repeated timeouts.",
        ),
    ];
    Ok(GenerationOutcome::Proposed(
        candidates
            .into_iter()
            .map(|(key, explanation)| DiagnosticHypothesis {
                hypothesis_id: stable_id(key, &support, &counter),
                subject: DiagnosticPattern::RepeatedTimeout,
                explanation: explanation.into(),
                supporting_evidence: support.clone(),
                contradicting_evidence: counter.clone(),
                confidence: HypothesisConfidence::Low,
                uncertainty: "The timeout observations do not distinguish transport delay from runtime delay; neither cause has been verified, and scanned windows may overlap.".into(),
                evidence_sufficient: false,
                generated_at,
                generator: GENERATOR.into(),
                status: HypothesisStatus::Proposed,
            })
            .collect(),
    ))
}

fn stable_id(key: &str, support: &[ObservationId], counter: &[ObservationId]) -> HypothesisId {
    let mut digest = Sha256::new();
    digest.update((support.len() as u64).to_be_bytes());
    digest.update((counter.len() as u64).to_be_bytes());
    for part in std::iter::once(key)
        .chain(support.iter().map(ObservationId::as_str))
        .chain(counter.iter().map(ObservationId::as_str))
    {
        digest.update((part.len() as u64).to_be_bytes());
        digest.update(part.as_bytes());
    }
    let hex = digest
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    HypothesisId::new(format!("diag-timeout-v1-{key}-{hex}")).expect("bounded stable ID")
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

    fn snapshot(with_timeouts: bool) -> DiagnosticSnapshot {
        let health = HealthStore::open_in_memory(HealthRetentionPolicy::default()).unwrap();
        let status = LocalIntelligenceStatus {
            available: true,
            model: "synthetic-model".into(),
        };
        for second in 0..10 {
            health
                .record_status_sample(&status, None, at(second))
                .unwrap();
        }
        if with_timeouts {
            for second in 100..103 {
                health
                    .record_inference_failure(
                        "synthetic-model",
                        OperationClass::CompleteDetailed,
                        LocalIntelligenceFailure::Timeout,
                        None,
                        at(second),
                    )
                    .unwrap();
            }
        }
        DiagnosticSnapshot::build(&health, at(0), at(200), 100).unwrap()
    }

    fn record(ledger: &Ledger, id: &str, at_time: u64, with_timeouts: bool) {
        let observation = DiagnosticObservation {
            observation_id: ObservationId::new(id).unwrap(),
            observed_at: at(at_time),
            snapshot: snapshot(with_timeouts),
        };
        assert_eq!(ledger.record(&observation), Ok(RecordOutcome::Recorded));
    }

    #[test]
    fn valid_evidence_produces_two_stable_competing_proposals_with_resolving_references() {
        let ledger = Ledger::open_in_memory(RetentionPolicy::default()).unwrap();
        record(&ledger, "support-1", 1, true);
        record(&ledger, "counter", 2, false);
        record(&ledger, "support-2", 3, true);
        let first = propose_repeated_timeout(&ledger, at(0), at(10), 10, at(10)).unwrap();
        let repeated = propose_repeated_timeout(&ledger, at(0), at(10), 10, at(10)).unwrap();
        assert_eq!(
            first, repeated,
            "same evidence gives the same IDs and proposals"
        );
        let GenerationOutcome::Proposed(hypotheses) = first else {
            panic!("expected proposals")
        };
        assert_eq!(hypotheses.len(), 2);
        assert_ne!(hypotheses[0].hypothesis_id, hypotheses[1].hypothesis_id);
        assert_ne!(hypotheses[0].explanation, hypotheses[1].explanation);
        for hypothesis in &hypotheses {
            assert_eq!(hypothesis.subject, DiagnosticPattern::RepeatedTimeout);
            assert_eq!(hypothesis.confidence, HypothesisConfidence::Low);
            assert_eq!(hypothesis.status, HypothesisStatus::Proposed);
            assert!(!hypothesis.evidence_sufficient);
            assert!(!hypothesis.uncertainty.is_empty());
            assert_eq!(hypothesis.generator, GENERATOR);
            assert_eq!(hypothesis.supporting_evidence.len(), 2);
            assert_eq!(hypothesis.contradicting_evidence.len(), 1);
            for id in hypothesis
                .supporting_evidence
                .iter()
                .chain(&hypothesis.contradicting_evidence)
            {
                assert!(ledger.get(id).unwrap().is_some());
            }
            assert_eq!(
                ledger.record_hypothesis(hypothesis),
                Ok(RecordOutcome::Recorded)
            );
            assert_eq!(
                ledger.record_hypothesis(hypothesis),
                Ok(RecordOutcome::AlreadyExists)
            );
        }
        assert_eq!(ledger.latest_hypotheses(10).unwrap().len(), 2);
        assert_eq!(ledger.coverage().unwrap().observation_count, 3);
    }

    #[test]
    fn absence_or_weak_evidence_returns_explicit_insufficiency() {
        let ledger = Ledger::open_in_memory(RetentionPolicy::default()).unwrap();
        assert_eq!(
            propose_repeated_timeout(&ledger, at(0), at(10), 10, at(10)).unwrap(),
            GenerationOutcome::InsufficientEvidence {
                observations_scanned: 0,
                indeterminate_observations: 0,
            }
        );
        let health = HealthStore::open_in_memory(HealthRetentionPolicy::default()).unwrap();
        let weak = DiagnosticObservation {
            observation_id: ObservationId::new("weak").unwrap(),
            observed_at: at(1),
            snapshot: DiagnosticSnapshot::build(&health, at(0), at(20), 100).unwrap(),
        };
        ledger.record(&weak).unwrap();
        assert_eq!(
            propose_repeated_timeout(&ledger, at(0), at(10), 10, at(10)).unwrap(),
            GenerationOutcome::InsufficientEvidence {
                observations_scanned: 1,
                indeterminate_observations: 1,
            }
        );
        assert!(ledger.latest_hypotheses(10).unwrap().is_empty());
    }

    #[test]
    fn invalid_or_unbounded_requests_fail_without_recording_hypotheses() {
        let ledger = Ledger::open_in_memory(RetentionPolicy::default()).unwrap();
        for (since, until, limit) in [
            (at(0), at(10), 0),
            (at(0), at(10), MAX_BOUNDED_SCAN + 1),
            (at(10), at(10), 1),
        ] {
            assert_eq!(
                propose_repeated_timeout(&ledger, since, until, limit, at(10)).err(),
                Some(GenerationError::InvalidInput)
            );
        }
        assert!(ledger.latest_hypotheses(10).unwrap().is_empty());
    }
}
