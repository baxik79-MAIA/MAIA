use maia_domain::*;
use maia_orchestrator::*;
use maia_policy::AuthorizationDecision;

const UUID: &str = "01900000-0000-7000-8000-000000000000";
const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn ts(value: &str) -> Timestamp {
    Timestamp::new(value).unwrap()
}
fn action(
    selection: ConnectorSelection,
    connector: Option<ConnectorProfileId>,
    connector_hash: Option<Sha256Hex>,
    risk: RiskClass,
    tool: Option<Sha256Hex>,
) -> Action {
    Action::new(
        UUID.parse().unwrap(),
        UUID.parse().unwrap(),
        Ordinal::new(0).unwrap(),
        ActionType::new("mail.send").unwrap(),
        connector,
        risk,
        ActionState::Planned,
        OpaqueRef::new("input").unwrap(),
        vec![],
        None,
        Version::new(1).unwrap(),
        Version::new(1).unwrap(),
        Sha256Hex::new(HASH).unwrap(),
        CanonicalizerRef::new(
            NonEmptyString::new("test").unwrap(),
            Version::new(1).unwrap(),
        )
        .unwrap(),
        selection,
        connector_hash,
        tool,
    )
    .unwrap()
}
fn approval(action: &Action, hash: Sha256Hex) -> Approval {
    Approval::new(
        UUID.parse().unwrap(),
        UUID.parse().unwrap(),
        action.id().clone(),
        PolicyId::new("policy").unwrap(),
        ApprovalState::Approved,
        ts("2026-09-11T12:00:00.000Z"),
        Some(ts("2026-09-11T12:01:00.000Z")),
        Some(ActorRef::new("actor").unwrap()),
        None,
        hash,
        Version::new(1).unwrap(),
        None,
        SurfaceId::new("desktop").unwrap(),
        Some(SurfaceId::new("desktop").unwrap()),
        *action.version(),
        Sha256Hex::new(HASH).unwrap(),
        PolicyDecision::Confirm,
        ApprovalAssurance::Confirm,
        ApprovalAssurance::Confirm,
    )
    .unwrap()
}
fn run(action: &Action, approval: &Approval, state: RunState) -> Run {
    Run::new(
        UUID.parse().unwrap(),
        Version::new(1).unwrap(),
        action.id().clone(),
        Attempt::new(1).unwrap(),
        None,
        None,
        None,
        None,
        state,
        state.outcome_certainty(),
        None,
        None,
        None,
        *action.version(),
        Sha256Hex::new(HASH).unwrap(),
        approval.id().clone(),
        *approval.version(),
    )
    .unwrap()
}

#[test]
fn run_creation_binds_exact_revision_and_approval() {
    let action = action(ConnectorSelection::None, None, None, RiskClass::Read, None);
    let hash = Sha256Hex::new(HASH).unwrap();
    let approval = approval(&action, hash.clone());
    let spec = build_run_spec(
        UUID.parse().unwrap(),
        &action,
        hash.clone(),
        &approval,
        Attempt::new(1).unwrap(),
        None,
    )
    .unwrap();
    let run = create_run(spec.clone()).unwrap();
    assert_eq!(*run.state(), RunState::Created);
    assert!(validate_run_binding(
        &run,
        RunBinding {
            action_id: &action.id().clone(),
            action_version: action.version(),
            action_hash: &hash,
            approval_id: approval.id(),
            approval_version: approval.version(),
        }
    ));
}

#[test]
fn attempts_reset_for_new_revision_and_overflow_fails_closed() {
    assert_eq!(next_attempt(None, false).unwrap().get(), 1);
    assert_eq!(
        next_attempt(Some(Attempt::new(2).unwrap()), true)
            .unwrap()
            .get(),
        3
    );
    assert_eq!(
        next_attempt(Some(Attempt::new(99).unwrap()), false)
            .unwrap()
            .get(),
        1
    );
    assert_eq!(
        next_attempt(Some(Attempt::new(u32::MAX).unwrap()), true),
        Err(DomainError::AttemptOverflow)
    );
}

#[test]
fn retry_requires_retryable_state_and_resolved_prior_effect() {
    let decision = evaluate_retry(
        RetryFacts {
            prior_state: RunState::RetryableError,
            current_policy_allows: true,
            current_execution_checks: true,
            prior_effect_resolved: true,
        },
        Attempt::new(1).unwrap(),
    );
    assert_eq!(
        decision,
        RetryDecision::Allowed {
            attempt: Attempt::new(2).unwrap()
        }
    );
    let blocked = evaluate_retry(
        RetryFacts {
            prior_state: RunState::OutcomeUnknown,
            current_policy_allows: false,
            current_execution_checks: false,
            prior_effect_resolved: false,
        },
        Attempt::new(1).unwrap(),
    );
    assert!(matches!(blocked, RetryDecision::Blocked(_)));
}

