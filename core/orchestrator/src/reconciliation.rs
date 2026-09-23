use maia_domain::{ReconciliationEvidence, ReconciliationResult, Run, RunState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReconciliationBlocker {
    WrongRunState,
    MissingEvidence,
    EvidenceMismatch,
    EffectUnresolved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconciliationDecision {
    NeedsEvidence,
    EffectConfirmed { next_state: RunState },
    EffectNotExecuted { next_state: RunState },
    Unresolved { next_state: RunState },
    Blocked(Vec<ReconciliationBlocker>),
}

pub fn evaluate_reconciliation(
    run: &Run,
    evidence: Option<&ReconciliationEvidence>,
) -> ReconciliationDecision {
    if !matches!(
        *run.state(),
        RunState::OutcomeUnknown | RunState::Reconciling
    ) {
        return ReconciliationDecision::Blocked(vec![ReconciliationBlocker::WrongRunState]);
    }
    let Some(evidence) = evidence else {
        return ReconciliationDecision::NeedsEvidence;
    };
    if evidence.validate().is_err()
        || evidence.run_id() != run.id()
        || evidence.action_id() != run.action_id()
        || evidence.action_version() != run.action_version()
        || evidence.action_hash() != run.action_hash()
    {
        return ReconciliationDecision::Blocked(vec![ReconciliationBlocker::EvidenceMismatch]);
    }
    match *evidence.result() {
        ReconciliationResult::EffectConfirmed => ReconciliationDecision::EffectConfirmed {
            next_state: RunState::Completed,
        },
        ReconciliationResult::EffectNotExecuted => ReconciliationDecision::EffectNotExecuted {
            next_state: RunState::RetryableError,
        },
        ReconciliationResult::Unresolved => ReconciliationDecision::Unresolved {
            next_state: RunState::Reconciling,
        },
    }
}
