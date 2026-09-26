## A.acceptance.yaml

```yaml
schema_version: 1
releases:
  alpha_local_legacy:
  - id: A001
    requirement: Application runs on a supported Windows machine without external commercial AI API keys.
  - id: A002
    requirement: Outlook Classic connector can read/search selected mailbox scope and create drafts through TrustedBridge
      without exposing credentials to MAIA core.
  - id: A003
    requirement: Local model endpoint can analyze messages and generate drafts; all sends require approval.
  - id: A004
    requirement: Task/Plan/Action/Run provenance survives restart and is auditable.
  - id: A005
    requirement: Commitment extraction produces source-linked candidate commitments and never silently marks them
      confirmed.
  - id: A006
    requirement: No plaintext secret exists in SQLite, logs, exports, bundles or fixtures.
  - id: A007
    requirement: Spec validator and generated-contract drift check pass in CI.
  - id: A008
    requirement: PL and EN UI strings are externalized.
  - id: A009
    requirement: All spec/*.yaml parse successfully and Spec Guard rejects risk-enum mismatch, missing dynamic-risk
      resolver references and incomplete FSM transitions.
  - id: A010
    requirement: Windows/COM dependencies are confined to the Outlook Classic adapter/surface boundary; MAIA Core
      has no direct Outlook COM/Win32 business dependency.
  - id: A011
    requirement: 'Task pause is quiescent: no new Run starts after pause request, non-cancellable in-flight side
      effects drain/reconcile, and Task enters paused only when no Run remains in flight. This includes superseded
      revisions and created-to-starting transitions.'
  - id: A012
    requirement: Outlook Classic sidecar uses a dedicated STA COM execution context, handles rejected/busy COM calls
      with bounded retry/message-filter behavior, and reports ConnectorHealth=degraded rather than hanging.
  mvp_universal:
  - id: M001
    requirement: All Alpha criteria pass.
  - id: M002
    requirement: Connector contract supports Outlook Classic and Microsoft Graph without core business logic changes.
  - id: M003
    requirement: Teams agent surface can receive a request, show an approval card and return task result using the
      same task identity as desktop.
  - id: M004
    requirement: Microsoft Graph mail sync uses delta/change notification strategy with recovery path.
  - id: M005
    requirement: User can add, validate, rotate and delete multiple model/API credential profiles without code changes.
  - id: M006
    requirement: WorkGraph links at least people, projects, threads, meetings, decisions and commitments with source
      provenance.
  - id: M007
    requirement: External send, destructive and privileged actions cannot auto-execute under default policy.
  - id: M008
    requirement: MCP client can connect to an approved server and MCP server can expose read-only scoped MAIA tools.
  - id: M009
    requirement: 'Approval decisions are compare-and-swap/versioned: first terminal decision wins and stale or concurrent
      decisions return ExternalConflict.'
  - id: M010
    requirement: MCP tool review/approval is pinned to a canonical tool-definition fingerprint; material definition
      change invalidates the previous trust decision.
  - id: M011
    requirement: Core/spec/connector-contract test suites pass on a non-Windows CI runner; platform-specific connector
      tests remain scoped to their platform.
  - id: M012
    requirement: Teams production topology uses a managed public HTTPS channel endpoint; a local-authoritative Core
      is reached only through an authenticated outbound relay and the cloud adapter never opens local SQLite directly.
  - id: M013
    requirement: MCP tool fingerprints use RFC 8785 JCS + SHA-256 and pass the same canonicalization golden vector
      across every supported runtime implementation.
  - id: M014
    requirement: Payload mutation invalidates every old approval binding for the superseding payload/version, revokes
      pending/approved approvals, preserves not_required/rejected/expired/revoked historical states, and recomputes
      the action hash for a new action/approval version that is risk-resolved and re-gated before execution; not_required
      remains a valid result under the canonical gate mapping.
  - id: M015
    requirement: Approval surfaces render untrusted content through a typed/sanitized presentation policy that blocks
      remote/data-URI resource exfiltration and untrusted Adaptive Card media/navigation elements.
  - id: M016
    requirement: Material side effects derived from mutable external objects perform a connector source-version
      freshness check; version mismatch invalidates stale plan/approval.
  - id: M017
    requirement: Ambiguous non-idempotent side-effect outcomes enter reconciliation and are never blindly retried.
  - id: M018
    requirement: ActionBindingV1 has deterministic RFC8785 JCS/SHA-256 with exact u64 decimal strings, complete
      canonical input, connector identity/scope and visible MCP fingerprint binding.
  - id: M019
    requirement: Immutable Action/Plan revisions and Run authorization record versions remain reconstructible; results
      affect only the exact executed revision.
  - id: M020
    requirement: Current policy/assurance, expiry and versioned CAS constrain execution; not_required is an audit
      binding without a human decision; deny never authorizes.
  - id: M021
    requirement: Every retry creates a new RunId in the exact Action revision attempt scope; unresolved effects
      block equivalent execution across IDs and revisions; authoritative non-execution evidence is required.
  - id: M022
    requirement: Freshness evidence is per Run; unverifiable material freshness fails closed; pause includes superseded
      in-flight revisions and prevents created Runs from starting.
  - id: M023
    requirement: Trusted recipient boundaries classify unknown as external, reject empty mail recipients and preserve
      explicit destructive/privileged operation floors.
  m0_2_1_contract_closure:
  - id: P001
    requirement: AgentTask and Run carry mutable record CAS versions distinct from immutable plan/action revisions and attempts.
  - id: P002
    requirement: Authoritative persistence identities, parent integrity and no-runtime-hard-delete history rules are canonical.
  - id: P003
    requirement: T1-T5 serialized atomic transaction boundaries are canonical and approval creation is not coupled to later Run creation.
  - id: P004
    requirement: Workspace-scoped append-only AuditRecord uses monotonic sequence, RFC8785-JCS SHA-256 hash chaining and privacy-minimized facts.
  - id: P005
    requirement: Local authority, migration ledger, backup/recovery, injected clock and fail-closed persistence error semantics are canonical without implementing SQLite.
  m0_7_execution_intelligence_contract:
  - id: E001
    requirement: Signal, Context, Commitment, Decision, Risk, Plan, Action, Run and Outcome remain distinct canonical concepts.
  - id: E002
    requirement: Business Outcome is separate from Run OutcomeCertainty and can be candidate, observed, verified, rejected, disputed or superseded.
  - id: E003
    requirement: Verified Outcome retains provenance, evidence and an explicit verification method; model confidence alone never verifies an Outcome.
  - id: E004
    requirement: ReasoningAssuranceLevel A0-A4 is independent from RiskClass and never grants execution permission.
  - id: E005
    requirement: A3 Round Table records independent reasoning, disagreement, evidence comparison, adjudication and measured usage without bypassing ApprovalGate.
  - id: E006
    requirement: A4 human reasoning confirmation does not replace any independently required ApprovalGate authorization.
  - id: E007
    requirement: TTFV and TTFA are instrumentable from authorized usable context without treating generic model output, diagnostics or no-op plumbing as qualifying value or action.
  - id: E008
    requirement: Product advantage claims have a named benchmark or instrumented measurement for successful Outcome rate, human effort, cost per successful Outcome, TTFV, TTFA and repeatability.
  - id: E009
    requirement: Unknown cost and unknown human effort remain unknown and are never silently represented as zero.
  - id: E010
    requirement: Personal and local Workspaces may exist without Tenant; commercial organizational mode requires the Tenant seam and fails closed when it is absent.
  - id: E011
    requirement: G2 alerts FORM COMPANY NOW before paid production, recurring B2B commercial contract or external-customer production-data processing; G3 and G4 remain outside Task and Action FSMs.
  - id: E012
    requirement: Microsoft 365 Execution Control remains a replaceable validation hypothesis and does not redefine generic Core execution contracts.
  m0_8_round_table_vertical_slice:
  - id: R001
    requirement: Round Table receives a DecisionRequest, collects response-isolated ParticipantResponses, records disagreement and evidence, adjudicates and returns a reasoning-only RoundTableDecision.
  - id: R002
    requirement: A3 and A4 preserve independent ApprovalGate and Action authorization requirements; RoundTableDecision never authorizes an Action.
  - id: R003
    requirement: Provider-neutral Participant and ModelProvider contracts permit a configured Anthropic adapter without making Claude, an API key or an organization identifier part of Core contracts.
  - id: R004
    requirement: Anthropic Messages adapter uses local credential boundary, bounded timeout, safe transient-only retry, request-id and returned usage capture, with no hidden provider fallback.
  - id: R005
    requirement: Fake participants exercise the vertical slice without network or credentials; the live Anthropic smoke test is explicitly opt-in and skipped by default.
  m0_9_assurance_routing:
  - id: Q001
    requirement: A provider-neutral router selects A0-A4 from explicit assurance signals and policy floor, records reason codes, and never grants execution or ApprovalGate authority.
  - id: Q002
    requirement: A0 is deterministic, A1 uses one enabled participant, A2 requires a meaningfully independent verifier, A3 uses the response-isolated Round Table, and A4 additionally requires recorded human reasoning acceptance.
  - id: Q003
    requirement: The registry records participant identity, provider, model reference, role capability, enabled state, assurance levels, cost-metadata capability and availability health without hardcoding providers or models.
  - id: Q004
    requirement: Estimated cost is checked before reasoning, measured cost is retained afterward when returned, unknown cost is never zero, and a session ceiling fails explicitly without hidden fallback.
  - id: Q005
    requirement: Provider, quota, timeout, verifier, adjudication and required-assurance failures return explicit InsufficientAssurance with a reason code; the router never silently downgrades.
  - id: Q006
    requirement: The audit record contains request, identities and model references, conclusions, evidence, disagreement/adjudication, assurance, usage/cost, provider request IDs when returned, outcome/confidence and A4 human acceptance.
  m0_12_claude_code_subscription_provider:
  - id: C001
    requirement: A ClaudeCodeProvider discovers the locally installed Claude Code executable, records its version, and fails closed with an explicit error when the executable is absent or not authenticated.
  - id: C002
    requirement: The provider invokes Claude Code non-interactively in print mode with an explicit working directory and argument-safe process invocation; no shell command is constructed from packet content.
  - id: C003
    requirement: The child environment is unconditionally scrubbed of ANTHROPIC_API_KEY and equivalent credential or endpoint overrides, so the invocation stays on subscription-backed authentication and never depends on an API key.
  - id: C004
    requirement: Tool authority is restricted by an explicit deny list; bypassPermissions is never used and an empty allowlist is never treated as deny-all.
  - id: C005
    requirement: A versioned ConsultationPacket produces a schema-validated ConsultationResult whose consultation_id matches the request; timeout, cancellation, non-zero exit, malformed JSON, empty response and schema violation all fail closed without fabricating a successful response.
  - id: C006
    requirement: Every invocation persists a local audit record containing the raw response, execution metadata and correlation ID; audit write failure fails the invocation and no scrubbed credential value is ever written.
  - id: C007
    requirement: A recursion guard environment marker prevents a Claude Code invocation from spawning a nested Claude Code invocation.
  - id: C008
    requirement: The provider is absent by construction from a DEPLOYMENT_LOCKED build; subprocess code is not compiled without the development-evolution feature and MAIA Core never depends on the provider.
  m0_13_round_table_live_orchestration:
  - id: D001
    requirement: A failing participant degrades only its own contribution and never aborts collection; surviving participants still produce outcomes, though the session may still terminate as InsufficientAssurance when quorum is no longer satisfied.
  - id: D002
    requirement: Falling below the A3 minimum of two valid independent first-round responses returns InsufficientAssurance with an explicit reason code; no silent downgrade and no single-participant Round Table.
  - id: D003
    requirement: Leader selection is deterministic, registry-driven, recorded, and hardcodes no provider or model.
  - id: D004
    requirement: Two interchangeable providers run the same session through the same provider-neutral port with no MAIA Core change.
  - id: D005
    requirement: Every contribution carries provenance including the model reference actually used.
  - id: D006
    requirement: First-round response isolation is preserved and provable under partial participant failure.
  - id: D007
    requirement: Round Table session history persists locally and survives process restart.
  - id: D008
    requirement: A reasoning-only participant never obtains execution authority and RoundTableDecision.execution_authority remains false on every path.
  - id: D009
    requirement: MAIA Core and shipped applications build and operate with the Round Table implementation absent from their dependency graph.
  - id: D010
    requirement: No participant, provider or model identifier is hardcoded in core/*.
  - id: D011
    requirement: Failed participant attempts remain visible in session provenance with classified failure metadata.
  - id: D012
    requirement: Adjudicator or leader participation never substitutes for a missing first-round quorum.
  - id: D013
    requirement: Session persistence and restart preserve both successful and failed contribution provenance.
  - id: D014
    requirement: Provider resolution failure is isolated and explicitly classified and never silently resolves to another provider.
  beta_enterprise:
  - id: B001
    requirement: Tenant-managed policies can override user policies toward stricter behavior.
  - id: B002
    requirement: Teams proactive briefing and scheduled jobs work with the same approval and privacy gates.
  - id: B003
    requirement: Relationship intelligence and commitment tracking pass source/provenance golden tests.
  - id: B004
    requirement: Connector and model fallbacks are visible before material side effects and persisted afterward.
  - id: B005
    requirement: Threat-model regression, dependency audit, secret scan and prompt-injection tests pass.
  - id: B006
    requirement: Credential-compromise runbook is tested for at least one API-key profile and one OAuth/tenant connector
      profile.
  - id: B007
    requirement: Third-party personal-data governance controls support subject-linked locate/export/rectify/restrict/erase-or-anonymize
      workflows according to configured retention/legal policy.
  - id: B008
    requirement: WorkGraph inferred edges are visibly marked as suggestions, source-linked, and cannot authorize
      side effects until confirmed or explicitly promoted by policy.
  - id: B009
    requirement: Third-party erasure workflow supports policy-driven deletion or pseudonymized random tombstones
      without deterministic hashes of PII and removes/suppresses derived indexes and memory.
  - id: B010
    requirement: Approval-fatigue controls use explicit scoped/expiring grants or approved templates; RelationshipProfile
      familiarity alone never grants execution authority.
  v1_0:
  - id: V001
    requirement: All Beta criteria pass.
  - id: V002
    requirement: 'Classic-to-new-Outlook migration path is tested: disabling COM connector does not orphan MAIA
      history or WorkGraph.'
  - id: V003
    requirement: Accessibility audit meets WCAG AA for applicable desktop/web controls.
  - id: V004
    requirement: No critical/high known vulnerability without documented risk acceptance.
  - id: V005
    requirement: Signed update verification and rollback/recovery runbook pass.
definition_of_done_global:
- tests_pass
- generated_contracts_current
- no_spec_drift
- docs_updated
- migration_added_if_needed
- security_impact_reviewed
- i18n_keys_added
- no_secret_leak
- acceptance_vector_updated
```

## A.action_binding.yaml

