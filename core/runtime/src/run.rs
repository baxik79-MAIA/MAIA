use maia_domain::{
    Action, Approval, Attempt, ConnectorProfileId, ModelProfileId, PolicyDecision, Run, RunId,
    RunState, Sha256Hex, Timestamp, Version,
};
use maia_orchestrator::{
    ExecutionEligibilityDecision, ExecutionEligibilityFacts, FreshnessDecision,
    ReconciliationDecision, RoutingDecision, RoutingFacts, RunBinding, RunBuildError,
    build_run_spec, check_routing, create_run, evaluate_execution_eligibility, evaluate_freshness,
    evaluate_reconciliation,
};
use maia_policy::{
    AuthorizationBlockerSet, AuthorizationDecision, BindingValidityFacts,
    CurrentAuthorizationFacts, authorize_execution,
};
use maia_store::{
    AuditEventDraft, EvidenceRecord, RunCreationCommand, RunRepository, RunTransitionCommand,
};

use crate::error::{RuntimeError, RuntimeResult};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoutingInput {
    pub requested_connector_id: Option<ConnectorProfileId>,
    pub actual_connector_id: Option<ConnectorProfileId>,
    pub current_connector_binding_hash: Option<Sha256Hex>,
    pub current_tool_definition_fingerprint: Option<Sha256Hex>,
    pub mcp_invocation: bool,
    pub policy_routed_compliant: bool,
    pub policy_routed_audited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunAuthorizationFacts {
    pub current_action_version: Version,
    pub current_action_hash: Sha256Hex,
    pub now: Timestamp,
    pub current_policy: PolicyDecision,
    pub approval_actor_authorized: bool,
    pub execution_actor_authorized: bool,
    pub freshness_ok: bool,
    pub tool_definition_ok: bool,
    pub routing: RoutingInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareRunCommand {
    pub task_id: maia_domain::AgentTaskId,
    pub action: Action,
    pub action_hash: Sha256Hex,
    pub approval: Approval,
    pub run_id: RunId,
    pub attempt: Attempt,
    pub requested_model_id: Option<ModelProfileId>,
    pub facts: RunAuthorizationFacts,
    pub audit: AuditEventDraft,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunPreparationOutcome {
    Created(Box<Run>),
    Blocked {
        authorization: Option<AuthorizationBlockerSet>,
        routing: Option<RoutingDecision>,
        eligibility: Option<maia_orchestrator::ExecutionEligibilityBlockerSet>,
    },
}

/// Composes current policy, binding and routing facts, builds an in-memory
/// Created Run, then enters atomic T3 only when all pure gates allow it.
pub fn prepare_run<S>(store: &S, command: PrepareRunCommand) -> RuntimeResult<RunPreparationOutcome>
where
    S: RunRepository,
{
    command
        .action
        .validate()
        .map_err(RuntimeError::InvalidInput)?;
    if command.action.version() != &command.facts.current_action_version
        || command.action_hash != command.facts.current_action_hash
    {
        return Err(RuntimeError::Conflict);
    }
    let routing = check_routing(RoutingFacts {
        action: &command.action,
        requested_connector_id: command.facts.routing.requested_connector_id.as_ref(),
        actual_connector_id: command.facts.routing.actual_connector_id.as_ref(),
        current_connector_binding_hash: command
            .facts
            .routing
            .current_connector_binding_hash
            .as_ref(),
        current_tool_definition_fingerprint: command
            .facts
            .routing
            .current_tool_definition_fingerprint
            .as_ref(),
        mcp_invocation: command.facts.routing.mcp_invocation,
        policy_routed_compliant: command.facts.routing.policy_routed_compliant,
        policy_routed_audited: command.facts.routing.policy_routed_audited,
    });
    let binding = BindingValidityFacts {
        action: &command.action,
        approval: &command.approval,
        current_action_version: &command.facts.current_action_version,
        current_action_hash: &command.facts.current_action_hash,
        now: &command.facts.now,
        current_policy: command.facts.current_policy,
        approval_actor_authorized: command.facts.approval_actor_authorized,
    };
    let authorization = authorize_execution(CurrentAuthorizationFacts {
        action: &command.action,
        approval: &command.approval,
        binding,
        freshness_ok: command.facts.freshness_ok,
        routing_ok: matches!(routing, RoutingDecision::Allowed),
        tool_definition_ok: command.facts.tool_definition_ok,
        execution_actor_authorized: command.facts.execution_actor_authorized,
    });
    let run_spec = match build_run_spec(
        command.run_id,
        &command.action,
        command.action_hash.clone(),
        &command.approval,
        command.attempt,
        command.requested_model_id.clone(),
    ) {
        Ok(value) => value,
        Err(RunBuildError::Domain(error)) => return Err(RuntimeError::InvalidInput(error)),
        Err(RunBuildError::ApprovalDoesNotBindAction) => return Err(RuntimeError::Conflict),
    };
    let run = create_run(run_spec).map_err(|error| match error {
        RunBuildError::Domain(value) => RuntimeError::InvalidInput(value),
        RunBuildError::ApprovalDoesNotBindAction => RuntimeError::Conflict,
    })?;
    let binding_for_eligibility = RunBinding {
        action_id: run.action_id(),
        action_version: run.action_version(),
        action_hash: run.action_hash(),
        approval_id: run.approval_id(),
        approval_version: run.approval_version(),
    };
    let eligibility = evaluate_execution_eligibility(ExecutionEligibilityFacts {
        authorization: &authorization,
        run: &run,
        expected_binding: binding_for_eligibility,
    });
    if !matches!(authorization, AuthorizationDecision::Allowed)
        || !matches!(eligibility, ExecutionEligibilityDecision::Allowed)
    {
        let authorization = match authorization {
            AuthorizationDecision::Allowed => None,
            AuthorizationDecision::Blocked(value) => Some(value),
        };
        let eligibility = match eligibility {
            ExecutionEligibilityDecision::Allowed => None,
            ExecutionEligibilityDecision::Blocked(value) => Some(value),
        };
        return Ok(RunPreparationOutcome::Blocked {
            authorization,
            routing: (!matches!(routing, RoutingDecision::Allowed)).then_some(routing),
            eligibility,
        });
    }
    let stored = store
        .create_run(&RunCreationCommand {
            task_id: command.task_id,
            run,
            current_authorization_valid: true,
            audit: command.audit,
        })
        .map_err(RuntimeError::from)?;
    Ok(RunPreparationOutcome::Created(Box::new(stored)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionRunCommand {
    pub run_id: RunId,
    pub expected_version: Version,
    pub expected_state: RunState,
    pub next_state: RunState,
    pub evidence: Option<EvidenceRecord>,
    pub audit: AuditEventDraft,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunTransitionOutcome {
    Transitioned(Box<Run>),
    NeedsEvidence(ReconciliationDecision),
    Blocked {
        freshness: Option<FreshnessDecision>,
        reconciliation: Option<ReconciliationDecision>,
    },
}

/// Applies pure freshness/reconciliation gates and legal Run FSM transition,
/// then delegates CAS, evidence append and audit to atomic T4.
pub fn transition_run<S>(
    store: &S,
    command: TransitionRunCommand,
) -> RuntimeResult<RunTransitionOutcome>
where
    S: RunRepository,
{
    let current = store.get_run(&command.run_id).map_err(RuntimeError::from)?;
    if current.version() != &command.expected_version || current.state() != &command.expected_state
    {
        return Err(RuntimeError::Conflict);
    }
    let mut freshness = None;
    let mut reconciliation = None;
    match command.evidence.as_ref() {
        Some(EvidenceRecord::Freshness(value)) => {
            let decision = evaluate_freshness(&current, value);
            if !matches!(decision, FreshnessDecision::Allowed) {
                freshness = Some(decision);
            }
        }
        Some(EvidenceRecord::Reconciliation(value)) => {
            let decision = evaluate_reconciliation(&current, Some(value));
            if !reconciliation_target_matches(&decision, command.next_state) {
                reconciliation = Some(decision);
            }
        }
        None if current.state() == &RunState::OutcomeUnknown
            && command.next_state == RunState::Reconciling => {}
        None if current.state() == &RunState::Reconciling => {
            let decision = evaluate_reconciliation(&current, None);
            return Ok(RunTransitionOutcome::NeedsEvidence(decision));
        }
        None => {}
    }
    if freshness.is_some() || reconciliation.is_some() {
        return Ok(RunTransitionOutcome::Blocked {
            freshness,
            reconciliation,
        });
    }
    let mut candidate = current.clone();
    candidate
        .transition_to(command.next_state)
        .map_err(RuntimeError::InvalidInput)?;
    let stored = store
        .transition_run(&RunTransitionCommand {
            run_id: command.run_id,
            expected_version: command.expected_version,
            expected_state: command.expected_state,
            candidate,
            evidence: command.evidence,
            audit: command.audit,
        })
        .map_err(RuntimeError::from)?;
    Ok(RunTransitionOutcome::Transitioned(Box::new(stored)))
}

fn reconciliation_target_matches(decision: &ReconciliationDecision, target: RunState) -> bool {
    match decision {
        ReconciliationDecision::EffectConfirmed { next_state }
        | ReconciliationDecision::EffectNotExecuted { next_state }
        | ReconciliationDecision::Unresolved { next_state } => *next_state == target,
        ReconciliationDecision::NeedsEvidence | ReconciliationDecision::Blocked(_) => false,
    }
}
