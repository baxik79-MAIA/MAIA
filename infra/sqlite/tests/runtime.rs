use maia_domain::*;
use maia_runtime::*;
use maia_sqlite::SqliteStore;
use maia_store::{
    ApprovalRepository, AuditRepository, EvidenceRecord, RevisionRepository, RunRepository,
    TaskRepository, audit_event,
};

const WS: &str = "01900000-0000-7000-8000-000000000000";
const TASK: &str = "01900000-0000-7000-8000-000000000001";
const PLAN: &str = "01900000-0000-7000-8000-000000000010";
const ACTION: &str = "01900000-0000-7000-8000-000000000011";
const APPROVAL: &str = "01900000-0000-7000-8000-000000000012";
const RUN: &str = "01900000-0000-7000-8000-000000000013";
const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SNAPSHOT: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const NOW: &str = "2026-09-11T12:34:56.789Z";

fn id<T: std::str::FromStr<Err = DomainError>>(value: &str) -> T {
    value.parse().unwrap()
}
fn now() -> Timestamp {
    id(NOW)
}
fn temp_path(label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "maia-runtime-{label}-{}-{}.db",
        std::process::id(),
        uuid_suffix()
    ));
    let _ = std::fs::remove_file(&path);
    path
}
fn uuid_suffix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}
fn task(state: AgentTaskState) -> AgentTask {
    AgentTask::new(
        id(TASK),
        Version::new(1).unwrap(),
        id(WS),
        NonEmptyString::new("runtime task").unwrap(),
        SurfaceId::new("test").unwrap(),
        None,
        state,
        PrivacyClass::LocalOnly,
        ActorRef::new("actor").unwrap(),
        now(),
    )
    .unwrap()
}
fn plan() -> ExecutionPlan {
    ExecutionPlan::new(
        id(PLAN),
        id(TASK),
        Version::new(1).unwrap(),
        RiskSummary::new([RiskClass::Read]),
        None,
        GateType::Allow,
        now(),
    )
    .unwrap()
}
fn action(version: u64) -> Action {
    Action::new(
        id(ACTION),
        id(PLAN),
        Ordinal::new(0).unwrap(),
        ActionType::new("test.read").unwrap(),
        None,
        RiskClass::Read,
        ActionState::Gated,
        OpaqueRef::new("input").unwrap(),
        vec![],
        None,
        Version::new(version).unwrap(),
        Version::new(1).unwrap(),
        id(HASH),
        CanonicalizerRef::new(
            NonEmptyString::new("test.v1").unwrap(),
            Version::new(1).unwrap(),
        )
        .unwrap(),
        ConnectorSelection::None,
        None,
        None,
    )
    .unwrap()
}
fn approval(
    state: ApprovalState,
    policy: PolicyDecision,
    version: u64,
    achieved: ApprovalAssurance,
) -> Approval {
    let human = matches!(state, ApprovalState::Approved | ApprovalState::Rejected);
    let required = policy
        .required_assurance()
        .unwrap_or(ApprovalAssurance::None);
    Approval::new(
        id(APPROVAL),
        id(TASK),
        id(ACTION),
        PolicyId::new("policy").unwrap(),
        state,
        now(),
        human.then_some(now()),
        human.then(|| ActorRef::new("actor").unwrap()),
        human.then_some("approved".to_owned()),
        id(HASH),
        Version::new(version).unwrap(),
        None,
        SurfaceId::new("test").unwrap(),
        human.then(|| SurfaceId::new("test").unwrap()),
        Version::new(1).unwrap(),
        id(SNAPSHOT),
        policy,
        required,
        achieved,
    )
    .unwrap()
}
fn event(number: u64, subject: &str) -> maia_store::AuditEventDraft {
    audit_event(
        id(&format!("01900000-0000-7000-8000-{number:012}")),
        id(WS),
        now(),
        Some(ActorRef::new("actor").unwrap()),
        AuditEventType::new("runtime.event").unwrap(),
        AuditSubjectType::new(subject).unwrap(),
        None,
        None,
        None,
        None,
    )
}
fn seed(path: &std::path::Path, state: AgentTaskState) -> SqliteStore {
    let store = SqliteStore::open(path, id(WS), now()).unwrap();
    store.insert_task(&task(state)).unwrap();
    store.insert_plan_revision(&plan()).unwrap();
    store
}
fn revise(store: &SqliteStore, policy: PolicyDecision, approval: Approval) -> Action {
    let current = action(1);
    let outcome = revise_action(
        store,
        ReviseActionCommand {
            action: current.clone(),
            action_hash: id(HASH),
            policy_layers: maia_policy::PolicyLayers::new(policy, policy, policy),
            approval: Some(approval),
            supersede_approval: None,
            audit: event(1, "action"),
        },
    )
    .unwrap();
    assert!(matches!(outcome, ActionRevisionOutcome::Created { .. }));
    current
}
fn run_facts(policy: PolicyDecision, action_version: u64) -> RunAuthorizationFacts {
    RunAuthorizationFacts {
        current_action_version: Version::new(action_version).unwrap(),
        current_action_hash: id(HASH),
        now: now(),
        current_policy: policy,
        approval_actor_authorized: true,
        execution_actor_authorized: true,
        freshness_ok: true,
        tool_definition_ok: true,
        routing: RoutingInput::default(),
    }
}