```yaml
schema_version: 1
scope: M0.1.1 canonical contract and reference conformance only; no production canonicalizers or resolvers.
revisions:
  action_identity: ActionId_is_logical_step
  revision_identity: [action_id, action_version]
  plan_revision_identity: [plan_id, plan_version]
  immutable_after: [approval_binding, any_Run]
  immutable_fields_source: spec/action_binding.yaml#action_binding.fields
  mutable_exclusions: [state, result_ref, input_ref]
  input_ref_change_cannot_change_input_bytes: true
  historical_revisions_reconstructible: true
  material_edit: next_version_exactly_once
  logical_identity_not_preservable: new_ActionId
  overflow: fail_closed_never_wrap_saturate_or_reuse
canonicalizer:
  reference_type: CanonicalizerRef
  registry_key: [action_type, id, version]
  material_identity_change: true
  material_failure: no_binding_no_execution
  failure_cases: [missing_registration, canonicalization_failure]
  core_may_guess_normalization: false
  production_implementations: deferred
canonical_input:
  name: CanonicalActionInput
  algorithm: sha256(utf8(jcs_rfc8785(canonical_action_input)))
  schema: action_type_specific_registered_contract
  number_domain: I-JSON_IEEE754_exact_semantics_required
  unsafe_integer_input: reject_unless_action_schema_defines_lossless_string_projection
  completeness_when_applicable:
  - resolved_target_account_tenant_resource_identity
  - effective_recipients_or_participants
  - subject_body_content_or_content_addressed_references
  - attachment_identity_and_content_hashes
  - material_operation_options
  - mcp_tool_definition_fingerprint
  - every_value_changing_externally_observable_effect
  input_ref_is_hash: false
  execution_recomputes_input_hash: true
  mutable_locator_mismatch: stale_fail_closed
connector_selection:
  values: [none, fixed, policy_routed]
  material_risks: [write_internal, send_internal, send_external, destructive, privileged]
  none:
    meaning: connectorless_not_late_selection
    null_fields: [connector_profile_id, connector_binding_hash]
  fixed:
    required_fields: [connector_profile_id, connector_binding_hash]
    actual_connector_must_match: true
    current_binding_hash_must_match: true
    mismatch: stale_fail_closed_rebuild_regate
  policy_routed:
    allowed_risks: [read, analyze, draft]
    required: [explicit, policy_compliant, audited, requested_actual_Run_identity]
    scope_broadening: reevaluate_policy_and_resulting_gate
  material_connector_execution_requires: fixed
  binding_field_change_always_requires_new_revision: true
connector_binding:
  name: ConnectorBindingV1
  algorithm: sha256(utf8(jcs_rfc8785(connector_binding_v1)))
  schema_tag: maia.connector-binding.v1
  fields:
    schema: literal_schema_tag
    connector_type: NonEmptyString
    connector_profile_id: ConnectorProfileId
    authority: connector_specific_canonical_account_tenant_identity_object
    resource_scope: connector_specific_canonical_resource_mailbox_scope_object
    execution_identity: connector_specific_canonical_non_secret_identity_object
  projection_contract: registered_connector_specific_schema_defines_all_identity_scope_material
  secrets_allowed: false
  runtime_resolvers: deferred
mcp:
  invocation_requires_fingerprint: true
  non_mcp_fingerprint: null
  fingerprint_source: spec/mcp.yaml#tool_definition_pinning
  operation_kind_source: trusted_action_type_contract_not_untrusted_payload
  visible_field: tool_definition_fingerprint
  input_and_binding_fingerprints_must_agree: true
  current_tool_fingerprint_must_match: true
action_binding:
  name: ActionBindingV1
  algorithm: sha256(utf8(jcs_rfc8785(action_binding_v1)))
  schema_tag: maia.action-binding.v1
  fields:
  - schema
  - action_id
  - action_version
  - plan_id
  - plan_version
  - ordinal
  - action_type
  - input_hash
  - input_canonicalizer
  - connector_selection
  - connector_profile_id
  - connector_binding_hash
  - tool_definition_fingerprint
  - risk_class
  - source_preconditions
  excluded_fields: [state, result_ref, input_ref, approval_state, approval_version, timestamps, surface, run_data, policy_decision]
  decimal_string_paths: [action_version, plan_version, input_canonicalizer.version]
  decimal_string_rule: ASCII_digits_no_sign_no_leading_zero_Version_range_1_to_u64_max
  integer_paths: [ordinal]
  nullable_paths: [connector_profile_id, connector_binding_hash, tool_definition_fingerprint]
  null_encoding: explicit_JSON_null_never_omission
  canonicalizer_fields: [id, version]
  source_precondition_fields: [external_id, source_version_token, normalized_payload_hash]
  source_sort_keys: [external_id, source_version_token, normalized_payload_hash]
  source_sort_order: raw_UTF8_bytes_lexicographic
  exact_duplicate_source_tuples: reject
  unicode_normalization: none
  unknown_fields: reject
material_revision_build_order:
- preserve_or_replace_logical_ActionId
- choose_next_Action_version_once
- bind_exact_ExecutionPlan_revision
- canonicalize_final_input
- compute_input_hash
- resolve_connector_selection_and_binding
- bind_mcp_fingerprint_if_applicable
- resolve_final_RiskClass
- construct_ActionBindingV1
- compute_action_hash
- evaluate_policy
- create_new_Approval_binding_unless_deny
- execution_may_become_eligible
```

## A.approval.yaml

```yaml
schema_version: 1
risk_classes:
  read: Read existing data within granted scope.
  analyze: Transform, summarize, classify or infer without external side effects.
  draft: Create unpublished content or draft objects.
  write_internal: Modify internal low-risk metadata/task state.
  send_internal: Send or post content to recipients inside approved organization boundary.
  send_external: Send/post data outside approved organization boundary.
  destructive: Delete, cancel, revoke, overwrite or irreversibly modify data.
  privileged: Change auth, permissions, policies, integrations, keys or admin-relevant settings.
default_gate:
  read: allow
  analyze: allow
  draft: allow
  write_internal: policy
  send_internal: confirm
  send_external: elevated_confirm
  destructive: elevated_confirm
  privileged: elevated_confirm
hard_rules:
- external_recipient_requires_recipient_preview
- reply_all_requires_recipient_diff
- bcc_or_hidden_recipient_requires_explicit_display
- attachment_egress_requires_attachment_list
- destructive_action_never_auto_approves
- approval_is_bound_to_action_hash_and_expires_on_material_change
- risk_is_resolved_before_gate_evaluation
- approved_action_hash_must_match_again_at_execution
- first_terminal_approval_decision_wins
gate_types:
  allow: No user decision object is required.
  policy: Evaluate current policies/grants; a deny blocks execution and creates no fake human decision.
  confirm: Create a pending approval requiring an authorized human decision.
  elevated_confirm: Create a pending approval with enhanced presentation and any policy-required reauthentication/second
    factor.
gate_to_approval_state:
  allow:
    result: not_required
  policy:
    if_policy_allows_without_confirmation: not_required
    otherwise: pending
    if_policy_denies: no_approval_record_execution_blocked
  confirm:
    result: pending
  elevated_confirm:
    result: pending
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
    rule: Reject empty effective recipients; external or unknown => send_external, all internal => send_internal;
      preserve explicit destructive/privileged operation floor.
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
    rule: Private no-attendee event => write_internal; attendee external/unknown => send_external, all internal
      => send_internal; preserve explicit destructive/privileged operation floor.
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
approval_fatigue_controls:
  principle: reduce prompts without learning permission from relationship familiarity
  allowed:
  - batch non-side-effect informational approvals where policy permits
  - explicit durable grants created by user/tenant policy for narrowly scoped internal actions
  - approved template families bound to template hash, recipient scope, action type and expiry
  forbidden:
  - relationship profile alone grants execution authority
  - silent auto-approval of external sends
  - wildcard durable grants without expiry or recipient/action scope
  durable_grant_fields:
  - grant_id
  - actor_or_policy_owner
  - action_type
  - recipient_scope
  - template_hash_or_schema
  - attachment_policy
  - expires_at
  - revocation_state
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
recipient_boundary:
  input_validation_order: [validate_complete_input, resolve_boundary_risk, preserve_operation_floor]
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
```

## A.assurance_routing.yaml

```yaml
schema_version: 1
contract:
  scope: M0.9_provider_neutral_assurance_routing_and_orchestration
  authority:
    execution_authority: forbidden
    approval_gate_substitution: forbidden
    risk_class_substitution: forbidden
  router:
    inputs: [impact, ambiguity, uncertainty, evidence_conflict, novelty, reversibility, policy_required_assurance, estimated_reasoning_cost, session_budget_ceiling, explicit_human_escalation]
    outputs: [required_assurance, selected_assurance, reason_codes, estimated_reasoning_cost, outcome]
    outcomes: [selected, insufficient_assurance]
    policy_can_raise_but_not_lower: true
    hidden_downgrade: forbidden
  paths:
    A0: deterministic_reasoning_path
    A1: one_enabled_participant
    A2: primary_plus_meaningfully_independent_verifier
    A3: response_isolated_round_table_with_disagreement_and_adjudication
    A4: A3_plus_recorded_human_reasoning_acceptance
  participant_registry:
    provider_neutral: true
    required_fields: [id, provider, model_ref, role_capabilities, enabled, assurance_levels, cost_metadata_capability, availability_health]
    provider_or_model_hardcoding: forbidden
    registry_is_not_adapter_configuration: true
  budget:
    estimate_before_reasoning: required
    measure_after_reasoning: required_when_returned
    unknown_cost: unknown_not_zero
    session_ceiling: enforced_before_reasoning
    exceeded_or_unestimable_required_budget: insufficient_assurance
    hidden_fallback: forbidden
  failures:
    explicit_reason_codes: [participant_unavailable, quota_exhausted, timeout, verifier_unavailable, adjudication_failed, budget_exceeded, cost_unknown, required_assurance_unachievable]
    silent_downgrade: forbidden
    result: insufficient_assurance
  audit:
    required_fields: [request, participant_identities, model_refs, conclusions, evidence, disagreement, adjudication, required_assurance, achieved_assurance, usage, cost, provider_request_ids, outcome, confidence, human_a4_acceptance]
    provider_request_ids_when_returned: required
    a4_human_acceptance: recorded_before_conclusion_is_accepted
    # M0.14: additive fields linking an audit record to the Round Table
    # session it realized, when the plan's path is A3 or A4. participant_
    # identities above means contributors whose response was used;
    # attempted_participants is the superset that also includes failed
    # attempts, disambiguating a field name that was previously silent on
    # the distinction.
    round_table_link:
      required_fields: [round_table_session_id, selected_leader, attempted_participants, contribution_outcomes, quorum_satisfied, failure_reason]
      session_id_recorded_even_on_failure: true
      leader_never_substitutes_missing_quorum: true
      synthesis_result_reuses: adjudication
live_validation:
  default: skipped
  explicit_opt_in_environment: M0_8_LIVE_ANTHROPIC=1
  provider_adapter_ref: spec/round_table.yaml#anthropic_adapter
```

## A.audit.yaml

```yaml
schema_version: 1
contract: authoritative_workspace_audit
record:
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
  identity: [workspace_id, sequence]
  append_only: true
  no_hard_delete_in_runtime: true
  registry_identifiers:
    event_type: validated_registry_string
    subject_type: validated_registry_string
  privacy:
    default_minimal_facts_only: true
    forbidden_by_default:
    - message_body
    - attachment_bytes
    - evidence_snapshot_content
    - source_locator
    - credentials
    - access_tokens
    - full_sensitive_payloads
    direct_personal_data_minimized_before_append: true
  erasure_reference_strategy: opaque_or_tombstone_reference
evidence_snapshot_audit:
  event_types:
  - workspace_evidence_imported
  - workspace_evidence_import_idempotent
  - workspace_evidence_import_refused
  success_and_idempotent_subject_type: artifact
  operation_ref: EvidenceImportRequestId
  provenance_reference: source_locator_hash_only
  content_or_raw_locator_in_audit: forbidden
hash:
  algorithm: SHA-256
  canonicalization: RFC8785-JCS
  projection: AuditRecordV1
  input_encoding: UTF-8
  excluded_fields: [record_hash]
  prev_hash_in_projection: true
  record_hash_must_match_projection: true
chain:
  scope: per_workspace
  sequence:
    type: unsigned_64
    starts_at: 1
    allocated_atomically_by_authoritative_store: true
    monotonically_increases: true
    overflow: fail_closed
  genesis:
    sequence: 1
    prev_hash: '0000000000000000000000000000000000000000000000000000000000000000'
  append: immutable
invalid_transition:
  entity_is_unchanged: true
  refusal_audit_is_allowed: true
  refusal_audit_atomicity: separate_atomic_append
authority:
  one_chain_per_workspace_authority: true
  append_serialized_with_authority_mutation: true
```

## A.briefing.yaml

```yaml
schema_version: 1
contract:
  scope: M0.10_local_evidence_briefing
  locality: local_only_loopback_provider
  external_egress: forbidden
  execution_authority: false
  approval_gate_substitution: forbidden
  automatic_promotion: forbidden
  packet:
    required_fields: [consultation_id, workspace_id, objective, evidence_references, evidence_hashes, bounded_excerpts, instructions, privacy_classification, response_contract, size_limits]
    evidence_source: MAIA_owned_immutable_artifacts_only
    context_selection: explicit_or_deterministic_bounded
    whole_workspace_default: forbidden
    provider_history: non_canonical
    identity:
      version: briefing_packet_v1
      canonical_hash_binds: [workspace_id, objective, evidence_references, evidence_hashes, citation_namespace, instructions, response_contract, privacy_classification, assurance_level, model_routing_constraints, instruction_template_version]
      excludes: [transport_timestamp, latency, provider_session_id]
  result:
    durable_entity: BriefingResult
    classification: [supported_by_evidence, inference, unknown_not_supported]
    required_provenance: [consultation_id, workspace_id, packet_hash, evidence_ids, evidence_hashes, requested_provider, requested_model, actual_provider, actual_model, locality, response_hash, timestamp, outcome, citation_validation, latency, usage, no_fallback]
    citations_must_be_packet_scoped: true
    unknown_or_out_of_packet_citation: reject
    execution_authority: false
    raw_provider_response: untrusted_non_accepted
    accepted_result: validated_before_persistence
    validation: [output_shape, citation_identity, citation_scope, evidence_membership, evidence_hash, result_limits, execution_authority_false, claim_classification]
    persistence: atomic_packet_result_provenance_citations_audit
    immutability: new_invocation_creates_new_result
    persona: presentation_only_not_semantic
    assurance: recorded_advisory_only
  limits:
    maximum_evidence_count: 8
    maximum_packet_bytes: 262144
    maximum_output_tokens: 1024
    timeout_seconds: 60
    retry_count: 0
    retry_semantics: same_semantic_packet_same_local_provider_policy_separate_attempt_provenance
  failures: [local_provider_unavailable, model_unavailable, packet_too_large, timeout, malformed_response, invalid_citation, unsupported_response, provider_error, result_validation_failed]
```

## A.claude_code_provider.yaml

```yaml
schema_version: 1
contract:
  scope: M0.12_claude_code_subscription_provider_vertical_slice
  boundary: infra_adapter_not_core
  round_table_position: optional_external_module_ref_ADR_0046
  execution_authority: forbidden
  capability_profile: DEVELOPMENT_EVOLUTION
provider:
  id: claude-code
  model_provider: anthropic
  transport: local_subprocess
  invocation_mode: non_interactive_print
  output_format: json
  shell_invocation: forbidden
  argument_safe_process_invocation: true
  prompt_delivery: child_stdin
  explicit_working_directory_required: true
  credential_source: local_subscription_session_only
  api_key_environment_injection: forbidden
  anthropic_api_key_dependency: forbidden
  other_provider_fallback: forbidden
  fabricated_success: forbidden
  failure_mode: fail_closed
  timeout_required: true
  cancellation_supported: true
environment:
  inherit: filtered
  scrubbed_variables:
  - ANTHROPIC_API_KEY
  - ANTHROPIC_AUTH_TOKEN
  - ANTHROPIC_BASE_URL
  - ANTHROPIC_CUSTOM_HEADERS
  - ANTHROPIC_MODEL
  - CLAUDE_CODE_USE_BEDROCK
  - CLAUDE_CODE_USE_VERTEX
  - AWS_BEARER_TOKEN_BEDROCK
  scrub_is_unconditional: true
  rationale: A present ANTHROPIC_API_KEY would silently move the child process off subscription-backed
    authentication onto metered API billing and a different credential boundary.
permission_model:
  bypass_permissions: forbidden
  empty_allowlist_is_deny_all: false
  empty_allowlist_observed_behaviour: tools_remain_available
  enforcement: explicit_deny_list
  denied_tools:
  - Bash
  - BashOutput
  - KillShell
  - Edit
  - Write
  - NotebookEdit
  - WebFetch
  - WebSearch
  - Task
  - SlashCommand
  read_only_tools_default_denied: true
  egress_tools_always_denied: true
recursion_guard:
  environment_marker: MAIA_CLAUDE_CODE_PROVIDER_ACTIVE
  nested_invocation: forbidden
  nested_invocation_detection: marker_present_in_parent_environment
  failure_is_permanent: true
capture:
  required_fields:
  - stdout
  - stderr
  - exit_code
  - duration_ms
  - cli_version
  - correlation_id
  - session_id
  raw_response_preserved_for_audit: true
  cost_invention: forbidden
  subscription_cost_basis_is_not_actual_spend: true
contracts:
  consultation_packet:
    version: maia.consultation_packet.v1
    required_fields:
    - schema_version
    - consultation_id
    - participant_id
    - role
    - task
    - context_references
    - evidence_requirements
    - constraints
    - timeout_seconds
  consultation_result:
    version: maia.consultation_result.v1
    required_fields:
    - schema_version
    - consultation_id
    - participant_id
    - response
    - findings
    - evidence
    - risks
    - disagreements
    - recommendation
    - confidence
    - execution
    validated_before_round_table_consumption: true
    consultation_id_must_match_request: true
    schema_violation_is_permanent_failure: true
audit:
  persistence: local_file_per_consultation
  raw_response_preserved: true
  secret_material_logging: forbidden
  scrubbed_variable_values_logged: false
  write_failure_is_invocation_failure: true
deployment_lock:
  profile_absent_in: DEPLOYMENT_LOCKED
  mechanism: build_graph_feature_absence_not_runtime_boolean
  cargo_feature: development-evolution
  default_enabled: false
  subprocess_code_compiled_without_feature: false
  runtime_reenable_by_instance: forbidden
  core_depends_on_provider: false
live_smoke:
  default: skipped
  explicit_opt_in_environment: MAIA_LIVE_CLAUDE_CODE=1
  read_only: true
  canonical_file_mutation: forbidden
  sensitive_prompt_or_credential_logging: forbidden
```

