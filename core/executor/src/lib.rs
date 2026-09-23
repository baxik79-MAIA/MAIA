//! One-shot execution boundary for an already-created MAIA Run.
//!
//! This crate performs no connector I/O, persistence implementation or
//! scheduling. It revalidates a persisted Run, atomically claims it through
//! the existing runtime/store ports, invokes one capability-shaped executor,
//! and persists the canonical outcome transition.
#![forbid(unsafe_code)]

mod driver;
mod recovery;
mod request;

pub use driver::{
    ExecuteRunCommand, ExecuteRunOutcome, ExecutionAudit, ExecutionBlocker, ExecutionBlockerSet,
    ExecutionFacts, execute_run,
};
pub use recovery::{RecoveryCandidate, recover_incomplete_runs};
pub use request::{ActionExecutor, ExecutionRequest, ExecutorOutcome};

#[cfg(feature = "test-support")]
pub mod testing;
