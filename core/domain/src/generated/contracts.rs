// GENERATED / DO NOT EDIT. Run python tools/generate_rust_domain.py.
// Canonical inputs: spec/domain.yaml, spec/approval.yaml, spec/modes.yaml, spec/states.yaml, spec/action_binding.yaml, spec/execution.yaml, spec/audit.yaml, spec/reasoning.yaml, spec/outcomes.yaml, spec/commercial.yaml, spec/metrics.yaml
// Audit contract snapshot: {"authority":{"append_serialized_with_authority_mutation":true,"one_chain_per_workspace_authority":true},"chain":{"append":"immutable","genesis":{"prev_hash":"0000000000000000000000000000000000000000000000000000000000000000","sequence":1},"scope":"per_workspace","sequence":{"allocated_atomically_by_authoritative_store":true,"monotonically_increases":true,"overflow":"fail_closed","starts_at":1,"type":"unsigned_64"}},"contract":"authoritative_workspace_audit","evidence_snapshot_audit":{"content_or_raw_locator_in_audit":"forbidden","event_types":["workspace_evidence_imported","workspace_evidence_import_idempotent","workspace_evidence_import_refused"],"operation_ref":"EvidenceImportRequestId","provenance_reference":"source_locator_hash_only","success_and_idempotent_subject_type":"artifact"},"hash":{"algorithm":"SHA-256","canonicalization":"RFC8785-JCS","excluded_fields":["record_hash"],"input_encoding":"UTF-8","prev_hash_in_projection":true,"projection":"AuditRecordV1","record_hash_must_match_projection":true},"invalid_transition":{"entity_is_unchanged":true,"refusal_audit_atomicity":"separate_atomic_append","refusal_audit_is_allowed":true},"record":{"append_only":true,"erasure_reference_strategy":"opaque_or_tombstone_reference","fields":{"actor_ref":{"nullable":true,"required":false,"type":"ActorRef"},"audit_id":{"nullable":false,"required":true,"type":"AuditId"},"decision_ref":{"nullable":true,"required":false,"type":"OpaqueRef"},"event_type":{"nullable":false,"required":true,"type":"AuditEventType"},"metadata_hash":{"nullable":true,"required":false,"type":"Sha256Hex"},"operation_ref":{"nullable":true,"required":false,"type":"OpaqueRef"},"prev_hash":{"nullable":false,"required":true,"type":"Sha256Hex"},"record_hash":{"nullable":false,"required":true,"type":"Sha256Hex"},"recorded_at":{"nullable":false,"required":true,"type":"Timestamp"},"sequence":{"nullable":false,"required":true,"type":"AuditSequence"},"subject_ref":{"nullable":true,"required":false,"type":"OpaqueRef"},"subject_type":{"nullable":false,"required":true,"type":"AuditSubjectType"},"workspace_id":{"nullable":false,"required":true,"type":"WorkspaceId"}},"identity":["workspace_id","sequence"],"no_hard_delete_in_runtime":true,"privacy":{"default_minimal_facts_only":true,"direct_personal_data_minimized_before_append":true,"forbidden_by_default":["message_body","attachment_bytes","evidence_snapshot_content","source_locator","credentials","access_tokens","full_sensitive_payloads"]},"registry_identifiers":{"event_type":"validated_registry_string","subject_type":"validated_registry_string"}},"schema_version":1}
use crate::*;