## A.commercial.yaml

```yaml
schema_version: 1
workspace_tenant_scope:
  personal_local:
    tenant_required: false
    commercial_mode: false
  commercial_organizational:
    tenant_required: true
    membership_required_for_multi_user_access: true
    missing_tenant_behavior: fail_closed
boundaries:
  tenant: security_policy_and_commercial_boundary_in_commercial_organizational_mode
  workspace: operational_context_that_may_be_personal_local_without_tenant
  membership: authenticated_user_or_person_role_binding_for_commercial_organizational_mode
  entitlement: centralized_capability_or_feature_seam_not_core_pricing_logic
principles:
- no_pricing_logic_in_core_domain
- no_premature_multitenant_runtime_infrastructure
- usage_is_metered_when_observable
- unknown_cost_or_human_effort_is_not_zero
- product_analytics_exclude_raw_customer_content_by_default
- g2_g3_g4_are_governance_or_strategy_gates_not_runtime_fsms
usage:
  cost_statuses:
  - known
  - unknown
  cost_rules:
    known_requires_cost_minor_and_currency: true
    unknown_forbids_implied_zero: true
  types:
  - model_input_tokens
  - model_output_tokens
  - model_request
  - tool_call
  - connector_operation
  - compute_ms
  - storage_bytes
  - external_api_cost
  - human_intervention
  - support_manual_execution
human_intervention:
  categories:
  - user_review_or_correction
  - operator_support_or_manual_service
  observable_or_explicitly_recorded_only: true
  passive_dwell_time_is_precise_labor_measurement: false
  monetary_cost_requires_explicit_rate_or_cost_basis: true
  absent_rate_or_cost_basis: monetary_cost_unknown
commercial_readiness_gates:
  G2:
    name: FORM COMPANY NOW
    kind: founder_legal_governance_alert
    trigger_before_earliest_of:
    - first_paid_production_pilot_with_external_customer
    - first_recurring_b2b_commercial_contract
    - external_customer_production_data_processing_beyond_controlled_non_production_demonstration
    behavior: stop_commercial_progression_and_alert_founder
    legal_form_selection: founder_legal_accounting_decision_not_canonical_spec
  G3:
    name: FUNDING READINESS
    kind: strategic_non_runtime_gate
    evidence:
    - repeatable_paid_value
    - retention_or_renewal_evidence
    - measured_product_advantage
    - sufficiently_understood_unit_economics
    alert: structured_fundraising_preparation_should_begin
  G4:
    name: SCALE CAPITAL
    kind: strategic_non_runtime_gate
    rule: capital_accelerates_demonstrated_demand_and_scale_not_absent_product_market_evidence
```

## A.compliance.yaml

```yaml
schema_version: 1
scope: Privacy/data-governance support for personal data concerning the user and third parties represented in communication/work
  context.
principles:
- lawful_basis_is_deployment_configuration_not_hardcoded_product_logic
- purpose_limitation
- data_minimization
- accuracy_and_source_provenance
- storage_limitation
- security_and_auditability
third_party_personal_data:
  examples:
  - Person
  - RelationshipProfile
  - Message participants
  - Meeting participants
  - MemoryClaim subjects
  minimum_product_capabilities:
  - locate_subject_linked_records
  - export_subject_linked_records
  - rectify_or_supersede_inaccurate_records
  - restrict_or_suppress_processing_by_policy
  - erase_or_anonymize_where_policy_and_law_permit
  - propagate_deletion_to_indexes_and_derived_memory_subject_to_audit/legal-retention rules
  sensitive_inference: forbidden_by_default
  erasure_strategy:
    strategy: policy_driven_erasure_or_pseudonymized_tombstone
    default_when_structural_record_must_be_retained: pseudonymized_tombstone
    tombstone_id: cryptographically_random_opaque_identifier; MUST NOT be a deterministic hash of email/name/other
      PII
    redact_fields:
    - display_name
    - emails
    - teams_ids
    - free_text_relationship_notes
    - direct_identifiers
    derived_data:
    - purge_or_recompute_search_indexes
    - purge_or_recompute_embeddings
    - suppress_or_remove_derived_memory_claims
    edge_policy: preserve only edges that remain necessary, non-identifying in context, and permitted by retention/legal
      policy; otherwise aggregate/remove
    audit_policy: retain only minimum legally/policy-required audit evidence in a separated protected retention
      domain
    terminology: A tombstone that remains linkable is pseudonymized personal data, not anonymous data.
governance:
  controller_processor_roles: deployment_decision
  legal_basis: deployment_decision
  retention_schedule: workspace_or_tenant_policy
  data_subject_request_owner: deployment_decision
  enterprise_release_gate: DPO/privacy/legal review required for the actual deployment context
notes:
- Rights such as erasure are not absolute; execution follows applicable law and controller policy. MAIA must provide
  technical controls without pretending to choose the legal basis itself.
```

## A.connectors.yaml

```yaml
schema_version: 1
contract:
  required_methods:
  - discover_capabilities
  - health_check
  - normalize_identity
  - execute
  - cancel
  - sanitize_error
  optional_methods:
  - subscribe
  - renew_subscription
  - delta_sync
  - webhook_ack
  - get_rate_limit_state
  execution_metadata:
  - requested_connector
  - actual_connector
  - capabilities_used
  - external_ids
  - tenant_scope
  - latency_ms
built_in:
  outlook_classic_com:
    phase: bootstrap
    surface: windows_desktop
    capabilities:
    - mail.read
    - mail.search
    - mail.draft
    - mail.send
    - mail.move
    - calendar.read
    notes: Current TrustedBridge path; unsupported by new Outlook. Calendar is read-only in the canonical legacy
      capability profile.
  microsoft_graph:
    phase: target
    surface: m365
    capabilities:
    - mail.read
    - mail.search
    - mail.draft
    - mail.send
    - mail.move
    - mail.delete
    - calendar.read
    - calendar.write
    - subscriptions
    - delta_sync
    - files.read
    - files.write
  teams_agent_channel:
    phase: target
    surface: teams
    capabilities:
    - conversation.receive
    - conversation.send
    - adaptive_cards
    - proactive_message
    - mentions
  outlook_web_addin:
    phase: target
    surface: outlook
    capabilities:
    - contextual_ui
    - compose_assist
    - read_item_context
    notes: UX surface, not the canonical mail store.
  local_files:
    phase: alpha
    surface: local
    capabilities:
    - files.read
    - files.write
    - files.search
  mcp_remote:
    phase: target
    surface: protocol
    capabilities:
    - tools.discover
    - tools.call
    - resources.read
  generic_imap_smtp:
    phase: future
    surface: mail
    capabilities:
    - mail.read
    - mail.search
    - mail.send
    notes: Optional non-M365 portability path.
principles:
- connector_capabilities_not_product_names_drive_orchestration
- mail_identity_normalization_is_connector_neutral
- no_connector_specific_business_logic_in_ui
- fallback_never_changes_side_effect_semantics_silently
- os_specific_dependencies_are_confined_to_connector_or_surface_adapters
execution_binding_ref: spec/action_binding.yaml#connector_binding
```

## A.deployment.yaml

```yaml
schema_version: 1
profiles:
  personal_local:
    core: desktop_local
    mail: outlook_classic_com_or_graph
    models: local/byok
    store: sqlite+os_keyring
    state_authority: local_sqlite
    teams_surface: disabled_unless_relay_profile_is_configured
    teams_transport: managed_channel_adapter_plus_outbound_relay
    direct_public_inbound_to_workstation: false
  enterprise_user:
    core: desktop_or_managed_service
    mail: graph
    channels: teams/outlook
    identity: entra
    policies: tenant_managed
    state_authority: managed_postgresql_reference
    channel_service: public_https_m365_agent_service
    desktop_role: thin_client_or_local_capability_worker
    direct_cloud_access_to_desktop_sqlite: false
  hybrid_enterprise:
    core: desktop+agent_service
    sensitive_processing: local_or_tenant
    external_models: policy_controlled
    state_authority: managed_service_unless_workspace_is_explicitly_local_authoritative
    local_worker_connection: outbound_authenticated_relay
    direct_public_inbound_to_workstation: false
    split_brain_authority: forbidden
  developer:
    core: local
    connectors: mock/sandbox
    secrets: dev_keyring
    audit: verbose_sanitized
update_policy:
- signed_artifacts_only
- schema_migration_preflight
- backup_before_update
- rollback_metadata
platform_strategy:
  core_contract: OS-neutral domain/orchestrator/policy/connector contracts.
  reference_desktop_alpha: Windows, because Outlook Classic COM bootstrap exists only there.
  windows_only_dependency: connectors/outlook-classic and any explicitly Windows-specific desktop integration.
  non_windows_target: Graph/Teams/MCP/service paths must not depend on COM or Win32; macOS/Linux desktop support
    is a roadmap/support-matrix decision, not a core-architecture blocker.
  ci_rule: Core/spec/connector-contract tests should run on at least one non-Windows CI runner before universal
    MVP.
teams_topology:
  production_channel_requirement: Teams/M365 channel service terminates at a publicly reachable HTTPS agent endpoint.
  developer_local_test: Dev Tunnel or Agents Playground may expose localhost for development only.
  local_or_hybrid_reference_pattern:
    cloud_component: MAIA Channel Adapter / M365 agent endpoint
    local_component: MAIA local Core/worker holding the local SQLite authority
    link: outbound authenticated bidirectional relay
    reference_implementation: Azure Relay Hybrid Connections (WebSocket/HTTPS over outbound 443) or equivalent tenant-approved
      relay
    cloud_component_must_not_open_local_sqlite: true
    inbound_firewall_port_on_workstation_required: false
  message_semantics: Channel adapter transports authenticated task/approval envelopes; it does not become a second
    MAIA brain.
```

## A.domain.yaml

