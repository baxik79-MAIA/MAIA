use maia_domain::{AgentTaskState, Run, RunState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PauseBlocker {
    InvalidTaskTransition,
    InFlightRunsRemain,
    NewRunForbiddenAfterRequest,
    StartForbiddenAfterRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PauseDecision {
    NoRequest,
    TransitionToPausing,
    WaitingForQuiescence,
    CanEnterPaused,
    AlreadyPaused,
    Blocked(Vec<PauseBlocker>),
}

pub struct PauseFacts<'a> {
    pub task_state: AgentTaskState,
    pub pause_requested: bool,
    pub runs: &'a [Run],
    /// `Created` models an attempted post-request Run creation; `Starting`
    /// models the forbidden created-to-starting edge after a pause request.
    pub proposed_run_state: Option<RunState>,
}

pub fn evaluate_pause(facts: PauseFacts<'_>) -> PauseDecision {
    let mut blockers = Vec::new();
    if facts.pause_requested {
        if facts.proposed_run_state == Some(RunState::Created) {
            blockers.push(PauseBlocker::NewRunForbiddenAfterRequest);
        }
        if facts.proposed_run_state == Some(RunState::Starting) {
            blockers.push(PauseBlocker::StartForbiddenAfterRequest);
        }
    }
    if !blockers.is_empty() {
        return PauseDecision::Blocked(blockers);
    }
    if !facts.pause_requested {
        return PauseDecision::NoRequest;
    }

    let in_flight = facts.runs.iter().any(|run| run.state().is_in_flight());
    match facts.task_state {
        AgentTaskState::Running => {
            if AgentTaskState::Running
                .validate_transition(AgentTaskState::Pausing)
                .is_err()
            {
                PauseDecision::Blocked(vec![PauseBlocker::InvalidTaskTransition])
            } else {
                PauseDecision::TransitionToPausing
            }
        }
        AgentTaskState::Pausing => {
            if in_flight {
                PauseDecision::WaitingForQuiescence
            } else if AgentTaskState::Pausing
                .validate_transition(AgentTaskState::Paused)
                .is_ok()
            {
                PauseDecision::CanEnterPaused
            } else {
                PauseDecision::Blocked(vec![PauseBlocker::InvalidTaskTransition])
            }
        }
        AgentTaskState::Paused => PauseDecision::AlreadyPaused,
        _ => PauseDecision::Blocked(vec![PauseBlocker::InvalidTaskTransition]),
    }
}
