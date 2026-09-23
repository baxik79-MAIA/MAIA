# GENERATED - do not edit

```yaml
schema_version: 1
scope: Pure contracts; no scheduler, policy engine, persistence implementation, connectors
  or runtime execution in M0.2.1.
persistence_contract_ref: spec/persistence.yaml#atomic_transactions
run:
  exact_binding_fields:
  - action_id
  - action_version
  - action_hash
  - approval_id
  - approval_version
  record_cas_field: Run.version
  record_cas_ref: spec/domain.yaml#execution_contracts.Run.rules
  result_affects: exact_action_revision_only
  newer_revision_may_receive_historical_result: false
  authorization_record_versions_reconstructible: true
  eligibility_ref: spec/approval.yaml#execution_authorization
  evidence_validation: match_run_id_action_id_action_version_action_hash
attempts:
  scope:
  - action_id
  - action_version
  first: 1
  retry: new_RunId_previous_attempt_plus_one
  new_revision: reset_to_1
  new_action: reset_to_1
  initial_state: created
  overflow: fail_closed_no_new_Run
  prior_retryable_result_mutable: false
  retry_preconditions:
  - current_policy_permits
  - current_execution_checks
  - prior_effect_resolved
reconciliation:
  values:
  - effect_confirmed
  - effect_not_executed
  - unresolved
  start_edge: outcome_unknown_to_reconciling
  effect_confirmed: completed_no_retry_of_same_effect
  effect_not_executed: authoritative_non_execution_proof_before_considering_new_Run
  effect_not_executed_run_state: retryable_error
  unresolved: remain_reconciling_block_retry
  required_evidence:
  - run_id
  - action_id
  - action_version
  - action_hash
  - effect_identity
  - source_ref
  - result
  non_authoritative_search_absence_is_proof: false
  authoritative_proof_source: connector_specific_canonical_reconciliation_contract
  unresolved_blocks_semantically_equivalent_effect_across_ids_and_revisions: true
  connector_implementations: deferred
freshness:
  values:
  - matched
  - mismatched
  - unverifiable
  enforcement:
  - strong_precondition
  - compare_before_execute
  required_evidence:
  - run_id
  - action_id
  - action_version
  - action_hash
  - enforcement
  - precondition
  - observed_source_version_token
  - observed_payload_hash
  - result
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
  forbidden_after_request:
  - create_run
  - schedule_run
  - created_to_starting
  paused_requires: zero_in_flight_runs
  supersession_hides_in_flight: false
  cancellability_source: explicit_operation_contract
  non_cancellable_runs: drain_or_reconcile
```