```yaml
schema_version: 1
entities:
  Workspace:
  - id
  - name
  - owner_id
  - policy_profile_id
  - created_at
  - updated_at
  Person:
  - id
  - display_name
  - emails
  - teams_ids
  - organization_id
  - relationship_profile_id
  - confidence
  - created_at
  - updated_at
  Organization:
  - id
  - name
  - domains
  - tags
  - created_at
  - updated_at
  Project:
  - id
  - name
  - status
  - tags
  - summary
  - owner_id
  - created_at
  - updated_at
  Thread:
  - id
  - channel
  - external_thread_id
  - subject
  - project_id
  - participants
  - last_activity_at
  - classification
  Message:
  - id
  - thread_id
  - external_id
  - external_version
  - sender_id
  - recipients
  - sent_at
  - received_at
  - body_ref
  - trust
  - classification
  Meeting:
  - id
  - external_id
  - external_version
  - title
  - start_at
  - end_at
  - participants
  - project_id
  - transcript_ref
  - status
  Commitment:
  - id
  - owner_person_id
  - beneficiary_person_id
  - project_id
  - source_ref
  - text
  - due_at
  - status
  - confidence
  Decision:
  - id
  - project_id
  - source_ref
  - statement
  - decided_by
  - decided_at
  - confidence
  - supersedes_id
  WorkItem:
  - id
  - project_id
  - title
  - description
  - owner_id
  - due_at
  - status
  - priority
  - source_ref
  RelationshipProfile:
  - id
  - person_id
  - language
  - tone
  - formality
  - response_style
  - working_context
  - last_contact_at
  MemoryClaim:
  - id
  - scope
  - subject_ref
  - predicate
  - value_json
  - source_ref
  - status
  - confidence
  - valid_from
  - valid_until
  AgentTask:
  - id
  - version
  - workspace_id
  - request_text
  - origin_surface
  - origin_ref
  - state
  - privacy_class
  - requested_by
  - created_at
  ExecutionPlan:
  - id
  - task_id
  - version
  - risk_summary
  - estimated_cost
  - approval_requirement
  - created_at
  Action:
  - id
  - plan_id
  - ordinal
  - action_type
  - connector_profile_id
  - risk_class
  - state
  - input_ref
  - source_preconditions
  - result_ref
  - version
  - plan_version
  - input_hash
  - input_canonicalizer
  - connector_selection
  - connector_binding_hash
  - tool_definition_fingerprint
  Run:
  - id
  - version
  - action_id
  - attempt
  - requested_connector_id
  - actual_connector_id
  - requested_model_id
  - actual_model_id
  - state
  - outcome_certainty
  - reconciliation_ref
  - started_at
  - ended_at
  - action_version
  - action_hash
  - approval_id
  - approval_version
  Approval:
  - id
  - task_id
  - action_id
  - policy_id
  - state
  - requested_at
  - decided_at
  - decided_by
  - decision_note
  - action_hash
  - version
  - expires_at
  - origin_surface
  - decided_surface
  - action_version
  - policy_snapshot_hash
  - policy_decision
  - required_assurance
  - achieved_assurance
  ConnectorProfile:
  - id
  - connector_type
  - label
  - capabilities
  - credential_profile_id
  - health
  - policy_tags
  CredentialProfile:
  - id
  - provider
  - label
  - secret_ref
  - scopes
  - status
  - last_validated_at
  ModelProfile:
  - id
  - provider
  - model_id
  - endpoint_profile_id
  - credential_profile_id
  - capabilities
  - privacy_tags
  - billing_mode
  ScheduledJob:
  - id
  - workspace_id
  - task_template_ref
  - schedule
  - state
  - next_run_at
  - last_run_at
  Artifact:
  - id
  - workspace_id
  - kind
  - name
  - media_type
  - content
  - content_sha256
  - byte_length
  - source_locator
  - source_locator_hash
  - import_request_id
  - trust
  - classification
  - created_at
  Tenant:
  - id
  - legal_name_or_label
  - status
  - data_region
  - policy_profile_id
  - billing_account_ref
  - created_at
  - updated_at
  Membership:
  - id
  - tenant_id
  - workspace_id
  - person_or_user_ref
  - role
  - status
  - created_at
  - updated_at
  Entitlement:
  - id
  - tenant_id
  - capability_or_feature
  - allowance
  - source_plan_ref
  - valid_from
  - valid_until
  Outcome:
  - id
  - workspace_id
  - task_id
  - plan_id
  - action_id
  - run_id
  - commitment_ref
  - project_ref
  - subject_ref
  - expected_result
  - observed_result
  - status
  - evidence_refs
  - verification_method
  - supersedes_outcome_id
  - created_at
  - observed_at
  - verified_at
  UsageRecord:
  - id
  - workspace_id
  - tenant_id
  - task_id
  - run_id
  - provider_ref
  - usage_type
  - quantity
  - unit
  - cost_status
  - cost_minor
  - currency
  - pricing_source
  - created_at
  Event:
  - id
  - task_id
  - run_id
  - event_type
  - payload_json
  - created_at
  AuditRecord:
  - audit_id
  - workspace_id
  - sequence
  - recorded_at
  - actor_ref
  - event_type
  - subject_type
  - subject_ref
  - operation_ref
  - decision_ref
  - metadata_hash
  - prev_hash
  - record_hash
provenance_rules:
- every_external_object_keeps_connector_and_external_id
- every_action_persists_requested_and_actual_connector
- every_model_run_persists_requested_and_actual_model
- extracted_commitments_keep_source_ref
- memory_claims_without_sources_are_never_authoritative
primitives:
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
value_objects:
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
artifact_snapshot:
  artifact_kinds:
  - imported_text_evidence
  media_types:
  - utf8_plain_text
  - markdown
  content_representation:
    decoding: strict_UTF-8
    newline_normalization: forbidden
    Unicode_normalization: forbidden
    content_sha256: SHA-256_over_exact_imported_source_bytes
    byte_length: exact_imported_source_byte_count
    snapshot_content: MAIA-owned_immutable_UTF-8_text
  provenance:
    source_locator: private_workspace_scoped_provenance_not_citation_identity
    source_locator_hash: SHA-256_over_exact_source_locator_UTF-8
  knowledge_boundary:
    creates_memory_claim: false
    creates_workgraph_edges: false
    infers_semantics: false
artifact_contracts:
  EvidenceImportSource:
    fields:
      source_locator:
        type: OpaqueRef
        required: true
        nullable: false
      name:
        type: NonEmptyString
        required: true
        nullable: false
      media_type:
        type: EvidenceMediaType
        required: true
        nullable: false
  EvidenceImportRequest:
    fields:
      id:
        type: EvidenceImportRequestId
        required: true
        nullable: false
      workspace_id:
        type: WorkspaceId
        required: true
        nullable: false
      requested_by:
        type: ActorRef
        required: true
        nullable: false
      requested_at:
        type: Timestamp
        required: true
        nullable: false
      sources:
        type: list<EvidenceImportSource>
        required: true
        nullable: false
    rules:
      input_paths_are_host_boundary: true
      persisted_request_identity: EvidenceImportRequestId
      partial_success: forbidden
  Artifact:
    fields:
      id:
        type: ArtifactId
        required: true
        nullable: false
      workspace_id:
        type: WorkspaceId
        required: true
        nullable: false
      kind:
        type: ArtifactKind
        required: true
        nullable: false
      name:
        type: NonEmptyString
        required: true
        nullable: false
      media_type:
        type: EvidenceMediaType
        required: true
        nullable: false
      content:
        type: Utf8String
        required: true
        nullable: false
      content_sha256:
        type: Sha256Hex
        required: true
        nullable: false
      byte_length:
        type: ByteCount
        required: true
        nullable: false
      source_locator:
        type: OpaqueRef
        required: true
        nullable: false
      source_locator_hash:
        type: Sha256Hex
        required: true
        nullable: false
      import_request_id:
        type: EvidenceImportRequestId
        required: true
        nullable: false
      trust:
        type: OpaqueRef
        required: true
        nullable: false
      classification:
        type: OpaqueRef
        required: true
        nullable: false
      created_at:
        type: Timestamp
        required: true
        nullable: false
    rules:
      immutable_after_write: true
      content_is_MAIA_owned_snapshot: true
      source_locator_is_private_provenance: true
      source_modification_or_deletion_changes_snapshot: false
      semantic_extraction: forbidden
  EvidenceCitation:
    fields:
      workspace_id:
        type: WorkspaceId
        required: true
        nullable: false
      artifact_id:
        type: ArtifactId
        required: true
        nullable: false
      content_sha256:
        type: Sha256Hex
        required: true
        nullable: false
      start_line:
        type: LineNumber
        required: false
        nullable: true
      end_line:
        type: LineNumber
        required: false
        nullable: true
    rules:
      canonical_identity: [workspace_id, artifact_id, content_sha256, start_line, end_line]
      external_source_locator_is_citation_identity: false
      line_positions_reference: immutable_MAIA_snapshot
      line_range: both_absent_for_artifact_citation_or_both_present_inclusive_start_lte_end
outcome_contracts:
  Outcome:
    fields:
      id:
        type: OutcomeId
        required: true
        nullable: false
      workspace_id:
        type: WorkspaceId
        required: true
        nullable: false
      task_id:
        type: AgentTaskId
        required: false
        nullable: true
      plan_id:
        type: ExecutionPlanId
        required: false
        nullable: true
      action_id:
        type: ActionId
        required: false
        nullable: true
      run_id:
        type: RunId
        required: false
        nullable: true
      commitment_ref:
        type: OpaqueRef
        required: false
        nullable: true
      project_ref:
        type: OpaqueRef
        required: false
        nullable: true
      subject_ref:
        type: OpaqueRef
        required: false
        nullable: true
      expected_result:
        type: Utf8String
        required: false
        nullable: true
      observed_result:
        type: Utf8String
        required: false
        nullable: true
      status:
        type: OutcomeStatus
        required: true
        nullable: false
      evidence_refs:
        type: list<OpaqueRef>
        required: true
        nullable: false
      verification_method:
        type: OpaqueRef
        required: false
        nullable: true
      supersedes_outcome_id:
        type: OutcomeId
        required: false
        nullable: true
      created_at:
        type: Timestamp
        required: true
        nullable: false
      observed_at:
        type: Timestamp
        required: false
        nullable: true
      verified_at:
        type: Timestamp
        required: false
        nullable: true
    rules:
      lifecycle_ref: spec/outcomes.yaml#lifecycle
      business_outcome_is_not_run_outcome_certainty: true
      context_ref: one_or_more_of_task_plan_action_run_commitment_project_or_subject
      model_confidence_alone_verifies_outcome: false
  UsageRecord:
    fields:
      id:
        type: UsageRecordId
        required: true
        nullable: false
      workspace_id:
        type: WorkspaceId
        required: true
        nullable: false
      tenant_id:
        type: TenantId
        required: false
        nullable: true
      task_id:
        type: AgentTaskId
        required: false
        nullable: true
      run_id:
        type: RunId
        required: false
        nullable: true
      provider_ref:
        type: OpaqueRef
        required: false
        nullable: true
      usage_type:
        type: OpaqueRef
        required: true
        nullable: false
      quantity:
        type: UsageQuantity
        required: true
        nullable: false
      unit:
        type: OpaqueRef
        required: true
        nullable: false
      cost_status:
        type: UsageCostStatus
        required: true
        nullable: false
      cost_minor:
        type: AmountMinor
        required: false
        nullable: true
      currency:
        type: CurrencyCode
        required: false
        nullable: true
      pricing_source:
        type: OpaqueRef
        required: false
        nullable: true
      created_at:
        type: Timestamp
        required: true
        nullable: false
    rules:
      cost_status_ref: spec/commercial.yaml#usage.cost_statuses
      unknown_cost_is_zero: false
      known_cost_requires_cost_minor_and_currency: true
execution_contracts:
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

## A.errors.yaml

```yaml
schema_version: 1
codes:
- InvalidStateTransition
- ApprovalRequired
- ApprovalExpired
- PolicyBlocked
- PrivacyBlocked
- CapabilityMissing
- ConnectorUnavailable
- CredentialMissing
- CredentialRejected
- RateLimited
- Timeout
- Canceled
- ExternalConflict
- RecipientRisk
- AttachmentRisk
- PromptInjectionSuspected
- McpToolBlocked
- SyncCursorInvalid
- SubscriptionExpired
- DuplicateAction
- UnknownCost
- PersistenceError
- MigrationError
- ValidationError
- UnsupportedClient
- InternalError
rules:
- provider_or_connector_errors_map_to_canonical_code
- raw_error_is_sanitized
- retryability_is_explicit
- user_message_uses_i18n_key
persistence_semantics:
  cas_mismatch: ExternalConflict
  corruption: fail_closed
  migration_incompatibility: MigrationError
  unavailable_writable_authority: PersistenceError
  exhausted_storage_busy: PersistenceError
```

## A.evolution_mutation_tier1.yaml

```yaml
schema_version: 1
milestone: M0.16.3
authority: docs/project/directives/MAIA_Architecture_Amendment_Autonomous_Evolution_Deployment_Lock_v5.1.md#3.0.2
placement:
  policy: ops/evolution-supervisor
  host_mutation_and_verification_adapter: ops/evolution-workspace-host
  core_dependency: forbidden
  shipped_application_dependency: forbidden
human_governance:
  improvement_must_be_human_approved_before_mutation: true
  approval_reference_required: true
  approval_verified_by_protected_host: true
  supervisor_and_kill_switch_outside_candidate_boundary: true
evolvable_surface:
  authority_model: positive_exact_path_allowlist
  paths:
    - apps/local-intelligence-host/src/lib.rs
  operators:
    - FUNCTION_REWRITE
  absent_or_unclassified: deny
  create_delete_rename: forbidden
  tests_manifests_tooling_specs_governance_and_evaluators: protected
mutation:
  candidate_state_required: ACTIVE
  tier0_outcome_required: ADMITTED
  exact_candidate_and_workspace_identity_required: true
  operation: unique_expected_text_replacement
  expected_file_digest_required: true
  line_endings: normalize_for_match_and_preserve_candidate_style
  max_replacement_bytes: 65536
  actual_blast_radius_rechecked: true
  path_resolution: canonical_candidate_workspace_relative
  traversal_absolute_paths_symlinks_and_escape: reject
  canonical_host_or_protected_state_target: reject
  git_authority: none
tier1:
  mandatory_checks:
    - post_mutation_candidate_identity
    - syntax_static_validation
    - formatting_and_lint
    - touched_component_build
    - targeted_unit_and_contract_tests
    - protected_surface_integrity
  candidate_supplied_commands: forbidden
  adapter_failure_or_timeout: INFRA_ERROR
  missing_or_ambiguous_evidence: fail_closed
  success_grants_promotion: false
evidence:
  durable_queryable_attempt_record_required: true
  fields:
    - candidate_id
    - workspace_id
    - generation_id
    - hypothesis_id
    - tier0_outcome_and_evidence_refs
    - approval_reference
    - requested_path_and_operation
    - mutation_outcome_and_reason
    - changed_file_paths_and_digests
    - tier1_check_results_and_verifier_identity
    - terminal_candidate_state_and_reason
  raw_replacement_content: forbidden
  credentials_prompts_and_secrets: forbidden
state:
  tier0_failure: discard_candidate_without_tier1
  tier1_failure: REJECTED_then_discard_candidate
  tier1_infrastructure_failure: INFRA_ERROR_then_discard_candidate
  tier1_success: remain_active_without_promotion
  rollback_baseline_implied: false
authority_limits:
  canonical_host_mutation: forbidden
  supervisor_or_kill_switch_mutation: forbidden
  git_commit_push_merge_or_protected_ref_change: forbidden
  deployment_locked_mutation: forbidden
  release_or_promotion_authority: forbidden
  arbitrary_subprocess: forbidden
  only_fixed_deterministic_tier1_adapter: allowed
```

## A.evolution_protected_runtime.yaml

```yaml
schema_version: 1
milestone: M0.16.4
authority: docs/project/directives/MAIA_Architecture_Amendment_Autonomous_Evolution_Deployment_Lock_v5.1.md
placement:
  runtime_authority: protected_host_process
  candidate_policy: ops/evolution-supervisor
  windows_containment_adapter: ops/evolution-process-host
  workspace_and_evidence_adapter: ops/evolution-workspace-host
  core_dependency: forbidden
  shipped_application_dependency: forbidden
protected_state:
  owner: protected_host_process
  authority_storage: protected_host_process_memory
  restart_without_attestation: fail_closed_locked_defaults
  fields:
    - runtime_profile
    - evolution_enabled
    - kill_switch_state
    - supervisor_identity
    - candidate_identity
    - generation_id
    - parent_commit
    - reservation_identity_and_expiry
    - tier0_admission_identity
    - protected_path_assumptions
    - human_approval_identity_and_validity
    - canonical_repository_identity_and_state
    - state_version
  candidate_may_supply_or_change: false
  unknown_or_ambiguous: fail_closed
  startup_defaults:
    runtime_profile: DEPLOYMENT_LOCKED
    evolution_enabled: false
    kill_switch_state: "ON"
    supervisor_integrity: unverified
    tier1_isolation: same_user_process_tree_contained
    explicit_host_attestation_required: true
  deployment_locked: deny_mutation_and_verification
  candidate_profile_switch: forbidden
gate_decision:
  immutable_operation_snapshot: required
  bind_fields: [runtime_profile, state_version, supervisor_identity, candidate_identity, generation_id, parent_commit, reservation_identity_and_expiry, tier0_admission_identity, protected_path_assumptions, human_approval_identity_and_validity, canonical_repository_identity_and_state]
  revalidate_before: [mutation, tier1_start]
  revalidate_after: [tier1]
  invalidated_by: [profile_change, evolution_disable, kill_switch_change, supervisor_identity_change, candidate_identity_change, reservation_expiry, approval_revocation, tier0_evidence_change, protected_path_change, parent_change, canonical_repository_drift]
kill_switch:
  protected_owner_only: true
  on_or_disabled_denies_new_mutation: true
  disabled_denies_new_mutation: true
  disabled_denies_new_tier1: true
  running_verifier: terminate_containment_job
  interruption_result: CANCELLED
  candidate_operation: discard_candidate
  preserve_and_flush_evidence: true
  unknown_state: fail_closed
containment:
  platform: windows
  process_tree: job_object
  create_suspended_assign_before_resume: true
  breakaway: forbidden
  kill_on_owner_close: true
  network: appcontainer_without_network_capabilities_required; unavailable_fails_closed
  filesystem: appcontainer_acl_read_only_workspace_toolchain_and_writable_candidate_target_required; unavailable_fails_closed
  capability_levels: [same_user_process_tree_contained, restricted_identity_network_and_filesystem]
  minimum_tier1_level: restricted_identity_network_and_filesystem
  inherited_handles: explicit_null_standard_handles_only
  candidate_or_environment_fallback: forbidden
  unavailable_capability: fail_closed
  achieved_identity_claim: same_user_process_tree_containment_only
  stronger_isolation_capabilities: unavailable_until_os_acl_and_no_network_are_verified
resource_limits:
  max_concurrent_verifier_trees: 1
  max_command_wall_seconds: 120
  cpu_rate_percent: 75
  job_memory_bytes: 4294967296
  active_process_limit: 64
  candidate_controls_limits: false
  resource_exhaustion_result: RESOURCE_LIMIT
  job_resource_notifications:
    active_process_limit: JOB_OBJECT_MSG_ACTIVE_PROCESS_LIMIT
    job_memory_limit: JOB_OBJECT_MSG_JOB_MEMORY_LIMIT
    process_memory_limit: JOB_OBJECT_MSG_PROCESS_MEMORY_LIMIT
  recognized_memory_termination_status:
    - STATUS_COMMITMENT_LIMIT
    - STATUS_NO_MEMORY
  target_directory: candidate_scoped
  cargo_network_mode: offline
  environment: fixed_allowlist
journal:
  path_owner: protected_host
  path_candidate_supplied: false
  location: outside_canonical_repository_and_candidate_root
  writer_lock: operating_system_exclusive_whole_file
  lock_before_chain_validation: true
  second_writer: INFRA_ERROR
  crash_releases_lock: true
  reopen_verifies_complete_hash_chain: true
  corruption: fail_closed_without_append
  authority_transition_flush_before_success: true
  lock_or_storage_failure: INFRA_ERROR
outcomes:
  verification: [PASS, TEST_FAILURE, RESOURCE_LIMIT, INFRA_ERROR, CANCELLED, ISOLATION_UNAVAILABLE]
  isolation_unavailable_operation: discard_candidate
  isolation_unavailable_terminal_state: ISOLATION_UNAVAILABLE
  cancellation_is_not_verification_failure: true
  candidate_failure_operation: discard_candidate
  cancellation_is_test_failure: false
  tier1_pass_grants_promotion: false
  rollback_baseline_implied: false
authority_limits:
  evolvable_paths: [apps/local-intelligence-host/src/lib.rs]
  arbitrary_filesystem_write: forbidden
  arbitrary_subprocess: forbidden
  provider_or_round_table_access: forbidden
  git_commit_push_merge_or_protected_ref_change: forbidden
  deployment_locked_mutation: forbidden
  promotion_deployment_or_active_version_change: forbidden
```

## A.evolution_supervisor.yaml

```yaml
schema_version: 1
milestone: M0.16.0
phase: contract_first_non_mutating
authority: docs/project/directives/MAIA_Architecture_Amendment_Autonomous_Evolution_Deployment_Lock_v5.1.md
placement:
  subsystem: development_operations
  crate: ops/evolution-supervisor
  core_dependency: forbidden
  shipped_application_dependency: forbidden
  hypothesis_ledger_generator_qualification_authority: advisory_only
trust_boundary:
  supervisor: outside_evolving_worker_mutable_boundary
  worker_may_control_supervisor: false
  worker_may_modify_supervisor_or_kill_switch: false
  host_isolation_required_before_mutation: true
  unknown_integrity_or_isolation: halt
capability_profiles:
  development_evolution: contract_review_only
  deployment_locked: supervisor_absent_from_shipped_build_graph
  runtime_reenable_by_deployed_instance: forbidden
authority:
  supervisor_owns:
  - start_pause_stop
  - kill_switch
  - resource_ceilings
  - snapshot_verification
  - rollback_baseline
  - protected_surface_enforcement
  - candidate_lineage_verification
  - integrity_heartbeat
  - failure_backoff
  - development_lineage_promotion
  hypothesis_and_qualification_grant_mutation_authority: false
  worker_may_self_classify_promotion_eligible: false
