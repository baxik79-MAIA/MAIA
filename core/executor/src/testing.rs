use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use super::{ActionExecutor, ExecutionRequest, ExecutorOutcome};

/// Deterministic test-only executor. It never performs an external effect.
#[derive(Debug, Clone)]
pub struct FakeExecutor {
    outcome: ExecutorOutcome,
    panic_before_result: bool,
    invocations: Arc<AtomicUsize>,
}

impl FakeExecutor {
    pub fn new(outcome: ExecutorOutcome) -> Self {
        Self {
            outcome,
            panic_before_result: false,
            invocations: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn panicking() -> Self {
        Self {
            outcome: ExecutorOutcome::OutcomeUnknown,
            panic_before_result: true,
            invocations: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn invocation_count(&self) -> usize {
        self.invocations.load(Ordering::SeqCst)
    }
}

impl ActionExecutor for FakeExecutor {
    fn execute(&self, _request: ExecutionRequest<'_>) -> ExecutorOutcome {
        self.invocations.fetch_add(1, Ordering::SeqCst);
        if self.panic_before_result {
            panic!("fake executor interrupted before returning a result");
        }
        self.outcome
    }
}
