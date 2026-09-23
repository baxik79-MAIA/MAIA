//! OS-neutral repository and transaction contracts for the authoritative MAIA store.
//!
//! This crate contains no SQL, filesystem access, policy evaluation, connector
//! calls or side effects.  Adapters implement the small, responsibility-scoped
//! repository traits below.
#![forbid(unsafe_code)]

use maia_briefing::{BriefingPacket, ValidatedBriefingResult};
use maia_domain::{
    Action, ActionId, AgentTask, AgentTaskId, AgentTaskState, Approval, ApprovalAssurance,
    ApprovalId, ApprovalState, Artifact, ArtifactId, AuditEventType, AuditId, AuditRecord,
    AuditSubjectType, DomainError, ExecutionPlan, ExecutionPlanId, FreshnessEvidence,
    ReconciliationEvidence, Run, RunId, RunState, Sha256Hex, Timestamp, Version,
};

mod audit;
mod error;

pub use audit::{AuditEventDraft, audit_record_projection, compute_audit_hash, make_audit_record};
pub use error::{StoreError, StoreResult};

/// A T1 action revision operation.  The adapter must apply the action,
/// optional old-approval supersession, optional new approval and audit append
/// in one serialized authority transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionRevisionCommand {
    pub action: Action,
    pub action_hash: Sha256Hex,
    pub supersede_approval: Option<ApprovalSupersession>,
    pub new_approval: Option<Approval>,
    pub audit: AuditEventDraft,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalSupersession {
    pub approval_id: ApprovalId,
    pub expected_version: Version,
    pub expected_state: ApprovalState,
    pub next_state: ApprovalState,
}

/// T2 approval decision CAS.  `candidate` contains the complete post-mutation
/// record; immutable binding and policy snapshot fields must be preserved by
/// the adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalDecisionCommand {
    pub approval_id: ApprovalId,
    pub expected_version: Version,
    pub expected_state: ApprovalState,
    pub candidate: Approval,
    pub audit: AuditEventDraft,
}

/// T3 run creation.  `current_authorization_valid` is an input from the
/// policy/orchestrator layer.  The store only revalidates persisted facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunCreationCommand {
    pub task_id: AgentTaskId,
    pub run: Run,
    pub current_authorization_valid: bool,
    pub audit: AuditEventDraft,
}

/// Evidence is immutable and scoped to one exact Run/action binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceRecord {
    Freshness(FreshnessEvidence),
    Reconciliation(ReconciliationEvidence),
}

/// T4 run transition/evidence CAS.  The candidate must be the complete
/// post-transition record with version equal to the expected version successor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunTransitionCommand {
    pub run_id: RunId,
    pub expected_version: Version,
    pub expected_state: RunState,
    pub candidate: Run,
    pub evidence: Option<EvidenceRecord>,
    pub audit: AuditEventDraft,
}

/// T5 task pause request.  The transition is deliberately only the canonical
/// `running -> pausing` request; quiescence is determined by the adapter from
/// all in-flight runs, including superseded revisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskPauseCommand {
    pub task_id: AgentTaskId,
    pub expected_version: Version,
    pub expected_state: AgentTaskState,
    pub audit: AuditEventDraft,
}

/// Immutable plan/action revision access and T1 material revision port.
pub trait RevisionRepository {
    fn insert_plan_revision(&self, plan: &ExecutionPlan) -> StoreResult<()>;
    fn insert_action_revision(&self, command: &ActionRevisionCommand) -> StoreResult<()>;
    fn get_plan_revision(
        &self,
        id: &ExecutionPlanId,
        version: Version,
    ) -> StoreResult<ExecutionPlan>;
    fn get_action_revision(&self, id: &ActionId, version: Version) -> StoreResult<Action>;
    fn get_current_action(&self, id: &ActionId) -> StoreResult<Action>;
}

/// Mutable AgentTask access and the serialized T5 pause request.
pub trait TaskRepository {
    fn insert_task(&self, task: &AgentTask) -> StoreResult<()>;
    fn get_task(&self, id: &AgentTaskId) -> StoreResult<AgentTask>;
    fn pause_task(&self, command: &TaskPauseCommand) -> StoreResult<AgentTask>;
}

/// Approval history and serialized T2 decision CAS.
pub trait ApprovalRepository {
    fn insert_approval(&self, approval: &Approval) -> StoreResult<()>;
    fn get_approval(&self, id: &ApprovalId) -> StoreResult<Approval>;
    fn decide_approval(&self, command: &ApprovalDecisionCommand) -> StoreResult<Approval>;
}

