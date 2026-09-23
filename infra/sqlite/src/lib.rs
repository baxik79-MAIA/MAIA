//! SQLite authoritative adapter for the M0.3 local store.
#![forbid(unsafe_code)]

use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use fs2::FileExt;
use maia_briefing::{packet_hash as briefing_packet_hash, result_hash as briefing_result_hash};
use maia_domain::*;
use maia_store::{
    ActionRevisionCommand, ApprovalDecisionCommand, ApprovalSupersession, ArtifactRepository,
    AuditEventDraft, AuditRepository, AuthorityStore, BriefingPersistence, BriefingRepository,
    EvidenceRecord, EvidenceRepository, RevisionRepository, RunCreationCommand, RunRepository,
    RunTransitionCommand, StoreError, StoreResult, TaskPauseCommand, TaskRepository,
    compute_audit_hash, make_audit_record,
};
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Row, Transaction, TransactionBehavior,
    backup::Backup, params,
};
use sha2::{Digest, Sha256};

const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_initial",
        include_str!("../migrations/0001_initial.sql"),
    ),
    (
        "0002_briefing",
        include_str!("../migrations/0002_briefing.sql"),
    ),
    (
        "0003_artifacts",
        include_str!("../migrations/0003_artifacts.sql"),
    ),
    (
        "0004_briefing_packet",
        include_str!("../migrations/0004_briefing_packet.sql"),
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenMode {
    ReadOnly,
    ReadWrite,
}

pub struct SqliteStore {
    path: PathBuf,
    workspace_id: WorkspaceId,
    connection: Mutex<Connection>,
    authority: Mutex<Option<File>>,
    writable: bool,
}

impl std::fmt::Debug for SqliteStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteStore")
            .field("path", &self.path)
            .field("workspace_id", &self.workspace_id)
            .field("writable", &self.is_writable_authority())
            .finish()
    }
}

impl SqliteStore {
    pub fn open(
        path: impl AsRef<Path>,
        workspace_id: WorkspaceId,
        now: Timestamp,
    ) -> StoreResult<Self> {
        Self::open_with_mode(path, workspace_id, now, OpenMode::ReadWrite)
    }

    pub fn open_read_only(
        path: impl AsRef<Path>,
        workspace_id: WorkspaceId,
        now: Timestamp,
    ) -> StoreResult<Self> {
        Self::open_with_mode(path, workspace_id, now, OpenMode::ReadOnly)
    }