preflight:
  result: advisory_contract_review_only
  success_grants_mutation_authority: false
  required_evidence:
  - development_evolution_profile
  - supervisor_integrity
  - host_isolation
  - kill_switch_operational
  - positive_evolvable_allowlist
  - parent_recovery_point
  - resource_budget_and_recovery_reserve
  - protected_path_dry_run
  - declared_operator
  - declared_blast_radius
  - hypothesis_evidence_refs
  unknown_or_missing: reject
  tier0_required_before_worktree_or_candidate_snapshot: true
  tier0_complete_in_this_milestone: false
  candidate_worktree_creation: forbidden
  candidate_snapshot_creation: forbidden
  mutation: forbidden
  promotion: forbidden
protected_surfaces:
  unclassified: non_writable
  worker_may_edit_policy_or_evaluator: false
  worker_may_edit_resource_hard_ceiling: false
  evolvable_allowlist: []
kill_switch:
  unreadable_or_unknown: halt
  worker_may_reset: false
  emergency_result: HALTED
release_boundary:
  development_promotion_is_release: false
  release_authority: external_human_controlled
  locked_artifact_can_reenable_evolution: false
```

## A.evolution_tier0.yaml

```yaml
schema_version: 1
milestone: M0.16.1
authority: docs/project/directives/MAIA_Architecture_Amendment_Autonomous_Evolution_Deployment_Lock_v5.1.md#3.0.1
placement: ops/evolution-supervisor
phase: admission_only_non_mutating
ordered_checks:
  - kill_switch_and_supervisor_state
  - hypothesis_admission_completeness
  - refuted_or_recently_rejected_ledger_hit
  - protected_path_dry_run
  - plan_blast_radius
  - budget_reservation
  - parent_recovery_point_verification
  - scratch_static_apply
outcomes: [ADMITTED, REJECTED, INFRA_ERROR, HALT_REQUIRED]
rejection_record:
  required: true
  machine_readable_reason: true
  evidence_refs: true
  recording_failure: INFRA_ERROR
boundaries:
  unknown_check: fail_closed
  candidate_worktree_before_admission: forbidden
  candidate_snapshot_before_admission: forbidden
  real_source_mutation: forbidden
  git_commit_push_promotion: forbidden
  provider_and_round_table_calls: forbidden
  arbitrary_subprocess: forbidden
  deployment_locked_mutation: forbidden
  supervisor_outside_worker_mutable_boundary: true
  active_evolvable_allowlist: empty_in_m0_16_1
  admitted_grants_mutation_authority: false
```

## A.evolution_workspace.yaml

```yaml
schema_version: 1
milestone: M0.16.2
authority: docs/project/directives/MAIA_Architecture_Amendment_Autonomous_Evolution_Deployment_Lock_v5.1.md#3.0.2
placement:
  policy: ops/evolution-supervisor
  host_adapter: ops/evolution-workspace-host
  core_dependency: forbidden
  shipped_application_dependency: forbidden
admission:
  required_outcome: ADMITTED
  exact_plan_binding_revalidated_by_protected_host: true
  supervisor_ready_at_allocation: true
  parent_identity_and_source_reverified: true
  reservation_live_and_bound: true
  protected_paths_rechecked: true
  unknown_or_adapter_error: fail_closed
identity_fields: [candidate_workspace_id, generation_id, hypothesis_id, parent_id, parent_kind, parent_snapshot_id, parent_source_commit, declared_operator, proposed_paths, proposed_symbols, estimated_changed_lines, max_changed_lines, max_files, semantic_fingerprint, implementation_fingerprint, profiling_fingerprint, evidence_refs, reservation_id, created_sequence]
lifecycle:
  states: [REQUESTED, ALLOCATED, ACTIVE, CLOSED]
  transitions:
    REQUESTED: [ALLOCATED, CLOSED]
    ALLOCATED: [ACTIVE, CLOSED]
    ACTIVE: [CLOSED]
    CLOSED: []
  terminal_outcomes: [REJECTED, CANCELLED, INFRA_ERROR]
  candidate_failure_operation: discard_candidate
  rollback_baseline_implied: false
host:
  workspace_root: dedicated_maia_controlled_candidate_root
  path_or_identity_collision: reject
  canonical_main_and_other_candidates: untouched
  candidate_scoped_cleanup: deterministic_idempotent
  evidence_preserved_before_cleanup: true
  no_worker_execution_in_milestone: true
authority_limits:
  protected_surface_write: forbidden
  supervisor_write: forbidden
  git_commit_push_promotion: forbidden
  release_authority: forbidden
  deployment_locked_allocation: forbidden
  provider_calls: forbidden
  source_mutation_logic: forbidden
```

## A.execution.yaml

```yaml
schema_version: 1
scope: Pure contracts; no scheduler, policy engine, persistence implementation, connectors or runtime execution in M0.2.1.
persistence_contract_ref: spec/persistence.yaml#atomic_transactions
run:
  exact_binding_fields: [action_id, action_version, action_hash, approval_id, approval_version]
  record_cas_field: Run.version
  record_cas_ref: spec/domain.yaml#execution_contracts.Run.rules
  result_affects: exact_action_revision_only
  newer_revision_may_receive_historical_result: false
  authorization_record_versions_reconstructible: true
  eligibility_ref: spec/approval.yaml#execution_authorization
  evidence_validation: match_run_id_action_id_action_version_action_hash
attempts:
  scope: [action_id, action_version]
  first: 1
  retry: new_RunId_previous_attempt_plus_one
  new_revision: reset_to_1
  new_action: reset_to_1
  initial_state: created
  overflow: fail_closed_no_new_Run
  prior_retryable_result_mutable: false
  retry_preconditions: [current_policy_permits, current_execution_checks, prior_effect_resolved]
reconciliation:
  values: [effect_confirmed, effect_not_executed, unresolved]
  start_edge: outcome_unknown_to_reconciling
  effect_confirmed: completed_no_retry_of_same_effect
  effect_not_executed: authoritative_non_execution_proof_before_considering_new_Run
  effect_not_executed_run_state: retryable_error
  unresolved: remain_reconciling_block_retry
  required_evidence: [run_id, action_id, action_version, action_hash, effect_identity, source_ref, result]
  non_authoritative_search_absence_is_proof: false
  authoritative_proof_source: connector_specific_canonical_reconciliation_contract
  unresolved_blocks_semantically_equivalent_effect_across_ids_and_revisions: true
  connector_implementations: deferred
freshness:
  values: [matched, mismatched, unverifiable]
  enforcement: [strong_precondition, compare_before_execute]
  required_evidence: [run_id, action_id, action_version, action_hash, enforcement, precondition, observed_source_version_token, observed_payload_hash, result]
  matched: observed_version_and_hash_equal_bound_precondition
  mismatched: block_execution_rebuild_revision_binding_risk_policy_gate
  unverifiable_material: fail_closed
  missing_observation: unverifiable_not_matched
  strong_precondition: connector_atomically_binds_execution_to_source_version
  compare_before_execute: immediately_before_effect_does_not_eliminate_TOCTOU
  evidence_scope: single_RunId
  reuse_across_runs: false
  arbitrary_ttl_seconds: null
  applies_to_ref: spec/sync.yaml#pre_side_effect_freshness.applies_to
pause:
  states_source: spec/states.yaml#pause_semantics.in_flight_states
  scope: all_task_runs_including_superseded_plan_action_revisions
  forbidden_after_request: [create_run, schedule_run, created_to_starting]
  paused_requires: zero_in_flight_runs
  supersession_hides_in_flight: false
  cancellability_source: explicit_operation_contract
  non_cancellable_runs: drain_or_reconcile
```

## A.ipc.yaml

```yaml
schema_version: 1
commands:
  task.preflight:
    risk: analyze
    cancellable: true
  task.start:
    risk: analyze
    cancellable: true
  task.cancel:
    risk: write_internal
    cancellable: false
  approval.decide:
    risk: privileged
    cancellable: false
  connectors.add:
    risk: privileged
    cancellable: false
  connectors.test:
    risk: read
    cancellable: true
  credentials.add:
    risk: privileged
    secret_input: true
  credentials.rotate:
    risk: privileged
    secret_input: true
  credentials.delete:
    risk: privileged
  mail.draft:
    risk: draft
    cancellable: true
  mail.send:
    cancellable: false
    risk_resolution:
      type: dynamic
      classifier: mail_recipient_boundary
      output_enum: RiskClass
      contract_ref: approval.dynamic_risk_classifiers.mail_recipient_boundary
      implementation_symbol: core.policy.classifiers.mail_recipient_boundary
      operation_risk_floor: null
  calendar.create:
    cancellable: false
    risk_resolution:
      type: dynamic
      classifier: calendar_participant_boundary
      output_enum: RiskClass
      contract_ref: approval.dynamic_risk_classifiers.calendar_participant_boundary
      implementation_symbol: core.policy.classifiers.calendar_participant_boundary
      operation_risk_floor: null
  scheduler.create:
    risk: write_internal
    cancellable: false
  mcp.server.toggle:
    risk: privileged
    cancellable: false
  export.bundle:
    risk: read
    cancellable: true
  audit.export:
    risk: read
    cancellable: true
contract:
  risk_field: Static RiskClass enum value when action semantics are invariant.
  risk_resolution: Use for actions whose risk depends on normalized payload/recipient boundary. Resolver MUST return
    a RiskClass before ApprovalGate.
  validation: Exactly one of risk or risk_resolution is required for each command.
  implementation_binding: Spec Guard validates symbolic classifier bindings now; once core exists, generated binding/compile
    tests MUST prove every symbol is implemented. Canonical spec must not depend on pre-existing code.
```

## A.mail.yaml

```yaml
schema_version: 1
mail_intelligence:
  pipeline:
  - ingest
  - normalize
  - thread_link
  - trust_classify
  - entity_link
  - project_link
  - intent_extract
  - commitment_extract
  - decision_extract
  - urgency_score
  - reply_need_score
  - draft_context_build
  signals:
  - sender_relationship
  - direct_question
  - explicit_deadline
  - overdue_commitment
  - meeting_followup
  - vip_or_priority_rule
  - external_domain
  - sensitivity_label
  - recipient_count
  - reply_all_risk
  outputs:
  - summary
  - why_it_matters
  - suggested_action
  - draft_reply
  - commitments
  - decisions
  - project_links
  - risk_flags
draft_rules:
- never_send_from_draft_generation
- preserve_thread_language_unless_user_policy_overrides
- tone_uses_relationship_profile_but_never_invents_facts
- quote_previous_decisions_only_with_source_ref
commitment_extraction:
  default_extracted_status: candidate
  never_auto_confirm_from_free_text: true
  required_checks:
  - speaker_or_author_attribution
  - quotation_boundary
  - negation
  - conditionality
  - modality_or_hedging
  - deadline_or_time_reference_if_present
  - source_ref
  language_profiles:
    pl:
      stronger_signals:
      - first-person future/perfective commitment such as "zrobię" when not negated/quoted/conditional
      - explicit owner + deliverable + deadline
      hedged_or_weak_signals:
      - '"postaram się"'
      - '"spróbuję"'
      - '"będę próbował"'
      - conditional/subjunctive or courtesy language
      rule: Aspect/modality adjusts confidence only. It never upgrades free-text extraction directly to confirmed.
    en:
      rule: Distinguish explicit commitment (I will / I commit to) from hedging (I will try / might / should) and
        preserve attribution/negation.
```

## A.mcp.yaml

```yaml
schema_version: 1
target_spec: '2026-07-28'
roles:
  client: Connect MAIA to approved external MCP servers/tools.
  server: Expose scoped MAIA capabilities to other approved agents/clients.
server_scopes:
- mail.read_summary
- mail.draft
- calendar.read
- workgraph.read
- commitments.read
- tasks.create
- artifacts.read
forbidden_default_server_scopes:
- mail.send
- mail.delete
- credentials.manage
- policy.modify
rules:
- tool_catalog_is_cacheable_but_revalidated
- authorization_is_scope_based
- tool_output_is_untrusted_until_validated
- external_mcp_cannot_bypass_approval_gate
- server_requests_are_audited
- tool_definition_fingerprint_is_bound_to_review_and_approval
- tool_definition_change_invalidates_prior_trust_decision
tool_definition_pinning:
  canonical_fingerprint_fields:
  - server_identity
  - tool_name
  - title
  - description
  - inputSchema
  - outputSchema
  - annotations
  fingerprint_algorithm: sha256(utf8(jcs_rfc8785(canonical_fields_object)))
  on_catalog_change: invalidate prior review/approval/allowlist decision for the changed tool definition and rerun
    policy evaluation
  execution_check: Tool fingerprint at execution MUST equal the fingerprint bound to the approved/reviewed action.
  stale_decision_error: ExternalConflict
  canonicalization_standard: RFC-8785-JCS
  canonical_object_construction:
    all_listed_fields_are_present: true
    missing_optional_fields: encode_as_json_null
    unicode_normalization: none_preserve_code_points_as_required_by_JCS
    number_domain: I-JSON / IEEE-754 compatible JSON numbers only
    utf8_encoding_after_jcs: true
  cross_runtime_requirement: Python/Rust/TypeScript implementations MUST pass the same RFC-8785 golden vectors.
action_binding_ref: spec/action_binding.yaml#mcp
```

## A.memory.yaml

```yaml
schema_version: 1
layers:
  ephemeral: Current task scratch/context only.
  working: Recent project/thread context with expiration.
  durable: User-approved or strongly sourced stable work facts.
  relationship: Professional interaction preferences and context; no sensitive profiling unless explicitly allowed.
claim_lifecycle:
- candidate
- accepted
- superseded
- expired
- rejected
rules:
- source_required_for_operational_fact
- user_correction_supersedes_inference
- sensitive_personal_attributes_not_inferred
- memory_can_be_disabled_per_workspace
- deletion_propagates_to_indexes_subject_to_audit_retention_policy
```

## A.metrics.yaml

```yaml
schema_version: 1
metrics:
  time_to_first_value:
    symbol: TTFV
    clock_starts_at: first_point_with_sufficient_authorized_usable_data_or_context
    qualifying_events:
    - explicit_user_confirmation_or_acceptance
    - confirmation_of_source_grounded_commitment_decision_or_risk
    - approval_or_use_of_proposed_follow_up_or_useful_action
    - verified_outcome
    - other_explicit_canonical_value_event
    excluded_events:
    - application_open
    - installer_launch
    - data_import_without_user_work_value
    - generic_text_generation
    - model_response_success
    event_classes:
    - system_detected_value_event
    - user_confirmed_value_event
  time_to_first_action:
    symbol: TTFA
    qualifying_action: real_semantic_MAIA_Action_reaching_its_existing_canonical_execution_state
    excluded_events:
    - diagnostic
    - no_op
    - installation_plumbing
    - configuration_plumbing
    - test_action
    approval_requirements_may_be_weakened_for_metric: false
  successful_outcome_rate:
    numerator: verified_Outcomes
    denominator: benchmarked_or_production_cases_under_declared_verification_rule
  human_intervention_effort:
    source: spec/commercial.yaml#human_intervention
  cost_per_successful_outcome:
    denominator: verified_Outcomes
    components_when_evidence_exists:
    - model_usage
    - infrastructure
    - third_party_or_tool_cost
    - human_intervention
    - support_or_manual_execution
    unknown_component_behavior: preserve_unknown_never_assume_zero
  repeatability_effort:
    measures:
    - onboarding_manual_setup_effort
    - custom_implementation_effort
    - operator_intervention
    - reusable_vs_customer_specific_configuration
    interpretation: materially_different_bespoke_work_indicates_service_or_consulting_behavior_not_proven_scalable_product_repeatability
measured_advantage_rule:
  competitive_or_product_advantage_requires: instrumented_measurement_or_named_benchmark
  minimum_falsifiable_measures:
  - successful_outcome_rate
  - human_review_or_correction_effort
  - cost_per_successful_outcome
  - time_to_first_value
  - time_to_first_action
  - repeatability_effort
  benchmark_requirements:
  - named_baseline
  - case_set
  - verification_rule
  - model_tool_versions_when_relevant
  - known_and_unknown_costs
  - preserved_failures_and_disagreement
privacy:
  raw_customer_content_in_product_telemetry: forbidden_by_default
