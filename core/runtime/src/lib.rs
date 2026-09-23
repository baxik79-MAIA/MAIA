//! Pure transactional runtime coordination for MAIA M0.4.
//!
//! The runtime composes already-resolved facts through `maia-policy` and
//! `maia-orchestrator`, then invokes one atomic `maia-store` operation.  It has
//! no SQLite, filesystem, connector, scheduler or side-effect dependency.
#![forbid(unsafe_code)]

mod action;
mod approval;
mod error;
mod pause;
mod run;

pub use action::{ActionRevisionOutcome, ReviseActionCommand, revise_action};
pub use approval::{ApprovalDecisionOutcome, DecideApprovalCommand, decide_approval};
pub use error::{RuntimeError, RuntimeResult};
pub use pause::{PauseTaskCommand, PauseTaskOutcome, pause_task};
pub use run::{
    PrepareRunCommand, RoutingInput, RunAuthorizationFacts, RunPreparationOutcome,
    RunTransitionOutcome, TransitionRunCommand, prepare_run, transition_run,
};