    pub fn open_with_mode(
        path: impl AsRef<Path>,
        workspace_id: WorkspaceId,
        now: Timestamp,
        mode: OpenMode,
    ) -> StoreResult<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| StoreError::PersistenceError)?;
        }
        let authority = if mode == OpenMode::ReadWrite {
            let lock_path = authority_path(&path);
            let file = OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .truncate(false)
                .open(lock_path)
                .map_err(|_| StoreError::AuthorityUnavailable)?;
            file.try_lock_exclusive()
                .map_err(|_| StoreError::AuthorityUnavailable)?;
            Some(file)
        } else {
            None
        };
        let flags = if mode == OpenMode::ReadOnly {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
        };
        let connection = match Connection::open_with_flags(&path, flags) {
            Ok(c) => c,
            Err(error) => {
                drop(authority);
                return Err(map_sqlite(error));
            }
        };
        let store = Self {
            path,
            workspace_id,
            connection: Mutex::new(connection),
            authority: Mutex::new(authority),
            writable: mode == OpenMode::ReadWrite,
        };
        store.configure()?;
        if mode == OpenMode::ReadWrite {
            store.ensure_schema(&now)?;
            store.ensure_workspace(&now)?;
            store.enable_wal()?;
        } else {
            store.check_existing_schema()?;
        }
        store.integrity_check()?;
        store.verify_audit_chain(&store.workspace_id)?;
        Ok(store)
    }

    fn configure(&self) -> StoreResult<()> {
        let conn = self.lock_connection()?;
        conn.busy_timeout(std::time::Duration::from_millis(500))
            .map_err(map_sqlite)?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(map_sqlite)?;
        Ok(())
    }

    fn enable_wal(&self) -> StoreResult<()> {
        let conn = self.lock_connection()?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(map_sqlite)
    }

    fn ensure_schema(&self, now: &Timestamp) -> StoreResult<()> {
        let mut conn = self.lock_connection()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        for (index, (id, sql)) in MIGRATIONS.iter().enumerate() {
            if index == 0 {
                tx.execute_batch(sql).map_err(map_sqlite)?;
            }
            let checksum = sha256_hex(sql.as_bytes());
            let applied: Option<String> = tx
                .query_row(
                    "SELECT checksum FROM migration_ledger WHERE migration_id=?1",
                    params![id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(map_sqlite)?;
            match applied {
                Some(value) if value == checksum => {}
                Some(_) => return Err(StoreError::MigrationError),
                None => {
                    if index > 0 {
                        tx.execute_batch(sql).map_err(map_sqlite)?;
                    }
                    tx.execute("INSERT INTO migration_ledger(migration_id,checksum,applied_at) VALUES(?1,?2,?3)",params![id,checksum,now.as_str()]).map_err(map_sqlite)?;
                }
            }
        }
        tx.execute("DELETE FROM schema_meta", [])
            .map_err(map_sqlite)?;
        tx.execute(
            "INSERT INTO schema_meta(schema_version) VALUES(?1)",
            params![MIGRATIONS.len() as i64],
        )
        .map_err(map_sqlite)?;
        tx.commit().map_err(map_sqlite)
    }

    fn check_existing_schema(&self) -> StoreResult<()> {
        let conn = self.lock_connection()?;
        let ledger_exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='migration_ledger')",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(map_sqlite)?
            != 0;
        if !ledger_exists {
            return Err(StoreError::MigrationError);
        }
        for (id, sql) in MIGRATIONS {
            let value: Option<String> = conn
                .query_row(
                    "SELECT checksum FROM migration_ledger WHERE migration_id=?1",
                    params![id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(map_sqlite)?;
            if value.as_deref() != Some(&sha256_hex(sql.as_bytes())) {
                return Err(StoreError::MigrationError);
            }
        }
        let version: Option<i64> = conn
            .query_row("SELECT schema_version FROM schema_meta LIMIT 1", [], |r| {
                r.get(0)
            })
            .optional()
            .map_err(map_sqlite)?;
        if version == Some(MIGRATIONS.len() as i64) {
            Ok(())
        } else {
            Err(StoreError::MigrationError)
        }
    }

    fn ensure_workspace(&self, now: &Timestamp) -> StoreResult<()> {
        let conn = self.lock_connection()?;
        conn.execute(
            "INSERT OR IGNORE INTO workspaces(workspace_id,created_at) VALUES(?1,?2)",
            params![self.workspace_id.as_str(), now.as_str()],
        )
        .map_err(map_sqlite)?;
        Ok(())
    }

    fn backup_to(&self, destination: &Path) -> StoreResult<()> {
        let conn = self.lock_connection()?;
        let mut target = Connection::open(destination).map_err(map_sqlite)?;
        let backup = Backup::new(&conn, &mut target).map_err(map_sqlite)?;
        backup
            .run_to_completion(5, std::time::Duration::from_millis(20), None)
            .map_err(map_sqlite)
    }

    /// Perform a consistent SQLite backup, including WAL state.
    pub fn backup_consistent(&self, destination: impl AsRef<Path>) -> StoreResult<()> {
        self.backup_to(destination.as_ref())
    }

    /// Explicit offline restore.  The destination must not have another
    /// writable authority; no automatic failover or startup fallback occurs.
    pub fn restore_offline(
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> StoreResult<()> {
        let source = source.as_ref();
        let destination = destination.as_ref();
        if source == destination {
            return Err(StoreError::InvalidInput(DomainError::EmptyValue));
        }
        let lock_path = authority_path(destination);
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(lock_path)
            .map_err(|_| StoreError::AuthorityUnavailable)?;
        lock.try_lock_exclusive()
            .map_err(|_| StoreError::AuthorityUnavailable)?;
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|_| StoreError::PersistenceError)?;
        }
        let source_connection =
            Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(map_sqlite)?;
        let mut destination_connection = Connection::open(destination).map_err(map_sqlite)?;
        let backup =
            Backup::new(&source_connection, &mut destination_connection).map_err(map_sqlite)?;
        backup
            .run_to_completion(5, std::time::Duration::from_millis(20), None)
            .map_err(map_sqlite)
    }

    fn lock_connection(&self) -> StoreResult<MutexGuard<'_, Connection>> {
        self.connection
            .lock()
            .map_err(|_| StoreError::PersistenceError)
    }

    fn ensure_writable(&self) -> StoreResult<()> {
        if !self.writable {
            return Err(StoreError::AuthorityUnavailable);
        }
        let authority = self
            .authority
            .lock()
            .map_err(|_| StoreError::PersistenceError)?;
        if authority.is_some() {
            Ok(())
        } else {
            Err(StoreError::AuthorityLost)
        }
    }

    /// Relinquish write authority; subsequent mutations fail closed.
    pub fn relinquish_authority(&self) -> StoreResult<()> {
        if !self.writable {
            return Err(StoreError::AuthorityUnavailable);
        }
        let mut authority = self
            .authority
            .lock()
            .map_err(|_| StoreError::PersistenceError)?;
        authority.take();
        Ok(())
    }

    fn write<T>(&self, f: impl FnOnce(&Transaction<'_>) -> StoreResult<T>) -> StoreResult<T> {
        self.ensure_writable()?;
        let mut conn = self.lock_connection()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        match f(&tx) {
            Ok(value) => {
                tx.commit().map_err(map_sqlite)?;
                Ok(value)
            }
            Err(error) => Err(error),
        }
    }

    fn read<T>(&self, f: impl FnOnce(&Connection) -> StoreResult<T>) -> StoreResult<T> {
        let conn = self.lock_connection()?;
        f(&conn)
    }

    fn audit_in_tx(
        &self,
        tx: &Transaction<'_>,
        event: &AuditEventDraft,
    ) -> StoreResult<AuditRecord> {
        if event.workspace_id != self.workspace_id {
            return Err(StoreError::ExternalConflict);
        }
        tx.execute("INSERT OR IGNORE INTO audit_heads(workspace_id,next_sequence,last_hash) VALUES(?1,'1',?2)", params![self.workspace_id.as_str(), GENESIS]).map_err(map_sqlite)?;
        let (next, prev): (String, String) = tx
            .query_row(
                "SELECT next_sequence,last_hash FROM audit_heads WHERE workspace_id=?1",
                params![self.workspace_id.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .map_err(map_sqlite)?;
        let sequence_value = parse_u64(&next)?;
        let sequence = AuditSequence::new(sequence_value).map_err(StoreError::InvalidInput)?;
        let prev_hash = Sha256Hex::new(prev).map_err(StoreError::InvalidInput)?;
        let record = make_audit_record(event, event.audit_id.clone(), sequence, prev_hash)?;
        tx.execute("INSERT INTO audit_records(workspace_id,sequence,audit_id,recorded_at,actor_ref,event_type,subject_type,subject_ref,operation_ref,decision_ref,metadata_hash,prev_hash,record_hash) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)", params![record.workspace_id().as_str(), sequence_value.to_string(), record.audit_id().as_str(), record.recorded_at().as_str(), opt_str(record.actor_ref()), record.event_type().as_str(), record.subject_type().as_str(), opt_str(record.subject_ref()), opt_str(record.operation_ref()), opt_str(record.decision_ref()), opt_str(record.metadata_hash()), record.prev_hash().as_str(), record.record_hash().as_str()]).map_err(map_sqlite)?;
        let next_value = sequence_value
            .checked_add(1)
            .ok_or(StoreError::Corruption)?;
        tx.execute(
            "UPDATE audit_heads SET next_sequence=?1,last_hash=?2 WHERE workspace_id=?3",
            params![
                next_value.to_string(),
                record.record_hash().as_str(),
                self.workspace_id.as_str()
            ],
        )
        .map_err(map_sqlite)?;
        Ok(record)
    }
}

fn authority_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.authority.lock", path.display()))
}
fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}
fn parse_u64(value: &str) -> StoreResult<u64> {
    value.parse().map_err(|_| StoreError::Corruption)
}
fn opt_str<T: ToString>(value: &Option<T>) -> Option<String> {
    value.as_ref().map(ToString::to_string)
}
fn map_sqlite(error: rusqlite::Error) -> StoreError {
    match error {
        rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound,
        rusqlite::Error::SqliteFailure(ref e, _)
            if matches!(e.code, rusqlite::ErrorCode::ConstraintViolation) =>
        {
            StoreError::Duplicate
        }
        rusqlite::Error::SqliteFailure(ref e, _)
            if matches!(
                e.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            ) =>
        {
            StoreError::StorageBusy
        }
        _ => StoreError::PersistenceError,
    }
}

impl AuthorityStore for SqliteStore {
    fn is_writable_authority(&self) -> bool {
        self.writable
            && self
                .authority
                .lock()
                .map(|guard| guard.is_some())
                .unwrap_or(false)
    }
    fn integrity_check(&self) -> StoreResult<()> {
        self.read(|conn| {
            let result: String = conn
                .query_row("PRAGMA integrity_check", [], |row| row.get(0))
                .map_err(map_sqlite)?;
            if result != "ok" {
                Err(StoreError::Corruption)
            } else {
                let mut statement = conn
                    .prepare("PRAGMA foreign_key_check")
                    .map_err(map_sqlite)?;
                let mut rows = statement.query([]).map_err(map_sqlite)?;
                if rows.next().map_err(map_sqlite)?.is_some() {
                    Err(StoreError::Corruption)
                } else {
                    Ok(())
                }
            }
        })
    }
}

impl TaskRepository for SqliteStore {
    fn insert_task(&self, task: &AgentTask) -> StoreResult<()> {
        if task.workspace_id() != &self.workspace_id {
            return Err(StoreError::ExternalConflict);
        }
        self.write(|tx| { tx.execute("INSERT INTO workspaces(workspace_id,created_at) VALUES(?1,?2) ON CONFLICT(workspace_id) DO NOTHING", params![task.workspace_id().as_str(), task.created_at().as_str()]).map_err(map_sqlite)?; tx.execute("INSERT INTO agent_tasks(id,version,workspace_id,request_text,origin_surface,origin_ref,state,privacy_class,requested_by,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)", params![task.id().as_str(), task.version().get().to_string(), task.workspace_id().as_str(), task.request_text().as_str(), task.origin_surface().as_str(), opt_str(task.origin_ref()), task.state().as_str(), task.privacy_class().as_str(), task.requested_by().as_str(), task.created_at().as_str()]).map_err(map_sqlite)?; Ok(()) })
    }
    fn get_task(&self, id: &AgentTaskId) -> StoreResult<AgentTask> {
        self.read(|conn| { let mut stmt=conn.prepare("SELECT id,version,workspace_id,request_text,origin_surface,origin_ref,state,privacy_class,requested_by,created_at FROM agent_tasks WHERE id=?1").map_err(map_sqlite)?; stmt.query_row(params![id.as_str()], task_from_row).map_err(map_sqlite) })
    }
    fn pause_task(&self, command: &TaskPauseCommand) -> StoreResult<AgentTask> {
        self.write(|tx| {
            let current = get_task_tx(tx, &command.task_id)?;
            if current.version() != &command.expected_version
                || current.state() != &command.expected_state
                || current.state() != &AgentTaskState::Running
            {
                return Err(StoreError::ExternalConflict);
            }
            let mut candidate = current.clone();
            candidate
                .transition_to(AgentTaskState::Pausing)
                .map_err(StoreError::InvalidInput)?;
            update_task_tx(tx, &candidate)?;
            self.audit_in_tx(tx, &command.audit)?;
            Ok(candidate)
        })
    }
}

impl RevisionRepository for SqliteStore {
    fn insert_plan_revision(&self, plan: &ExecutionPlan) -> StoreResult<()> {
        self.write(|tx| { tx.execute("INSERT INTO execution_plans(id,version,task_id,risk_summary,estimated_amount_micros,estimated_currency,approval_requirement,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)", params![plan.id().as_str(), plan.version().get().to_string(), plan.task_id().as_str(), risk_to_db(plan.risk_summary()), plan.estimated_cost().as_ref().map(|v| v.amount_micros().get().to_string()), plan.estimated_cost().as_ref().map(|v| v.currency().as_str().to_owned()), plan.approval_requirement().as_str(), plan.created_at().as_str()]).map_err(map_sqlite)?; Ok(()) })
    }
    fn insert_action_revision(&self, command: &ActionRevisionCommand) -> StoreResult<()> {
        self.write(|tx| {
            command
                .action
                .validate()
                .map_err(StoreError::InvalidInput)?;
            let expected = next_revision_tx(tx, command.action.id().as_str(), "actions")?;
            if command.action.version().get() != expected { return Err(StoreError::ExternalConflict); }
            let action = &command.action;
            let sources = serde_json::to_string(&action.source_preconditions().iter().map(|s| vec![s.external_id().as_str(),s.source_version_token().as_str(),s.normalized_payload_hash().as_str()]).collect::<Vec<_>>()).map_err(|_| StoreError::PersistenceError)?;
            tx.execute("INSERT INTO actions(id,version,plan_id,plan_version,ordinal,action_type,connector_profile_id,risk_class,state,input_ref,source_preconditions_json,result_ref,input_hash,input_canonicalizer_id,input_canonicalizer_version,connector_selection,connector_binding_hash,tool_definition_fingerprint,action_hash) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)", params![action.id().as_str(),action.version().get().to_string(),action.plan_id().as_str(),action.plan_version().get().to_string(),action.ordinal().get(),action.action_type().as_str(),opt_str(action.connector_profile_id()),action.risk_class().as_str(),action.state().as_str(),action.input_ref().as_str(),sources,opt_str(action.result_ref()),action.input_hash().as_str(),action.input_canonicalizer().id().as_str(),action.input_canonicalizer().version().get().to_string(),action.connector_selection().as_str(),opt_str(action.connector_binding_hash()),opt_str(action.tool_definition_fingerprint()),command.action_hash.as_str()]).map_err(map_sqlite)?;
            if let Some(old) = &command.supersede_approval { supersede_approval_tx(tx, old)?; }
            if let Some(new_approval) = &command.new_approval { if new_approval.action_id()!=action.id() || new_approval.action_version()!=action.version() || new_approval.action_hash()!=&command.action_hash { return Err(StoreError::ExternalConflict); } insert_approval_tx(tx,new_approval)?; }
            self.audit_in_tx(tx,&command.audit)?; Ok(())
        })
    }
    fn get_plan_revision(
        &self,
        id: &ExecutionPlanId,
        version: Version,
    ) -> StoreResult<ExecutionPlan> {
        self.read(|conn| conn.query_row("SELECT id,version,task_id,risk_summary,estimated_amount_micros,estimated_currency,approval_requirement,created_at FROM execution_plans WHERE id=?1 AND version=?2", params![id.as_str(),version.get().to_string()], plan_from_row).map_err(map_sqlite))
    }
    fn get_action_revision(&self, id: &ActionId, version: Version) -> StoreResult<Action> {
        self.read(|conn| conn.query_row("SELECT id,version,plan_id,plan_version,ordinal,action_type,connector_profile_id,risk_class,state,input_ref,source_preconditions_json,result_ref,input_hash,input_canonicalizer_id,input_canonicalizer_version,connector_selection,connector_binding_hash,tool_definition_fingerprint FROM actions WHERE id=?1 AND version=?2", params![id.as_str(),version.get().to_string()], action_from_row).map_err(map_sqlite))
    }
    fn get_current_action(&self, id: &ActionId) -> StoreResult<Action> {
        self.read(|conn| conn.query_row("SELECT id,version,plan_id,plan_version,ordinal,action_type,connector_profile_id,risk_class,state,input_ref,source_preconditions_json,result_ref,input_hash,input_canonicalizer_id,input_canonicalizer_version,connector_selection,connector_binding_hash,tool_definition_fingerprint FROM actions WHERE id=?1 ORDER BY LENGTH(version) DESC, version DESC LIMIT 1", params![id.as_str()], action_from_row).map_err(map_sqlite))
    }
}

impl maia_store::ApprovalRepository for SqliteStore {
    fn insert_approval(&self, approval: &Approval) -> StoreResult<()> {
        self.write(|tx| insert_approval_tx(tx, approval))
    }
    fn get_approval(&self, id: &ApprovalId) -> StoreResult<Approval> {
        self.read(|conn| conn.query_row("SELECT id,task_id,action_id,action_version,policy_id,state,requested_at,decided_at,decided_by,decision_note,action_hash,version,expires_at,origin_surface,decided_surface,policy_snapshot_hash,policy_decision,required_assurance,achieved_assurance FROM approvals WHERE id=?1", params![id.as_str()], approval_from_row).map_err(map_sqlite))
    }
    fn decide_approval(&self, command: &ApprovalDecisionCommand) -> StoreResult<Approval> {
        self.write(|tx| {
            let current = get_approval_tx(tx, &command.approval_id)?;
            if current.version() != &command.expected_version
                || current.state() != &command.expected_state
            {
                return Err(StoreError::ExternalConflict);
            }
            if current.action_id() != command.candidate.action_id()
                || current.action_version() != command.candidate.action_version()
                || current.action_hash() != command.candidate.action_hash()
                || current.policy_snapshot_hash() != command.candidate.policy_snapshot_hash()
            {
                return Err(StoreError::ExternalConflict);
            }
            if command.candidate.version().get()
                != command
                    .expected_version
                    .get()
                    .checked_add(1)
                    .ok_or(StoreError::ExternalConflict)?
            {
                return Err(StoreError::ExternalConflict);
            }
            command
                .candidate
                .validate()
                .map_err(StoreError::InvalidInput)?;
            update_approval_tx(tx, &command.candidate)?;
            self.audit_in_tx(tx, &command.audit)?;
            Ok(command.candidate.clone())
        })
    }
}

impl RunRepository for SqliteStore {
    fn create_run(&self, command: &RunCreationCommand) -> StoreResult<Run> {
        self.write(|tx| {
            let task = get_task_tx(tx, &command.task_id)?;
            if !matches!(
                task.state(),
                AgentTaskState::Queued
                    | AgentTaskState::Running
                    | AgentTaskState::PartiallyCompleted
            ) {
                return Err(StoreError::ExternalConflict);
            }
            if !command.current_authorization_valid {
                return Err(StoreError::ExternalConflict);
            }
            let action = get_action_tx(tx, command.run.action_id(), *command.run.action_version())?;
            let action_task_id =
                action_task_id_tx(tx, command.run.action_id(), *command.run.action_version())?;
            if action_task_id != command.task_id {
                return Err(StoreError::ExternalConflict);
            }
            if action.id() != command.run.action_id()
                || command.run.action_hash()
                    != &action_hash_tx(tx, command.run.action_id(), *command.run.action_version())?
            {
                return Err(StoreError::ExternalConflict);
            }
            let approval = get_approval_tx(tx, command.run.approval_id())?;
            if approval.task_id() != &command.task_id
                || approval.version() != command.run.approval_version()
                || approval.action_id() != command.run.action_id()
                || approval.action_version() != command.run.action_version()
                || approval.action_hash() != command.run.action_hash()
                || !matches!(
                    approval.state(),
                    ApprovalState::Approved | ApprovalState::NotRequired
                )
            {
                return Err(StoreError::ExternalConflict);
            }
            if command.run.version().get() != 1 || command.run.state() != &RunState::Created {
                return Err(StoreError::InvalidInput(
                    DomainError::InvalidStateTransition {
                        machine: "Run",
                        from: command.run.state().as_str(),
                        to: "created",
                    },
                ));
            }
            insert_run_tx(tx, &command.task_id, &command.run)?;
            self.audit_in_tx(tx, &command.audit)?;
            Ok(command.run.clone())
        })
    }
    fn get_run(&self, id: &RunId) -> StoreResult<Run> {
        self.read(|conn| conn.query_row("SELECT id,version,action_id,action_version,attempt,requested_connector_id,actual_connector_id,requested_model_id,actual_model_id,state,outcome_certainty,reconciliation_ref,started_at,ended_at,action_hash,approval_id,approval_version FROM runs WHERE id=?1", params![id.as_str()], run_from_row).map_err(map_sqlite))
    }
    fn transition_run(&self, command: &RunTransitionCommand) -> StoreResult<Run> {
        self.write(|tx| {
            let current = get_run_tx(tx, &command.run_id)?;
            if current.version() != &command.expected_version
                || current.state() != &command.expected_state
            {
                return Err(StoreError::ExternalConflict);
            }
            if command.candidate.version().get()
                != command
                    .expected_version
                    .get()
                    .checked_add(1)
                    .ok_or(StoreError::ExternalConflict)?
            {
                return Err(StoreError::ExternalConflict);
            }
            if !maia_store::exact_run_binding_matches(
                &command.candidate,
                current.action_id(),
                *current.action_version(),
                current.action_hash(),
                current.approval_id(),
                *current.approval_version(),
            ) {
                return Err(StoreError::ExternalConflict);
            }
            command
                .candidate
                .validate()
                .map_err(StoreError::InvalidInput)?;
            if command.candidate.state() == &RunState::Starting {
                let task_id = run_task_id_tx(tx, &current)?;
                let task = get_task_tx(tx, &task_id)?;
                if matches!(
                    task.state(),
                    AgentTaskState::Pausing | AgentTaskState::Paused
                ) {
                    return Err(StoreError::ExternalConflict);
                }
            }
            if let Some(evidence) = &command.evidence {
                validate_evidence_for_run(&command.candidate, evidence)?;
                insert_evidence_tx(tx, evidence)?;
            } else if current.state() == &RunState::Reconciling
                && command.candidate.state() != &RunState::Reconciling
            {
                return Err(StoreError::InvalidInput(
                    DomainError::InvalidFreshnessEvidence,
                ));
            }
            update_run_tx(tx, &command.candidate)?;
            self.audit_in_tx(tx, &command.audit)?;
            Ok(command.candidate.clone())
        })
    }
}

impl EvidenceRepository for SqliteStore {
    fn append_evidence(&self, evidence: &EvidenceRecord) -> StoreResult<()> {
        self.write(|tx| {
            let run = get_run_tx(tx, evidence_run_id(evidence))?;
            validate_evidence_for_run(&run, evidence)?;
            insert_evidence_tx(tx, evidence)
        })
    }
    fn get_evidence_for_run(&self, run_id: &RunId) -> StoreResult<Vec<EvidenceRecord>> {
        self.read(|conn| { let mut out=Vec::new(); let mut stmt=conn.prepare("SELECT run_id,action_id,action_version,action_hash,enforcement,precondition_external_id,precondition_source_version_token,precondition_payload_hash,observed_source_version_token,observed_payload_hash,result FROM freshness_evidence WHERE run_id=?1").map_err(map_sqlite)?; if let Some(v)=stmt.query_row(params![run_id.as_str()], freshness_from_row).optional().map_err(map_sqlite)? { out.push(EvidenceRecord::Freshness(v)); } let mut stmt=conn.prepare("SELECT run_id,action_id,action_version,action_hash,effect_identity,source_ref,result FROM reconciliation_evidence WHERE run_id=?1").map_err(map_sqlite)?; if let Some(v)=stmt.query_row(params![run_id.as_str()], reconciliation_from_row).optional().map_err(map_sqlite)? { out.push(EvidenceRecord::Reconciliation(v)); } Ok(out) })
    }
}

impl ArtifactRepository for SqliteStore {
    fn persist_artifact(
        &self,
        artifact: &Artifact,
        audit: &AuditEventDraft,
    ) -> StoreResult<Artifact> {
        if artifact.workspace_id() != &self.workspace_id
            || sha256_hex(artifact.content().as_str().as_bytes())
                != artifact.content_sha256().as_str()
            || artifact.content().as_str().len() as u64 != artifact.byte_length().get()
            || sha256_hex(artifact.source_locator().as_str().as_bytes())
                != artifact.source_locator_hash().as_str()
        {
            return Err(StoreError::PersistenceError);
        }
        self.write(|tx| {
            let existing = tx.query_row(
                "SELECT artifact_id,workspace_id,kind,name,media_type,content,artifacts.content_sha256,artifacts.byte_length,source_locator,source_locator_hash,import_request_id,trust,classification,created_at FROM artifacts JOIN artifact_contents USING(workspace_id,content_sha256) WHERE artifacts.workspace_id=?1 AND source_locator_hash=?2 AND artifacts.content_sha256=?3",
                params![artifact.workspace_id().as_str(), artifact.source_locator_hash().as_str(), artifact.content_sha256().as_str()],
                artifact_from_row,
            ).optional().map_err(map_sqlite)?;
            let stored = if let Some(existing) = existing {
                existing
            } else {
                tx.execute("INSERT OR IGNORE INTO artifact_contents(workspace_id,content_sha256,content,byte_length) VALUES(?1,?2,?3,?4)", params![artifact.workspace_id().as_str(),artifact.content_sha256().as_str(),artifact.content().as_str(),artifact.byte_length().get().to_string()]).map_err(map_sqlite)?;
                tx.execute("INSERT INTO artifacts(artifact_id,workspace_id,kind,name,media_type,content_sha256,byte_length,source_locator,source_locator_hash,import_request_id,trust,classification,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",params![artifact.id().as_str(),artifact.workspace_id().as_str(),artifact.kind().as_str(),artifact.name().as_str(),artifact.media_type().as_str(),artifact.content_sha256().as_str(),artifact.byte_length().get().to_string(),artifact.source_locator().as_str(),artifact.source_locator_hash().as_str(),artifact.import_request_id().as_str(),artifact.trust().as_str(),artifact.classification().as_str(),artifact.created_at().as_str()]).map_err(map_sqlite)?;
                artifact.clone()
            };
            self.audit_in_tx(tx, audit)?;
            Ok(stored)
        })
    }
    fn get_artifact(&self, id: &ArtifactId) -> StoreResult<Artifact> {
        self.read(|conn| conn.query_row(
            "SELECT artifact_id,workspace_id,kind,name,media_type,content,artifacts.content_sha256,artifacts.byte_length,source_locator,source_locator_hash,import_request_id,trust,classification,created_at FROM artifacts JOIN artifact_contents USING(workspace_id,content_sha256) WHERE artifact_id=?1",
            params![id.as_str()], artifact_from_row,
        ).map_err(map_sqlite))
    }
    fn list_artifacts(&self) -> StoreResult<Vec<Artifact>> {
        self.read(|conn| {
            let mut statement = conn.prepare(
                "SELECT artifact_id,workspace_id,kind,name,media_type,content,artifacts.content_sha256,artifacts.byte_length,source_locator,source_locator_hash,import_request_id,trust,classification,created_at FROM artifacts JOIN artifact_contents USING(workspace_id,content_sha256) WHERE artifacts.workspace_id=?1 ORDER BY created_at DESC"
            ).map_err(map_sqlite)?;
            let rows = statement.query_map(params![self.workspace_id.as_str()], artifact_from_row)
                .map_err(map_sqlite)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(map_sqlite)
        })
    }
}

impl BriefingRepository for SqliteStore {
    fn persist_briefing(
        &self,
        record: &BriefingPersistence,
        audit: &AuditEventDraft,
    ) -> StoreResult<()> {
        if record.workspace_id != self.workspace_id.as_str()
            || record.packet.workspace_id != record.workspace_id
            || record.result.execution_authority
            || record.packet_hash != record.result.packet_hash
            || briefing_packet_hash(&record.packet).ok().as_deref() != Some(&record.packet_hash)
            || record.result_hash != briefing_result_hash(&record.result)
            || record.result_id.trim().is_empty()
            || record.result.packet_id.trim().is_empty()
        {
            return Err(StoreError::PersistenceError);
        }
        let json =
            serde_json::to_string(&record.result).map_err(|_| StoreError::PersistenceError)?;
        let packet_json =
            serde_json::to_string(&record.packet).map_err(|_| StoreError::PersistenceError)?;
        self.write(|tx| {
            tx.execute("INSERT INTO briefing_results(result_id,workspace_id,packet_id,packet_hash,requested_provider,requested_model,actual_provider,actual_model,assurance,result_json,result_hash,created_at,execution_authority,packet_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,0,?13)", params![record.result_id,record.workspace_id,record.result.packet_id,record.packet_hash,record.requested_provider,record.requested_model,record.actual_provider,record.actual_model,record.assurance,json,record.result_hash,record.created_at.as_str(),packet_json]).map_err(map_sqlite)?;
            for claim in &record.result.claims { for citation in &claim.citations { tx.execute("INSERT INTO briefing_citations(result_id,citation_id) VALUES(?1,?2)", params![record.result_id,citation]).map_err(map_sqlite)?; } }
            self.audit_in_tx(tx, audit)?; Ok(())
        })
    }
    fn get_briefing(&self, result_id: &str) -> StoreResult<BriefingPersistence> {
        self.read(|conn| conn.query_row("SELECT result_id,workspace_id,packet_hash,requested_provider,requested_model,actual_provider,actual_model,assurance,result_json,result_hash,created_at,packet_json FROM briefing_results WHERE result_id=?1",params![result_id],|r| {
            let raw:String=r.get(8)?; let result=serde_json::from_str(&raw).map_err(|_|rusqlite::Error::InvalidQuery)?;
            let packet_raw:String=r.get(11)?; let packet=serde_json::from_str(&packet_raw).map_err(|_|rusqlite::Error::InvalidQuery)?;
            Ok(BriefingPersistence{result_id:r.get(0)?,workspace_id:r.get(1)?,packet,packet_hash:r.get(2)?,requested_provider:r.get(3)?,requested_model:r.get(4)?,actual_provider:r.get(5)?,actual_model:r.get(6)?,assurance:r.get(7)?,result,result_hash:r.get(9)?,created_at:r.get::<_,String>(10)?.parse().map_err(|_|rusqlite::Error::InvalidQuery)?})
        }).map_err(map_sqlite))
    }
    fn list_briefings(&self) -> StoreResult<Vec<BriefingPersistence>> {
        self.read(|conn| { let mut statement=conn.prepare("SELECT result_id,workspace_id,packet_hash,requested_provider,requested_model,actual_provider,actual_model,assurance,result_json,result_hash,created_at,packet_json FROM briefing_results WHERE workspace_id=?1 ORDER BY created_at DESC").map_err(map_sqlite)?; let rows=statement.query_map(params![self.workspace_id.as_str()],|r| { let raw:String=r.get(8)?; let result=serde_json::from_str(&raw).map_err(|_|rusqlite::Error::InvalidQuery)?; let packet_raw:String=r.get(11)?; let packet=serde_json::from_str(&packet_raw).map_err(|_|rusqlite::Error::InvalidQuery)?; Ok(BriefingPersistence{result_id:r.get(0)?,workspace_id:r.get(1)?,packet,packet_hash:r.get(2)?,requested_provider:r.get(3)?,requested_model:r.get(4)?,actual_provider:r.get(5)?,actual_model:r.get(6)?,assurance:r.get(7)?,result,result_hash:r.get(9)?,created_at:r.get::<_,String>(10)?.parse().map_err(|_|rusqlite::Error::InvalidQuery)?}) }).map_err(map_sqlite)?; rows.collect::<Result<Vec<_>,_>>().map_err(map_sqlite) })
    }
}

fn evidence_run_id(evidence: &EvidenceRecord) -> &RunId {
    match evidence {
        EvidenceRecord::Freshness(value) => value.run_id(),
        EvidenceRecord::Reconciliation(value) => value.run_id(),
    }
}

impl AuditRepository for SqliteStore {
    fn append_audit(&self, event: &AuditEventDraft) -> StoreResult<AuditRecord> {
        self.write(|tx| self.audit_in_tx(tx, event))
    }
    fn list_audit(&self, workspace_id: &WorkspaceId) -> StoreResult<Vec<AuditRecord>> {
        self.read(|conn| { let mut stmt=conn.prepare("SELECT audit_id,workspace_id,sequence,recorded_at,actor_ref,event_type,subject_type,subject_ref,operation_ref,decision_ref,metadata_hash,prev_hash,record_hash FROM audit_records WHERE workspace_id=?1 ORDER BY LENGTH(sequence), sequence").map_err(map_sqlite)?; let rows=stmt.query_map(params![workspace_id.as_str()], audit_from_row).map_err(map_sqlite)?; rows.collect::<Result<Vec<_>,_>>().map_err(map_sqlite) })
    }
    fn verify_audit_chain(&self, workspace_id: &WorkspaceId) -> StoreResult<()> {
        let records = self.list_audit(workspace_id)?;
        let mut previous = GENESIS.to_owned();
        let mut expected = 1u64;
        for record in records {
            if record.sequence().get() != expected || record.prev_hash().as_str() != previous {
                return Err(StoreError::Corruption);
            }
            if compute_audit_hash(&record)? != *record.record_hash() {
                return Err(StoreError::Corruption);
            }
            previous = record.record_hash().to_string();
            expected = expected.checked_add(1).ok_or(StoreError::Corruption)?;
        }
        let head = self.read(|conn| {
            conn.query_row(
                "SELECT next_sequence,last_hash FROM audit_heads WHERE workspace_id=?1",
                params![workspace_id.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(map_sqlite)
        })?;
        if let Some((next_sequence, last_hash)) = head {
            if next_sequence != expected.to_string() || last_hash != previous {
                return Err(StoreError::Corruption);
            }
        } else if expected != 1 {
            return Err(StoreError::Corruption);
        }
        Ok(())
    }
}

fn next_revision_tx(tx: &Transaction<'_>, id: &str, table: &str) -> StoreResult<u64> {
    let sql = format!(
        "SELECT version FROM {table} WHERE id=?1 ORDER BY LENGTH(version) DESC, version DESC LIMIT 1"
    );
    let last: Option<String> = tx
        .query_row(&sql, params![id], |row| row.get(0))
        .optional()
        .map_err(map_sqlite)?;
    match last {
        None => Ok(1),
        Some(v) => parse_u64(&v)?
            .checked_add(1)
            .ok_or(StoreError::ExternalConflict),
    }
}
fn action_hash_tx(tx: &Transaction<'_>, id: &ActionId, version: Version) -> StoreResult<Sha256Hex> {
    let s: String = tx
        .query_row(
            "SELECT action_hash FROM actions WHERE id=?1 AND version=?2",
            params![id.as_str(), version.get().to_string()],
            |r| r.get(0),
        )
        .map_err(map_sqlite)?;
    Sha256Hex::new(s).map_err(StoreError::InvalidInput)
}
fn action_task_id_tx(
    tx: &Transaction<'_>,
    id: &ActionId,
    version: Version,
) -> StoreResult<AgentTaskId> {
    let value: String = tx
        .query_row(
            "SELECT p.task_id FROM actions a JOIN execution_plans p ON p.id=a.plan_id AND p.version=a.plan_version WHERE a.id=?1 AND a.version=?2",
            params![id.as_str(), version.get().to_string()],
            |row| row.get(0),
        )
        .map_err(map_sqlite)?;
    AgentTaskId::new(value).map_err(StoreError::InvalidInput)
}
fn get_task_tx(tx: &Transaction<'_>, id: &AgentTaskId) -> StoreResult<AgentTask> {
    tx.query_row("SELECT id,version,workspace_id,request_text,origin_surface,origin_ref,state,privacy_class,requested_by,created_at FROM agent_tasks WHERE id=?1",params![id.as_str()],task_from_row).map_err(map_sqlite)
}
fn update_task_tx(tx: &Transaction<'_>, task: &AgentTask) -> StoreResult<()> {
    tx.execute(
        "UPDATE agent_tasks SET version=?1,state=?2 WHERE id=?3",
        params![
            task.version().get().to_string(),
            task.state().as_str(),
            task.id().as_str()
        ],
    )
    .map_err(map_sqlite)?;
    Ok(())
}
fn insert_approval_tx(tx: &Transaction<'_>, a: &Approval) -> StoreResult<()> {
    a.validate().map_err(StoreError::InvalidInput)?;
    let task_exists: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM agent_tasks WHERE id=?1",
            params![a.task_id().as_str()],
            |row| row.get(0),
        )
        .map_err(map_sqlite)?;
    if task_exists != 1 {
        return Err(StoreError::ExternalConflict);
    }
    let action_hash: Option<String> = tx
        .query_row(
            "SELECT action_hash FROM actions WHERE id=?1 AND version=?2",
            params![a.action_id().as_str(), a.action_version().get().to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_sqlite)?;
    if action_hash.as_deref() != Some(a.action_hash().as_str()) {
        return Err(StoreError::ExternalConflict);
    }
    let action_task_id: String = tx
        .query_row(
            "SELECT p.task_id FROM actions a JOIN execution_plans p ON p.id=a.plan_id AND p.version=a.plan_version WHERE a.id=?1 AND a.version=?2",
            params![a.action_id().as_str(), a.action_version().get().to_string()],
            |row| row.get(0),
        )
        .map_err(map_sqlite)?;
    if action_task_id != a.task_id().as_str() {
        return Err(StoreError::ExternalConflict);
    }
    tx.execute("INSERT INTO approvals(id,task_id,action_id,action_version,policy_id,state,requested_at,decided_at,decided_by,decision_note,action_hash,version,expires_at,origin_surface,decided_surface,policy_snapshot_hash,policy_decision,required_assurance,achieved_assurance) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",params![a.id().as_str(),a.task_id().as_str(),a.action_id().as_str(),a.action_version().get().to_string(),a.policy_id().as_str(),a.state().as_str(),a.requested_at().as_str(),opt_str(a.decided_at()),opt_str(a.decided_by()),a.decision_note().clone(),a.action_hash().as_str(),a.version().get().to_string(),opt_str(a.expires_at()),a.origin_surface().as_str(),opt_str(a.decided_surface()),a.policy_snapshot_hash().as_str(),a.policy_decision().as_str(),a.required_assurance().as_str(),a.achieved_assurance().as_str()]).map_err(map_sqlite)?;
    Ok(())
}
fn get_approval_tx(tx: &Transaction<'_>, id: &ApprovalId) -> StoreResult<Approval> {
    tx.query_row("SELECT id,task_id,action_id,action_version,policy_id,state,requested_at,decided_at,decided_by,decision_note,action_hash,version,expires_at,origin_surface,decided_surface,policy_snapshot_hash,policy_decision,required_assurance,achieved_assurance FROM approvals WHERE id=?1",params![id.as_str()],approval_from_row).map_err(map_sqlite)
}
fn update_approval_tx(tx: &Transaction<'_>, a: &Approval) -> StoreResult<()> {
    tx.execute("UPDATE approvals SET state=?1,decided_at=?2,decided_by=?3,decision_note=?4,version=?5,decided_surface=?6,achieved_assurance=?7 WHERE id=?8",params![a.state().as_str(),opt_str(a.decided_at()),opt_str(a.decided_by()),a.decision_note().clone(),a.version().get().to_string(),opt_str(a.decided_surface()),a.achieved_assurance().as_str(),a.id().as_str()]).map_err(map_sqlite)?;
    Ok(())
}
fn supersede_approval_tx(tx: &Transaction<'_>, s: &ApprovalSupersession) -> StoreResult<()> {
    let current = get_approval_tx(tx, &s.approval_id)?;
    if current.version() != &s.expected_version || current.state() != &s.expected_state {
        return Err(StoreError::ExternalConflict);
    };
    let mut c = current.clone();
    c.transition_to(s.next_state)
        .map_err(StoreError::InvalidInput)?;
    update_approval_tx(tx, &c)
}
fn get_action_tx(tx: &Transaction<'_>, id: &ActionId, version: Version) -> StoreResult<Action> {
    tx.query_row("SELECT id,version,plan_id,plan_version,ordinal,action_type,connector_profile_id,risk_class,state,input_ref,source_preconditions_json,result_ref,input_hash,input_canonicalizer_id,input_canonicalizer_version,connector_selection,connector_binding_hash,tool_definition_fingerprint FROM actions WHERE id=?1 AND version=?2",params![id.as_str(),version.get().to_string()],action_from_row).map_err(map_sqlite)
}
fn insert_run_tx(tx: &Transaction<'_>, task_id: &AgentTaskId, r: &Run) -> StoreResult<()> {
    tx.execute("INSERT INTO runs(id,version,action_id,action_version,attempt,requested_connector_id,actual_connector_id,requested_model_id,actual_model_id,state,outcome_certainty,reconciliation_ref,started_at,ended_at,action_hash,approval_id,approval_version) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",params![r.id().as_str(),r.version().get().to_string(),r.action_id().as_str(),r.action_version().get().to_string(),r.attempt().get().to_string(),opt_str(r.requested_connector_id()),opt_str(r.actual_connector_id()),opt_str(r.requested_model_id()),opt_str(r.actual_model_id()),r.state().as_str(),r.outcome_certainty().as_str(),opt_str(r.reconciliation_ref()),opt_str(r.started_at()),opt_str(r.ended_at()),r.action_hash().as_str(),r.approval_id().as_str(),r.approval_version().get().to_string()]).map_err(map_sqlite)?;
    let _ = task_id;
    Ok(())
}
fn get_run_tx(tx: &Transaction<'_>, id: &RunId) -> StoreResult<Run> {
    tx.query_row("SELECT id,version,action_id,action_version,attempt,requested_connector_id,actual_connector_id,requested_model_id,actual_model_id,state,outcome_certainty,reconciliation_ref,started_at,ended_at,action_hash,approval_id,approval_version FROM runs WHERE id=?1",params![id.as_str()],run_from_row).map_err(map_sqlite)
}
fn update_run_tx(tx: &Transaction<'_>, r: &Run) -> StoreResult<()> {
    tx.execute("UPDATE runs SET version=?1,state=?2,outcome_certainty=?3,reconciliation_ref=?4,started_at=?5,ended_at=?6,actual_connector_id=?7,actual_model_id=?8 WHERE id=?9",params![r.version().get().to_string(),r.state().as_str(),r.outcome_certainty().as_str(),opt_str(r.reconciliation_ref()),opt_str(r.started_at()),opt_str(r.ended_at()),opt_str(r.actual_connector_id()),opt_str(r.actual_model_id()),r.id().as_str()]).map_err(map_sqlite)?;
    Ok(())
}
fn run_task_id_tx(tx: &Transaction<'_>, r: &Run) -> StoreResult<AgentTaskId> {
    let s: String = tx
        .query_row(
            "SELECT task_id FROM approvals WHERE id=?1",
            params![r.approval_id().as_str()],
            |row| row.get(0),
        )
        .map_err(map_sqlite)?;
    AgentTaskId::new(s).map_err(StoreError::InvalidInput)
}
fn validate_evidence_for_run(r: &Run, e: &EvidenceRecord) -> StoreResult<()> {
    match e {
        EvidenceRecord::Freshness(v) => {
            if v.run_id() != r.id()
                || v.action_id() != r.action_id()
                || v.action_version() != r.action_version()
                || v.action_hash() != r.action_hash()
            {
                return Err(StoreError::ExternalConflict);
            };
            v.validate().map_err(StoreError::InvalidInput)
        }
        EvidenceRecord::Reconciliation(v) => {
            if v.run_id() != r.id()
                || v.action_id() != r.action_id()
                || v.action_version() != r.action_version()
                || v.action_hash() != r.action_hash()
            {
                return Err(StoreError::ExternalConflict);
            };
            Ok(())
        }
    }
}
fn insert_evidence_tx(tx: &Transaction<'_>, e: &EvidenceRecord) -> StoreResult<()> {
    match e {
        EvidenceRecord::Freshness(v) => {
            v.validate().map_err(StoreError::InvalidInput)?;
            tx.execute("INSERT INTO freshness_evidence(run_id,action_id,action_version,action_hash,enforcement,precondition_external_id,precondition_source_version_token,precondition_payload_hash,observed_source_version_token,observed_payload_hash,result) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",params![v.run_id().as_str(),v.action_id().as_str(),v.action_version().get().to_string(),v.action_hash().as_str(),v.enforcement().as_str(),v.precondition().external_id().as_str(),v.precondition().source_version_token().as_str(),v.precondition().normalized_payload_hash().as_str(),opt_str(v.observed_source_version_token()),opt_str(v.observed_payload_hash()),v.result().as_str()]).map_err(map_sqlite)?;
        }
        EvidenceRecord::Reconciliation(v) => {
            tx.execute("INSERT INTO reconciliation_evidence(run_id,action_id,action_version,action_hash,effect_identity,source_ref,result) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![v.run_id().as_str(),v.action_id().as_str(),v.action_version().get().to_string(),v.action_hash().as_str(),v.effect_identity().as_str(),v.source_ref().as_str(),v.result().as_str()]).map_err(map_sqlite)?;
        }
    }
    Ok(())
}

fn artifact_from_row(row: &Row<'_>) -> rusqlite::Result<Artifact> {
    Artifact::new(
        ArtifactId::new(row.get::<_, String>(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        WorkspaceId::new(row.get::<_, String>(1)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        parse_enum(row.get::<_, String>(2)?)?,
        NonEmptyString::new(row.get::<_, String>(3)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        parse_enum(row.get::<_, String>(4)?)?,
        row.get::<_, String>(5)?,
        Sha256Hex::new(row.get::<_, String>(6)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        ByteCount::new(
            parse_u64(&row.get::<_, String>(7)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        )
        .map_err(|_| rusqlite::Error::InvalidQuery)?,
        OpaqueRef::new(row.get::<_, String>(8)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        Sha256Hex::new(row.get::<_, String>(9)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        EvidenceImportRequestId::new(row.get::<_, String>(10)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        OpaqueRef::new(row.get::<_, String>(11)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        OpaqueRef::new(row.get::<_, String>(12)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        Timestamp::new(row.get::<_, String>(13)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
    )
    .map_err(|_| rusqlite::Error::InvalidQuery)
}
fn task_from_row(row: &Row<'_>) -> rusqlite::Result<AgentTask> {
    AgentTask::new(
        AgentTaskId::new(row.get::<_, String>(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        Version::new(
            parse_u64(&row.get::<_, String>(1)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        )
        .map_err(|_| rusqlite::Error::InvalidQuery)?,
        WorkspaceId::new(row.get::<_, String>(2)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        NonEmptyString::new(row.get::<_, String>(3)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        SurfaceId::new(row.get::<_, String>(4)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        opt_parse(row.get::<_, Option<String>>(5)?)?,
        parse_enum(row.get::<_, String>(6)?)?,
        parse_enum(row.get::<_, String>(7)?)?,
        ActorRef::new(row.get::<_, String>(8)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        Timestamp::new(row.get::<_, String>(9)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
    )
    .map_err(|_| rusqlite::Error::InvalidQuery)
}
fn plan_from_row(row: &Row<'_>) -> rusqlite::Result<ExecutionPlan> {
    let risk: String = row.get::<_, String>(3)?;
    let risk_values = risk.split(',').filter_map(parse_risk).collect::<Vec<_>>();
    let cost = match (
        row.get::<_, Option<String>>(4)?,
        row.get::<_, Option<String>>(5)?,
    ) {
        (Some(a), Some(c)) => Some(
            CostEstimate::new(
                AmountMicros::new(parse_u64(&a).map_err(|_| rusqlite::Error::InvalidQuery)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                CurrencyCode::new(c).map_err(|_| rusqlite::Error::InvalidQuery)?,
            )
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        ),
        _ => None,
    };
    ExecutionPlan::new(
        ExecutionPlanId::new(row.get::<_, String>(0)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        AgentTaskId::new(row.get::<_, String>(2)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        Version::new(
            parse_u64(&row.get::<_, String>(1)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        )
        .map_err(|_| rusqlite::Error::InvalidQuery)?,
        RiskSummary::new(risk_values),
        cost,
        parse_enum(row.get::<_, String>(6)?)?,
        Timestamp::new(row.get::<_, String>(7)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
    )
    .map_err(|_| rusqlite::Error::InvalidQuery)
}
fn action_from_row(row: &Row<'_>) -> rusqlite::Result<Action> {
    let raw: String = row.get::<_, String>(10)?;
    let tuples: Vec<Vec<String>> =
        serde_json::from_str(&raw).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let sources = tuples
        .into_iter()
        .map(|v| {
            SourcePrecondition::new(
                OpaqueRef::new(v.first().cloned().unwrap_or_default())
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                OpaqueRef::new(v.get(1).cloned().unwrap_or_default())
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                Sha256Hex::new(v.get(2).cloned().unwrap_or_default())
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
            )
            .map_err(|_| rusqlite::Error::InvalidQuery)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Action::new(
        ActionId::new(row.get::<_, String>(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        ExecutionPlanId::new(row.get::<_, String>(2)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        Ordinal::new(row.get::<_, u32>(4)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        ActionType::new(row.get::<_, String>(5)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        opt_parse(row.get::<_, Option<String>>(6)?)?,
        parse_enum(row.get::<_, String>(7)?)?,
        parse_enum(row.get::<_, String>(8)?)?,
        OpaqueRef::new(row.get::<_, String>(9)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        sources,
        opt_parse(row.get::<_, Option<String>>(11)?)?,
        Version::new(
            parse_u64(&row.get::<_, String>(1)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        )
        .map_err(|_| rusqlite::Error::InvalidQuery)?,
        Version::new(
            parse_u64(&row.get::<_, String>(3)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        )
        .map_err(|_| rusqlite::Error::InvalidQuery)?,
        Sha256Hex::new(row.get::<_, String>(12)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        CanonicalizerRef::new(
            NonEmptyString::new(row.get::<_, String>(13)?)
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            Version::new(
                parse_u64(&row.get::<_, String>(14)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
            )
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        )
        .map_err(|_| rusqlite::Error::InvalidQuery)?,
        parse_enum(row.get::<_, String>(15)?)?,
        opt_parse(row.get::<_, Option<String>>(16)?)?,
        opt_parse(row.get::<_, Option<String>>(17)?)?,
    )
    .map_err(|_| rusqlite::Error::InvalidQuery)
}
fn approval_from_row(row: &Row<'_>) -> rusqlite::Result<Approval> {
    Approval::new(
        ApprovalId::new(row.get::<_, String>(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        AgentTaskId::new(row.get::<_, String>(1)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        ActionId::new(row.get::<_, String>(2)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        PolicyId::new(row.get::<_, String>(4)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        parse_enum(row.get::<_, String>(5)?)?,
        Timestamp::new(row.get::<_, String>(6)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        opt_parse(row.get::<_, Option<String>>(7)?)?,
        opt_parse(row.get::<_, Option<String>>(8)?)?,
        row.get::<_, Option<String>>(9)?,
        Sha256Hex::new(row.get::<_, String>(10)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        Version::new(
            parse_u64(&row.get::<_, String>(11)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        )
        .map_err(|_| rusqlite::Error::InvalidQuery)?,
        opt_parse(row.get::<_, Option<String>>(12)?)?,
        SurfaceId::new(row.get::<_, String>(13)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        opt_parse(row.get::<_, Option<String>>(14)?)?,
        Version::new(
            parse_u64(&row.get::<_, String>(3)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        )
        .map_err(|_| rusqlite::Error::InvalidQuery)?,
        Sha256Hex::new(row.get::<_, String>(15)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        parse_enum(row.get::<_, String>(16)?)?,
        parse_enum(row.get::<_, String>(17)?)?,
        parse_enum(row.get::<_, String>(18)?)?,
    )
    .map_err(|_| rusqlite::Error::InvalidQuery)
}
fn run_from_row(row: &Row<'_>) -> rusqlite::Result<Run> {
    let attempt = u32::try_from(
        parse_u64(&row.get::<_, String>(4)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
    )
    .map_err(|_| rusqlite::Error::InvalidQuery)?;
    Run::new(
        RunId::new(row.get::<_, String>(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        Version::new(
            parse_u64(&row.get::<_, String>(1)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        )
        .map_err(|_| rusqlite::Error::InvalidQuery)?,
        ActionId::new(row.get::<_, String>(2)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        Attempt::new(attempt).map_err(|_| rusqlite::Error::InvalidQuery)?,
        opt_parse(row.get::<_, Option<String>>(5)?)?,
        opt_parse(row.get::<_, Option<String>>(6)?)?,
        opt_parse(row.get::<_, Option<String>>(7)?)?,
        opt_parse(row.get::<_, Option<String>>(8)?)?,
        parse_enum(row.get::<_, String>(9)?)?,
        parse_enum(row.get::<_, String>(10)?)?,
        opt_parse(row.get::<_, Option<String>>(11)?)?,
        opt_parse(row.get::<_, Option<String>>(12)?)?,
        opt_parse(row.get::<_, Option<String>>(13)?)?,
        Version::new(
            parse_u64(&row.get::<_, String>(3)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        )
        .map_err(|_| rusqlite::Error::InvalidQuery)?,
        Sha256Hex::new(row.get::<_, String>(14)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        ApprovalId::new(row.get::<_, String>(15)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        Version::new(
            parse_u64(&row.get::<_, String>(16)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        )
        .map_err(|_| rusqlite::Error::InvalidQuery)?,
    )
    .map_err(|_| rusqlite::Error::InvalidQuery)
}
fn freshness_from_row(row: &Row<'_>) -> rusqlite::Result<FreshnessEvidence> {
    FreshnessEvidence::new(
        RunId::new(row.get::<_, String>(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        ActionId::new(row.get::<_, String>(1)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        Version::new(
            parse_u64(&row.get::<_, String>(2)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        )
        .map_err(|_| rusqlite::Error::InvalidQuery)?,
        Sha256Hex::new(row.get::<_, String>(3)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        parse_enum(row.get::<_, String>(4)?)?,
        SourcePrecondition::new(
            OpaqueRef::new(row.get::<_, String>(5)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
            OpaqueRef::new(row.get::<_, String>(6)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
            Sha256Hex::new(row.get::<_, String>(7)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        )
        .map_err(|_| rusqlite::Error::InvalidQuery)?,
        opt_parse(row.get::<_, Option<String>>(8)?)?,
        opt_parse(row.get::<_, Option<String>>(9)?)?,
        parse_enum(row.get::<_, String>(10)?)?,
    )
    .map_err(|_| rusqlite::Error::InvalidQuery)
}
fn reconciliation_from_row(row: &Row<'_>) -> rusqlite::Result<ReconciliationEvidence> {
    ReconciliationEvidence::new(
        RunId::new(row.get::<_, String>(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        ActionId::new(row.get::<_, String>(1)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        Version::new(
            parse_u64(&row.get::<_, String>(2)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        )
        .map_err(|_| rusqlite::Error::InvalidQuery)?,
        Sha256Hex::new(row.get::<_, String>(3)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        OpaqueRef::new(row.get::<_, String>(4)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        OpaqueRef::new(row.get::<_, String>(5)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        parse_enum(row.get::<_, String>(6)?)?,
    )
    .map_err(|_| rusqlite::Error::InvalidQuery)
}
fn audit_from_row(row: &Row<'_>) -> rusqlite::Result<AuditRecord> {
    AuditRecord::new(
        AuditId::new(row.get::<_, String>(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        WorkspaceId::new(row.get::<_, String>(1)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        AuditSequence::new(
            parse_u64(&row.get::<_, String>(2)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        )
        .map_err(|_| rusqlite::Error::InvalidQuery)?,
        Timestamp::new(row.get::<_, String>(3)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        opt_parse(row.get::<_, Option<String>>(4)?)?,
        AuditEventType::new(row.get::<_, String>(5)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        AuditSubjectType::new(row.get::<_, String>(6)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        opt_parse(row.get::<_, Option<String>>(7)?)?,
        opt_parse(row.get::<_, Option<String>>(8)?)?,
        opt_parse(row.get::<_, Option<String>>(9)?)?,
        opt_parse(row.get::<_, Option<String>>(10)?)?,
        Sha256Hex::new(row.get::<_, String>(11)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
        Sha256Hex::new(row.get::<_, String>(12)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
    )
    .map_err(|_| rusqlite::Error::InvalidQuery)
}
fn risk_to_db(summary: &RiskSummary) -> String {
    summary
        .as_slice()
        .iter()
        .map(|v| v.as_str())
        .collect::<Vec<_>>()
        .join(",")
}
fn parse_risk(value: &str) -> Option<RiskClass> {
    RiskClass::ALL.iter().copied().find(|v| v.as_str() == value)
}
fn parse_enum<T: EnumParse>(value: String) -> rusqlite::Result<T> {
    T::parse(&value).ok_or(rusqlite::Error::InvalidQuery)
}
trait EnumParse: Sized {
    fn parse(value: &str) -> Option<Self>;
}
impl EnumParse for AgentTaskState {
    fn parse(v: &str) -> Option<Self> {
        match v {
            "draft" => Some(Self::Draft),
            "preflight" => Some(Self::Preflight),
            "awaiting_approval" => Some(Self::AwaitingApproval),
            "queued" => Some(Self::Queued),
            "running" => Some(Self::Running),
            "pausing" => Some(Self::Pausing),
            "paused" => Some(Self::Paused),
            "partially_completed" => Some(Self::PartiallyCompleted),
            "completed" => Some(Self::Completed),
            "canceled" => Some(Self::Canceled),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}
impl EnumParse for ActionState {
    fn parse(v: &str) -> Option<Self> {
        ActionState::ALL.iter().copied().find(|x| x.as_str() == v)
    }
}
impl EnumParse for RunState {
    fn parse(v: &str) -> Option<Self> {
        RunState::ALL.iter().copied().find(|x| x.as_str() == v)
    }
}
impl EnumParse for ApprovalState {
    fn parse(v: &str) -> Option<Self> {
        ApprovalState::ALL.iter().copied().find(|x| x.as_str() == v)
    }
}
impl EnumParse for RiskClass {
    fn parse(v: &str) -> Option<Self> {
        parse_risk(v)
    }
}
impl EnumParse for PrivacyClass {
    fn parse(v: &str) -> Option<Self> {
        PrivacyClass::ALL.iter().copied().find(|x| x.as_str() == v)
    }
}
impl EnumParse for ArtifactKind {
    fn parse(v: &str) -> Option<Self> {
        ArtifactKind::ALL.iter().copied().find(|x| x.as_str() == v)
    }
}
impl EnumParse for EvidenceMediaType {
    fn parse(v: &str) -> Option<Self> {
        EvidenceMediaType::ALL
            .iter()
            .copied()
            .find(|x| x.as_str() == v)
    }
}
impl EnumParse for GateType {
    fn parse(v: &str) -> Option<Self> {
        GateType::ALL.iter().copied().find(|x| x.as_str() == v)
    }
}
impl EnumParse for ConnectorSelection {
    fn parse(v: &str) -> Option<Self> {
        ConnectorSelection::ALL
            .iter()
            .copied()
            .find(|x| x.as_str() == v)
    }
}
impl EnumParse for PolicyDecision {
    fn parse(v: &str) -> Option<Self> {
        PolicyDecision::ALL
            .iter()
            .copied()
            .find(|x| x.as_str() == v)
    }
}
impl EnumParse for ApprovalAssurance {
    fn parse(v: &str) -> Option<Self> {
        ApprovalAssurance::ALL
            .iter()
            .copied()
            .find(|x| x.as_str() == v)
    }
}
impl EnumParse for OutcomeCertainty {
    fn parse(v: &str) -> Option<Self> {
        OutcomeCertainty::ALL
            .iter()
            .copied()
            .find(|x| x.as_str() == v)
    }
}
impl EnumParse for FreshnessEnforcement {
    fn parse(v: &str) -> Option<Self> {
        FreshnessEnforcement::ALL
            .iter()
            .copied()
            .find(|x| x.as_str() == v)
    }
}
impl EnumParse for FreshnessResult {
    fn parse(v: &str) -> Option<Self> {
        FreshnessResult::ALL
            .iter()
            .copied()
            .find(|x| x.as_str() == v)
    }
}
impl EnumParse for ReconciliationResult {
    fn parse(v: &str) -> Option<Self> {
        ReconciliationResult::ALL
            .iter()
            .copied()
            .find(|x| x.as_str() == v)
    }
}
fn opt_parse<T: std::str::FromStr<Err = DomainError>>(
    value: Option<String>,
) -> rusqlite::Result<Option<T>> {
    value
        .map(|v| v.parse().map_err(|_| rusqlite::Error::InvalidQuery))
        .transpose()
}