```

## A.models.yaml

```yaml
schema_version: 1
model_router:
  providers: dynamic_registry
  supported_profile_types:
  - local_openai_compatible
  - openai
  - anthropic
  - azure_openai
  - google
  - custom_openai_compatible
  - mcp_sampling_if_policy_allows
  selection_order:
  - hard_privacy_rules
  - required_capabilities
  - data_residency
  - credential_health
  - user_preference
  - quality_fit
  - latency
  - cost
  fallback: explicit_and_auditable
credential_policy:
- keys_rotate_without_code_changes
- multiple_profiles_per_provider
- secrets_never_return_to_frontend_after_save
- provider_model_catalog_is_runtime_data_not_business_logic
```

## A.modes.yaml

```yaml
schema_version: 1
execution_modes:
  local_legacy:
    external_api_required: false
    mail_transport: outlook_classic_com
    llm: local_or_manual
    egress: policy_dependent
    purpose: Current bootstrap mode for locked-down Windows environments.
  connected_m365:
    external_api_required: true
    mail_transport: microsoft_graph
    channels:
    - teams
    - outlook
    - m365_copilot
    egress: tenant_and_policy_controlled
  connected_generic:
    external_api_required: true
    mail_transport: connector_defined
    channels:
    - desktop
    - web
    - third_party
    egress: connector_and_policy_controlled
  hybrid:
    external_api_required: optional
    mail_transport: best_available_connector
    llm: local_and_cloud
    egress: per_task_policy
  policy_auto:
    external_api_required: depends
    mail_transport: routed
    llm: routed
    egress: depends
    purpose: Select only among policy-compliant connectors and models.
privacy_classes:
  local_only: No payload leaves the local device except user-approved corporate-client actions already performed
    by that client.
  tenant_only: Data may move only inside approved organization/M365 tenant boundaries.
  controlled_external: External AI/API allowed only for data classes and providers explicitly approved by policy.
  external_allowed: External providers allowed subject to connector scopes, user policy and action approval.
```

## A.outcomes.yaml

```yaml
schema_version: 1
lifecycle:
  statuses:
  - candidate
  - observed
  - verified
  - rejected
  - disputed
  - superseded
  transitions:
    candidate:
    - observed
    - rejected
    - disputed
    - superseded
    observed:
    - verified
    - rejected
    - disputed
    - superseded
    verified:
    - disputed
    - superseded
    rejected:
    - superseded
    disputed:
    - observed
    - verified
    - rejected
    - superseded
    superseded: []
rules:
  candidate_may_result_from_inference: true
  observed_requires_source_or_evidence: true
  verified_requires_verification_method: true
  verified_requires_sufficient_evidence: true
  model_confidence_alone_promotes_to_verified: false
  verified_retains_provenance: true
  rejected_means_available_evidence_disproves_proposed_outcome: true
  disputed_means_authoritative_or_material_evidence_conflicts: true
  superseded_preserves_history_and_points_to_newer_outcome: true
  business_outcome_is_not_run_outcome_certainty: true
  outcome_may_exist_without_run: true
measurement:
  verified_status_is_successful_outcome_denominator: true
  evidence_refs_are_source_linked: true
```

## A.persistence.yaml

```yaml
schema_version: 1
engine: SQLite+FTS5 for local desktop baseline; repository interfaces allow enterprise store later.
tables:
- workspaces
- people
- organizations
- projects
- threads
- messages
- meetings
- commitments
- decisions
- work_items
- relationship_profiles
- memory_claims
- agent_tasks
- execution_plans
- actions
- runs
- approvals
- connector_profiles
- credential_profiles
- model_profiles
- scheduled_jobs
- artifacts
- artifact_contents
- events
- audit_records
- sync_cursors
- subscriptions
- settings
never_store_plaintext:
- api_keys
- oauth_refresh_tokens
- passwords
- private_keys
indexes:
- project_id
- thread_id
- person_id
- status
- due_at
- external_id
- created_at
- [workspace_id, source_locator_hash, content_sha256]
- [workspace_id, content_sha256]
- fts_user_content
migration_rules:
- versioned
- backup_before_write
- fixture_test_old_to_new
- fail_closed_on_migration_error
- no_silent_database_reset
store_profiles:
  local_authoritative:
    engine: SQLite+FTS5
    scope: single local workspace authority
  enterprise_authoritative_reference:
    engine: PostgreSQL
    scope: managed/headless Core; tenant deployment may substitute an equivalent transactional store through repository
      contracts
  rule: Exactly one authoritative mutable store per workspace. Local and managed stores may synchronize/cache but
    must not operate as independent multi-master authorities.
execution_revision_contract_ref: spec/execution.yaml#run
authoritative_store:
  exactly_one_mutable_authority_per_workspace: true
  authority_changing_write_transactions_serialized: true
  local_authoritative_profile:
    one_writable_authority_process_per_workspace: true
    exclusive_os_level_file_lock_during_writable_authority: true
    lock_unavailable: read_only_or_fail_closed
    backup_is_never_parallel_writable_authority: true
  enterprise_profile:
    equivalent_transactional_authority_guarantee_required: true
    distributed_split_brain_resolution: adapter_specific_lease_or_consensus
identities:
  unique:
  - AgentTaskId
  - ApprovalId
  - RunId
  - [ExecutionPlanId, version]
  - [ActionId, version]
  - [ActionId, action_version, attempt]
  parent_integrity:
  - AgentTask.workspace_id
  - ExecutionPlan.task_id
  - Action.plan_id
  - Approval.task_id
  - Approval.action_id
  - Run.action_id
  - Artifact.workspace_id
  foreign_key_integrity_required: true
aggregate_boundaries:
  workspace_authority:
    owns: [Workspace, AgentTask, AuditRecord]
  plan_identity:
    owns: [ExecutionPlan, Action]
    revisions_immutable: true
  approval_binding:
    owns: [Approval]
    binds: [ActionId, Action.version, action_hash]
  run_attempt:
    owns: [Run]
    binds: [ActionId, Action.version, action_hash, ApprovalId, Approval.version]
  evidence:
    immutable_after_write: true
    exact_run_binding_required: true
  workspace_evidence_snapshot:
    owns: [Artifact]
    immutable_after_write: true
    creates_memory_claims: false
    creates_workgraph_edges: false
history:
  plan_action_approval_run_hard_delete_runtime_api: forbidden
  supersession_preserves_history: true
atomic_transactions:
  authority_boundary: serialized_authority_changing_write
  T1_material_action_revision:
    steps:
    - validate_current_revision
    - persist_next_action_revision_exactly_once
    - apply_state_aware_old_approval_mutation
    - create_new_approval_binding_when_required_or_not_required
    - append_audit_event
    run_creation_included: false
  T2_approval_decision_cas:
    steps:
    - revalidate_approval_version_state_binding_actor_expiry
    - mutate_approval
    - increment_approval_version
    - append_audit_event
  T3_run_creation:
    steps:
    - verify_task_allows_new_run
    - verify_exact_current_action_revision_and_hash
    - verify_exact_approval_record_and_version
    - verify_current_authorization_requirements
    - enforce_attempt_uniqueness
    - insert_created_run
    - append_audit_event
    action_approval_creation_same_transaction_required: false
  T4_run_transition_evidence:
    steps:
    - match_expected_run_version_and_state
    - validate_required_evidence_reference_or_content
    - transition_run
    - increment_run_version
    - append_evidence_when_required
    - append_audit_event
    evidence_required_for_reconciliation_or_freshness_transitions: true
  T5_task_pause:
    steps:
    - match_expected_task_version_and_state
    - request_pausing
    - increment_task_version
    - append_audit_event
    race_guards:
    - run_creation_rechecks_current_task_state
    - created_to_starting_rechecks_current_task_state
  T6_workspace_evidence_import:
    steps:
    - require_complete_host_acquisition_before_transaction
    - validate_request_workspace_and_bounds
    - reuse_or_create_workspace_scoped_immutable_content_by_content_sha256
    - reuse_or_create_artifact_by_workspace_source_locator_hash_and_content_sha256
    - append_one_minimal_audit_record_per_artifact_result
    atomic_all_or_nothing: true
    partial_artifacts_on_failure: forbidden
    refusal_audit: separate_atomic_append_allowed
evidence:
  immutable_after_write: true
  exact_binding: [run_id, action_id, action_version, action_hash]
  identity: content_addressed_or_integrity_protected
  connector_production_out_of_scope: true
workspace_evidence_import:
  contract_ref: spec/domain.yaml#artifact_contracts
  supported_media_types: [utf8_plain_text, markdown]
  bounds:
    maximum_files_per_request: 8
    maximum_file_bytes: 262144
    maximum_total_bytes: 1048576
  input_boundary:
    caller_supplies_each_path_explicitly: true
    local_regular_files_only: true
    UNC_or_network_paths: forbidden
    directory_input: forbidden
    recursive_discovery: forbidden
    symlink_junction_reparse_points: forbidden
    unsupported_encoding: fail_closed
    source_change_during_acquisition: fail_closed
    acquisition_rule: read_exactly_one_opened_regular_file_then_revalidate_identity_and_length
  duplicate_behavior:
    same_workspace_same_source_same_bytes: return_existing_artifact_and_append_idempotent_audit
    same_workspace_same_source_changed_bytes: create_new_immutable_artifact_preserve_prior_artifact
    same_workspace_different_source_identical_bytes: distinct_artifact_provenance_reuse_workspace_scoped_content_storage
    different_workspace_identical_bytes: no_cross_workspace_content_deduplication
  snapshot_read:
    reads_MAIA_owned_content_not_external_source: true
    restart_reopen_returns_same_artifact_and_citation_identity: true
    external_source_change_or_deletion_affects_existing_snapshot: false
  lifecycle:
    artifact_runtime_hard_delete: forbidden
    source_deletion_does_not_delete_artifact: true
    workspace_erasure_policy: existing_audit_tombstone_or_later_explicit_policy
  audit:
    success_event_type: workspace_evidence_imported
    idempotent_event_type: workspace_evidence_import_idempotent
    refusal_event_type: workspace_evidence_import_refused
    subject_type: artifact
    subject_ref: ArtifactId
    operation_ref: EvidenceImportRequestId
    provenance_reference: source_locator_hash
    content_or_raw_locator_in_audit: forbidden
policy_snapshot:
  approval_policy_snapshot_hash_preserved: true
  provider_or_administration_out_of_scope: true
  opaque_reference_allowed: true
  historical_audit_must_not_substitute_current_policy: true
migrations:
  ledger_fields: [migration_id, checksum, applied_at]
  monotonic_sequence: true
  applied_checksum_immutable: true
  backup_before_write: true
  transactional_when_engine_allows: true
  newer_schema_than_binary: fail_closed
  checksum_mismatch: fail_closed
  silent_repair: forbidden
backup_recovery:
  consistent_backup_before_migration: true
  backup_is_not_writable_authority: true
  restore_mode: offline_explicit
  automatic_backup_failover: forbidden
  startup_integrity_failure: fail_closed
  engine_crash_recovery: SQLite_WAL_allowed
clock:
  application_time_source: injected_runtime_clock
  store_is_not_time_authority: true
  audit_order_source: per_workspace_sequence
erasure:
  execution_audit_scope: minimize_direct_personal_data_before_append
  normal_history_cascade_delete: forbidden
  full_person_memory_workgraph_erasure: later_milestone
errors:
  cas_mismatch: ExternalConflict
  corruption: fail_closed
  migration_incompatibility: MigrationError
  unavailable_writable_authority: PersistenceError
  exhausted_storage_busy: PersistenceError
```

## A.product.yaml

```yaml
schema_version: 1
product:
  id: maia
  name: MAIA
  expanded_name: Multichannel Automation & Intelligent Assistance
  version: 1.9.0
  edition: Trusted Execution Intelligence - M0.10 Local Evidence Briefing
  snapshot_date: '2026-09-11'
  promise: A human-governed trusted execution intelligence system that turns signals and context into source-linked
    commitments, decisions, risks, actions and measurable outcomes across communication, meetings, documents and tools.
principles:
- channel_neutral_core
- mail_is_a_strategic_first_class_domain_but_not_a_transport_lock_in
- human_is_highest_authority
- connector_and_model_independence
- local_first_when_required
- no_hidden_execution
- auditable_provenance
- least_privilege
- approval_before_material_risk
- canonical_spec_first
- future_m365_ready
- trusted_execution_intelligence
- signal_to_outcome_continuity
- outcome_is_distinct_from_run_completion
- risk_class_is_distinct_from_reasoning_assurance
- reasoning_assurance_never_grants_execution_permission
- round_table_is_selective_assurance_escalation
- measurable_product_advantage
- narrow_wedge_broad_architecture_global_ambition
- commercial_readiness_is_governance_not_runtime_fsm
non_goals:
- hard_dependency_on_outlook_classic
- hard_dependency_on_microsoft_graph
- single_model_lock_in
- secret_storage_in_sqlite
- silent_external_send
- silent_destructive_actions
- consumer_ai_dom_scraping
- security_bypass_to_reach_enterprise_data
- hardcode_initial_wedge_into_core_domain
- round_table_for_every_request
- pricing_or_legal_form_logic_in_core_domain
- unmeasured_competitive_superiority_claims
- unknown_cost_or_human_effort_treated_as_zero
value_chain:
- Signal
- Context
- Commitment
- Decision
- Risk
- Plan
- Action
- Run
- Outcome
validation_hypotheses:
  initial_wedge:
    id: microsoft_365_execution_control
    name: Microsoft 365 Execution Control
    status: validate_not_assume
    replaceable: true
    does_not_define_core: true
    promise: Detect, organize and drive to completion source-linked commitments, decisions, awaited replies, risks and actions arising from Microsoft 365 communication, meetings and documents.
    target_contexts:
    - project_intensive_organizations
    - service_organizations
    - technical_organizations
```

## A.reasoning.yaml

```yaml
schema_version: 1
assurance_levels:
  A0:
    symbol: deterministic
    description: Deterministic or rule-based reasoning; no LLM reasoning is required where possible.
  A1:
    symbol: single_model
    description: One appropriate policy-approved model or reasoning path.
  A2:
    symbol: verified_reasoning
    description: Primary reasoning plus a meaningfully independent verification or evidence check; self-repetition is insufficient.
  A3:
    symbol: round_table
    description: Selective multi-path or multi-model reasoning with disagreement capture, evidence comparison, adjudication and measured usage.
  A4:
    symbol: human_decision_required
    description: A3 plus mandatory human confirmation of the high-impact reasoning conclusion; it is not side-effect authorization.
selection_factors:
- impact
- ambiguity
- uncertainty
- conflicting_evidence
- novelty
- reversibility
- policy
hard_rules:
- reasoning_assurance_is_independent_from_risk_class
- reasoning_assurance_is_independent_from_approval_assurance
- assurance_level_never_grants_execution_permission
- model_consensus_never_bypasses_approval_gate
- round_table_is_not_default
- unknown_or_conflicting_evidence_can_raise_assurance
- policy_can_raise_but_not_silently_lower_required_assurance
- insufficient_assurance_is_explicit_and_never_silently_downgrades
- reasoning_budget_is_checked_before_invocation_and_unknown_cost_is_not_zero
round_table:
  associated_levels:
  - A3
  - A4
  required_properties:
  - independent_reasoning_contributions
  - disagreement_record
  - evidence_comparison
  - adjudication_result
  - model_or_reasoning_path_provenance
  - usage_and_cost_recording
  forbidden_shortcuts:
  - same_path_repetition_presented_as_independent_verification
  - majority_vote_as_only_truth_rule
  - direct_execution_privilege
  - approval_gate_bypass
  M0_8_vertical_slice_ref: spec/round_table.yaml
  M0_9_orchestration_ref: spec/assurance_routing.yaml
a4_human_confirmation:
  applies_to: reasoning_conclusion_acceptance
  substitutes_for_approval_gate: false
  read_only_analysis_may_be_presented: true
  high_impact_conclusion_before_confirmation: unconfirmed
```

## A.round_table.yaml

```yaml
schema_version: 1
contract:
  scope: M0.8_provider_neutral_reasoning_only_vertical_slice
  pipeline:
  - DecisionRequest
  - RoundTableSession
  - independent_participant_invocations
  - ParticipantResponse
  - disagreement_and_evidence_capture
  - adjudication
  - RoundTableDecision
  execution_authority: forbidden