/// Run attempts, serialized T3 creation and T4 transition/evidence CAS.
pub trait RunRepository {
    fn create_run(&self, command: &RunCreationCommand) -> StoreResult<Run>;
    fn get_run(&self, id: &RunId) -> StoreResult<Run>;
    fn transition_run(&self, command: &RunTransitionCommand) -> StoreResult<Run>;
}

/// Immutable evidence persistence and exact binding lookup.
pub trait EvidenceRepository {
    fn append_evidence(&self, evidence: &EvidenceRecord) -> StoreResult<()>;
    fn get_evidence_for_run(&self, run_id: &RunId) -> StoreResult<Vec<EvidenceRecord>>;
}

/// Immutable local evidence snapshots. Host acquisition happens before this
/// port; this port accepts only the acquired MAIA-owned snapshot.
pub trait ArtifactRepository {
    fn persist_artifact(
        &self,
        artifact: &Artifact,
        audit: &AuditEventDraft,
    ) -> StoreResult<Artifact>;
    fn get_artifact(&self, id: &ArtifactId) -> StoreResult<Artifact>;
    /// Lists immutable snapshots for the authoritative workspace only.
    fn list_artifacts(&self) -> StoreResult<Vec<Artifact>>;
}

/// Immutable, MAIA-owned accepted briefing results. Raw provider responses are
/// intentionally absent: they must be validated before reaching this port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BriefingPersistence {
    pub result_id: String,
    pub workspace_id: String,
    pub packet: BriefingPacket,
    pub packet_hash: String,
    pub requested_provider: String,
    pub requested_model: String,
    pub actual_provider: Option<String>,
    pub actual_model: Option<String>,
    pub assurance: String,
    pub result: ValidatedBriefingResult,
    pub result_hash: String,
    pub created_at: Timestamp,
}
pub trait BriefingRepository {
    fn persist_briefing(
        &self,
        record: &BriefingPersistence,
        audit: &AuditEventDraft,
    ) -> StoreResult<()>;
    fn get_briefing(&self, result_id: &str) -> StoreResult<BriefingPersistence>;
    fn list_briefings(&self) -> StoreResult<Vec<BriefingPersistence>>;
}

/// Workspace-scoped append-only audit chain.
pub trait AuditRepository {
    fn append_audit(&self, event: &AuditEventDraft) -> StoreResult<AuditRecord>;
    fn list_audit(&self, workspace_id: &maia_domain::WorkspaceId) -> StoreResult<Vec<AuditRecord>>;
    fn verify_audit_chain(&self, workspace_id: &maia_domain::WorkspaceId) -> StoreResult<()>;
}

/// Marker/health surface for an authoritative adapter.  A writable adapter
/// must fail closed after losing its authority lock.
pub trait AuthorityStore {
    fn is_writable_authority(&self) -> bool;
    fn integrity_check(&self) -> StoreResult<()>;
}

/// Convenience constructor for the common audit event shape.  Keeping this in
/// the contract crate makes transaction call sites explicit and testable.
#[allow(clippy::too_many_arguments)]
pub fn audit_event(
    audit_id: AuditId,
    workspace_id: maia_domain::WorkspaceId,
    recorded_at: Timestamp,
    actor_ref: Option<maia_domain::ActorRef>,
    event_type: AuditEventType,
    subject_type: AuditSubjectType,
    subject_ref: Option<maia_domain::OpaqueRef>,
    operation_ref: Option<maia_domain::OpaqueRef>,
    decision_ref: Option<maia_domain::OpaqueRef>,
    metadata_hash: Option<Sha256Hex>,
) -> AuditEventDraft {
    AuditEventDraft {
        audit_id,
        workspace_id,
        recorded_at,
        actor_ref,
        event_type,
        subject_type,
        subject_ref,
        operation_ref,
        decision_ref,
        metadata_hash,
    }
}

/// A small helper for adapters validating exact immutable bindings.
pub fn exact_run_binding_matches(
    run: &Run,
    action_id: &ActionId,
    action_version: Version,
    action_hash: &Sha256Hex,
    approval_id: &ApprovalId,
    approval_version: Version,
) -> bool {
    run.action_id() == action_id
        && *run.action_version() == action_version
        && run.action_hash() == action_hash
        && run.approval_id() == approval_id
        && *run.approval_version() == approval_version
}

pub fn assurance_is_sufficient(achieved: ApprovalAssurance, required: ApprovalAssurance) -> bool {
    match required {
        ApprovalAssurance::None => true,
        ApprovalAssurance::Confirm => {
            matches!(
                achieved,
                ApprovalAssurance::Confirm | ApprovalAssurance::ElevatedConfirm
            )
        }
        ApprovalAssurance::ElevatedConfirm => achieved == ApprovalAssurance::ElevatedConfirm,
    }
}

impl From<DomainError> for StoreError {
    fn from(value: DomainError) -> Self {
        Self::InvalidInput(value)
    }
}
