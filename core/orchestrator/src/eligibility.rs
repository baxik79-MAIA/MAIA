use maia_domain::Run;
use maia_policy::AuthorizationDecision;

use crate::run::{RunBinding, validate_run_binding};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExecutionEligibilityBlocker {
    AuthorizationBlocked,
    RunBindingMismatch,
    RunAlreadyStarted,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecutionEligibilityBlockerSet(Vec<ExecutionEligibilityBlocker>);

impl ExecutionEligibilityBlockerSet {
    pub fn new(values: impl IntoIterator<Item = ExecutionEligibilityBlocker>) -> Self {
        let mut values: Vec<_> = values.into_iter().collect();
        values.sort_unstable();
        values.dedup();
        Self(values)
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn as_slice(&self) -> &[ExecutionEligibilityBlocker] {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionEligibilityDecision {
    Allowed,
    Blocked(ExecutionEligibilityBlockerSet),
}

pub struct ExecutionEligibilityFacts<'a> {
    pub authorization: &'a AuthorizationDecision,
    pub run: &'a Run,
    pub expected_binding: RunBinding<'a>,
}

pub fn evaluate_execution_eligibility(
    facts: ExecutionEligibilityFacts<'_>,
) -> ExecutionEligibilityDecision {
    let mut blockers = Vec::new();
    if !matches!(facts.authorization, AuthorizationDecision::Allowed) {
        blockers.push(ExecutionEligibilityBlocker::AuthorizationBlocked);
    }
    if !validate_run_binding(facts.run, facts.expected_binding) {
        blockers.push(ExecutionEligibilityBlocker::RunBindingMismatch);
    }
    if *facts.run.state() != maia_domain::RunState::Created {
        blockers.push(ExecutionEligibilityBlocker::RunAlreadyStarted);
    }
    let blockers = ExecutionEligibilityBlockerSet::new(blockers);
    if blockers.is_empty() {
        ExecutionEligibilityDecision::Allowed
    } else {
        ExecutionEligibilityDecision::Blocked(blockers)
    }
}
