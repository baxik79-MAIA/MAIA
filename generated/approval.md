**read**

- **Znaczenie:** Read existing data within granted scope.

- **Domyślna bramka:** allow

**analyze**

- **Znaczenie:** Transform, summarize, classify or infer without external side effects.

- **Domyślna bramka:** allow

**draft**

- **Znaczenie:** Create unpublished content or draft objects.

- **Domyślna bramka:** allow

**write_internal**

- **Znaczenie:** Modify internal low-risk metadata/task state.

- **Domyślna bramka:** policy

**send_internal**

- **Znaczenie:** Send or post content to recipients inside approved organization boundary.

- **Domyślna bramka:** confirm

**send_external**

- **Znaczenie:** Send/post data outside approved organization boundary.

- **Domyślna bramka:** elevated_confirm

**destructive**

- **Znaczenie:** Delete, cancel, revoke, overwrite or irreversibly modify data.

- **Domyślna bramka:** elevated_confirm

**privileged**

- **Znaczenie:** Change auth, permissions, policies, integrations, keys or admin-relevant settings.

- **Domyślna bramka:** elevated_confirm

**gate:allow**

- **result:** not_required

**gate:policy**

- **if_policy_allows_without_confirmation:** not_required

- **otherwise:** pending

- **if_policy_denies:** no_approval_record_execution_blocked

**gate:confirm**

- **result:** pending

**gate:elevated_confirm**

- **result:** pending

**mutation_policy**

- **on_payload_divergence:** invalidate_binding_and_supersede_state_aware

- **old_binding_valid_for_new_payload_version:** False

- **revocable_prior_states:** ['pending', 'approved']

- **revocable_handling:** {'transition_to': 'revoked', 'reason': 'action_hash_divergence'}

- **preserved_prior_states:** ['not_required', 'rejected', 'expired', 'revoked']

- **preserved_handling:** preserve_historical_state_without_transition

- **new_version_required:** True

- **new_version_steps:** ['build_material_revision_per_action_binding_contract', 'create_new_approval_id_at_record_version_1_unless_policy_denies']

- **new_approval_state_source:** spec/approval.yaml#policy_decision.decision_to_record

- **not_required_is_valid_gate_result:** True

- **build_order_ref:** spec/action_binding.yaml#material_revision_build_order

## Execution authorization contracts