participants:
  required_contracts:
  - Participant
  - ParticipantId
  - ModelProvider
  - ModelRef
  - ParticipantRequest
  - ParticipantResponse
  - RoundTableSession
  - RoundTableDecision
  - Disagreement
  - EvidenceReference
  - UsageCostMetadata
  provider_neutral: true
  model_configurable: true
  hidden_provider_or_model_fallback: forbidden
  type_contracts:
    Participant:
      required_members: [descriptor, invoke]
    ParticipantId:
      role: stable_participant_identity
    ModelProvider:
      role: provider_identity_not_part_of_core_routing_policy
    ModelRef:
      role: configured_model_reference
    ParticipantRequest:
      required_fields: [session_id, decision, max_output_tokens]
      first_round_contains_other_participant_responses: false
    ParticipantResponse:
      required_fields: [participant, response_text, evidence, provider_request_id, usage]
    RoundTableSession:
      required_fields: [id, assurance, decision, responses, disagreements, adjudication]
    RoundTableDecision:
      required_fields: [session_id, conclusion, evidence, disagreement_ids, execution_authority]
      execution_authority_must_be_false: true
    Disagreement:
      required_fields: [id, participants, summary, evidence]
    EvidenceReference:
      required_fields: [id, source_ref]
    UsageCostMetadata:
      required_fields: [input_tokens, output_tokens, cost_known, cost_minor, currency]
independence:
  first_round_request_contains_other_participant_responses: false
  responses_persisted_or_retained_separately_before_adjudication: true
  adjudication_receives_independent_responses: true
  disagreement_explicit: true
assurance:
  allowed_levels:
  - A3
  - A4
  preserves_ref: spec/reasoning.yaml
  a3_or_a4_increases_action_execution_authority: false
  approval_gate_bypass: forbidden
anthropic_adapter:
  provider: anthropic
  boundary: infra_adapter_not_core
  api: messages
  model_source: local_configuration
  credential_source: local_secret_or_environment_boundary_only
  organization_id_required: false
  request_timeout_required: true
  retries: safe_transient_failures_only
  request_id_capture: true
  usage_capture_when_returned: true
  other_provider_fallback: forbidden
live_smoke:
  default: skipped
  explicit_opt_in_environment: M0_8_LIVE_ANTHROPIC=1
  credential_environment: ANTHROPIC_API_KEY
  model_environment: MAIA_ANTHROPIC_MODEL
  sensitive_prompt_or_credential_logging: forbidden
orchestration:
  scope: M0.13_provider_neutral_round_table_live_orchestration
  contract_evolution:
    m0_8_entry_point: preserved_as_compatibility_shim
    shim_delegates_to_new_implementation: true
    spec_precedes_implementation: true
  participant_outcomes:
    collect_all_before_quorum_evaluation: true
    single_participant_failure_aborts_collection: false
    outcome_kinds:
    - responded
    - failed
    failed_contribution_retained_in_provenance: true
    required_failure_fields:
    - kind
    - reason_code
  quorum:
    evaluated_after_collection: true
    a3_minimum_independent_responses: 2
    below_minimum_result: insufficient_assurance
    below_minimum_reason_code: participant_unavailable
    silent_downgrade: forbidden
    single_participant_round_table: forbidden
    leader_substitutes_for_missing_first_round_response: forbidden
    degraded_panel_adjudication: forbidden
  leader_selection:
    deterministic: true
    registry_driven: true
    recorded: true
    provider_or_model_hardcoding: forbidden
    eligibility:
    - enabled
    - adjudicator_role_capability
    - supports_requested_assurance
    - healthy_availability
    authority:
      scope: coordination_and_synthesis_within_session
      execution_authority: forbidden
      approval_authority: forbidden
      policy_authority: forbidden
      core_authority: forbidden
      persistence_ownership_outside_session_contract: forbidden
    independent_of_provider_resolution: true
  participant_resolver:
    role: maps_registration_to_live_participant
    core_depends_on_port_only: true
    provider_executable_model_or_credential_in_core: forbidden
    resolution_failure_is_isolated_and_classified: true
    silent_resolution_to_another_provider: forbidden
  provenance:
    required_fields:
    - participant_id
    - provider
    - model_ref_as_used
    - role
    - sequence
    - started_at
    - finished_at
    - provider_request_id
    - result
    - reason_code
    - first_round_isolated
    failed_attempts_recorded: true
    secret_or_credential_material: forbidden
  session_store:
    port_location: roundtable
    implementation_location: infra_adapter_not_core
    state_ownership: round_table_module_not_core
    reuse_existing_persistence_when_ownership_preserved: preferred
    records_versioned: true
    persists_failed_contributions: true
  core_independence:
    core_depends_on_round_table_implementation: false
    shipped_application_depends_on_round_table_implementation: false
    verified_by: dependency_graph_not_workspace_layout
```

## A.scheduler.yaml

```yaml
schema_version: 1
job_types:
- time_based
- recurring
- condition_watch
- follow_up_watch
rules:
- scheduled_execution_reuses_same_policy_gates_as_interactive_tasks
- send_external_never_becomes_auto_approved_only_because_job_is_scheduled
- condition_watch_emits_only_on_state_change_or_threshold
- jobs_have_owner_timezone
- missed_run_policy_is_explicit
examples:
- daily_priority_brief
- meeting_prebrief
- overdue_commitment_check
- awaited_reply_followup
- weekly_project_digest
```

## A.security.yaml

```yaml
schema_version: 1
protected_assets:
- mail_content
- teams_content
- calendar
- attachments
- credentials
- relationship_context
- workgraph
- audit_records
- policies
- connector_tokens
threats:
  prompt_injection_mail_or_attachment:
  - untrusted_content_boundary
  - instruction_source_labels
  - no_tool_call_from_untrusted_text_without_planner_validation
  - approval_rendering_allowlist
  - remote_resource_suppression
  wrong_recipient_or_reply_all:
  - recipient_diff
  - external_domain_warning
  - approval_hash
  secret_exfiltration:
  - os_keyring
  - redaction
  - frontend_non_return
  - no_secret_in_logs_db_exports
  connector_overprivilege:
  - least_scopes
  - capability_registry
  - admin_policy
  - periodic_scope_review
  silent_model_or_connector_swap:
  - requested_actual_provenance
  - no_hidden_fallback
  webhook_spoofing:
  - signature_or_token_validation
  - client_state
  - replay_window
  - idempotency
  ssrf_custom_endpoint:
  - scheme_allowlist
  - dns_recheck
  - link_local_block
  - https_remote_default
  malicious_mcp_tool:
  - allowlist
  - schema_validation
  - risk_class_mapping
  - sandbox
  - approval_gate
  - tool_definition_fingerprint
  - definition_change_invalidates_approval
  - server_identity_binding
  supply_chain:
  - signed_updates
  - lockfiles
  - dependency_audit
  - secret_scan
  data_retention_mismatch:
  - workspace_retention_policy
  - export_delete_runbooks
  - tenant_policy_override
  credential_compromise:
  - immediate_profile_suspend
  - provider_or_tenant_revoke
  - forced_rotation
  - invalidate_dependent_sessions_and_subscriptions
  - audit_since_last_known_good
  - blast_radius_review
  - incident_record
  concurrent_or_stale_approval:
  - compare_and_swap_version
  - action_hash_binding
  - first_terminal_decision_wins
  - ExternalConflict_on_stale_decision
  third_party_personal_data:
  - data_minimization
  - purpose_and_retention_policy
  - subject_linked_provenance
  - subject_request_workflow
  - sensitive_inference_block
  data_exfiltration_via_approval_rendering:
  - desktop_csp_blocks_remote_images_media_frames
  - teams_cards_generated_from_typed_fields_not_untrusted_card_json
  - no_untrusted_Image_Media_BackgroundImage_or_Action_OpenUrl_elements
  - escape_or_strip_markdown_links_in_untrusted_snippets
  - data_uri_blocked_in_approval_surfaces
  - trusted_static_assets_are_app_owned_or_tenant_allowlisted
  graph_poisoning_by_inference:
  - inferred_edges_are_explicitly_marked
  - source_ref_and_confidence_required
  - inferred_commitments_default_to_candidate
  - unconfirmed_inference_cannot_authorize_side_effects
  - ui_badge_suggested_until_confirmed_or_policy_promoted
invariants:
- human_authority
- least_privilege
- all_side_effects_are_auditable
- secrets_are_never_plaintext_persistent
- content_is_data_not_instruction
- enterprise_policy_can_be_stricter_than_user_policy
approval_rendering_policy:
  untrusted_content_rendering: plain_text_or_sanitized_text_only
  desktop:
    csp: default-src 'self'; img-src 'self'; media-src 'none'; frame-src 'none'; object-src 'none'
    block_data_uri: true
    external_navigation: explicit_user_action_only
  teams_adaptive_cards:
    payload_source: typed MAIA card schema only
    forbid_untrusted_elements:
    - Image
    - ImageSet
    - Media
    - BackgroundImage
    - Action.OpenUrl
    untrusted_markdown_links: escape_or_strip
    static_images: app_owned_or_tenant_allowlisted_only
  outlook_surface:
    same_typed_fields_and_sanitization: true
    remote_content_from_message_body_not_embedded_in_approval: true
execution_authorization_ref: spec/approval.yaml#execution_authorization
```

## A.sources.snapshot.yaml

```yaml
schema_version: 1
snapshot_date: '2026-09-11'
sources:
- id: MS_AGENTS_SDK
  url: https://learn.microsoft.com/en-us/microsoft-365/agents-sdk/agents-sdk-overview
  purpose: Multichannel agent/channel abstraction and model-agnostic positioning.
- id: MS_TEAMS_AGENTS
  url: https://learn.microsoft.com/en-us/microsoftteams/platform/agents-in-teams/overview
  purpose: Teams agent surfaces and extension to Outlook/M365.
- id: MS_OUTLOOK_NEW
  url: https://learn.microsoft.com/en-us/office/dev/add-ins/outlook/one-outlook
  purpose: New Outlook does not support COM/VSTO; web add-ins are migration path.
- id: MS_GRAPH_MAIL_DELTA
  url: https://learn.microsoft.com/en-us/graph/delta-query-messages
  purpose: Incremental mail synchronization.
- id: MS_GRAPH_NOTIFICATIONS
  url: https://learn.microsoft.com/en-us/graph/api/resources/change-notifications-api-overview?view=graph-rest-1.0
  purpose: Change notification subscriptions for Outlook/Teams resources.
- id: MCP_2026_07_28
  url: https://blog.modelcontextprotocol.io/posts/2026-07-28/
  purpose: 'Current MCP protocol direction: stateless core, auth hardening, extensions/tasks.'
- id: VIKTOR_START
  url: https://viktor.com/docs/getting-started
  purpose: 'Benchmark: Slack/Teams coworker, broad integrations.'
- id: VIKTOR_SCHEDULED
  url: https://viktor.com/docs/scheduled-tasks
  purpose: 'Benchmark: conversational scheduling.'
- id: VIKTOR_API
  url: https://viktor.com/docs/public-api
  purpose: 'Benchmark: scoped API keys, asynchronous runs, MCP preference.'
- id: VIKTOR_TOOLS
  url: https://viktor.com/docs/connect-your-tools
  purpose: 'Benchmark: OAuth tool connections and MCP/custom APIs.'
- id: EU_GDPR_ART5_17
  url: https://eur-lex.europa.eu/legal-content/EN-PL/TXT/?uri=CELEX:32016R0679
  purpose: GDPR principles including purpose limitation/data minimisation and data-subject rectification/erasure
    rights.
- id: EDPB_DATA_SUBJECT_RIGHTS
  url: https://www.edpb.europa.eu/topics/key-gdpr-concepts/data-subject-rights_en
  purpose: Current EDPB overview of data-subject rights and controller procedures.
- id: RFC8785_JCS
  url: https://www.rfc-editor.org/rfc/rfc8785.html
  purpose: JSON Canonicalization Scheme used for deterministic MCP tool-definition fingerprints.
- id: MS_TEAMS_CORE_CONCEPTS
  url: https://learn.microsoft.com/en-us/microsoftteams/platform/teams-sdk/teams/core-concepts
  purpose: Teams/Bot routing requires an agent endpoint; local development uses DevTunnel.
- id: MS_AGENTS_DEPLOY
  url: https://learn.microsoft.com/en-us/microsoft-365/agents-sdk/deploy-azure-bot-service-manually
  purpose: Production Agents SDK agent is a deployed web application with a public messaging endpoint.
- id: MS_AZURE_RELAY
  url: https://learn.microsoft.com/en-us/azure/azure-relay/relay-what-is-it
  purpose: Hybrid Connections provide bidirectional HTTP/WebSocket relay without opening inbound firewall ports.
- id: MS_COM_MESSAGE_FILTER
  url: https://learn.microsoft.com/en-us/windows/win32/api/objidl/nf-objidl-imessagefilter-retryrejectedcall
  purpose: COM IMessageFilter retry semantics for SERVERCALL_RETRYLATER / SERVERCALL_REJECTED.
- id: MS_OFFICE_THREADING
  url: https://learn.microsoft.com/en-us/visualstudio/vsto/threading-support-in-office
  purpose: Office COM can reject calls while busy/modal; callers must handle/retry rejected calls.
- id: MS_GRAPH_MESSAGE_RESOURCE
  url: https://learn.microsoft.com/en-us/graph/api/resources/message?view=graph-rest-1.0
  purpose: Message changeKey is the version token; messages support delta/change notifications.
- id: MS_GRAPH_MESSAGE_SEND
  url: https://learn.microsoft.com/en-us/graph/api/message-send?view=graph-rest-1.0
  purpose: Send existing draft is POST and does not document an If-Match request header.
- id: MS_TEAMS_CARD_FORMAT
  url: https://learn.microsoft.com/en-us/microsoftteams/platform/task-modules-and-cards/cards/cards-format
  purpose: Teams Adaptive Cards support Markdown links; Markdown images are not supported, while explicit Image
    elements can reference URLs.
- id: ADAPTIVE_CARD_IMAGE
  url: https://adaptivecards.io/explorer/Image.html
  purpose: Adaptive Card Image.url supports URI and data URI in schema 1.2+, motivating typed element allowlisting
    for approval cards.
```

## A.states.yaml

```yaml
schema_version: 1
machines:
  AgentTaskState:
    states:
    - draft
    - preflight
    - awaiting_approval
    - queued
    - running
    - pausing
    - paused
    - partially_completed
    - completed
    - canceled
    - failed
    transitions:
      draft:
      - preflight
      - canceled
      preflight:
      - awaiting_approval
      - queued
      - failed
      - canceled
      awaiting_approval:
      - queued
      - canceled
      - failed
      queued:
      - running
      - canceled
      - failed
      running:
      - pausing
      - partially_completed
      - completed
      - failed
      - canceled
      paused:
      - queued
      - canceled
      partially_completed:
      - queued
      - completed
      - failed
      - canceled
      pausing:
      - paused
      - partially_completed
      - completed
      - failed
      - canceled
  ActionState:
    states:
    - planned
    - gated
    - awaiting_approval
    - queued
    - running
    - reconciling
    - retryable_error
    - completed
    - skipped
    - failed
    - canceled
    transitions:
      planned:
      - gated
      - canceled
      gated:
      - awaiting_approval
      - queued
      - skipped
      - failed
      awaiting_approval:
      - queued
      - canceled
      - failed
      queued:
      - running
      - canceled
      running:
      - reconciling
      - retryable_error
      - completed
      - failed
      - canceled
      retryable_error:
      - queued
      - failed
      - canceled
      reconciling:
      - completed
      - retryable_error
      - failed
      - canceled
  ApprovalState:
    states:
    - not_required
    - pending
    - approved
    - rejected
    - expired
    - revoked
    transitions:
      pending:
      - approved
      - rejected
      - expired
      - revoked
      approved:
      - revoked
  CommitmentStatus:
    states:
    - candidate
    - proposed
    - confirmed
    - in_progress
    - fulfilled
    - overdue
    - canceled
    - disputed
    transitions:
      candidate:
      - proposed
      - confirmed
      - canceled
      proposed:
      - confirmed
      - canceled
      - disputed
      confirmed:
      - in_progress
      - fulfilled
      - overdue
      - canceled
      - disputed
      in_progress:
      - fulfilled
      - overdue
      - canceled
      - disputed
      overdue:
      - fulfilled
      - canceled
      - disputed
  ConnectorHealth:
    states:
    - unconfigured
    - needs_auth
    - connecting
    - healthy
    - degraded
    - rate_limited
    - blocked
    - error
    transitions:
      unconfigured:
      - needs_auth
      - connecting
      - blocked
      needs_auth:
      - connecting
      - blocked
      - error
      - unconfigured
      connecting:
      - healthy
      - degraded
      - rate_limited
      - blocked
      - error
      - needs_auth
      - unconfigured
      healthy:
      - degraded
      - rate_limited
      - blocked
      - error
      - needs_auth
      - unconfigured
      degraded:
      - healthy
      - connecting
      - rate_limited
      - blocked
      - error
      - needs_auth
      - unconfigured
      rate_limited:
      - connecting
      - healthy
      - degraded
      - blocked
      - error
      - needs_auth
      - unconfigured
      blocked:
      - needs_auth
      - connecting
      - error
      - unconfigured
      error:
      - connecting
      - healthy
      - degraded
      - needs_auth
      - blocked
      - unconfigured
    semantics: observed_health_state_with_explicit_allowed_transitions
  RunState:
    terminal_states: [retryable_error, completed, failed, canceled]
    states:
    - created
    - starting
    - running
    - retryable_error
    - outcome_unknown
    - reconciling
    - completed
    - failed
    - canceled
    transitions:
      created:
      - starting
      - canceled
      starting:
      - running
      - retryable_error
      - failed
      - canceled
      running:
      - retryable_error
      - outcome_unknown
      - completed
      - failed
      - canceled
      outcome_unknown:
      - reconciling
      reconciling:
      - completed
      - retryable_error
      - failed
    semantics: one execution attempt; retryable_error is terminal; retry creates a new RunId; unresolved outcomes
      require reconciliation