#[test]
fn reconciliation_and_freshness_are_bound_to_one_run() {
    let action = action(ConnectorSelection::None, None, None, RiskClass::Read, None);
    let hash = Sha256Hex::new(HASH).unwrap();
    let approval = approval(&action, hash.clone());
    let unknown = run(&action, &approval, RunState::OutcomeUnknown);
    let evidence = ReconciliationEvidence::new(
        unknown.id().clone(),
        unknown.action_id().clone(),
        *unknown.action_version(),
        unknown.action_hash().clone(),
        OpaqueRef::new("effect").unwrap(),
        OpaqueRef::new("source").unwrap(),
        ReconciliationResult::EffectNotExecuted,
    )
    .unwrap();
    assert_eq!(
        evaluate_reconciliation(&unknown, Some(&evidence)),
        ReconciliationDecision::EffectNotExecuted {
            next_state: RunState::RetryableError
        }
    );
    assert_eq!(
        evaluate_reconciliation(&unknown, None),
        ReconciliationDecision::NeedsEvidence
    );
    let precondition = SourcePrecondition::new(
        OpaqueRef::new("source").unwrap(),
        OpaqueRef::new("v1").unwrap(),
        hash.clone(),
    )
    .unwrap();
    let fresh = FreshnessEvidence::new(
        unknown.id().clone(),
        unknown.action_id().clone(),
        *unknown.action_version(),
        unknown.action_hash().clone(),
        FreshnessEnforcement::CompareBeforeExecute,
        precondition.clone(),
        Some(OpaqueRef::new("v1").unwrap()),
        Some(hash.clone()),
        FreshnessResult::Matched,
    )
    .unwrap();
    assert_eq!(
        evaluate_freshness(&unknown, &fresh),
        FreshnessDecision::Allowed
    );
    let mismatched = FreshnessEvidence::new(
        unknown.id().clone(),
        unknown.action_id().clone(),
        *unknown.action_version(),
        unknown.action_hash().clone(),
        FreshnessEnforcement::CompareBeforeExecute,
        precondition,
        Some(OpaqueRef::new("v2").unwrap()),
        Some(Sha256Hex::new("b".repeat(64)).unwrap()),
        FreshnessResult::Mismatched,
    )
    .unwrap();
    assert!(matches!(
        evaluate_freshness(&unknown, &mismatched),
        FreshnessDecision::Blocked(_)
    ));
}

#[test]
fn fixed_and_mcp_routing_require_exact_current_bindings() {
    let connector: ConnectorProfileId = UUID.parse().unwrap();
    let binding_hash = Sha256Hex::new(HASH).unwrap();
    let fixed_action = action(
        ConnectorSelection::Fixed,
        Some(connector.clone()),
        Some(binding_hash.clone()),
        RiskClass::Read,
        None,
    );
    assert_eq!(
        check_routing(RoutingFacts {
            action: &fixed_action,
            requested_connector_id: Some(&connector),
            actual_connector_id: Some(&connector),
            current_connector_binding_hash: Some(&binding_hash),
            current_tool_definition_fingerprint: None,
            mcp_invocation: false,
            policy_routed_compliant: false,
            policy_routed_audited: false,
        }),
        RoutingDecision::Allowed
    );
    let fingerprint = Sha256Hex::new("b".repeat(64)).unwrap();
    let mcp_action = action(
        ConnectorSelection::None,
        None,
        None,
        RiskClass::Read,
        Some(fingerprint.clone()),
    );
    assert!(matches!(
        check_routing(RoutingFacts {
            action: &mcp_action,
            requested_connector_id: None,
            actual_connector_id: None,
            current_connector_binding_hash: None,
            current_tool_definition_fingerprint: None,
            mcp_invocation: true,
            policy_routed_compliant: false,
            policy_routed_audited: false,
        }),
        RoutingDecision::Blocked(_)
    ));
}

#[test]
fn pause_is_quiescent_and_counts_superseded_in_flight_runs() {
    let action = action(ConnectorSelection::None, None, None, RiskClass::Read, None);
    let approval = approval(&action, Sha256Hex::new(HASH).unwrap());
    let in_flight = run(&action, &approval, RunState::Running);
    assert_eq!(
        evaluate_pause(PauseFacts {
            task_state: AgentTaskState::Running,
            pause_requested: true,
            runs: std::slice::from_ref(&in_flight),
            proposed_run_state: None
        }),
        PauseDecision::TransitionToPausing
    );
    assert_eq!(
        evaluate_pause(PauseFacts {
            task_state: AgentTaskState::Pausing,
            pause_requested: true,
            runs: &[in_flight],
            proposed_run_state: None
        }),
        PauseDecision::WaitingForQuiescence
    );
    let no_runs: Vec<Run> = vec![];
    assert_eq!(
        evaluate_pause(PauseFacts {
            task_state: AgentTaskState::Pausing,
            pause_requested: true,
            runs: &no_runs,
            proposed_run_state: None
        }),
        PauseDecision::CanEnterPaused
    );
    assert!(matches!(
        evaluate_pause(PauseFacts {
            task_state: AgentTaskState::Paused,
            pause_requested: true,
            runs: &no_runs,
            proposed_run_state: Some(RunState::Created)
        }),
        PauseDecision::Blocked(_)
    ));
}

#[test]
fn eligibility_requires_authorization_exact_binding_and_created_run() {
    let action = action(ConnectorSelection::None, None, None, RiskClass::Read, None);
    let hash = Sha256Hex::new(HASH).unwrap();
    let approval = approval(&action, hash.clone());
    let created = run(&action, &approval, RunState::Created);
    let binding_action_id = action.id().clone();
    let authorization = AuthorizationDecision::Allowed;
    let facts = ExecutionEligibilityFacts {
        authorization: &authorization,
        run: &created,
        expected_binding: RunBinding {
            action_id: &binding_action_id,
            action_version: action.version(),
            action_hash: &hash,
            approval_id: approval.id(),
            approval_version: approval.version(),
        },
    };
    assert_eq!(
        evaluate_execution_eligibility(facts),
        ExecutionEligibilityDecision::Allowed
    );
}
