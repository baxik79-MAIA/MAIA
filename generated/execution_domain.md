# GENERATED - do not edit

## primitives

```yaml
WorkspaceId:
  wire_type: string
  format: uuid
  uuid_version: 7
  canonical_text: lowercase_hyphenated
  rust_type: distinct_strong_newtype
AgentTaskId:
  wire_type: string
  format: uuid
  uuid_version: 7
  canonical_text: lowercase_hyphenated
  rust_type: distinct_strong_newtype
ExecutionPlanId:
  wire_type: string
  format: uuid
  uuid_version: 7
  canonical_text: lowercase_hyphenated
  rust_type: distinct_strong_newtype
ActionId:
  wire_type: string
  format: uuid
  uuid_version: 7
  canonical_text: lowercase_hyphenated
  rust_type: distinct_strong_newtype
RunId:
  wire_type: string
  format: uuid
  uuid_version: 7
  canonical_text: lowercase_hyphenated
  rust_type: distinct_strong_newtype
ApprovalId:
  wire_type: string
  format: uuid
  uuid_version: 7
  canonical_text: lowercase_hyphenated
  rust_type: distinct_strong_newtype
ConnectorProfileId:
  wire_type: string
  format: uuid
  uuid_version: 7
  canonical_text: lowercase_hyphenated
  rust_type: distinct_strong_newtype
ModelProfileId:
  wire_type: string
  format: uuid
  uuid_version: 7
  canonical_text: lowercase_hyphenated
  rust_type: distinct_strong_newtype
AuditId:
  wire_type: string
  format: uuid
  uuid_version: 7
  canonical_text: lowercase_hyphenated
  rust_type: distinct_strong_newtype
ArtifactId:
  wire_type: string
  format: uuid
  uuid_version: 7
  canonical_text: lowercase_hyphenated
  rust_type: distinct_strong_newtype
EvidenceImportRequestId:
  wire_type: string
  format: uuid
  uuid_version: 7
  canonical_text: lowercase_hyphenated
  rust_type: distinct_strong_newtype
TenantId:
  wire_type: string
  format: uuid
  uuid_version: 7
  canonical_text: lowercase_hyphenated
  rust_type: distinct_strong_newtype
MembershipId:
  wire_type: string
  format: uuid
  uuid_version: 7
  canonical_text: lowercase_hyphenated
  rust_type: distinct_strong_newtype
EntitlementId:
  wire_type: string
  format: uuid
  uuid_version: 7
  canonical_text: lowercase_hyphenated
  rust_type: distinct_strong_newtype
OutcomeId:
  wire_type: string
  format: uuid
  uuid_version: 7
  canonical_text: lowercase_hyphenated
  rust_type: distinct_strong_newtype
UsageRecordId:
  wire_type: string
  format: uuid
  uuid_version: 7
  canonical_text: lowercase_hyphenated
  rust_type: distinct_strong_newtype
Utf8String:
  wire_type: string
  encoding: UTF-8
NonEmptyString:
  wire_type: string
  encoding: UTF-8
  min_length: 1
PolicyId:
  wire_type: string
  encoding: UTF-8
  min_length: 1
  semantics: opaque
  normalization: none
  uuid_required: false
ActorRef:
  wire_type: string
  encoding: UTF-8
  min_length: 1
  semantics: opaque
  normalization: none
  represents: authorized_user_service_or_policy_actor
OpaqueRef:
  wire_type: string
  encoding: UTF-8
  min_length: 1
  semantics: opaque
  normalization: none
  infer_path_or_uri_semantics: false
SurfaceId:
  wire_type: string
  encoding: UTF-8
  min_length: 1
  registry: open
  examples_non_normative:
  - desktop
  - teams
  - outlook
  - web
ActionType:
  wire_type: string
  pattern: ^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$
Timestamp:
  wire_type: string
  format: RFC3339
  timezone: UTC
  canonical_serialization: YYYY-MM-DDTHH:MM:SS.sssZ
  fractional_second_digits: 3
  canonical_timezone_suffix: Z
  parser_acceptance: valid_RFC3339_may_be_accepted_and_normalized
Version:
  wire_type: integer
  signed: false
  bits: 64
  minimum: 1
  maximum: 18446744073709551615
  monotonic_scope: versioned_aggregate_or_object
  monotonically_increases: true
  overflow: fail_closed_never_wrap_saturate_or_reuse
Ordinal:
  wire_type: integer
  signed: false
  bits: 32
  minimum: 0
  maximum: 4294967295
Attempt:
  wire_type: integer
  signed: false
  bits: 32
  minimum: 1
  maximum: 4294967295
AuditSequence:
  wire_type: integer
  signed: false
  bits: 64
  minimum: 1
  maximum: 18446744073709551615
  monotonic_scope: workspace_audit_chain
  monotonically_increases: true
  overflow: fail_closed_never_wrap_saturate_or_reuse
AmountMicros:
  wire_type: integer
  signed: false
  bits: 64
  minimum: 0
  maximum: 18446744073709551615
AmountMinor:
  wire_type: integer
  signed: false
  bits: 64
  minimum: 0
  maximum: 18446744073709551615
UsageQuantity:
  wire_type: integer
  signed: false
  bits: 64
  minimum: 0
  maximum: 18446744073709551615
ByteCount:
  wire_type: integer
  signed: false
  bits: 64
  minimum: 0
  maximum: 18446744073709551615
LineNumber:
  wire_type: integer
  signed: false
  bits: 64
  minimum: 1
  maximum: 18446744073709551615
Sha256Hex:
  wire_type: string
  pattern: ^[0-9a-f]{64}$
CurrencyCode:
  wire_type: string
  pattern: ^[A-Z]{3}$
  semantics: ISO-4217-compatible_code_shape
PrivacyClass:
  wire_type: string
  enum_source: spec/modes.yaml#privacy_classes
  source_selection: mapping_keys
RiskClass:
  wire_type: string
  enum_source: spec/approval.yaml#risk_classes
  source_selection: mapping_keys
GateType:
  wire_type: string
  enum_source: spec/approval.yaml#gate_types
  source_selection: mapping_keys
AgentTaskState:
  wire_type: string
  enum_source: spec/states.yaml#machines.AgentTaskState.states
  transitions_source: spec/states.yaml#machines.AgentTaskState.transitions
ActionState:
  wire_type: string
  enum_source: spec/states.yaml#machines.ActionState.states
  transitions_source: spec/states.yaml#machines.ActionState.transitions
RunState:
  wire_type: string
  enum_source: spec/states.yaml#machines.RunState.states
  transitions_source: spec/states.yaml#machines.RunState.transitions
ApprovalState:
  wire_type: string
  enum_source: spec/states.yaml#machines.ApprovalState.states
  transitions_source: spec/states.yaml#machines.ApprovalState.transitions
ConnectorSelection:
  wire_type: string
  enum_source: spec/action_binding.yaml#connector_selection.values
PolicyDecision:
  wire_type: string
  enum_source: spec/approval.yaml#policy_decision.values
ApprovalAssurance:
  wire_type: string
  enum_source: spec/approval.yaml#assurance.values
ApprovalBindingValidity:
  wire_type: string
  enum_source: spec/approval.yaml#binding_validity.values
ReconciliationResult:
  wire_type: string
  enum_source: spec/execution.yaml#reconciliation.values
FreshnessResult:
  wire_type: string
  enum_source: spec/execution.yaml#freshness.values
FreshnessEnforcement:
  wire_type: string
  enum_source: spec/execution.yaml#freshness.enforcement
RecipientBoundary:
  wire_type: string
  enum_source: spec/approval.yaml#recipient_boundary.values
AuditEventType:
  wire_type: string
  encoding: UTF-8
  min_length: 1
  semantics: validated_registry_identifier
AuditSubjectType:
  wire_type: string
  encoding: UTF-8
  min_length: 1
  semantics: validated_registry_identifier
ArtifactKind:
  wire_type: string
  enum_source: spec/domain.yaml#artifact_snapshot.artifact_kinds
EvidenceMediaType:
  wire_type: string
  enum_source: spec/domain.yaml#artifact_snapshot.media_types
OutcomeStatus:
  wire_type: string
  enum_source: spec/outcomes.yaml#lifecycle.statuses
ReasoningAssuranceLevel:
  wire_type: string
  enum_source: spec/reasoning.yaml#assurance_levels
  source_selection: mapping_keys
UsageCostStatus:
  wire_type: string
  enum_source: spec/commercial.yaml#usage.cost_statuses
```