string_type!(WorkspaceId, validate_uuid_v7);
string_type!(AgentTaskId, validate_uuid_v7);
string_type!(ExecutionPlanId, validate_uuid_v7);
string_type!(ActionId, validate_uuid_v7);
string_type!(RunId, validate_uuid_v7);
string_type!(ApprovalId, validate_uuid_v7);
string_type!(ConnectorProfileId, validate_uuid_v7);
string_type!(ModelProfileId, validate_uuid_v7);
string_type!(AuditId, validate_uuid_v7);
string_type!(ArtifactId, validate_uuid_v7);
string_type!(EvidenceImportRequestId, validate_uuid_v7);
string_type!(TenantId, validate_uuid_v7);
string_type!(MembershipId, validate_uuid_v7);
string_type!(EntitlementId, validate_uuid_v7);
string_type!(OutcomeId, validate_uuid_v7);
string_type!(UsageRecordId, validate_uuid_v7);
pub type Utf8String = String;
string_type!(NonEmptyString, validate_nonempty);
string_type!(PolicyId, validate_nonempty);
string_type!(ActorRef, validate_nonempty);
string_type!(OpaqueRef, validate_nonempty);
string_type!(SurfaceId, validate_nonempty);
string_type!(ActionType, validate_action_type);
string_type!(Timestamp, validate_timestamp);
integer_type!(Version, u64, 1);
integer_type!(Ordinal, u32, 0);
integer_type!(Attempt, u32, 1);
integer_type!(AuditSequence, u64, 1);
integer_type!(AmountMicros, u64, 0);
integer_type!(AmountMinor, u64, 0);
integer_type!(UsageQuantity, u64, 0);
integer_type!(ByteCount, u64, 0);
integer_type!(LineNumber, u64, 1);
string_type!(Sha256Hex, validate_sha256);
string_type!(CurrencyCode, validate_currency);
canonical_enum!(PrivacyClass {
    LocalOnly => "local_only",
    TenantOnly => "tenant_only",
    ControlledExternal => "controlled_external",
    ExternalAllowed => "external_allowed",
});
canonical_enum!(RiskClass {
    Read => "read",
    Analyze => "analyze",
    Draft => "draft",
    WriteInternal => "write_internal",
    SendInternal => "send_internal",
    SendExternal => "send_external",
    Destructive => "destructive",
    Privileged => "privileged",
});
canonical_enum!(GateType {
    Allow => "allow",
    Policy => "policy",
    Confirm => "confirm",
    ElevatedConfirm => "elevated_confirm",
});
canonical_enum!(AgentTaskState {
    Draft => "draft",
    Preflight => "preflight",
    AwaitingApproval => "awaiting_approval",
    Queued => "queued",
    Running => "running",
    Pausing => "pausing",
    Paused => "paused",
    PartiallyCompleted => "partially_completed",
    Completed => "completed",
    Canceled => "canceled",
    Failed => "failed",
});
canonical_fsm!(AgentTaskState {
    Draft => Preflight,
    Draft => Canceled,
    Preflight => AwaitingApproval,
    Preflight => Queued,
    Preflight => Failed,
    Preflight => Canceled,
    AwaitingApproval => Queued,
    AwaitingApproval => Canceled,
    AwaitingApproval => Failed,
    Queued => Running,
    Queued => Canceled,
    Queued => Failed,
    Running => Pausing,
    Running => PartiallyCompleted,
    Running => Completed,
    Running => Failed,
    Running => Canceled,
    Paused => Queued,
    Paused => Canceled,
    PartiallyCompleted => Queued,
    PartiallyCompleted => Completed,
    PartiallyCompleted => Failed,
    PartiallyCompleted => Canceled,
    Pausing => Paused,
    Pausing => PartiallyCompleted,
    Pausing => Completed,
    Pausing => Failed,
    Pausing => Canceled,
});
canonical_enum!(ActionState {
    Planned => "planned",
    Gated => "gated",
    AwaitingApproval => "awaiting_approval",
    Queued => "queued",
    Running => "running",
    Reconciling => "reconciling",
    RetryableError => "retryable_error",
    Completed => "completed",
    Skipped => "skipped",
    Failed => "failed",
    Canceled => "canceled",
});
canonical_fsm!(ActionState {
    Planned => Gated,
    Planned => Canceled,
    Gated => AwaitingApproval,
    Gated => Queued,
    Gated => Skipped,
    Gated => Failed,
    AwaitingApproval => Queued,
    AwaitingApproval => Canceled,
    AwaitingApproval => Failed,
    Queued => Running,
    Queued => Canceled,
    Running => Reconciling,
    Running => RetryableError,
    Running => Completed,
    Running => Failed,
    Running => Canceled,
    RetryableError => Queued,
    RetryableError => Failed,
    RetryableError => Canceled,
    Reconciling => Completed,
    Reconciling => RetryableError,
    Reconciling => Failed,
    Reconciling => Canceled,
});
canonical_enum!(RunState {
    Created => "created",
    Starting => "starting",
    Running => "running",
    RetryableError => "retryable_error",
    OutcomeUnknown => "outcome_unknown",
    Reconciling => "reconciling",
    Completed => "completed",
    Failed => "failed",
    Canceled => "canceled",
});
canonical_fsm!(RunState {
    Created => Starting,
    Created => Canceled,
    Starting => Running,
    Starting => RetryableError,
    Starting => Failed,
    Starting => Canceled,
    Running => RetryableError,
    Running => OutcomeUnknown,
    Running => Completed,
    Running => Failed,
    Running => Canceled,
    OutcomeUnknown => Reconciling,
    Reconciling => Completed,
    Reconciling => RetryableError,
    Reconciling => Failed,
});
canonical_enum!(ApprovalState {
    NotRequired => "not_required",
    Pending => "pending",
    Approved => "approved",
    Rejected => "rejected",
    Expired => "expired",
    Revoked => "revoked",
});
canonical_fsm!(ApprovalState {
    Pending => Approved,
    Pending => Rejected,
    Pending => Expired,
    Pending => Revoked,
    Approved => Revoked,
});
canonical_enum!(ConnectorSelection {
    None => "none",
    Fixed => "fixed",
    PolicyRouted => "policy_routed",
});
canonical_enum!(PolicyDecision {
    Allow => "allow",
    Confirm => "confirm",
    ElevatedConfirm => "elevated_confirm",
    Deny => "deny",
});
canonical_enum!(ApprovalAssurance {
    None => "none",
    Confirm => "confirm",
    ElevatedConfirm => "elevated_confirm",
});
canonical_enum!(ApprovalBindingValidity {
    Valid => "valid",
    StaleAction => "stale_action",
    Expired => "expired",
    PolicyDenied => "policy_denied",
    ActorUnauthorized => "actor_unauthorized",
    InsufficientAssurance => "insufficient_assurance",
});
canonical_enum!(ReconciliationResult {
    EffectConfirmed => "effect_confirmed",
    EffectNotExecuted => "effect_not_executed",
    Unresolved => "unresolved",
});
canonical_enum!(FreshnessResult {
    Matched => "matched",
    Mismatched => "mismatched",
    Unverifiable => "unverifiable",
});
canonical_enum!(FreshnessEnforcement {
    StrongPrecondition => "strong_precondition",
    CompareBeforeExecute => "compare_before_execute",
});
canonical_enum!(RecipientBoundary {
    Internal => "internal",
    External => "external",
    Unknown => "unknown",
});
string_type!(AuditEventType, validate_nonempty);
string_type!(AuditSubjectType, validate_nonempty);
canonical_enum!(ArtifactKind {
    ImportedTextEvidence => "imported_text_evidence",
});
canonical_enum!(EvidenceMediaType {
    Utf8PlainText => "utf8_plain_text",
    Markdown => "markdown",
});
canonical_enum!(OutcomeStatus {
    Candidate => "candidate",
    Observed => "observed",
    Verified => "verified",
    Rejected => "rejected",
    Disputed => "disputed",
    Superseded => "superseded",
});
canonical_enum!(ReasoningAssuranceLevel {
    A0 => "A0",
    A1 => "A1",
    A2 => "A2",
    A3 => "A3",
    A4 => "A4",
});
canonical_enum!(UsageCostStatus {
    Known => "known",
    Unknown => "unknown",
});
canonical_enum!(OutcomeCertainty {
    NotApplicable => "not_applicable",
    Known => "known",
    Unknown => "unknown",
});
impl RunState {
    pub const fn outcome_certainty(self) -> OutcomeCertainty {
        match self {
            Self::Created => OutcomeCertainty::NotApplicable,
            Self::Starting => OutcomeCertainty::NotApplicable,
            Self::Running => OutcomeCertainty::NotApplicable,
            Self::OutcomeUnknown => OutcomeCertainty::Unknown,
            Self::Reconciling => OutcomeCertainty::Unknown,
            Self::RetryableError => OutcomeCertainty::Known,
            Self::Completed => OutcomeCertainty::Known,
            Self::Failed => OutcomeCertainty::Known,
            Self::Canceled => OutcomeCertainty::Known,
        }
    }
}
impl RiskClass {
    pub const fn permits_policy_routing(self) -> bool {
        matches!(self, Self::Read | Self::Analyze | Self::Draft)
    }
}
impl RunState {
    pub const fn is_in_flight(self) -> bool {
        matches!(self, Self::Starting | Self::Running | Self::OutcomeUnknown | Self::Reconciling)
    }
}
impl PolicyDecision {
    pub const fn required_assurance(self) -> Option<ApprovalAssurance> {
        match self {
            Self::Allow => Some(ApprovalAssurance::None),
            Self::Confirm => Some(ApprovalAssurance::Confirm),
            Self::ElevatedConfirm => Some(ApprovalAssurance::ElevatedConfirm),
            Self::Deny => None,
        }
    }
}
impl PolicyDecision {
    pub const fn permits_record_state(self, state: ApprovalState) -> bool {
        match self {
            Self::Allow => matches!(state, ApprovalState::NotRequired),
            Self::Confirm => matches!(state, ApprovalState::Pending | ApprovalState::Approved | ApprovalState::Rejected | ApprovalState::Expired | ApprovalState::Revoked),
            Self::ElevatedConfirm => matches!(state, ApprovalState::Pending | ApprovalState::Approved | ApprovalState::Rejected | ApprovalState::Expired | ApprovalState::Revoked),
            Self::Deny => false,
        }
    }
}
domain_struct!(CostEstimate {
    amount_micros: AmountMicros,
    currency: CurrencyCode,
});
domain_struct!(SourcePrecondition {
    external_id: OpaqueRef,
    source_version_token: OpaqueRef,
    normalized_payload_hash: Sha256Hex,
});
domain_struct!(CanonicalizerRef {
    id: NonEmptyString,
    version: Version,
});
domain_struct!(ReconciliationEvidence {
    run_id: RunId,
    action_id: ActionId,
    action_version: Version,
    action_hash: Sha256Hex,
    effect_identity: OpaqueRef,
    source_ref: OpaqueRef,
    result: ReconciliationResult,
});
domain_struct!(FreshnessEvidence {
    run_id: RunId,
    action_id: ActionId,
    action_version: Version,
    action_hash: Sha256Hex,
    enforcement: FreshnessEnforcement,
    precondition: SourcePrecondition,
    observed_source_version_token: Option<OpaqueRef>,
    observed_payload_hash: Option<Sha256Hex>,
    result: FreshnessResult,
});
domain_struct!(EvidenceImportSource {
    source_locator: OpaqueRef,
    name: NonEmptyString,
    media_type: EvidenceMediaType,
});
domain_struct!(EvidenceImportRequest {
    id: EvidenceImportRequestId,
    workspace_id: WorkspaceId,
    requested_by: ActorRef,
    requested_at: Timestamp,
    sources: Vec<EvidenceImportSource>,
});
domain_struct!(Artifact {
    id: ArtifactId,
    workspace_id: WorkspaceId,
    kind: ArtifactKind,
    name: NonEmptyString,
    media_type: EvidenceMediaType,
    content: Utf8String,
    content_sha256: Sha256Hex,
    byte_length: ByteCount,
    source_locator: OpaqueRef,
    source_locator_hash: Sha256Hex,
    import_request_id: EvidenceImportRequestId,
    trust: OpaqueRef,
    classification: OpaqueRef,
    created_at: Timestamp,
});
domain_struct!(EvidenceCitation {
    workspace_id: WorkspaceId,
    artifact_id: ArtifactId,
    content_sha256: Sha256Hex,
    start_line: Option<LineNumber>,
    end_line: Option<LineNumber>,
});
domain_struct!(Outcome {
    id: OutcomeId,
    workspace_id: WorkspaceId,
    task_id: Option<AgentTaskId>,
    plan_id: Option<ExecutionPlanId>,
    action_id: Option<ActionId>,
    run_id: Option<RunId>,
    commitment_ref: Option<OpaqueRef>,
    project_ref: Option<OpaqueRef>,
    subject_ref: Option<OpaqueRef>,
    expected_result: Option<Utf8String>,
    observed_result: Option<Utf8String>,
    status: OutcomeStatus,
    evidence_refs: Vec<OpaqueRef>,
    verification_method: Option<OpaqueRef>,
    supersedes_outcome_id: Option<OutcomeId>,
    created_at: Timestamp,
    observed_at: Option<Timestamp>,
    verified_at: Option<Timestamp>,
});
domain_struct!(UsageRecord {
    id: UsageRecordId,
    workspace_id: WorkspaceId,
    tenant_id: Option<TenantId>,
    task_id: Option<AgentTaskId>,
    run_id: Option<RunId>,
    provider_ref: Option<OpaqueRef>,
    usage_type: OpaqueRef,
    quantity: UsageQuantity,
    unit: OpaqueRef,
    cost_status: UsageCostStatus,
    cost_minor: Option<AmountMinor>,
    currency: Option<CurrencyCode>,
    pricing_source: Option<OpaqueRef>,
    created_at: Timestamp,
});
domain_struct!(AgentTask {
    id: AgentTaskId,
    version: Version,
    workspace_id: WorkspaceId,
    request_text: NonEmptyString,
    origin_surface: SurfaceId,
    origin_ref: Option<OpaqueRef>,
    state: AgentTaskState,
    privacy_class: PrivacyClass,
    requested_by: ActorRef,
    created_at: Timestamp,
});
domain_struct!(ExecutionPlan {
    id: ExecutionPlanId,
    task_id: AgentTaskId,
    version: Version,
    risk_summary: RiskSummary,
    estimated_cost: Option<CostEstimate>,
    approval_requirement: GateType,
    created_at: Timestamp,
});
domain_struct!(Action {
    id: ActionId,
    plan_id: ExecutionPlanId,
    ordinal: Ordinal,
    action_type: ActionType,
    connector_profile_id: Option<ConnectorProfileId>,
    risk_class: RiskClass,
    state: ActionState,
    input_ref: OpaqueRef,
    source_preconditions: Vec<SourcePrecondition>,
    result_ref: Option<OpaqueRef>,
    version: Version,
    plan_version: Version,
    input_hash: Sha256Hex,
    input_canonicalizer: CanonicalizerRef,
    connector_selection: ConnectorSelection,
    connector_binding_hash: Option<Sha256Hex>,
    tool_definition_fingerprint: Option<Sha256Hex>,
});
domain_struct!(Run {
    id: RunId,
    version: Version,
    action_id: ActionId,
    attempt: Attempt,
    requested_connector_id: Option<ConnectorProfileId>,
    actual_connector_id: Option<ConnectorProfileId>,
    requested_model_id: Option<ModelProfileId>,
    actual_model_id: Option<ModelProfileId>,
    state: RunState,
    outcome_certainty: OutcomeCertainty,
    reconciliation_ref: Option<OpaqueRef>,
    started_at: Option<Timestamp>,
    ended_at: Option<Timestamp>,
    action_version: Version,
    action_hash: Sha256Hex,
    approval_id: ApprovalId,
    approval_version: Version,
});
domain_struct!(Approval {
    id: ApprovalId,
    task_id: AgentTaskId,
    action_id: ActionId,
    policy_id: PolicyId,
    state: ApprovalState,
    requested_at: Timestamp,
    decided_at: Option<Timestamp>,
    decided_by: Option<ActorRef>,
    decision_note: Option<Utf8String>,
    action_hash: Sha256Hex,
    version: Version,
    expires_at: Option<Timestamp>,
    origin_surface: SurfaceId,
    decided_surface: Option<SurfaceId>,
    action_version: Version,
    policy_snapshot_hash: Sha256Hex,
    policy_decision: PolicyDecision,
    required_assurance: ApprovalAssurance,
    achieved_assurance: ApprovalAssurance,
});
domain_struct!(AuditRecord {
    audit_id: AuditId,
    workspace_id: WorkspaceId,
    sequence: AuditSequence,
    recorded_at: Timestamp,
    actor_ref: Option<ActorRef>,
    event_type: AuditEventType,
    subject_type: AuditSubjectType,
    subject_ref: Option<OpaqueRef>,
    operation_ref: Option<OpaqueRef>,
    decision_ref: Option<OpaqueRef>,
    metadata_hash: Option<Sha256Hex>,
    prev_hash: Sha256Hex,
    record_hash: Sha256Hex,
});
