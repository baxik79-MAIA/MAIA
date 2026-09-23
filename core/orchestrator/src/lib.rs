//! Pure execution eligibility and orchestration semantics.
//!
//! This crate constructs and validates in-memory domain values only. It has no
//! persistence, connector, scheduler, clock, ID allocator, or side effect.
#![forbid(unsafe_code)]

mod eligibility;
mod freshness;
mod pause;
mod reconciliation;
mod retry;
mod routing;
mod run;

pub use eligibility::{
    ExecutionEligibilityBlocker, ExecutionEligibilityBlockerSet, ExecutionEligibilityDecision,
    ExecutionEligibilityFacts, evaluate_execution_eligibility,
};
pub use freshness::{FreshnessBlocker, FreshnessDecision, evaluate_freshness};
pub use pause::{PauseBlocker, PauseDecision, PauseFacts, evaluate_pause};
pub use reconciliation::{ReconciliationBlocker, ReconciliationDecision, evaluate_reconciliation};
pub use retry::{RetryBlocker, RetryDecision, RetryFacts, evaluate_retry, next_attempt};
pub use routing::{RoutingBlocker, RoutingDecision, RoutingFacts, check_routing};
pub use run::{
    RunBinding, RunBuildError, RunSpec, build_run_spec, create_run, validate_run_binding,
};
