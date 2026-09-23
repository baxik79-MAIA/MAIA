use maia_domain::{AgentTask, AgentTaskState, Run, Version};
use maia_orchestrator::{PauseBlocker, PauseDecision, PauseFacts, evaluate_pause};
use maia_store::{AuditEventDraft, TaskPauseCommand, TaskRepository};

use crate::error::{RuntimeError, RuntimeResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PauseTaskCommand {
    pub task_id: maia_domain::AgentTaskId,
    pub expected_version: Version,
    pub expected_state: AgentTaskState,
    pub runs: Vec<Run>,
    pub audit: AuditEventDraft,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PauseTaskOutcome {
    Requested(AgentTask),
    WaitingForQuiescence,
    CanEnterPaused,
    AlreadyPaused,
    Blocked(Vec<PauseBlocker>),
}

/// Evaluates quiescent pause facts supplied by the caller.  The only mutation
/// in this use case is the atomic T5 `Running -> Pausing` request; the store's
/// T3 and T4 rechecks close the post-request creation/start race.
pub fn pause_task<S>(store: &S, command: PauseTaskCommand) -> RuntimeResult<PauseTaskOutcome>
where
    S: TaskRepository,
{
    let task = store
        .get_task(&command.task_id)
        .map_err(RuntimeError::from)?;
    if task.version() != &command.expected_version || task.state() != &command.expected_state {
        return Err(RuntimeError::Conflict);
    }
    let decision = evaluate_pause(PauseFacts {
        task_state: *task.state(),
        pause_requested: true,
        runs: &command.runs,
        proposed_run_state: None,
    });
    match decision {
        PauseDecision::TransitionToPausing => {
            let updated = store
                .pause_task(&TaskPauseCommand {
                    task_id: command.task_id,
                    expected_version: command.expected_version,
                    expected_state: command.expected_state,
                    audit: command.audit,
                })
                .map_err(RuntimeError::from)?;
            Ok(PauseTaskOutcome::Requested(updated))
        }
        PauseDecision::WaitingForQuiescence => Ok(PauseTaskOutcome::WaitingForQuiescence),
        PauseDecision::CanEnterPaused => Ok(PauseTaskOutcome::CanEnterPaused),
        PauseDecision::AlreadyPaused => Ok(PauseTaskOutcome::AlreadyPaused),
        PauseDecision::Blocked(value) => Ok(PauseTaskOutcome::Blocked(value)),
        PauseDecision::NoRequest => unreachable!("pause command always requests pause"),
    }
}
