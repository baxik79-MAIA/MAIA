use maia_domain::{Attempt, DomainError, RunState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RetryBlocker {
    RunNotRetryable,
    CurrentPolicyDenied,
    CurrentExecutionChecksFailed,
    PriorEffectUnresolved,
    AttemptOverflow,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RetryBlockerSet(Vec<RetryBlocker>);

impl RetryBlockerSet {
    fn new(values: impl IntoIterator<Item = RetryBlocker>) -> Self {
        let mut values: Vec<_> = values.into_iter().collect();
        values.sort_unstable();
        values.dedup();
        Self(values)
    }
    pub fn as_slice(&self) -> &[RetryBlocker] {
        &self.0
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryDecision {
    Allowed { attempt: Attempt },
    Blocked(RetryBlockerSet),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryFacts {
    pub prior_state: RunState,
    pub current_policy_allows: bool,
    pub current_execution_checks: bool,
    pub prior_effect_resolved: bool,
}

pub fn next_attempt(
    previous: Option<Attempt>,
    same_action_revision: bool,
) -> Result<Attempt, DomainError> {
    match (previous, same_action_revision) {
        (Some(previous), true) => previous.checked_next(),
        _ => Attempt::new(1),
    }
}

pub fn evaluate_retry(facts: RetryFacts, previous_attempt: Attempt) -> RetryDecision {
    let mut blockers = Vec::new();
    if facts.prior_state != RunState::RetryableError {
        blockers.push(RetryBlocker::RunNotRetryable);
    }
    if !facts.current_policy_allows {
        blockers.push(RetryBlocker::CurrentPolicyDenied);
    }
    if !facts.current_execution_checks {
        blockers.push(RetryBlocker::CurrentExecutionChecksFailed);
    }
    if !facts.prior_effect_resolved {
        blockers.push(RetryBlocker::PriorEffectUnresolved);
    }
    let next = previous_attempt.checked_next();
    if next.is_err() {
        blockers.push(RetryBlocker::AttemptOverflow);
    }
    let blockers = RetryBlockerSet::new(blockers);
    if blockers.is_empty() {
        RetryDecision::Allowed {
            attempt: next.expect("checked above"),
        }
    } else {
        RetryDecision::Blocked(blockers)
    }
}
