use maia_domain::{Action, Approval, PolicyDecision, Run, RunId, RunState, Timestamp};
use maia_orchestrator::{
    ExecutionEligibilityBlockerSet, ExecutionEligibilityDecision, ExecutionEligibilityFacts,
    RoutingDecision, RoutingFacts, RunBinding, check_routing, evaluate_execution_eligibility,
};
use maia_policy::{
    AuthorizationBlockerSet, AuthorizationDecision, BindingValidityFacts,
    CurrentAuthorizationFacts, authorize_execution,
};
use maia_runtime::{RoutingInput, RunTransitionOutcome, TransitionRunCommand, transition_run};
use maia_store::{ApprovalRepository, AuditEventDraft, RevisionRepository, RunRepository};

use crate::{ActionExecutor, ExecutionRequest, ExecutorOutcome};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionFacts {
    pub now: Timestamp,
    pub current_policy: PolicyDecision,
    pub approval_actor_authorized: bool,
    pub execution_actor_authorized: bool,
    pub freshness_ok: bool,
    pub tool_definition_ok: bool,
    pub routing: RoutingInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionAudit {
    pub claim: AuditEventDraft,
    pub running: AuditEventDraft,
    pub outcome: AuditEventDraft,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecuteRunCommand {
    pub run_id: RunId,
    pub facts: ExecutionFacts,
    pub audit: ExecutionAudit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionBlocker {
    RunNotCreated,
    CurrentActionRevisionMismatch,
    ApprovalBindingMismatch,
    Authorization(AuthorizationBlockerSet),
    Routing(RoutingDecision),
    Eligibility(ExecutionEligibilityBlockerSet),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecutionBlockerSet(Vec<ExecutionBlocker>);

impl ExecutionBlockerSet {
    pub fn new(values: impl IntoIterator<Item = ExecutionBlocker>) -> Self {
        Self(values.into_iter().collect())
    }

    pub fn as_slice(&self) -> &[ExecutionBlocker] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecuteRunOutcome {
    EffectSucceeded(Box<Run>),
    EffectFailedFinal(Box<Run>),
    EffectFailedRetryable(Box<Run>),
    OutcomeUnknown(Box<Run>),
    Blocked(ExecutionBlockerSet),
}

/// Revalidates one persisted Created Run, claims it with T4, invokes exactly
/// one executor, then persists the canonical result transition with T4.
pub fn execute_run<S, E>(
    store: &S,
    executor: &E,
    command: ExecuteRunCommand,
) -> maia_runtime::RuntimeResult<ExecuteRunOutcome>
where
    S: RunRepository + RevisionRepository + ApprovalRepository,
    E: ActionExecutor,
{
    let run = store
        .get_run(&command.run_id)
        .map_err(maia_runtime::RuntimeError::from)?;
    let action = store
        .get_action_revision(run.action_id(), *run.action_version())
        .map_err(maia_runtime::RuntimeError::from)?;
    let current_action = store
        .get_current_action(run.action_id())
        .map_err(maia_runtime::RuntimeError::from)?;
    let approval = store
        .get_approval(run.approval_id())
        .map_err(maia_runtime::RuntimeError::from)?;

    let blockers = preflight_blockers(&run, &action, &current_action, &approval, &command.facts);
    if !blockers.is_empty() {
        return Ok(ExecuteRunOutcome::Blocked(blockers));
    }

    let claimed = transition_created_to_starting(store, &run, command.audit.claim.clone())?;
    let running = transition_starting_to_running(store, &claimed, command.audit.running.clone())?;
    let outcome = executor.execute(ExecutionRequest {
        run: &running,
        action: &action,
        approval: &approval,
    });
    let next_state = match outcome {
        ExecutorOutcome::EffectSucceeded => RunState::Completed,
        ExecutorOutcome::EffectFailedFinal => RunState::Failed,
        ExecutorOutcome::EffectFailedRetryable => RunState::RetryableError,
        ExecutorOutcome::OutcomeUnknown => RunState::OutcomeUnknown,
    };
    let persisted = transition_running_to(store, &running, next_state, command.audit.outcome)?;
    Ok(match outcome {
        ExecutorOutcome::EffectSucceeded => ExecuteRunOutcome::EffectSucceeded(Box::new(persisted)),
        ExecutorOutcome::EffectFailedFinal => {
            ExecuteRunOutcome::EffectFailedFinal(Box::new(persisted))
        }
        ExecutorOutcome::EffectFailedRetryable => {
            ExecuteRunOutcome::EffectFailedRetryable(Box::new(persisted))
        }
        ExecutorOutcome::OutcomeUnknown => ExecuteRunOutcome::OutcomeUnknown(Box::new(persisted)),
    })
}

fn preflight_blockers(
    run: &Run,
    action: &Action,
    current_action: &Action,
    approval: &Approval,
    facts: &ExecutionFacts,
) -> ExecutionBlockerSet {
    let mut blockers = Vec::new();
    if run.state() != &RunState::Created {
        blockers.push(ExecutionBlocker::RunNotCreated);
    }
    if current_action.version() != run.action_version()
        || current_action.id() != run.action_id()
        || action.version() != run.action_version()
    {
        blockers.push(ExecutionBlocker::CurrentActionRevisionMismatch);
    }
    if approval.id() != run.approval_id()
        || approval.version() != run.approval_version()
        || approval.action_id() != run.action_id()
        || approval.action_version() != run.action_version()
        || approval.action_hash() != run.action_hash()
        || run.requested_connector_id() != action.connector_profile_id()
    {
        blockers.push(ExecutionBlocker::ApprovalBindingMismatch);
    }

    let routing = check_routing(RoutingFacts {
        action,
        requested_connector_id: facts.routing.requested_connector_id.as_ref(),
        actual_connector_id: facts.routing.actual_connector_id.as_ref(),
        current_connector_binding_hash: facts.routing.current_connector_binding_hash.as_ref(),
        current_tool_definition_fingerprint: facts
            .routing
            .current_tool_definition_fingerprint
            .as_ref(),
        mcp_invocation: facts.routing.mcp_invocation,
        policy_routed_compliant: facts.routing.policy_routed_compliant,
        policy_routed_audited: facts.routing.policy_routed_audited,
    });
    let binding = BindingValidityFacts {
        action,
        approval,
        current_action_version: run.action_version(),
        current_action_hash: run.action_hash(),
        now: &facts.now,
        current_policy: facts.current_policy,
        approval_actor_authorized: facts.approval_actor_authorized,
    };
    let authorization = authorize_execution(CurrentAuthorizationFacts {
        action,
        approval,
        binding,
        freshness_ok: facts.freshness_ok,
        routing_ok: matches!(routing, RoutingDecision::Allowed),
        tool_definition_ok: facts.tool_definition_ok,
        execution_actor_authorized: facts.execution_actor_authorized,
    });
    let eligibility = evaluate_execution_eligibility(ExecutionEligibilityFacts {
        authorization: &authorization,
        run,
        expected_binding: RunBinding {
            action_id: run.action_id(),
            action_version: run.action_version(),
            action_hash: run.action_hash(),
            approval_id: run.approval_id(),
            approval_version: run.approval_version(),
        },
    });

    if !matches!(routing, RoutingDecision::Allowed) {
        blockers.push(ExecutionBlocker::Routing(routing));
    }
    if let AuthorizationDecision::Blocked(value) = authorization {
        blockers.push(ExecutionBlocker::Authorization(value));
    }
    if let ExecutionEligibilityDecision::Blocked(value) = eligibility {
        blockers.push(ExecutionBlocker::Eligibility(value));
    }
    ExecutionBlockerSet::new(blockers)
}

fn transition_created_to_starting<S>(
    store: &S,
    run: &Run,
    audit: AuditEventDraft,
) -> maia_runtime::RuntimeResult<Run>
where
    S: RunRepository,
{
    match transition_run(
        store,
        TransitionRunCommand {
            run_id: run.id().clone(),
            expected_version: *run.version(),
            expected_state: RunState::Created,
            next_state: RunState::Starting,
            evidence: None,
            audit,
        },
    )? {
        RunTransitionOutcome::Transitioned(value) => Ok(*value),
        RunTransitionOutcome::NeedsEvidence(_) | RunTransitionOutcome::Blocked { .. } => {
            Err(maia_runtime::RuntimeError::Conflict)
        }
    }
}

fn transition_starting_to_running<S>(
    store: &S,
    run: &Run,
    audit: AuditEventDraft,
) -> maia_runtime::RuntimeResult<Run>
where
    S: RunRepository,
{
    match transition_run(
        store,
        TransitionRunCommand {
            run_id: run.id().clone(),
            expected_version: *run.version(),
            expected_state: RunState::Starting,
            next_state: RunState::Running,
            evidence: None,
            audit,
        },
    )? {
        RunTransitionOutcome::Transitioned(value) => Ok(*value),
        RunTransitionOutcome::NeedsEvidence(_) | RunTransitionOutcome::Blocked { .. } => {
            Err(maia_runtime::RuntimeError::Conflict)
        }
    }
}

fn transition_running_to<S>(
    store: &S,
    run: &Run,
    next_state: RunState,
    audit: AuditEventDraft,
) -> maia_runtime::RuntimeResult<Run>
where
    S: RunRepository,
{
    match transition_run(
        store,
        TransitionRunCommand {
            run_id: run.id().clone(),
            expected_version: *run.version(),
            expected_state: RunState::Running,
            next_state,
            evidence: None,
            audit,
        },
    )? {
        RunTransitionOutcome::Transitioned(value) => Ok(*value),
        RunTransitionOutcome::NeedsEvidence(_) | RunTransitionOutcome::Blocked { .. } => {
            Err(maia_runtime::RuntimeError::Conflict)
        }
    }
}