#[test]
fn allow_path_revises_action_creates_run_and_audits_every_mutation() {
    let path = temp_path("allow");
    let store = seed(&path, AgentTaskState::Queued);
    let action = revise(
        &store,
        PolicyDecision::Allow,
        approval(
            ApprovalState::NotRequired,
            PolicyDecision::Allow,
            1,
            ApprovalAssurance::None,
        ),
    );
    let result = prepare_run(
        &store,
        PrepareRunCommand {
            task_id: id(TASK),
            action,
            action_hash: id(HASH),
            approval: approval(
                ApprovalState::NotRequired,
                PolicyDecision::Allow,
                1,
                ApprovalAssurance::None,
            ),
            run_id: id(RUN),
            attempt: Attempt::new(1).unwrap(),
            requested_model_id: None,
            facts: run_facts(PolicyDecision::Allow, 1),
            audit: event(2, "run"),
        },
    )
    .unwrap();
    assert!(matches!(result, RunPreparationOutcome::Created(_)));
    assert_eq!(store.list_audit(&id(WS)).unwrap().len(), 2);
    store.verify_audit_chain(&id(WS)).unwrap();
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn confirm_path_blocks_then_decides_then_creates_run() {
    let path = temp_path("confirm");
    let store = seed(&path, AgentTaskState::Queued);
    let action = revise(
        &store,
        PolicyDecision::Confirm,
        approval(
            ApprovalState::Pending,
            PolicyDecision::Confirm,
            1,
            ApprovalAssurance::None,
        ),
    );
    let pending = approval(
        ApprovalState::Pending,
        PolicyDecision::Confirm,
        1,
        ApprovalAssurance::None,
    );
    let blocked = prepare_run(
        &store,
        PrepareRunCommand {
            task_id: id(TASK),
            action: action.clone(),
            action_hash: id(HASH),
            approval: pending,
            run_id: id(RUN),
            attempt: Attempt::new(1).unwrap(),
            requested_model_id: None,
            facts: run_facts(PolicyDecision::Confirm, 1),
            audit: event(3, "run"),
        },
    )
    .unwrap();
    assert!(matches!(blocked, RunPreparationOutcome::Blocked { .. }));
    let updated = decide_approval(
        &store,
        DecideApprovalCommand {
            approval_id: id(APPROVAL),
            expected_version: Version::new(1).unwrap(),
            expected_state: ApprovalState::Pending,
            candidate: approval(
                ApprovalState::Approved,
                PolicyDecision::Confirm,
                2,
                ApprovalAssurance::Confirm,
            ),
            current_action_version: Version::new(1).unwrap(),
            current_action_hash: id(HASH),
            current_policy: PolicyDecision::Confirm,
            now: now(),
            approval_actor_authorized: true,
            audit: event(4, "approval"),
        },
    )
    .unwrap();
    assert!(matches!(updated, ApprovalDecisionOutcome::Updated(_)));
    let approved = store.get_approval(&id(APPROVAL)).unwrap();
    let created = prepare_run(
        &store,
        PrepareRunCommand {
            task_id: id(TASK),
            action,
            action_hash: id(HASH),
            approval: approved,
            run_id: id(RUN),
            attempt: Attempt::new(1).unwrap(),
            requested_model_id: None,
            facts: run_facts(PolicyDecision::Confirm, 1),
            audit: event(5, "run"),
        },
    )
    .unwrap();
    assert!(matches!(created, RunPreparationOutcome::Created(_)));
    assert_eq!(store.list_audit(&id(WS)).unwrap().len(), 3);
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn elevated_assurance_is_required_for_elevated_policy() {
    let path = temp_path("elevated");
    let store = seed(&path, AgentTaskState::Queued);
    let action = revise(
        &store,
        PolicyDecision::ElevatedConfirm,
        approval(
            ApprovalState::Pending,
            PolicyDecision::ElevatedConfirm,
            1,
            ApprovalAssurance::None,
        ),
    );
    let insufficient = prepare_run(
        &store,
        PrepareRunCommand {
            task_id: id(TASK),
            action: action.clone(),
            action_hash: id(HASH),
            approval: approval(
                ApprovalState::Approved,
                PolicyDecision::Confirm,
                2,
                ApprovalAssurance::Confirm,
            ),
            run_id: id(RUN),
            attempt: Attempt::new(1).unwrap(),
            requested_model_id: None,
            facts: run_facts(PolicyDecision::ElevatedConfirm, 1),
            audit: event(17, "run"),
        },
    )
    .unwrap();
    assert!(matches!(
        insufficient,
        RunPreparationOutcome::Blocked { .. }
    ));
    let decided = decide_approval(
        &store,
        DecideApprovalCommand {
            approval_id: id(APPROVAL),
            expected_version: Version::new(1).unwrap(),
            expected_state: ApprovalState::Pending,
            candidate: approval(
                ApprovalState::Approved,
                PolicyDecision::ElevatedConfirm,
                2,
                ApprovalAssurance::ElevatedConfirm,
            ),
            current_action_version: Version::new(1).unwrap(),
            current_action_hash: id(HASH),
            current_policy: PolicyDecision::ElevatedConfirm,
            now: now(),
            approval_actor_authorized: true,
            audit: event(18, "approval"),
        },
    )
    .unwrap();
    assert!(matches!(decided, ApprovalDecisionOutcome::Updated(_)));
    let approved = store.get_approval(&id(APPROVAL)).unwrap();
    let allowed = prepare_run(
        &store,
        PrepareRunCommand {
            task_id: id(TASK),
            action,
            action_hash: id(HASH),
            approval: approved,
            run_id: id(RUN),
            attempt: Attempt::new(1).unwrap(),
            requested_model_id: None,
            facts: run_facts(PolicyDecision::ElevatedConfirm, 1),
            audit: event(19, "run"),
        },
    )
    .unwrap();
    assert!(matches!(allowed, RunPreparationOutcome::Created(_)));
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn stale_revision_and_freshness_mismatch_create_no_run() {
    let path = temp_path("stale");
    let store = seed(&path, AgentTaskState::Queued);
    let action_v1 = revise(
        &store,
        PolicyDecision::Allow,
        approval(
            ApprovalState::NotRequired,
            PolicyDecision::Allow,
            1,
            ApprovalAssurance::None,
        ),
    );
    let action_v2 = action(2);
    let denied_revision = revise_action(
        &store,
        ReviseActionCommand {
            action: action_v2,
            action_hash: id(HASH),
            policy_layers: maia_policy::PolicyLayers::new(
                PolicyDecision::Deny,
                PolicyDecision::Deny,
                PolicyDecision::Deny,
            ),
            approval: None,
            supersede_approval: None,
            audit: event(6, "action"),
        },
    )
    .unwrap();
    assert!(matches!(
        denied_revision,
        ActionRevisionOutcome::Blocked { .. }
    ));
    let stale = prepare_run(
        &store,
        PrepareRunCommand {
            task_id: id(TASK),
            action: action_v1.clone(),
            action_hash: id(HASH),
            approval: approval(
                ApprovalState::NotRequired,
                PolicyDecision::Allow,
                1,
                ApprovalAssurance::None,
            ),
            run_id: id(RUN),
            attempt: Attempt::new(1).unwrap(),
            requested_model_id: None,
            facts: run_facts(PolicyDecision::Allow, 2),
            audit: event(7, "run"),
        },
    );
    assert_eq!(stale, Err(RuntimeError::Conflict));
    let fresh_blocked = prepare_run(
        &store,
        PrepareRunCommand {
            task_id: id(TASK),
            action: action_v1,
            action_hash: id(HASH),
            approval: approval(
                ApprovalState::NotRequired,
                PolicyDecision::Allow,
                1,
                ApprovalAssurance::None,
            ),
            run_id: id(RUN),
            attempt: Attempt::new(1).unwrap(),
            requested_model_id: None,
            facts: RunAuthorizationFacts {
                freshness_ok: false,
                ..run_facts(PolicyDecision::Allow, 1)
            },
            audit: event(8, "run"),
        },
    )
    .unwrap();
    assert!(matches!(
        fresh_blocked,
        RunPreparationOutcome::Blocked { .. }
    ));
    assert_eq!(
        store.get_run(&id(RUN)),
        Err(maia_store::StoreError::NotFound)
    );
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn pause_request_is_atomic_and_blocks_new_run_afterwards() {
    let path = temp_path("pause");
    let store = seed(&path, AgentTaskState::Running);
    let action = revise(
        &store,
        PolicyDecision::Allow,
        approval(
            ApprovalState::NotRequired,
            PolicyDecision::Allow,
            1,
            ApprovalAssurance::None,
        ),
    );
    let paused = pause_task(
        &store,
        PauseTaskCommand {
            task_id: id(TASK),
            expected_version: Version::new(1).unwrap(),
            expected_state: AgentTaskState::Running,
            runs: vec![],
            audit: event(9, "task"),
        },
    )
    .unwrap();
    assert!(matches!(paused, PauseTaskOutcome::Requested(_)));
    let result = prepare_run(
        &store,
        PrepareRunCommand {
            task_id: id(TASK),
            action,
            action_hash: id(HASH),
            approval: approval(
                ApprovalState::NotRequired,
                PolicyDecision::Allow,
                1,
                ApprovalAssurance::None,
            ),
            run_id: id(RUN),
            attempt: Attempt::new(1).unwrap(),
            requested_model_id: None,
            facts: run_facts(PolicyDecision::Allow, 1),
            audit: event(10, "run"),
        },
    );
    assert_eq!(result, Err(RuntimeError::Conflict));
    assert_eq!(
        store.get_run(&id(RUN)),
        Err(maia_store::StoreError::NotFound)
    );
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn unresolved_reconciliation_is_blocked_without_retry() {
    let path = temp_path("reconciliation");
    let store = seed(&path, AgentTaskState::Queued);
    let action = revise(
        &store,
        PolicyDecision::Allow,
        approval(
            ApprovalState::NotRequired,
            PolicyDecision::Allow,
            1,
            ApprovalAssurance::None,
        ),
    );
    let created = match prepare_run(
        &store,
        PrepareRunCommand {
            task_id: id(TASK),
            action,
            action_hash: id(HASH),
            approval: approval(
                ApprovalState::NotRequired,
                PolicyDecision::Allow,
                1,
                ApprovalAssurance::None,
            ),
            run_id: id(RUN),
            attempt: Attempt::new(1).unwrap(),
            requested_model_id: None,
            facts: run_facts(PolicyDecision::Allow, 1),
            audit: event(11, "run"),
        },
    )
    .unwrap()
    {
        RunPreparationOutcome::Created(run) => *run,
        other => panic!("unexpected outcome: {other:?}"),
    };
    let starting = transition(&store, &created, RunState::Starting, None, 12);
    let running = transition(&store, &starting, RunState::Running, None, 13);
    let unknown = transition(&store, &running, RunState::OutcomeUnknown, None, 14);
    let reconciling = transition(&store, &unknown, RunState::Reconciling, None, 15);
    let evidence = ReconciliationEvidence::new(
        reconciling.id().clone(),
        reconciling.action_id().clone(),
        *reconciling.action_version(),
        reconciling.action_hash().clone(),
        OpaqueRef::new("effect").unwrap(),
        OpaqueRef::new("source").unwrap(),
        ReconciliationResult::Unresolved,
    )
    .unwrap();
    let blocked = transition_run(
        &store,
        TransitionRunCommand {
            run_id: reconciling.id().clone(),
            expected_version: *reconciling.version(),
            expected_state: *reconciling.state(),
            next_state: RunState::RetryableError,
            evidence: Some(EvidenceRecord::Reconciliation(evidence)),
            audit: event(16, "run_transition"),
        },
    )
    .unwrap();
    assert!(matches!(blocked, RunTransitionOutcome::Blocked { .. }));
    assert_eq!(
        store.get_run(&id(RUN)).unwrap().state(),
        &RunState::Reconciling
    );
    drop(store);
    let _ = std::fs::remove_file(path);
}

fn transition(
    store: &SqliteStore,
    current: &Run,
    next_state: RunState,
    evidence: Option<EvidenceRecord>,
    audit_number: u64,
) -> Run {
    match transition_run(
        store,
        TransitionRunCommand {
            run_id: current.id().clone(),
            expected_version: *current.version(),
            expected_state: *current.state(),
            next_state,
            evidence,
            audit: event(audit_number, "run_transition"),
        },
    )
    .unwrap()
    {
        RunTransitionOutcome::Transitioned(run) => *run,
        other => panic!("unexpected transition outcome: {other:?}"),
    }
}
