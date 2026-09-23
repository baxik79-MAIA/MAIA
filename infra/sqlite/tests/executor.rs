use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::thread;

use maia_domain::*;
use maia_executor::testing::FakeExecutor;
use maia_executor::{
    ActionExecutor, ExecuteRunCommand, ExecuteRunOutcome, ExecutionAudit, ExecutionFacts,
    ExecutorOutcome, execute_run, recover_incomplete_runs,
};
use maia_policy::PolicyLayers;
use maia_runtime::{
    ActionRevisionOutcome, PrepareRunCommand, ReviseActionCommand, RoutingInput,
    RunAuthorizationFacts, prepare_run, revise_action,
};
use maia_sqlite::SqliteStore;
use maia_store::{
    ApprovalDecisionCommand, ApprovalRepository, AuditRepository, RevisionRepository,
    RunRepository, TaskRepository, audit_event,
};

const WS: &str = "01900000-0000-7000-8000-000000001000";
const TASK: &str = "01900000-0000-7000-8000-000000001001";
const PLAN: &str = "01900000-0000-7000-8000-000000001010";
const ACTION: &str = "01900000-0000-7000-8000-000000001011";
const APPROVAL: &str = "01900000-0000-7000-8000-000000001012";
const APPROVAL_V2: &str = "01900000-0000-7000-8000-000000001014";
const RUN: &str = "01900000-0000-7000-8000-000000001013";
const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HASH_V2: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
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
        "maia-executor-{label}-{}-{}.db",
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
        NonEmptyString::new("executor task").unwrap(),
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
fn action(version: u64, hash: &str) -> Action {
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
        id(hash),
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
    approval_id: &str,
    state: ApprovalState,
    policy: PolicyDecision,
    version: u64,
    action_version: u64,
    action_hash: &str,
    achieved: ApprovalAssurance,
) -> Approval {
    let human = matches!(state, ApprovalState::Approved | ApprovalState::Rejected);
    let required = policy
        .required_assurance()
        .unwrap_or(ApprovalAssurance::None);
    Approval::new(
        id(approval_id),
        id(TASK),
        id(ACTION),
        PolicyId::new("policy").unwrap(),
        state,
        now(),
        human.then_some(now()),
        human.then(|| ActorRef::new("actor").unwrap()),
        human.then_some("approved".to_owned()),
        id(action_hash),
        Version::new(version).unwrap(),
        None,
        SurfaceId::new("test").unwrap(),
        human.then(|| SurfaceId::new("test").unwrap()),
        Version::new(action_version).unwrap(),
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
        AuditEventType::new("executor.event").unwrap(),
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
fn create_allow_run(path: &std::path::Path, task_state: AgentTaskState) -> SqliteStore {
    let store = seed(path, task_state);
    let action_value = action(1, HASH);
    let approval_value = approval(
        APPROVAL,
        ApprovalState::NotRequired,
        PolicyDecision::Allow,
        1,
        1,
        HASH,
        ApprovalAssurance::None,
    );
    let revised = revise_action(
        &store,
        ReviseActionCommand {
            action: action_value.clone(),
            action_hash: id(HASH),
            policy_layers: PolicyLayers::new(
                PolicyDecision::Allow,
                PolicyDecision::Allow,
                PolicyDecision::Allow,
            ),
            approval: Some(approval_value.clone()),
            supersede_approval: None,
            audit: event(1, "action"),
        },
    )
    .unwrap();
    assert!(matches!(revised, ActionRevisionOutcome::Created { .. }));
    let prepared = prepare_run(
        &store,
        PrepareRunCommand {
            task_id: id(TASK),
            action: action_value,
            action_hash: id(HASH),
            approval: approval_value,
            run_id: id(RUN),
            attempt: Attempt::new(1).unwrap(),
            requested_model_id: None,
            facts: RunAuthorizationFacts {
                current_action_version: Version::new(1).unwrap(),
                current_action_hash: id(HASH),
                now: now(),
                current_policy: PolicyDecision::Allow,
                approval_actor_authorized: true,
                execution_actor_authorized: true,
                freshness_ok: true,
                tool_definition_ok: true,
                routing: RoutingInput::default(),
            },
            audit: event(2, "run"),
        },
    )
    .unwrap();
    assert!(matches!(
        prepared,
        maia_runtime::RunPreparationOutcome::Created(_)
    ));
    store
}
fn execute_command(run_id: RunId, base: u64) -> ExecuteRunCommand {
    ExecuteRunCommand {
        run_id,
        facts: ExecutionFacts {
            now: now(),
            current_policy: PolicyDecision::Allow,
            approval_actor_authorized: true,
            execution_actor_authorized: true,
            freshness_ok: true,
            tool_definition_ok: true,
            routing: RoutingInput::default(),
        },
        audit: ExecutionAudit {
            claim: event(base, "run_claim"),
            running: event(base + 1, "run_running"),
            outcome: event(base + 2, "run_outcome"),
        },
    }
}

#[test]
fn success_persists_completed_run_and_audit() {
    let path = temp_path("success");
    let store = create_allow_run(&path, AgentTaskState::Queued);
    let executor = FakeExecutor::new(ExecutorOutcome::EffectSucceeded);
    let result = execute_run(&store, &executor, execute_command(id(RUN), 3)).unwrap();
    assert!(
        matches!(result, ExecuteRunOutcome::EffectSucceeded(run) if run.state() == &RunState::Completed)
    );
    assert_eq!(executor.invocation_count(), 1);
    assert_eq!(store.list_audit(&id(WS)).unwrap().len(), 5);
    store.verify_audit_chain(&id(WS)).unwrap();
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn final_and_retryable_failures_are_distinct_terminal_states() {
    for (label, outcome, state) in [
        (
            "final",
            ExecutorOutcome::EffectFailedFinal,
            RunState::Failed,
        ),
        (
            "retryable",
            ExecutorOutcome::EffectFailedRetryable,
            RunState::RetryableError,
        ),
    ] {
        let path = temp_path(label);
        let store = create_allow_run(&path, AgentTaskState::Queued);
        let executor = FakeExecutor::new(outcome);
        let result = execute_run(&store, &executor, execute_command(id(RUN), 3)).unwrap();
        assert!(
            matches!(result, ExecuteRunOutcome::EffectFailedFinal(ref run) if *run.state() == state)
                || matches!(result, ExecuteRunOutcome::EffectFailedRetryable(ref run) if *run.state() == state)
        );
        assert_eq!(executor.invocation_count(), 1);
        drop(store);
        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn unknown_outcome_is_persisted_without_retry_and_recovery_is_observational() {
    let path = temp_path("unknown");
    let store = create_allow_run(&path, AgentTaskState::Queued);
    let executor = FakeExecutor::new(ExecutorOutcome::OutcomeUnknown);
    let result = execute_run(&store, &executor, execute_command(id(RUN), 3)).unwrap();
    assert!(
        matches!(result, ExecuteRunOutcome::OutcomeUnknown(run) if run.state() == &RunState::OutcomeUnknown)
    );
    let run = store.get_run(&id(RUN)).unwrap();
    let candidates = recover_incomplete_runs(std::slice::from_ref(&run));
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].state, RunState::OutcomeUnknown);
    assert_eq!(executor.invocation_count(), 1);
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn panic_leaves_running_run_for_reconciliation() {
    let path = temp_path("panic");
    let store = create_allow_run(&path, AgentTaskState::Queued);
    let executor = FakeExecutor::panicking();
    let result = catch_unwind(AssertUnwindSafe(|| {
        execute_run(&store, &executor, execute_command(id(RUN), 3))
    }));
    assert!(result.is_err());
    let run = store.get_run(&id(RUN)).unwrap();
    assert_eq!(*run.state(), RunState::Running);
    assert_eq!(recover_incomplete_runs(std::slice::from_ref(&run)).len(), 1);
    assert_eq!(executor.invocation_count(), 1);
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn stale_current_action_blocks_executor() {
    let path = temp_path("stale-action");
    let store = create_allow_run(&path, AgentTaskState::Queued);
    let next_action = action(2, HASH_V2);
    let next_approval = approval(
        APPROVAL_V2,
        ApprovalState::NotRequired,
        PolicyDecision::Allow,
        1,
        2,
        HASH_V2,
        ApprovalAssurance::None,
    );
    revise_action(
        &store,
        ReviseActionCommand {
            action: next_action,
            action_hash: id(HASH_V2),
            policy_layers: PolicyLayers::new(
                PolicyDecision::Allow,
                PolicyDecision::Allow,
                PolicyDecision::Allow,
            ),
            approval: Some(next_approval),
            supersede_approval: None,
            audit: event(3, "action_v2"),
        },
    )
    .unwrap();
    let executor = FakeExecutor::new(ExecutorOutcome::EffectSucceeded);
    let result = execute_run(&store, &executor, execute_command(id(RUN), 4)).unwrap();
    assert!(matches!(result, ExecuteRunOutcome::Blocked(_)));
    assert_eq!(executor.invocation_count(), 0);
    assert_eq!(*store.get_run(&id(RUN)).unwrap().state(), RunState::Created);
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn revoked_or_stale_approval_blocks_executor() {
    let path = temp_path("stale-approval");
    let store = seed(&path, AgentTaskState::Queued);
    let action_value = action(1, HASH);
    let pending = approval(
        APPROVAL,
        ApprovalState::Pending,
        PolicyDecision::Confirm,
        1,
        1,
        HASH,
        ApprovalAssurance::None,
    );
    revise_action(
        &store,
        ReviseActionCommand {
            action: action_value.clone(),
            action_hash: id(HASH),
            policy_layers: PolicyLayers::new(
                PolicyDecision::Confirm,
                PolicyDecision::Confirm,
                PolicyDecision::Confirm,
            ),
            approval: Some(pending.clone()),
            supersede_approval: None,
            audit: event(1, "action"),
        },
    )
    .unwrap();
    let approved = maia_runtime::decide_approval(
        &store,
        maia_runtime::DecideApprovalCommand {
            approval_id: id(APPROVAL),
            expected_version: Version::new(1).unwrap(),
            expected_state: ApprovalState::Pending,
            candidate: approval(
                APPROVAL,
                ApprovalState::Approved,
                PolicyDecision::Confirm,
                2,
                1,
                HASH,
                ApprovalAssurance::Confirm,
            ),
            current_action_version: Version::new(1).unwrap(),
            current_action_hash: id(HASH),
            current_policy: PolicyDecision::Confirm,
            now: now(),
            approval_actor_authorized: true,
            audit: event(2, "approval"),
        },
    )
    .unwrap();
    assert!(matches!(
        approved,
        maia_runtime::ApprovalDecisionOutcome::Updated(_)
    ));
    let prepared = prepare_run(
        &store,
        PrepareRunCommand {
            task_id: id(TASK),
            action: action_value,
            action_hash: id(HASH),
            approval: approval(
                APPROVAL,
                ApprovalState::Approved,
                PolicyDecision::Confirm,
                2,
                1,
                HASH,
                ApprovalAssurance::Confirm,
            ),
            run_id: id(RUN),
            attempt: Attempt::new(1).unwrap(),
            requested_model_id: None,
            facts: RunAuthorizationFacts {
                current_action_version: Version::new(1).unwrap(),
                current_action_hash: id(HASH),
                now: now(),
                current_policy: PolicyDecision::Confirm,
                approval_actor_authorized: true,
                execution_actor_authorized: true,
                freshness_ok: true,
                tool_definition_ok: true,
                routing: RoutingInput::default(),
            },
            audit: event(3, "run"),
        },
    )
    .unwrap();
    assert!(matches!(
        prepared,
        maia_runtime::RunPreparationOutcome::Created(_)
    ));
    let current = store.get_approval(&id(APPROVAL)).unwrap();
    let mut revoked = current.clone();
    revoked.transition_to(ApprovalState::Revoked).unwrap();
    store
        .decide_approval(&ApprovalDecisionCommand {
            approval_id: id(APPROVAL),
            expected_version: *current.version(),
            expected_state: *current.state(),
            candidate: revoked,
            audit: event(4, "approval_revoked"),
        })
        .unwrap();
    let executor = FakeExecutor::new(ExecutorOutcome::EffectSucceeded);
    let result = execute_run(&store, &executor, execute_command(id(RUN), 5)).unwrap();
    assert!(matches!(result, ExecuteRunOutcome::Blocked(_)));
    assert_eq!(executor.invocation_count(), 0);
    assert_eq!(*store.get_run(&id(RUN)).unwrap().state(), RunState::Created);
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn pausing_task_rejects_created_to_starting_without_executor_call() {
    let path = temp_path("pause");
    let store = create_allow_run(&path, AgentTaskState::Running);
    let task_value = store.get_task(&id(TASK)).unwrap();
    let paused = maia_runtime::pause_task(
        &store,
        maia_runtime::PauseTaskCommand {
            task_id: id(TASK),
            expected_version: *task_value.version(),
            expected_state: AgentTaskState::Running,
            runs: vec![],
            audit: event(3, "pause"),
        },
    )
    .unwrap();
    assert!(matches!(
        paused,
        maia_runtime::PauseTaskOutcome::Requested(_)
    ));
    let executor = FakeExecutor::new(ExecutorOutcome::EffectSucceeded);
    let result = execute_run(&store, &executor, execute_command(id(RUN), 4));
    assert!(matches!(result, Err(maia_runtime::RuntimeError::Conflict)));
    assert_eq!(executor.invocation_count(), 0);
    assert_eq!(*store.get_run(&id(RUN)).unwrap().state(), RunState::Created);
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn double_execute_has_at_most_one_executor_invocation() {
    let path = temp_path("double");
    let store = Arc::new(create_allow_run(&path, AgentTaskState::Queued));
    let executor = Arc::new(FakeExecutor::new(ExecutorOutcome::EffectSucceeded));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let store = Arc::clone(&store);
        let executor = Arc::clone(&executor);
        handles.push(thread::spawn(move || {
            execute_run(&*store, &*executor, execute_command(id(RUN), 3))
        }));
    }
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(executor.invocation_count(), 1);
    assert!(
        results
            .iter()
            .any(|result| matches!(result, Ok(ExecuteRunOutcome::EffectSucceeded(_))))
    );
    assert_eq!(
        *store.get_run(&id(RUN)).unwrap().state(),
        RunState::Completed
    );
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[allow(dead_code)]
fn _executor_trait_is_object_safe(executor: &dyn ActionExecutor) {
    let _ = executor;
}
