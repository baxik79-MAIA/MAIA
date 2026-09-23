use maia_domain::{Run, RunId, RunState};

/// A run that must be examined after an interrupted one-shot invocation.
/// This is an observation only; it performs no transition and no retry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryCandidate {
    pub run_id: RunId,
    pub state: RunState,
}

/// Identifies all canonical in-flight states that can remain after a process
/// interruption. The caller must reconcile them before considering a new Run.
pub fn recover_incomplete_runs(runs: &[Run]) -> Vec<RecoveryCandidate> {
    runs.iter()
        .filter(|run| run.state().is_in_flight())
        .map(|run| RecoveryCandidate {
            run_id: run.id().clone(),
            state: *run.state(),
        })
        .collect()
}