## value_objects

```yaml
CostEstimate:
  fields:
    amount_micros:
      type: AmountMicros
      required: true
      nullable: false
    currency:
      type: CurrencyCode
      required: true
      nullable: false
  amount_unit: millionths_of_major_currency_unit
  floating_point_allowed: false
  informational_only: true
  authorizes_execution: false
  lowers_risk: false
SourcePrecondition:
  fields:
    external_id:
      type: OpaqueRef
      required: true
      nullable: false
    source_version_token:
      type: OpaqueRef
      required: true
      nullable: false
    normalized_payload_hash:
      type: Sha256Hex
      required: true
      nullable: false
  semantics: freshness_condition_for_mutable_external_source
  connector_context: Action.connector_profile_id
  mismatch_policy_ref: spec/sync.yaml#pre_side_effect_freshness
  connector_specific_semantics: false
OutcomeCertainty:
  wire_type: string
  values:
  - not_applicable
  - known
  - unknown
  state_source: spec/states.yaml#machines.RunState.states
  state_coupling:
    created: not_applicable
    starting: not_applicable
    running: not_applicable
    outcome_unknown: unknown
    reconciling: unknown
    retryable_error: known
    completed: known
    failed: known
    canceled: known
  reconciliation_resolution: unknown_to_known_only_when_outcome_resolved_enough_to_leave_reconciliation
RiskSummary:
  wire_type: array
  items: RiskClass
  unique_items: true
  ordering: declaration_order
  ordering_source: spec/approval.yaml#risk_classes
  informational_only: true
  replaces_per_action_risk_or_gate: false
CanonicalizerRef:
  fields:
    id:
      type: NonEmptyString
      required: true
      nullable: false
    version:
      type: Version
      required: true
      nullable: false
  semantics: versioned_action_type_specific_canonicalizer
ReconciliationEvidence:
  fields:
    run_id:
      type: RunId
      required: true
      nullable: false
    action_id:
      type: ActionId
      required: true
      nullable: false
    action_version:
      type: Version
      required: true
      nullable: false
    action_hash:
      type: Sha256Hex
      required: true
      nullable: false
    effect_identity:
      type: OpaqueRef
      required: true
      nullable: false
    source_ref:
      type: OpaqueRef
      required: true
      nullable: false
    result:
      type: ReconciliationResult
      required: true
      nullable: false
  rules_ref: spec/execution.yaml#reconciliation
FreshnessEvidence:
  fields:
    run_id:
      type: RunId
      required: true
      nullable: false
    action_id:
      type: ActionId
      required: true
      nullable: false
    action_version:
      type: Version
      required: true
      nullable: false
    action_hash:
      type: Sha256Hex
      required: true
      nullable: false
    enforcement:
      type: FreshnessEnforcement
      required: true
      nullable: false
    precondition:
      type: SourcePrecondition
      required: true
      nullable: false
    observed_source_version_token:
      type: OpaqueRef
      required: false
      nullable: true
    observed_payload_hash:
      type: Sha256Hex
      required: false
      nullable: true
    result:
      type: FreshnessResult
      required: true
      nullable: false
  rules_ref: spec/execution.yaml#freshness
```