```yaml
policy_decision:
  values:
  - allow
  - confirm
  - elevated_confirm
  - deny
  restriction_order:
  - allow
  - confirm
  - elevated_confirm
  - deny
  composition: maximum_applicable_restriction
  deny_absorbing: true
  layers:
  - tenant
  - workspace
  - user
  lower_layer_may_weaken: false
  default_gate_mapping:
    allow: allow
    confirm: confirm
    elevated_confirm: elevated_confirm
    policy: evaluate_policies_and_grants
  policy_gate_without_explicit_autonomous_permission: confirm
  grant:
    requires_explicit_tenant_workspace_scope_permission: true
    autonomous_reduction_only_for_gate: policy
    cannot_auto_allow_gates:
    - confirm
    - elevated_confirm
    relationship_authority: false
  snapshot:
    hash_field: Approval.policy_snapshot_hash
    semantics: sha256_of_canonical_effective_policy_snapshot
    administration_and_snapshot_materialization: deferred
    reevaluate_before_material_execution: true
  decision_to_record:
    allow: not_required
    confirm: pending
    elevated_confirm: pending
    deny: no_record_policy_evaluation_audited
assurance:
  values:
  - none
  - confirm
  - elevated_confirm
  order:
  - none
  - confirm
  - elevated_confirm
  required_by_decision:
    allow: none
    confirm: confirm
    elevated_confirm: elevated_confirm
  technology_specific_evidence: future_authentication_and_audit_records
record:
  snapshot_state_compatibility:
    allow:
    - not_required
    confirm:
    - pending
    - approved
    - rejected
    - expired
    - revoked
    elevated_confirm:
    - pending
    - approved
    - rejected
    - expired
    - revoked
    deny: []
  pending:
    null_fields:
    - decided_by
    - decided_at
    - decided_surface
    - decision_note
    achieved_assurance: none
  human_rejected_requires:
  - decided_by
  - decided_at
  - decided_surface
  human_rejected_minimum_assurance: confirm
  rejection_requires_approval_level_assurance: false
  rejected_is_human_decision_not_policy_deny: true
  new_id_per_binding: true
  initial_version: 1
  version_scope: Approval_record_CAS_not_Action_revision
  immutable_fields:
  - action_id
  - action_version
  - action_hash
  - policy_id
  - policy_snapshot_hash
  - origin_surface
  - required_assurance
  eligible_revision_requires_record: true
  not_required:
    human_decision: false
    null_fields:
    - decided_by
    - decided_at
    - decided_surface
    achieved_assurance: none
  overflow: fail_closed_never_wrap_saturate_or_reuse
  policy_decision: snapshot_of_binding_evaluation_not_current_policy
  human_approved_requires:
  - decided_by
  - decided_at
  - decided_surface
  human_approved_minimum_assurance: confirm
  mutable_metadata: only_lifecycle_and_decision_metadata_per_legal_transition
binding_validity:
  values:
  - valid
  - stale_action
  - expired
  - policy_denied
  - actor_unauthorized
  - insufficient_assurance
  valid_is_not_execution_authorization: true
  expiry: now_greater_than_or_equal_to_expires_at
  null_expiry: no_time_expiry
  approved_expiry: preserve_approved_history_block_execution
  pending_expiry: legal_pending_to_expired_increment_record_version
  actor_context: currently_authorized_actor_for_the_operation_being_checked
  material_mutation_policy_ref: spec/approval.yaml#concurrency.mutation_policy
  multiple_failures: all_failures_block_execution_no_priority_inferred
execution_authorization:
  all_require:
  - valid_binding
  - current_action_revision_and_hash
  - freshness_checks
  - routing_checks
  - tool_definition_checks
  states_never_authorize:
  - pending
  - rejected
  - revoked
  - expired
  matrix:
    allow:
      states:
      - not_required
      - approved
      minimum_assurance: none
    confirm:
      states:
      - approved
      minimum_assurance: confirm
    elevated_confirm:
      states:
      - approved
      minimum_assurance: elevated_confirm
    deny:
      states: []
      minimum_assurance: null
  uses_current_policy: true
  prior_approval_overrides_deny: false
concurrency:
  decision_model: compare_and_swap
  required_match:
  - approval_id
  - expected_approval_version
  - required_current_state
  - action_id
  - action_version
  - action_hash
  - current_executable_action_revision_and_hash
  first_terminal_decision_wins: true
  stale_or_second_decision_error: ExternalConflict
  material_plan_change: apply_mutation_policy
  mutation_policy:
    on_payload_divergence: invalidate_binding_and_supersede_state_aware
    old_binding_valid_for_new_payload_version: false
    revocable_prior_states:
    - pending
    - approved
    revocable_handling:
      transition_to: revoked
      reason: action_hash_divergence
    preserved_prior_states:
    - not_required
    - rejected
    - expired
    - revoked
    preserved_handling: preserve_historical_state_without_transition
    new_version_required: true
    new_version_steps:
    - build_material_revision_per_action_binding_contract
    - create_new_approval_id_at_record_version_1_unless_policy_denies
    new_approval_state_source: spec/approval.yaml#policy_decision.decision_to_record
    not_required_is_valid_gate_result: true
    build_order_ref: spec/action_binding.yaml#material_revision_build_order
  decision_assurance_ref: spec/approval.yaml#record
  decision_timestamp: authoritative_now_not_request_timestamp
  human_decision_state: pending
  human_decision_destinations:
  - approved
  - rejected
  additional_human_decision_checks:
  - now_before_expires_at_if_present
  - actor_currently_authorized
  - assurance_satisfies_confirmation_requirement_for_approval
  successful_mutation:
    increment_record_version: 1
    immutable_binding_fields_preserved: true
    metadata_matches_transition: true
    overflow: fail_closed
  atomicity: spec/persistence.yaml#atomic_transactions.T2_approval_decision_cas
  lifecycle_mutations: expiry_and_material_revocation_are_system_lifecycle_transitions_not_human_approve_reject_decisions
recipient_boundary:
  input_validation_order:
  - validate_complete_input
  - resolve_boundary_risk
  - preserve_operation_floor
  invalid_input_with_floor: ValidationError_fail_closed
  values:
  - internal
  - external
  - unknown
  trusted_input_required: true
  untrusted_payload_can_self_declare_internal: false
  precedence:
  - external
  - unknown
  - internal
  risk_by_boundary:
    internal: send_internal
    external: send_external
    unknown: send_external
  unresolved_group_membership: unknown_unless_trusted_resolver_proves_boundary
dynamic_risk_classifiers:
  mail_recipient_boundary:
    inputs:
    - to
    - cc
    - bcc
    - effective_recipient_boundaries
    - operation_risk_floor
    outputs:
    - send_internal
    - send_external
    - destructive
    - privileged
    rule: Reject empty effective recipients; external or unknown => send_external,
      all internal => send_internal; preserve explicit destructive/privileged operation
      floor.
    trusted_input_source: trusted_effective_recipient_boundary_resolver
    unknown_behavior: send_external_policy_may_deny
    operation_risk_floor:
      source: explicit_operation_contract
      preserve:
      - destructive
      - privileged
      global_risk_order_inferred: false
    boundary_outputs:
    - send_internal
    - send_external
    empty_recipients: ValidationError_fail_closed
  calendar_participant_boundary:
    inputs:
    - organizer
    - attendees
    - effective_attendee_boundaries
    - operation_kind
    - private_event
    - operation_risk_floor
    outputs:
    - write_internal
    - send_internal
    - send_external
    - destructive
    - privileged
    rule: Private no-attendee event => write_internal; attendee external/unknown =>
      send_external, all internal => send_internal; preserve explicit destructive/privileged
      operation floor.
    trusted_input_source: trusted_effective_recipient_boundary_resolver
    unknown_behavior: send_external_policy_may_deny
    operation_risk_floor:
      source: explicit_operation_contract
      preserve:
      - destructive
      - privileged
      global_risk_order_inferred: false
    boundary_outputs:
    - write_internal
    - send_internal
    - send_external
    incomplete_inputs: ValidationError_fail_closed_except_unknown_boundary_which_is_send_external
```
