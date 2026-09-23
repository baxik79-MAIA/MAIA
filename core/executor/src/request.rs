use maia_domain::{Action, Approval, Run};

/// The only material supplied to a connector-facing executor. The store and
/// workspace are deliberately absent; the action and approval remain
/// available for exact binding and capability-specific interpretation.
#[derive(Debug, Clone, Copy)]
pub struct ExecutionRequest<'a> {
    pub run: &'a Run,
    pub action: &'a Action,
    pub approval: &'a Approval,
}

/// A connector/tool implementation classifies the effect semantically. A
/// missing or uncertain result must use `OutcomeUnknown`; it is never silently
/// converted into a retryable failure.
pub trait ActionExecutor {
    fn execute(&self, request: ExecutionRequest<'_>) -> ExecutorOutcome;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorOutcome {
    EffectSucceeded,
    EffectFailedFinal,
    EffectFailedRetryable,
    OutcomeUnknown,
}