## execution_contracts

```yaml
AgentTask:
  fields:
    id:
      type: AgentTaskId
      required: true
      nullable: false
    version:
      type: Version
      required: true
      nullable: false
    workspace_id:
      type: WorkspaceId
      required: true
      nullable: false
    request_text:
      type: NonEmptyString
      required: true
      nullable: false
    origin_surface:
      type: SurfaceId
      required: true
      nullable: false
    origin_ref:
      type: OpaqueRef
      required: false
      nullable: true
    state:
      type: AgentTaskState
      required: true
      nullable: false
    privacy_class:
      type: PrivacyClass
      required: true
      nullable: false
    requested_by:
      type: ActorRef
      required: true
      nullable: false
    created_at:
      type: Timestamp
      required: true
      nullable: false
  rules:
    version_semantics: mutable_record_cas_version
    initial_version: 1
    successful_mutation: expected_version_and_state_then_increment
    overflow: fail_closed
ExecutionPlan:
  fields:
    id:
      type: ExecutionPlanId
      required: true
      nullable: false
    task_id:
      type: AgentTaskId
      required: true
      nullable: false
    version:
      type: Version
      required: true
      nullable: false
    risk_summary:
      type: RiskSummary
      required: true
      nullable: false
    estimated_cost:
      type: CostEstimate
      required: false
      nullable: true
    approval_requirement:
      type: GateType
      required: true
      nullable: false
    created_at:
      type: Timestamp
      required: true
      nullable: false
  rules:
    estimated_cost_absent_or_null: no_reliable_monetary_estimate
    zero_cost: valid_known_estimate
    approval_requirement: strongest_worst_case_gate_under_evaluated_policy_context
    approval_requirement_informational_only: true
    each_action_independently_risk_resolved_and_gated: true
    revision_identity: immutable_id_and_version_pair
    policy_deny: separate_PolicyDecision_deny_never_encoded_as_GateType
Action:
  fields:
    id:
      type: ActionId
      required: true
      nullable: false
    plan_id:
      type: ExecutionPlanId
      required: true
      nullable: false
    ordinal:
      type: Ordinal
      required: true
      nullable: false
    action_type:
      type: ActionType
      required: true
      nullable: false
    connector_profile_id:
      type: ConnectorProfileId
      required: false
      nullable: true
    risk_class:
      type: RiskClass
      required: true
      nullable: false
    state:
      type: ActionState
      required: true
      nullable: false
    input_ref:
      type: OpaqueRef
      required: true
      nullable: false
    source_preconditions:
      type: list<SourcePrecondition>
      required: true
      nullable: false
    result_ref:
      type: OpaqueRef
      required: false
      nullable: true
    version:
      type: Version
      required: true
      nullable: false
    plan_version:
      type: Version
      required: true
      nullable: false
    input_hash:
      type: Sha256Hex
      required: true
      nullable: false
    input_canonicalizer:
      type: CanonicalizerRef
      required: true
      nullable: false
    connector_selection:
      type: ConnectorSelection
      required: true
      nullable: false
    connector_binding_hash:
      type: Sha256Hex
      required: false
      nullable: true
    tool_definition_fingerprint:
      type: Sha256Hex
      required: false
      nullable: true
  rules:
    nonempty_source_preconditions_requires: connector_profile_id
    empty_source_preconditions_valid: true
    revision_contract_ref: spec/action_binding.yaml#revisions
    selection_contract_ref: spec/action_binding.yaml#connector_selection
    mcp_contract_ref: spec/action_binding.yaml#mcp
    duplicate_source_preconditions: reject_exact_tuples
Run:
  fields:
    id:
      type: RunId
      required: true
      nullable: false
    version:
      type: Version
      required: true
      nullable: false
    action_id:
      type: ActionId
      required: true
      nullable: false
    attempt:
      type: Attempt
      required: true
      nullable: false
    requested_connector_id:
      type: ConnectorProfileId
      required: false
      nullable: true
    actual_connector_id:
      type: ConnectorProfileId
      required: false
      nullable: true
    requested_model_id:
      type: ModelProfileId
      required: false
      nullable: true
    actual_model_id:
      type: ModelProfileId
      required: false
      nullable: true
    state:
      type: RunState
      required: true
      nullable: false
    outcome_certainty:
      type: OutcomeCertainty
      required: true
      nullable: false
    reconciliation_ref:
      type: OpaqueRef
      required: false
      nullable: true
    started_at:
      type: Timestamp
      required: false
      nullable: true
    ended_at:
      type: Timestamp
      required: false
      nullable: true
    action_version:
      type: Version
      required: true
      nullable: false
    action_hash:
      type: Sha256Hex
      required: true
      nullable: false
    approval_id:
      type: ApprovalId
      required: true
      nullable: false
    approval_version:
      type: Version
      required: true
      nullable: false
  rules:
    version_semantics: mutable_record_cas_version_not_attempt_or_action_revision
    initial_version: 1
    successful_mutation: expected_version_and_state_then_increment
    overflow: fail_closed
    executor_ids_independently_optional: true
    actual_identity_divergence_requires: explicit_visible_policy_compliant_routing_or_fallback_decision
    hidden_fallback_allowed: false
    execution_binding_ref: spec/execution.yaml#run
    attempt_scope_ref: spec/execution.yaml#attempts
Approval:
  fields:
    id:
      type: ApprovalId
      required: true
      nullable: false
    task_id:
      type: AgentTaskId
      required: true
      nullable: false
    action_id:
      type: ActionId
      required: true
      nullable: false
    policy_id:
      type: PolicyId
      required: true
      nullable: false
    state:
      type: ApprovalState
      required: true
      nullable: false
    requested_at:
      type: Timestamp
      required: true
      nullable: false
    decided_at:
      type: Timestamp
      required: false
      nullable: true
    decided_by:
      type: ActorRef
      required: false
      nullable: true
    decision_note:
      type: Utf8String
      required: false
      nullable: true
    action_hash:
      type: Sha256Hex
      required: true
      nullable: false
    version:
      type: Version
      required: true
      nullable: false
    expires_at:
      type: Timestamp
      required: false
      nullable: true
    origin_surface:
      type: SurfaceId
      required: true
      nullable: false
    decided_surface:
      type: SurfaceId
      required: false
      nullable: true
    action_version:
      type: Version
      required: true
      nullable: false
    policy_snapshot_hash:
      type: Sha256Hex
      required: true
      nullable: false
    policy_decision:
      type: PolicyDecision
      required: true
      nullable: false
    required_assurance:
      type: ApprovalAssurance
      required: true
      nullable: false
    achieved_assurance:
      type: ApprovalAssurance
      required: true
      nullable: false
  rules:
    binding_ref: spec/approval.yaml#record
    authorization_ref: spec/approval.yaml#execution_authorization
    cas_ref: spec/approval.yaml#concurrency
AuditRecord:
  fields:
    audit_id:
      type: AuditId
      required: true
      nullable: false
    workspace_id:
      type: WorkspaceId
      required: true
      nullable: false
    sequence:
      type: AuditSequence
      required: true
      nullable: false
    recorded_at:
      type: Timestamp
      required: true
      nullable: false
    actor_ref:
      type: ActorRef
      required: false
      nullable: true
    event_type:
      type: AuditEventType
      required: true
      nullable: false
    subject_type:
      type: AuditSubjectType
      required: true
      nullable: false
    subject_ref:
      type: OpaqueRef
      required: false
      nullable: true
    operation_ref:
      type: OpaqueRef
      required: false
      nullable: true
    decision_ref:
      type: OpaqueRef
      required: false
      nullable: true
    metadata_hash:
      type: Sha256Hex
      required: false
      nullable: true
    prev_hash:
      type: Sha256Hex
      required: true
      nullable: false
    record_hash:
      type: Sha256Hex
      required: true
      nullable: false
  rules_ref: spec/audit.yaml#record
```
