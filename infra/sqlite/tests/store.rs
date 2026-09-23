use std::{sync::Arc, thread};

use maia_domain::*;
use maia_sqlite::SqliteStore;
use maia_store::{
    ApprovalDecisionCommand, ApprovalRepository, AuditRepository, AuthorityStore,
    RevisionRepository, RunCreationCommand, RunRepository, StoreError, TaskPauseCommand,
    TaskRepository, audit_event, compute_audit_hash,
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
fn temp_path(label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "maia-{label}-{}-{}.db",
        std::process::id(),
        uuid_suffix()
    ));
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}.authority.lock", path.display()));
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    let _ = std::fs::remove_file(path.with_extension("pre-migration.bak"));
    path
}
fn uuid_suffix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}
fn now() -> Timestamp {
    id(NOW)
}

fn task(state: AgentTaskState) -> AgentTask {
    AgentTask::new(
        id(TASK),
        Version::new(1).unwrap(),
        id(WS),
        NonEmptyString::new("persisted task").unwrap(),
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
fn action() -> Action {
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
        Version::new(1).unwrap(),
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
fn approval(state: ApprovalState) -> Approval {
    Approval::new(
        id(APPROVAL),
        id(TASK),
        id(ACTION),
        PolicyId::new("policy").unwrap(),
        state,
        now(),
        (state == ApprovalState::Approved).then_some(now()),
        (state == ApprovalState::Approved).then(|| ActorRef::new("actor").unwrap()),
        (state == ApprovalState::Approved).then(|| "approved".to_owned()),
        id(HASH),
        Version::new(1).unwrap(),
        None,
        SurfaceId::new("test").unwrap(),
        (state == ApprovalState::Approved).then(|| SurfaceId::new("test").unwrap()),
        Version::new(1).unwrap(),
        id(SNAPSHOT),
        PolicyDecision::Allow,
        ApprovalAssurance::None,
        ApprovalAssurance::None,
    )
    .unwrap()
}
fn pending_approval() -> Approval {
    Approval::new(
        id(APPROVAL),
        id(TASK),
        id(ACTION),
        PolicyId::new("policy").unwrap(),
        ApprovalState::Pending,
        now(),
        None,
        None,
        None,
        id(HASH),
        Version::new(1).unwrap(),
        None,
        SurfaceId::new("test").unwrap(),
        None,
        Version::new(1).unwrap(),
        id(SNAPSHOT),
        PolicyDecision::Confirm,
        ApprovalAssurance::Confirm,
        ApprovalAssurance::None,
    )
    .unwrap()
}
fn approved_approval() -> Approval {
    Approval::new(
        id(APPROVAL),
        id(TASK),
        id(ACTION),
        PolicyId::new("policy").unwrap(),
        ApprovalState::Approved,
        now(),
        Some(now()),
        Some(ActorRef::new("actor").unwrap()),
        Some("approved".to_owned()),
        id(HASH),
        Version::new(2).unwrap(),
        None,
        SurfaceId::new("test").unwrap(),
        Some(SurfaceId::new("test").unwrap()),
        Version::new(1).unwrap(),
        id(SNAPSHOT),
        PolicyDecision::Confirm,
        ApprovalAssurance::Confirm,
        ApprovalAssurance::Confirm,
    )
    .unwrap()
}
fn run() -> Run {
    Run::new(
        id(RUN),
        Version::new(1).unwrap(),
        id(ACTION),
        Attempt::new(1).unwrap(),
        None,
        None,
        None,
        None,
        RunState::Created,
        OutcomeCertainty::NotApplicable,
        None,
        None,
        None,
        Version::new(1).unwrap(),
        id(HASH),
        id(APPROVAL),
        Version::new(1).unwrap(),
    )
    .unwrap()
}
fn event(number: u64, subject: &str) -> maia_store::AuditEventDraft {
    audit_event(
        id(&format!("01900000-0000-7000-8000-{number:012}")),
        id(WS),
        now(),
        Some(ActorRef::new("actor").unwrap()),
        AuditEventType::new("test.event").unwrap(),
        AuditSubjectType::new(subject).unwrap(),
        None,
        None,
        None,
        None,
    )
}

#[test]
fn opens_authority_and_roundtrips_entities_and_audit_chain() {
    let path = temp_path("roundtrip");
    let store = SqliteStore::open(&path, id(WS), now()).unwrap();
    assert!(store.is_writable_authority());
    store.insert_task(&task(AgentTaskState::Queued)).unwrap();
    store.insert_plan_revision(&plan()).unwrap();
    store
        .insert_action_revision(&maia_store::ActionRevisionCommand {
            action: action(),
            action_hash: id(HASH),
            supersede_approval: None,
            new_approval: None,
            audit: event(20, "action"),
        })
        .unwrap();
    store
        .insert_approval(&approval(ApprovalState::NotRequired))
        .unwrap();
    let created = store
        .create_run(&RunCreationCommand {
            task_id: id(TASK),
            run: run(),
            current_authorization_valid: true,
            audit: event(21, "run"),
        })
        .unwrap();
    assert_eq!(
        store.create_run(&RunCreationCommand {
            task_id: id(TASK),
            run: run(),
            current_authorization_valid: true,
            audit: event(23, "run"),
        }),
        Err(StoreError::Duplicate)
    );
    assert_eq!(
        store.get_task(&id(TASK)).unwrap(),
        task(AgentTaskState::Queued)
    );
    assert_eq!(store.get_run(created.id()).unwrap(), created);
    store.append_audit(&event(22, "manual")).unwrap();
    store.verify_audit_chain(&id(WS)).unwrap();
    let records = store.list_audit(&id(WS)).unwrap();
    assert_eq!(records.len(), 3);
    assert_eq!(records[0].sequence().get(), 1);
    assert_eq!(
        compute_audit_hash(&records[0]).unwrap(),
        *records[0].record_hash()
    );
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn audit_projection_is_canonical_and_hash_is_golden() {
    let path = temp_path("audit");
    let store = SqliteStore::open(&path, id(WS), now()).unwrap();
    let record = store.append_audit(&event(30, "run")).unwrap();
    assert_eq!(
        maia_store::audit_record_projection(&record),
        format!(
            "{{\"actor_ref\":\"actor\",\"audit_id\":\"{}\",\"decision_ref\":null,\"event_type\":\"test.event\",\"metadata_hash\":null,\"operation_ref\":null,\"prev_hash\":\"{}\",\"recorded_at\":\"{}\",\"sequence\":1,\"subject_ref\":null,\"subject_type\":\"run\",\"workspace_id\":\"{}\"}}",
            record.audit_id(),
            "0".repeat(64),
            NOW,
            WS
        )
    );
    assert_eq!(record.record_hash().as_str().len(), 64);
    store.verify_audit_chain(&id(WS)).unwrap();
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn failed_cas_does_not_mutate_approval_and_second_authority_is_denied() {
    let path = temp_path("cas");
    let store = Arc::new(SqliteStore::open(&path, id(WS), now()).unwrap());
    store.insert_task(&task(AgentTaskState::Queued)).unwrap();
    store.insert_plan_revision(&plan()).unwrap();
    store
        .insert_action_revision(&maia_store::ActionRevisionCommand {
            action: action(),
            action_hash: id(HASH),
            supersede_approval: None,
            new_approval: None,
            audit: event(39, "action"),
        })
        .unwrap();
    store
        .insert_approval(&approval(ApprovalState::NotRequired))
        .unwrap();
    let current = store.get_approval(&id(APPROVAL)).unwrap();
    let candidate = current.clone();
    let bad = ApprovalDecisionCommand {
        approval_id: id(APPROVAL),
        expected_version: Version::new(99).unwrap(),
        expected_state: ApprovalState::NotRequired,
        candidate: candidate.clone(),
        audit: event(40, "approval"),
    };
    assert_eq!(
        store.decide_approval(&bad),
        Err(StoreError::ExternalConflict)
    );
    assert_eq!(store.get_approval(&id(APPROVAL)).unwrap(), current);
    let second = SqliteStore::open(&path, id(WS), now());
    assert_eq!(second.unwrap_err(), StoreError::AuthorityUnavailable);
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn task_pause_and_run_creation_are_serialized() {
    let path = temp_path("race");
    let store = Arc::new(SqliteStore::open(&path, id(WS), now()).unwrap());
    store.insert_task(&task(AgentTaskState::Running)).unwrap();
    store.insert_plan_revision(&plan()).unwrap();
    store
        .insert_action_revision(&maia_store::ActionRevisionCommand {
            action: action(),
            action_hash: id(HASH),
            supersede_approval: None,
            new_approval: None,
            audit: event(49, "action"),
        })
        .unwrap();
    store
        .insert_approval(&approval(ApprovalState::NotRequired))
        .unwrap();
    let left = Arc::clone(&store);
    let pause = thread::spawn(move || {
        left.pause_task(&TaskPauseCommand {
            task_id: id(TASK),
            expected_version: Version::new(1).unwrap(),
            expected_state: AgentTaskState::Running,
            audit: event(50, "task"),
        })
    });
    let right = Arc::clone(&store);
    let create = thread::spawn(move || {
        right.create_run(&RunCreationCommand {
            task_id: id(TASK),
            run: run(),
            current_authorization_valid: true,
            audit: event(51, "run"),
        })
    });
    let p = pause.join().unwrap();
    let c = create.join().unwrap();
    assert!(
        p.is_ok(),
        "task pause request must be serialized successfully: {p:?}"
    );
    assert!(c.is_ok() || c == Err(StoreError::ExternalConflict));
    let current_task = store.get_task(&id(TASK)).unwrap();
    let state = current_task.state();
    assert!(matches!(
        state,
        AgentTaskState::Pausing | AgentTaskState::Running
    ));
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn migration_checksum_mismatch_is_fail_closed() {
    let path = temp_path("migration");
    {
        let store = SqliteStore::open(&path, id(WS), now()).unwrap();
        assert!(store.is_writable_authority());
    }
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE migration_ledger SET checksum='bad' WHERE migration_id='0001_initial'",
            [],
        )
        .unwrap();
    drop(connection);
    let reopened = SqliteStore::open(&path, id(WS), now());
    assert!(matches!(reopened, Err(StoreError::MigrationError)));
    let _ = std::fs::remove_file(path);
}

#[test]
fn consistent_backup_and_explicit_offline_restore_roundtrip() {
    let path = temp_path("backup-source");
    let backup = path.with_extension("backup.db");
    let restored = path.with_extension("restored.db");
    {
        let store = SqliteStore::open(&path, id(WS), now()).unwrap();
        store.append_audit(&event(58, "backup")).unwrap();
        store.backup_consistent(&backup).unwrap();
    }
    SqliteStore::restore_offline(&backup, &restored).unwrap();
    let restored_store = SqliteStore::open(&restored, id(WS), now()).unwrap();
    restored_store.verify_audit_chain(&id(WS)).unwrap();
    drop(restored_store);
    for candidate in [path.clone(), backup, restored] {
        let _ = std::fs::remove_file(candidate);
    }
}

#[test]
fn concurrent_audit_appends_have_contiguous_chain() {
    let path = temp_path("audit-race");
    let store = Arc::new(SqliteStore::open(&path, id(WS), now()).unwrap());
    let mut workers = Vec::new();
    for number in 60..68 {
        let store = Arc::clone(&store);
        workers.push(thread::spawn(move || {
            store.append_audit(&event(number, "concurrent"))
        }));
    }
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert!(
        results.iter().all(Result::is_ok),
        "audit append errors: {results:?}"
    );
    let records = store.list_audit(&id(WS)).unwrap();
    assert_eq!(records.len(), 8);
    for (index, record) in records.iter().enumerate() {
        assert_eq!(record.sequence().get(), (index + 1) as u64);
    }
    store.verify_audit_chain(&id(WS)).unwrap();
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn relinquished_authority_fails_closed_for_writes() {
    let path = temp_path("authority-loss");
    let store = SqliteStore::open(&path, id(WS), now()).unwrap();
    store.relinquish_authority().unwrap();
    assert!(!store.is_writable_authority());
    assert_eq!(
        store.append_audit(&event(70, "authority")),
        Err(StoreError::AuthorityLost)
    );
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn concurrent_approval_cas_has_one_winner() {
    let path = temp_path("approval-race");
    let store = Arc::new(SqliteStore::open(&path, id(WS), now()).unwrap());
    store.insert_task(&task(AgentTaskState::Queued)).unwrap();
    store.insert_plan_revision(&plan()).unwrap();
    store
        .insert_action_revision(&maia_store::ActionRevisionCommand {
            action: action(),
            action_hash: id(HASH),
            supersede_approval: None,
            new_approval: None,
            audit: event(79, "action"),
        })
        .unwrap();
    store.insert_approval(&pending_approval()).unwrap();
    let first = Arc::clone(&store);
    let left = thread::spawn(move || {
        first.decide_approval(&ApprovalDecisionCommand {
            approval_id: id(APPROVAL),
            expected_version: Version::new(1).unwrap(),
            expected_state: ApprovalState::Pending,
            candidate: approved_approval(),
            audit: event(80, "approval"),
        })
    });
    let second = Arc::clone(&store);
    let right = thread::spawn(move || {
        second.decide_approval(&ApprovalDecisionCommand {
            approval_id: id(APPROVAL),
            expected_version: Version::new(1).unwrap(),
            expected_state: ApprovalState::Pending,
            candidate: approved_approval(),
            audit: event(81, "approval"),
        })
    });
    let outcomes = [left.join().unwrap(), right.join().unwrap()];
    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|result| **result == Err(StoreError::ExternalConflict))
            .count(),
        1
    );
    assert_eq!(
        store.get_approval(&id(APPROVAL)).unwrap().state(),
        &ApprovalState::Approved
    );
    store.verify_audit_chain(&id(WS)).unwrap();
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn crash_before_commit_leaves_no_partial_row() {
    if let Ok(path) = std::env::var("MAIA_CRASH_DB") {
        let mut connection = rusqlite::Connection::open(path).unwrap();
        let transaction = connection.transaction().unwrap();
        transaction
            .execute(
                "INSERT INTO agent_tasks(id,version,workspace_id,request_text,origin_surface,state,privacy_class,requested_by,created_at) VALUES('01900000-0000-7000-8000-000000000099','1','01900000-0000-7000-8000-000000000000','crash','test','queued','local_only','actor','2026-09-11T12:34:56.789Z')",
                [],
            )
            .unwrap();
        std::process::exit(77);
    }
    let path = temp_path("crash");
    {
        let store = SqliteStore::open(&path, id(WS), now()).unwrap();
        store.insert_task(&task(AgentTaskState::Queued)).unwrap();
    }
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("crash_before_commit_leaves_no_partial_row")
        .arg("--nocapture")
        .env("MAIA_CRASH_DB", &path)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(77));
    let connection = rusqlite::Connection::open(&path).unwrap();
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM agent_tasks WHERE id='01900000-0000-7000-8000-000000000099'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
    let _ = std::fs::remove_file(path);
}

#[test]
fn crash_after_commit_preserves_data_and_audit() {
    if let Ok(path) = std::env::var("MAIA_COMMIT_DB") {
        let store = SqliteStore::open(&path, id(WS), now()).unwrap();
        store.append_audit(&event(91, "committed")).unwrap();
        std::process::exit(78);
    }
    let path = temp_path("commit-crash");
    {
        let store = SqliteStore::open(&path, id(WS), now()).unwrap();
        assert!(store.is_writable_authority());
    }
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("crash_after_commit_preserves_data_and_audit")
        .arg("--nocapture")
        .env("MAIA_COMMIT_DB", &path)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(78));
    let store = SqliteStore::open(&path, id(WS), now()).unwrap();
    let records = store.list_audit(&id(WS)).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].subject_type().as_str(), "committed");
    store.verify_audit_chain(&id(WS)).unwrap();
    drop(store);
    let _ = std::fs::remove_file(path);
}