invalid_transition: return InvalidStateTransition and write audit event; never repair silently
pause_semantics:
  model: quiescent_non_preemptive
  task_transition: running -> pausing -> paused
  on_pause_request:
  - stop_scheduling_new_actions_or_runs
  - request_cancel_only_for_in_flight_runs_whose_contract_is_cancellable
  - allow_non_cancellable_in_flight_runs_to_reach_terminal_or_outcome_unknown
  - enter_paused_only_when_no_in_flight_run_remains
  action_state_paused_is_intentionally_absent: true
  rationale: A generic Action pause would falsely promise preemption for non-cancellable side effects such as mail.send.
  in_flight_states:
  - starting
  - running
  - outcome_unknown
  - reconciling
  run_scope: all_task_runs_including_superseded_plan_action_revisions
  after_request_forbidden:
  - create_run
  - schedule_run
  - created_to_starting
```

## A.sync.yaml

```yaml
schema_version: 1
strategies:
  outlook_classic_com: event_or_poll_based_best_effort with bounded scans and stable entry IDs when available
  microsoft_graph_mail: delta query per mail folder plus change notifications where deployment permits
  microsoft_graph_teams: change notifications/subscriptions plus targeted fetch
  calendar: delta/change notifications when available
delivery_semantics: at_least_once; deduplicate by connector + external_id + version/change token
idempotency:
- incoming_event_key
- action_idempotency_key
- webhook_replay_guard
- send_action_draft_hash
recovery:
- subscription_renewal
- lifecycle_notifications
- missed_notification_resync
- delta_token_reset_requires_bounded_full_resync
pre_side_effect_freshness:
  applies_to:
  - reply_or_forward_based_on_mutable_message
  - send_existing_draft
  - calendar_update_or_cancel
  - side_effect_using_mutable_external_object
  stored_tokens:
  - connector_external_id
  - source_version_token
  - normalized_payload_hash
  graph_mail_version_sources:
  - changeKey
  - '@odata.etag_when_returned'
  rule: Immediately before material side effect, re-read/conditionally validate the source version. If the connector
    does not document a strong If-Match precondition for that operation, perform compare-before-execute and fail
    closed on mismatch.
  on_mismatch:
  - block_execution
  - invalidate_stale_binding_per_approval_mutation_policy
  - build_material_revision_per_action_binding_contract
  approval_mutation_policy_ref: spec/approval.yaml#concurrency.mutation_policy
  send_note: Microsoft Graph send-draft is POST and does not document If-Match on that operation; MAIA MUST NOT
    assume conditional-send support.
  build_order_ref: spec/action_binding.yaml#material_revision_build_order
  evidence_contract_ref: spec/execution.yaml#freshness
side_effect_reconciliation:
  rule: A timeout/disconnect after a non-idempotent side effect may create outcome_unknown. Do not automatically
    retry until connector-specific reconciliation proves whether the effect happened.
  recommended_send_marker: Where connector permits, stamp a stable MAIA action/idempotency marker on the draft before
    send and search/reconcile by that marker after ambiguous outcomes.
  action_state: reconciling
  run_state: outcome_unknown -> reconciling
  evidence_contract_ref: spec/execution.yaml#reconciliation
```

## A.testing.yaml

```yaml
schema_version: 1
suites:
  spec:
  - schema_valid
  - unique_ids
  - state_transitions
  - acceptance_refs
  - forbidden_secret_fields
  - generated_drift
  - risk_enum_integrity
  - dynamic_risk_classifier_refs
  - approval_gate_state_mapping
  - all_fsm_transitions_explicit
  - mcp_rfc8785_declaration
  - pause_semantics
  - teams_store_authority_topology
  - approval_mutation_policy
  - execution_domain_typed_contracts
  - sync_state_aware_approval_binding_invalidation
  - transport_envelope_contract
  - action_binding_v1_golden_and_negative_vectors
  - execution_authorization_contract_closure
  - typed_execution_evidence
  - approval_execution_conformance_hardening_F1_F4
  - persistence_contract_closure
  - audit_hash_chain_contract
  - cas_version_contract
  - migration_authority_contract
  - execution_intelligence_contract
  core:
  - planner
  - approval_gate
  - privacy_gate
  - model_router
  - connector_router
  - workgraph
  - commitments
  - memory
  - scheduler
  - idempotency
  - approval_concurrency
  - approval_action_hash_recheck
  - quiescent_pause
  - run_outcome_unknown_reconciliation
  - source_freshness_precondition
  - reasoning_assurance
  - outcome_lifecycle
  - usage_metering
  - commercial_readiness_governance
  - round_table_vertical_slice
  - assurance_routing_and_budgeted_orchestration
  - inference_cannot_authorize_side_effect
  connector_contract:
  - capability_discovery
  - health
  - cancellation
  - timeouts
  - error_mapping
  - provenance
  - auth_failure
  - rate_limit
  - outlook_com_sta
  - outlook_com_busy_retry
  - source_version_recheck
  - ambiguous_send_reconciliation
  e2e:
  - classic_read_to_draft
  - teams_request_to_approval
  - graph_delta_resync
  - meeting_prebrief
  - overdue_commitment
  - api_key_rotation
  - mcp_read_tool
  security:
  - prompt_injection_mail
  - recipient_swap
  - reply_all
  - ssrf
  - webhook_replay
  - malicious_mcp
  - secret_grep
  - archive_bomb
  - policy_bypass
  - mcp_tool_definition_rug_pull
  - credential_compromise_response
  - stale_approval_decision
  - approval_remote_resource_exfiltration
  - unsafe_adaptive_card_element
  - approval_fatigue_grant_scope
  migration:
  - classic_to_graph
  - schema_upgrade
  - credential_reference_remap
golden_vectors:
- untrusted_mail_cannot_issue_tool_command
- reply_all_external_delta_forces_confirmation
- send_action_hash_change_invalidates_approval
- connector_fallback_identity_is_visible
- memory_claim_requires_source
- overdue_commitment_keeps_source
- tenant_only_blocks_external_ai
- mcp_tool_cannot_bypass_approval
- duplicate_webhook_does_not_duplicate_action
- mcp_tool_definition_change_invalidates_approval
- second_approval_decision_returns_external_conflict
- legacy_calendar_write_rejected_by_capability_gate
- core_has_no_direct_com_dependency
- task_pause_waits_for_non_cancellable_run_before_paused
- mcp_rfc8785_fingerprint_vector_is_cross_runtime_stable
- mcp_missing_optional_fingerprint_fields_are_null
- teams_cloud_adapter_never_opens_local_sqlite
- payload_mutation_revokes_revocable_old_approval_preserves_terminal_history_and_regates_new_version
- untrusted_card_content_cannot_load_remote_image_or_data_uri
- inferred_workgraph_edge_is_suggested_and_cannot_trigger_send
- graph_change_key_mismatch_replans_before_send
- ambiguous_send_is_reconciled_before_retry
- polish_hedged_commitment_stays_candidate
- u64_versions_above_2pow53_remain_distinct_in_binding
- every_binding_field_is_material
- duplicate_source_tuple_rejected
- current_assurance_tightening_blocks_execution
- approved_expiry_preserves_history
- approval_CAS_matches_current_action_revision
- retryable_run_is_terminal
- retry_resets_only_on_new_action_revision
- unresolved_effect_blocks_equivalent_new_identity
- freshness_evidence_cannot_cross_run
- pause_counts_superseded_runs
- unknown_recipient_is_external_and_operation_floor_preserved
- outcome_certainty_is_not_business_outcome
- reasoning_assurance_cannot_lower_action_risk
- round_table_cannot_authorize_execution
- a4_human_reasoning_confirmation_cannot_substitute_for_approval_gate
- unknown_cost_is_not_zero
- unknown_human_intervention_is_not_zero
- verified_outcome_requires_evidence
- m365_wedge_does_not_become_core_boundary
- personal_workspace_does_not_require_tenant
- commercial_organizational_mode_requires_tenant
- g2_precedes_paid_production_or_external_customer_production_data
- g3_g4_are_not_task_or_action_fsm_states
- diagnostic_or_noop_event_does_not_qualify_for_ttfa
- generic_model_output_does_not_qualify_for_ttfv
- m06_artifact_snapshot_contract_remains_unchanged
- round_table_first_round_is_response_isolated
- round_table_decision_never_authorizes_action
- anthropic_adapter_has_no_hidden_provider_fallback
- anthropic_live_smoke_is_opt_in_and_credential_gated
- assurance_router_policy_floor_never_lowers
- assurance_router_budget_failure_is_insufficient_assurance
- claude_code_provider_never_depends_on_anthropic_api_key
- claude_code_provider_scrubs_api_key_from_child_environment
- claude_code_provider_never_uses_bypass_permissions
- claude_code_provider_refuses_nested_invocation
- claude_code_provider_schema_violation_fails_closed
- claude_code_provider_absent_without_development_evolution_feature
- claude_code_provider_audit_write_failure_fails_invocation
- claude_code_live_smoke_is_opt_in_and_read_only
- round_table_participant_failure_does_not_abort_collection
- round_table_quorum_is_evaluated_after_all_outcomes
- round_table_below_quorum_is_insufficient_assurance_not_downgrade
- round_table_leader_never_substitutes_for_missing_quorum
- round_table_leader_selection_is_deterministic_and_unhardcoded
- round_table_failed_contribution_remains_in_provenance
- round_table_resolver_failure_never_falls_back_to_another_provider
- round_table_session_persistence_preserves_failed_contributions
- core_does_not_depend_on_round_table_implementation
- assurance_router_unknown_cost_is_not_zero
- assurance_router_unavailable_participant_is_insufficient_assurance
- assurance_router_a2_requires_independent_verifier
- assurance_router_a3_requires_response_isolated_round_table
- assurance_router_a4_requires_recorded_human_reasoning_acceptance
- assurance_router_never_grants_execution_or_approval_authority
milestone_M0_1_1:
  coverage: contract_oracles_and_structural_domain_conformance_not_production_execution
  deferred: production_policy_connectors_scheduler_persistence_UI
milestone_M0_2_1:
  coverage: canonical_persistence_and_audit_contracts_without_store_implementation
  required_golden_vectors:
  - audit_genesis_zero_prev_hash
  - audit_record_hash_excludes_record_hash
  - task_run_cas_version_overflow_fails_closed
  - action_revision_and_run_cas_are_distinct
  - t1_t5_atomic_transaction_boundaries
  - local_authority_lock_loss_fails_closed
  deferred:
  - SQLite implementation
  - core_store
  - infra_sqlite
  - persistence_dependencies
```

## A.transport.yaml

```yaml
schema_version: 1
principles:
- single_state_authority_per_workspace
- channel_service_is_not_a_second_brain
- no_direct_cloud_sqlite_access
- outbound_only_local_relay_when_local_authority_is_used
- authenticated_envelopes
- at_least_once_delivery_with_idempotency
topologies:
  desktop_only:
    channel_endpoint: none
    core: local
    state_authority: sqlite
    public_inbound_required: false
  local_with_teams_relay:
    channel_endpoint: managed_https_agent_service
    core: local
    state_authority: sqlite
    relay: outbound_bidirectional_wss
    reference: azure_relay_hybrid_connections
    public_inbound_to_workstation: false
  enterprise_managed:
    channel_endpoint: managed_https_agent_service
    core: managed_headless
    state_authority: postgresql_reference
    desktop: thin_client_or_capability_worker
  hybrid_enterprise:
    channel_endpoint: managed_https_agent_service
    core: managed_plus_local_worker
    state_authority: one_explicit_authority_per_workspace
    relay: outbound_authenticated_channel
envelope:
  required_fields:
  - envelope_id
  - workspace_id
  - task_or_approval_id
  - kind
  - actor_id
  - origin_surface
  - created_at
  - expires_at
  - nonce
  - payload_hash
  - payload
  - auth_context
  security:
  - short_lived_auth
  - replay_guard
  - workspace_binding
  - actor_binding
  - payload_hash_validation
  - idempotency_key
  approval_rules:
  - relay_cannot_mutate_action_payload
  - approval_decision_still_uses_CAS_in_authoritative_core
  - stale_relay_message_returns_ExternalConflict
offline_behavior:
  local_core_unreachable: channel service may queue bounded envelopes but may not execute local-authority side effects
  expiry: expired approval/action envelopes fail closed and require refresh
```

## A.ux.yaml

```yaml
schema_version: 1
zones:
  command_center:
  - today_brief
  - what_changed
  - needs_your_decision
  - waiting_for_others
  - you_promised
  - at_risk
  - maia_can_handle
  - completed_for_you
  - priority_inbox
  - commitments
  - meetings
  - awaiting_approvals
  conversation:
  - chat_with_maia
  - task_trace
  - sources
  - approval_cards
  mail:
  - thread_summary
  - why_it_matters
  - draft
  - commitments
  - relationship_context
  workgraph:
  - people
  - projects
  - threads
  - decisions
  - commitments
  - timeline
  settings:
  - connectors
  - models
  - credentials
  - policies
  - memory
  - scheduler
  - mcp
  - audit
surfaces:
- desktop
- teams
- outlook_addin
- m365_copilot_future
- api_mcp
async_states:
- idle
- loading
- success
- error
- canceled
- awaiting_approval
rules:
- same_task_identity_across_surfaces
- approval_card_shows_exact_side_effect
- no_color_only_status
- keyboard_accessible
- dark_light_ready
- inferred_state_shows_source_or_evidence_access_and_inference_status
- inbox_is_source_drill_down_not_product_home
activation:
  default_question_first_experience: forbidden
  first_value_goal: source_linked_operational_view
  qualifying_value_excludes:
  - application_open
  - data_import_without_user_work_value
  - generic_text_generation
  - model_response_success
  metrics:
  - time_to_first_value
  - time_to_first_action
```

## A.workgraph.yaml

```yaml
schema_version: 1
node_types:
- person
- organization
- project
- thread
- message
- meeting
- commitment
- decision
- work_item
- artifact
- deadline
- dependency
- risk
- outcome
edge_types:
- works_for
- participates_in
- belongs_to_project
- replies_to
- mentions
- depends_on
- promised_to
- decided_in
- follow_up_to
- supersedes
- attached_to
- scheduled_for
rules:
- edges_keep_source_and_confidence
- inference_edges_are_distinct_from_explicit_edges
- project_linking_can_be_corrected_by_user
- deleted_external_content_does_not_silently_delete_audit_history
use_cases:
- meeting_briefing
- thread_context
- commitment_tracking
- relationship_brief
- project_status
- contradiction_detection
- deadline_drift_detection
- dependency_block_detection
- risk_state_change
- outcome_tracking
- execution_history
- what_changed_since_last_review
edge_provenance:
  required_fields:
  - source_ref
  - confidence
  - is_inferred
  - status
  inferred_default:
    is_inferred: true
    status: candidate
    ui_badge: suggested_by_ai
    may_authorize_side_effect: false
  promotion: human confirmation or explicit policy-controlled deterministic evidence may promote; promotion is audited
  correction: user correction supersedes inference and triggers recomputation of derived context
execution_intelligence_rules:
- inferred_deadline_dependency_or_risk_is_candidate_until_confirmed_or_policy_promoted
- inferred_edge_never_authorizes_a_side_effect
- outcome_evidence_edges_keep_source_ref
- outcome_is_distinct_from_technical_run_completion
```
