#!/usr/bin/env python3
from pathlib import Path
from evolution_supervisor_contract import validate as validate_evolution_supervisor
import sys, yaml, subprocess, shutil
ROOT=Path(__file__).resolve().parents[1]
SPEC=ROOT/'spec'
errors=[]; loaded={}
for f in sorted(SPEC.glob('*.yaml')):
    try:
        d=yaml.safe_load(f.read_text(encoding='utf-8'))
        if not isinstance(d,dict): errors.append(f'{f.name}: root must be mapping')
        elif not isinstance(d.get('schema_version'),int): errors.append(f'{f.name}: schema_version must be integer')
        loaded[f.name]=d
    except Exception as e: errors.append(f'{f.name}: YAML parse error: {e}')
if errors:
    print('\n'.join(errors)); sys.exit(1)

approval=loaded['approval.yaml']; states=loaded['states.yaml']; ipc=loaded['ipc.yaml']
risk=set(approval['risk_classes']); gates=set(approval['gate_types']); approval_states=set(states['machines']['ApprovalState']['states'])
if set(approval['default_gate']) != risk: errors.append('approval.yaml: default_gate keys must exactly match risk_classes')
for r,g in approval['default_gate'].items():
    if g not in gates: errors.append(f'approval.yaml: default_gate[{r}] invalid gate {g}')
if set(approval['gate_to_approval_state']) != gates: errors.append('approval.yaml: gate_to_approval_state must cover all gate_types')
for g,m in approval['gate_to_approval_state'].items():
    for k,v in m.items():
        if k in ('result','if_policy_allows_without_confirmation','otherwise') and v not in approval_states:
            errors.append(f'approval.yaml: gate {g} maps to unknown ApprovalState {v}')

# IPC risk/classifier contract. Spec-first: validate binding symbols now; implementation registry/compile test becomes mandatory once core exists.
classifiers=approval.get('dynamic_risk_classifiers',{})
for cmd,meta in ipc['commands'].items():
    has_r='risk' in meta; has_dyn='risk_resolution' in meta
    if has_r == has_dyn: errors.append(f'ipc.yaml:{cmd}: exactly one of risk/risk_resolution required')
    if has_r and meta['risk'] not in risk: errors.append(f'ipc.yaml:{cmd}: invalid risk {meta["risk"]}')
    if has_dyn:
        rr=meta['risk_resolution']; name=rr.get('classifier')
        if rr.get('type')!='dynamic': errors.append(f'ipc.yaml:{cmd}: risk_resolution.type must be dynamic')
        if name not in classifiers: errors.append(f'ipc.yaml:{cmd}: unknown classifier {name}'); continue
        outs=set(classifiers[name]['outputs']); bad=outs-risk
        if bad: errors.append(f'approval.yaml:{name}: outputs not in RiskClass: {sorted(bad)}')
        expected='approval.dynamic_risk_classifiers.'+name
        if rr.get('contract_ref')!=expected: errors.append(f'ipc.yaml:{cmd}: contract_ref must be {expected}')
        if not str(rr.get('implementation_symbol','')).endswith('.'+name): errors.append(f'ipc.yaml:{cmd}: implementation_symbol must bind classifier {name}')

# FSM completeness/validity
for name,m in states['machines'].items():
    st=set(m['states']); tr=m.get('transitions')
    if not isinstance(tr,dict): errors.append(f'states.yaml:{name}: transitions must be explicit mapping'); continue
    unknown=set(tr)-st
    if unknown: errors.append(f'states.yaml:{name}: transition sources not states {sorted(unknown)}')
    for src,dests in tr.items():
        if not isinstance(dests,list): errors.append(f'states.yaml:{name}.{src}: destinations must be list'); continue
        bad=set(dests)-st
        if bad: errors.append(f'states.yaml:{name}.{src}: unknown destinations {sorted(bad)}')
    terminals = m.get('terminal_states', [])
    if not isinstance(terminals, list) or any(not isinstance(s, str) for s in terminals):
        errors.append(f'states.yaml:{name}: terminal_states must be a list of state names')
    elif len(terminals) != len(set(terminals)) or set(terminals) - st:
        errors.append(f'states.yaml:{name}: terminal_states must be unique declared states')
    else:
        for terminal in terminals:
            if tr.get(terminal, []) != []:
                errors.append(f'states.yaml:{name}.{terminal}: terminal state cannot have outgoing transitions')
# Pause semantics are deliberately Task-level, not generic Action preemption.
task=set(states['machines']['AgentTaskState']['states']); action=set(states['machines']['ActionState']['states'])
if 'pausing' not in task or 'paused' not in task: errors.append('states.yaml: AgentTaskState must include pausing and paused')
if 'paused' in action: errors.append('states.yaml: generic ActionState.paused is forbidden; pause is quiescent/non-preemptive')
ps=states.get('pause_semantics',{})
if ps.get('model')!='quiescent_non_preemptive' or ps.get('action_state_paused_is_intentionally_absent') is not True:
    errors.append('states.yaml: pause_semantics must declare quiescent_non_preemptive and intentional Action pause absence')
if 'RunState' not in states['machines'] or 'outcome_unknown' not in states['machines']['RunState']['states']:
    errors.append('states.yaml: RunState with outcome_unknown reconciliation state is required')
if 'reconciling' not in states['machines']['ActionState']['states']:
    errors.append('states.yaml: ActionState.reconciling is required')

# Approval concurrency/mutation
conc=approval.get('concurrency',{})
if conc.get('stale_or_second_decision_error')!='ExternalConflict': errors.append('approval.yaml: stale approval conflict must map to ExternalConflict')
mut=conc.get('mutation_policy',{})
def require_contract(actual, expected, path):
    """Validate required contract structure and exact scalar/list semantics."""
    if isinstance(expected, dict):
        if not isinstance(actual, dict):
            errors.append(f'{path}: required mapping missing or invalid')
            return
        for key, value in expected.items():
            require_contract(actual.get(key), value, f'{path}.{key}')
    elif type(actual) is not type(expected) or actual != expected:
        errors.append(f'{path}: expected {expected!r}, got {actual!r}')

require_contract(mut, {'on_payload_divergence': 'invalidate_binding_and_supersede_state_aware',
 'old_binding_valid_for_new_payload_version': False,
 'revocable_prior_states': ['pending', 'approved'],
 'revocable_handling': {'transition_to': 'revoked', 'reason': 'action_hash_divergence'},
 'preserved_prior_states': ['not_required', 'rejected', 'expired', 'revoked'],
 'preserved_handling': 'preserve_historical_state_without_transition',
 'new_version_required': True,
 'new_version_steps': ['build_material_revision_per_action_binding_contract',
                       'create_new_approval_id_at_record_version_1_unless_policy_denies'],
 'new_approval_state_source': 'spec/approval.yaml#policy_decision.decision_to_record',
 'not_required_is_valid_gate_result': True,
 'build_order_ref': 'spec/action_binding.yaml#material_revision_build_order'}, 'approval.yaml:concurrency.mutation_policy')
if any(key in mut for key in ('previous_state', 'new_approval_state', 'revocation_reason')):
    errors.append('approval.yaml: ambiguous legacy mutation directives must not override state-aware handling')
revocable = mut.get('revocable_prior_states', [])
preserved = mut.get('preserved_prior_states', [])
if isinstance(revocable, list) and isinstance(preserved, list) and all(isinstance(x, str) for x in revocable + preserved):
    if set(revocable) & set(preserved) or set(revocable + preserved) != approval_states:
        errors.append('approval.yaml: mutation state sets must be disjoint and cover all ApprovalState values')
    transitions = states['machines']['ApprovalState']['transitions']
    for state in revocable:
        if 'revoked' not in transitions.get(state, []):
            errors.append(f'approval.yaml: {state} lacks explicit transition to revoked')
    for state in preserved:
        if 'revoked' in transitions.get(state, []):
            errors.append(f'approval.yaml: preserved {state} must not transition to revoked')

# Execution-domain wire contracts. Expectations enforce the architecture decision;
# enum values and FSM transitions are resolved from their canonical sources below.
domain = loaded['domain.yaml']
required_domain_contract = {'primitives': {'WorkspaceId': {'wire_type': 'string',
                                'format': 'uuid',
                                'uuid_version': 7,
                                'canonical_text': 'lowercase_hyphenated',
                                'rust_type': 'distinct_strong_newtype'},
                'AgentTaskId': {'wire_type': 'string',
                                'format': 'uuid',
                                'uuid_version': 7,
                                'canonical_text': 'lowercase_hyphenated',
                                'rust_type': 'distinct_strong_newtype'},
                'ExecutionPlanId': {'wire_type': 'string',
                                    'format': 'uuid',
                                    'uuid_version': 7,
                                    'canonical_text': 'lowercase_hyphenated',
                                    'rust_type': 'distinct_strong_newtype'},
                'ActionId': {'wire_type': 'string',
                             'format': 'uuid',
                             'uuid_version': 7,
                             'canonical_text': 'lowercase_hyphenated',
                             'rust_type': 'distinct_strong_newtype'},
                'RunId': {'wire_type': 'string',
                          'format': 'uuid',
                          'uuid_version': 7,
                          'canonical_text': 'lowercase_hyphenated',
                          'rust_type': 'distinct_strong_newtype'},
                'ApprovalId': {'wire_type': 'string',
                               'format': 'uuid',
                               'uuid_version': 7,
                               'canonical_text': 'lowercase_hyphenated',
                               'rust_type': 'distinct_strong_newtype'},
                'ConnectorProfileId': {'wire_type': 'string',
                                       'format': 'uuid',
                                       'uuid_version': 7,
                                       'canonical_text': 'lowercase_hyphenated',
                                       'rust_type': 'distinct_strong_newtype'},
                'ModelProfileId': {'wire_type': 'string',
                                   'format': 'uuid',
                                   'uuid_version': 7,
                                   'canonical_text': 'lowercase_hyphenated',
                                   'rust_type': 'distinct_strong_newtype'},
                'Utf8String': {'wire_type': 'string', 'encoding': 'UTF-8'},
                'NonEmptyString': {'wire_type': 'string', 'encoding': 'UTF-8', 'min_length': 1},
                'PolicyId': {'wire_type': 'string',
                             'encoding': 'UTF-8',
                             'min_length': 1,
                             'semantics': 'opaque',
                             'normalization': 'none',
                             'uuid_required': False},
                'ActorRef': {'wire_type': 'string',
                             'encoding': 'UTF-8',
                             'min_length': 1,
                             'semantics': 'opaque',
                             'normalization': 'none',
                             'represents': 'authorized_user_service_or_policy_actor'},
                'OpaqueRef': {'wire_type': 'string',
                              'encoding': 'UTF-8',
                              'min_length': 1,
                              'semantics': 'opaque',
                              'normalization': 'none',
                              'infer_path_or_uri_semantics': False},
                'SurfaceId': {'wire_type': 'string',
                              'encoding': 'UTF-8',
                              'min_length': 1,
                              'registry': 'open',
                              'examples_non_normative': ['desktop', 'teams', 'outlook', 'web']},
                'ActionType': {'wire_type': 'string', 'pattern': '^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)+$'},
                'Timestamp': {'wire_type': 'string',
                              'format': 'RFC3339',
                              'timezone': 'UTC',
                              'canonical_serialization': 'YYYY-MM-DDTHH:MM:SS.sssZ',
                              'fractional_second_digits': 3,
                              'canonical_timezone_suffix': 'Z',
                              'parser_acceptance': 'valid_RFC3339_may_be_accepted_and_normalized'},
                'Version': {'wire_type': 'integer',
                            'signed': False,
                            'bits': 64,
                            'minimum': 1,
                            'maximum': 18446744073709551615,
                            'monotonic_scope': 'versioned_aggregate_or_object',
                            'monotonically_increases': True,
                            'overflow': 'fail_closed_never_wrap_saturate_or_reuse'},
                'Ordinal': {'wire_type': 'integer',
                            'signed': False,
                            'bits': 32,
                            'minimum': 0,
                            'maximum': 4294967295},
                'Attempt': {'wire_type': 'integer',
                            'signed': False,
                            'bits': 32,
                            'minimum': 1,
                            'maximum': 4294967295},
                'AmountMicros': {'wire_type': 'integer',
                                 'signed': False,
                                 'bits': 64,
                                 'minimum': 0,
                                 'maximum': 18446744073709551615},
                'Sha256Hex': {'wire_type': 'string', 'pattern': '^[0-9a-f]{64}$'},
                'CurrencyCode': {'wire_type': 'string',
                                 'pattern': '^[A-Z]{3}$',
                                 'semantics': 'ISO-4217-compatible_code_shape'},
                'PrivacyClass': {'wire_type': 'string',
                                 'enum_source': 'spec/modes.yaml#privacy_classes',
                                 'source_selection': 'mapping_keys'},
                'RiskClass': {'wire_type': 'string',
                              'enum_source': 'spec/approval.yaml#risk_classes',
                              'source_selection': 'mapping_keys'},
                'GateType': {'wire_type': 'string',
                             'enum_source': 'spec/approval.yaml#gate_types',
                             'source_selection': 'mapping_keys'},
                'AgentTaskState': {'wire_type': 'string',
                                   'enum_source': 'spec/states.yaml#machines.AgentTaskState.states',
                                   'transitions_source': 'spec/states.yaml#machines.AgentTaskState.transitions'},
                'ActionState': {'wire_type': 'string',
                                'enum_source': 'spec/states.yaml#machines.ActionState.states',
                                'transitions_source': 'spec/states.yaml#machines.ActionState.transitions'},
                'RunState': {'wire_type': 'string',
                             'enum_source': 'spec/states.yaml#machines.RunState.states',
                             'transitions_source': 'spec/states.yaml#machines.RunState.transitions'},
                'ApprovalState': {'wire_type': 'string',
                                  'enum_source': 'spec/states.yaml#machines.ApprovalState.states',
                                  'transitions_source': 'spec/states.yaml#machines.ApprovalState.transitions'},
                'ConnectorSelection': {'wire_type': 'string',
                                       'enum_source': 'spec/action_binding.yaml#connector_selection.values'},
                'PolicyDecision': {'wire_type': 'string',
                                   'enum_source': 'spec/approval.yaml#policy_decision.values'},
                'ApprovalAssurance': {'wire_type': 'string', 'enum_source': 'spec/approval.yaml#assurance.values'},
                'ApprovalBindingValidity': {'wire_type': 'string',
                                            'enum_source': 'spec/approval.yaml#binding_validity.values'},
                'ReconciliationResult': {'wire_type': 'string',
                                         'enum_source': 'spec/execution.yaml#reconciliation.values'},
                'FreshnessResult': {'wire_type': 'string', 'enum_source': 'spec/execution.yaml#freshness.values'},
                'FreshnessEnforcement': {'wire_type': 'string',
                                         'enum_source': 'spec/execution.yaml#freshness.enforcement'},
                'RecipientBoundary': {'wire_type': 'string',
                                      'enum_source': 'spec/approval.yaml#recipient_boundary.values'}},
 'value_objects': {'CostEstimate': {'fields': {'amount_micros': {'type': 'AmountMicros',
                                                                 'required': True,
                                                                 'nullable': False},
                                               'currency': {'type': 'CurrencyCode',
                                                            'required': True,
                                                            'nullable': False}},
                                    'amount_unit': 'millionths_of_major_currency_unit',
                                    'floating_point_allowed': False,
                                    'informational_only': True,
                                    'authorizes_execution': False,
                                    'lowers_risk': False},
                   'SourcePrecondition': {'fields': {'external_id': {'type': 'OpaqueRef',
                                                                     'required': True,
                                                                     'nullable': False},
                                                     'source_version_token': {'type': 'OpaqueRef',
                                                                              'required': True,
                                                                              'nullable': False},
                                                     'normalized_payload_hash': {'type': 'Sha256Hex',
                                                                                 'required': True,
                                                                                 'nullable': False}},
                                          'semantics': 'freshness_condition_for_mutable_external_source',
                                          'connector_context': 'Action.connector_profile_id',
                                          'mismatch_policy_ref': 'spec/sync.yaml#pre_side_effect_freshness',
                                          'connector_specific_semantics': False},
                   'OutcomeCertainty': {'wire_type': 'string',
                                        'values': ['not_applicable', 'known', 'unknown'],
                                        'state_source': 'spec/states.yaml#machines.RunState.states',
                                        'state_coupling': {'created': 'not_applicable',
                                                           'starting': 'not_applicable',
                                                           'running': 'not_applicable',
                                                           'outcome_unknown': 'unknown',
                                                           'reconciling': 'unknown',
                                                           'retryable_error': 'known',
                                                           'completed': 'known',
                                                           'failed': 'known',
                                                           'canceled': 'known'},
                                        'reconciliation_resolution': 'unknown_to_known_only_when_outcome_resolved_enough_to_leave_reconciliation'},
                   'RiskSummary': {'wire_type': 'array',
                                   'items': 'RiskClass',
                                   'unique_items': True,
                                   'ordering': 'declaration_order',
                                   'ordering_source': 'spec/approval.yaml#risk_classes',
                                   'informational_only': True,
                                   'replaces_per_action_risk_or_gate': False},
                   'CanonicalizerRef': {'fields': {'id': {'type': 'NonEmptyString',
                                                          'required': True,
                                                          'nullable': False},
                                                   'version': {'type': 'Version',
                                                               'required': True,
                                                               'nullable': False}},
                                        'semantics': 'versioned_action_type_specific_canonicalizer'},
                   'ReconciliationEvidence': {'fields': {'run_id': {'type': 'RunId',
                                                                    'required': True,
                                                                    'nullable': False},
                                                         'action_id': {'type': 'ActionId',
                                                                       'required': True,
                                                                       'nullable': False},
                                                         'action_version': {'type': 'Version',
                                                                            'required': True,
                                                                            'nullable': False},
                                                         'action_hash': {'type': 'Sha256Hex',
                                                                         'required': True,
                                                                         'nullable': False},
                                                         'effect_identity': {'type': 'OpaqueRef',
                                                                             'required': True,
                                                                             'nullable': False},
                                                         'source_ref': {'type': 'OpaqueRef',
                                                                        'required': True,
                                                                        'nullable': False},
                                                         'result': {'type': 'ReconciliationResult',
                                                                    'required': True,
                                                                    'nullable': False}},
                                              'rules_ref': 'spec/execution.yaml#reconciliation'},
                   'FreshnessEvidence': {'fields': {'run_id': {'type': 'RunId',
                                                               'required': True,
                                                               'nullable': False},
                                                    'action_id': {'type': 'ActionId',
                                                                  'required': True,
                                                                  'nullable': False},
                                                    'action_version': {'type': 'Version',
                                                                       'required': True,
                                                                       'nullable': False},
                                                    'action_hash': {'type': 'Sha256Hex',
                                                                    'required': True,
                                                                    'nullable': False},
                                                    'enforcement': {'type': 'FreshnessEnforcement',
                                                                    'required': True,
                                                                    'nullable': False},
                                                    'precondition': {'type': 'SourcePrecondition',
                                                                     'required': True,
                                                                     'nullable': False},
                                                    'observed_source_version_token': {'type': 'OpaqueRef',
                                                                                      'required': False,
                                                                                      'nullable': True},
                                                    'observed_payload_hash': {'type': 'Sha256Hex',
                                                                              'required': False,
                                                                              'nullable': True},
                                                    'result': {'type': 'FreshnessResult',
                                                               'required': True,
                                                               'nullable': False}},
                                         'rules_ref': 'spec/execution.yaml#freshness'}},
 'execution_contracts': {'AgentTask': {'fields': {'id': {'type': 'AgentTaskId',
                                                         'required': True,
                                                         'nullable': False},
                                                  'workspace_id': {'type': 'WorkspaceId',
                                                                   'required': True,
                                                                   'nullable': False},
                                                  'request_text': {'type': 'NonEmptyString',
                                                                   'required': True,
                                                                   'nullable': False},
                                                  'origin_surface': {'type': 'SurfaceId',
                                                                     'required': True,
                                                                     'nullable': False},
                                                  'origin_ref': {'type': 'OpaqueRef',
                                                                 'required': False,
                                                                 'nullable': True},
                                                  'state': {'type': 'AgentTaskState',
                                                            'required': True,
                                                            'nullable': False},
                                                  'privacy_class': {'type': 'PrivacyClass',
                                                                    'required': True,
                                                                    'nullable': False},
                                                  'requested_by': {'type': 'ActorRef',
                                                                   'required': True,
                                                                   'nullable': False},
                                                  'created_at': {'type': 'Timestamp',
                                                                 'required': True,
                                                                 'nullable': False}}},
                         'ExecutionPlan': {'fields': {'id': {'type': 'ExecutionPlanId',
                                                             'required': True,
                                                             'nullable': False},
                                                      'task_id': {'type': 'AgentTaskId',
                                                                  'required': True,
                                                                  'nullable': False},
                                                      'version': {'type': 'Version',
                                                                  'required': True,
                                                                  'nullable': False},
                                                      'risk_summary': {'type': 'RiskSummary',
                                                                       'required': True,
                                                                       'nullable': False},
                                                      'estimated_cost': {'type': 'CostEstimate',
                                                                         'required': False,
                                                                         'nullable': True},
                                                      'approval_requirement': {'type': 'GateType',
                                                                               'required': True,
                                                                               'nullable': False},
                                                      'created_at': {'type': 'Timestamp',
                                                                     'required': True,
                                                                     'nullable': False}},
                                           'rules': {'estimated_cost_absent_or_null': 'no_reliable_monetary_estimate',
                                                     'zero_cost': 'valid_known_estimate',
                                                     'approval_requirement': 'strongest_worst_case_gate_under_evaluated_policy_context',
                                                     'approval_requirement_informational_only': True,
                                                     'each_action_independently_risk_resolved_and_gated': True,
                                                     'revision_identity': 'immutable_id_and_version_pair',
                                                     'policy_deny': 'separate_PolicyDecision_deny_never_encoded_as_GateType'}},
                         'Action': {'fields': {'id': {'type': 'ActionId', 'required': True, 'nullable': False},
                                               'plan_id': {'type': 'ExecutionPlanId',
                                                           'required': True,
                                                           'nullable': False},
                                               'ordinal': {'type': 'Ordinal', 'required': True, 'nullable': False},
                                               'action_type': {'type': 'ActionType',
                                                               'required': True,
                                                               'nullable': False},
                                               'connector_profile_id': {'type': 'ConnectorProfileId',
                                                                        'required': False,
                                                                        'nullable': True},
                                               'risk_class': {'type': 'RiskClass',
                                                              'required': True,
                                                              'nullable': False},
                                               'state': {'type': 'ActionState',
                                                         'required': True,
                                                         'nullable': False},
                                               'input_ref': {'type': 'OpaqueRef',
                                                             'required': True,
                                                             'nullable': False},
                                               'source_preconditions': {'type': 'list<SourcePrecondition>',
                                                                        'required': True,
                                                                        'nullable': False},
                                               'result_ref': {'type': 'OpaqueRef',
                                                              'required': False,
                                                              'nullable': True},
                                               'version': {'type': 'Version', 'required': True, 'nullable': False},
                                               'plan_version': {'type': 'Version',
                                                                'required': True,
                                                                'nullable': False},
                                               'input_hash': {'type': 'Sha256Hex',
                                                              'required': True,
                                                              'nullable': False},
                                               'input_canonicalizer': {'type': 'CanonicalizerRef',
                                                                       'required': True,
                                                                       'nullable': False},
                                               'connector_selection': {'type': 'ConnectorSelection',
                                                                       'required': True,
                                                                       'nullable': False},
                                               'connector_binding_hash': {'type': 'Sha256Hex',
                                                                          'required': False,
                                                                          'nullable': True},
                                               'tool_definition_fingerprint': {'type': 'Sha256Hex',
                                                                               'required': False,
                                                                               'nullable': True}},
                                    'rules': {'nonempty_source_preconditions_requires': 'connector_profile_id',
                                              'empty_source_preconditions_valid': True,
                                              'revision_contract_ref': 'spec/action_binding.yaml#revisions',
                                              'selection_contract_ref': 'spec/action_binding.yaml#connector_selection',
                                              'mcp_contract_ref': 'spec/action_binding.yaml#mcp',
                                              'duplicate_source_preconditions': 'reject_exact_tuples'}},
                         'Run': {'fields': {'id': {'type': 'RunId', 'required': True, 'nullable': False},
                                            'action_id': {'type': 'ActionId', 'required': True, 'nullable': False},
                                            'attempt': {'type': 'Attempt', 'required': True, 'nullable': False},
                                            'requested_connector_id': {'type': 'ConnectorProfileId',
                                                                       'required': False,
                                                                       'nullable': True},
                                            'actual_connector_id': {'type': 'ConnectorProfileId',
                                                                    'required': False,
                                                                    'nullable': True},
                                            'requested_model_id': {'type': 'ModelProfileId',
                                                                   'required': False,
                                                                   'nullable': True},
                                            'actual_model_id': {'type': 'ModelProfileId',
                                                                'required': False,
                                                                'nullable': True},
                                            'state': {'type': 'RunState', 'required': True, 'nullable': False},
                                            'outcome_certainty': {'type': 'OutcomeCertainty',
                                                                  'required': True,
                                                                  'nullable': False},
                                            'reconciliation_ref': {'type': 'OpaqueRef',
                                                                   'required': False,
                                                                   'nullable': True},
                                            'started_at': {'type': 'Timestamp',
                                                           'required': False,
                                                           'nullable': True},
                                            'ended_at': {'type': 'Timestamp', 'required': False, 'nullable': True},
                                            'action_version': {'type': 'Version',
                                                               'required': True,
                                                               'nullable': False},
                                            'action_hash': {'type': 'Sha256Hex',
                                                            'required': True,
                                                            'nullable': False},
                                            'approval_id': {'type': 'ApprovalId',
                                                            'required': True,
                                                            'nullable': False},
                                            'approval_version': {'type': 'Version',
                                                                 'required': True,
                                                                 'nullable': False}},
                                 'rules': {'executor_ids_independently_optional': True,
                                           'actual_identity_divergence_requires': 'explicit_visible_policy_compliant_routing_or_fallback_decision',
                                           'hidden_fallback_allowed': False,
                                           'execution_binding_ref': 'spec/execution.yaml#run',
                                           'attempt_scope_ref': 'spec/execution.yaml#attempts'}},
                         'Approval': {'fields': {'id': {'type': 'ApprovalId', 'required': True, 'nullable': False},
                                                 'task_id': {'type': 'AgentTaskId',
                                                             'required': True,
                                                             'nullable': False},
                                                 'action_id': {'type': 'ActionId',
                                                               'required': True,
                                                               'nullable': False},
                                                 'policy_id': {'type': 'PolicyId',
                                                               'required': True,
                                                               'nullable': False},
                                                 'state': {'type': 'ApprovalState',
                                                           'required': True,
                                                           'nullable': False},
                                                 'requested_at': {'type': 'Timestamp',
                                                                  'required': True,
                                                                  'nullable': False},
                                                 'decided_at': {'type': 'Timestamp',
                                                                'required': False,
                                                                'nullable': True},
                                                 'decided_by': {'type': 'ActorRef',
                                                                'required': False,
                                                                'nullable': True},
                                                 'decision_note': {'type': 'Utf8String',
                                                                   'required': False,
                                                                   'nullable': True},
                                                 'action_hash': {'type': 'Sha256Hex',
                                                                 'required': True,
                                                                 'nullable': False},
                                                 'version': {'type': 'Version',
                                                             'required': True,
                                                             'nullable': False},
                                                 'expires_at': {'type': 'Timestamp',
                                                                'required': False,
                                                                'nullable': True},
                                                 'origin_surface': {'type': 'SurfaceId',
                                                                    'required': True,
                                                                    'nullable': False},
                                                 'decided_surface': {'type': 'SurfaceId',
                                                                     'required': False,
                                                                     'nullable': True},
                                                 'action_version': {'type': 'Version',
                                                                    'required': True,
                                                                    'nullable': False},
                                                 'policy_snapshot_hash': {'type': 'Sha256Hex',
                                                                          'required': True,
                                                                          'nullable': False},
                                                 'policy_decision': {'type': 'PolicyDecision',
                                                                     'required': True,
                                                                     'nullable': False},
                                                 'required_assurance': {'type': 'ApprovalAssurance',
                                                                        'required': True,
                                                                        'nullable': False},
                                                 'achieved_assurance': {'type': 'ApprovalAssurance',
                                                                        'required': True,
                                                                        'nullable': False}},
                                      'rules': {'binding_ref': 'spec/approval.yaml#record',
                                                'authorization_ref': 'spec/approval.yaml#execution_authorization',
                                                'cas_ref': 'spec/approval.yaml#concurrency'}}}}
require_contract(domain, required_domain_contract, 'domain.yaml')
def resolve_source(ref):
    if not isinstance(ref, str) or '#' not in ref:
        raise ValueError('source must be spec/file.yaml#path')
    file, path = ref.split('#', 1)
    if not file.startswith('spec/'):
        raise ValueError('source must reference canonical spec')
    value = loaded[file[5:]]
    for key in path.split('.'):
        value = value[key]
    return value

try:
    primitives = domain.get('primitives', {})
    objects = domain.get('value_objects', {})
    contracts = {**domain.get('artifact_contracts', {}), **domain.get('outcome_contracts', {}), **domain.get('execution_contracts', {})}
    for name, primitive in primitives.items():
        for key in ('enum_source', 'transitions_source'):
            if key in primitive:
                source = resolve_source(primitive[key])
                expected_type = dict if key == 'transitions_source' or primitive.get('source_selection') == 'mapping_keys' else list
                if not isinstance(source, expected_type) or not source:
                    errors.append(f'domain.yaml:{name}: invalid {key} target')
        if 'enum_source' in primitive and ('values' in primitive or 'transitions' in primitive):
            errors.append(f'domain.yaml:{name}: must not duplicate sourced enum or transitions')
    known_types = set(primitives) | set(objects) | set(contracts)
    if 'ApprovalRequirement' in primitives or 'ApprovalRequirement' in objects:
        errors.append('domain.yaml: duplicate ApprovalRequirement enum is forbidden; use GateType')
    for name, contract in {**objects, **contracts}.items():
        if name in required_domain_contract['value_objects']:
            expected_fields = required_domain_contract['value_objects'][name].get('fields')
            if expected_fields is not None and set(contract.get('fields', {})) != set(expected_fields):
                errors.append(f'domain.yaml:{name}: value object fields must match required contract')
        for field, meta in contract.get('fields', {}).items():
            type_name = meta['type']
            if type_name.startswith('list<') and type_name.endswith('>'):
                type_name = type_name[5:-1]
            if type_name not in known_types:
                errors.append(f'domain.yaml:{name}.{field}: unresolved type {type_name}')
        if name in contracts and name in domain.get('entities', {}) and set(contract['fields']) != set(domain['entities'][name]):
            errors.append(f'domain.yaml:{name}: typed fields must match entity inventory')
    certainty = objects['OutcomeCertainty']
    if set(certainty['state_coupling']) != set(resolve_source(certainty['state_source'])):
        errors.append('domain.yaml: OutcomeCertainty coupling must cover exactly all RunState values')
    resolve_source(objects['RiskSummary']['ordering_source'])
    resolve_source(objects['SourcePrecondition']['mismatch_policy_ref'])
except (KeyError, TypeError, ValueError, AttributeError) as exc:
    errors.append(f'domain.yaml: malformed or unresolved typed contract: {exc}')

# MCP RFC 8785 declaration
mcp=loaded['mcp.yaml']; pin=mcp.get('tool_definition_pinning',{})
if pin.get('canonicalization_standard')!='RFC-8785-JCS': errors.append('mcp.yaml: canonicalization_standard must be RFC-8785-JCS')
if pin.get('fingerprint_algorithm')!='sha256(utf8(jcs_rfc8785(canonical_fields_object)))': errors.append('mcp.yaml: fingerprint_algorithm must use RFC-8785 JCS UTF-8 + SHA-256')
fields=pin.get('canonical_fingerprint_fields',[])
required={'server_identity','tool_name','description','inputSchema'}
if not required.issubset(set(fields)): errors.append('mcp.yaml: fingerprint fields missing required identity/schema fields')
if len(fields)!=len(set(fields)): errors.append('mcp.yaml: duplicate canonical_fingerprint_fields')
obj=pin.get('canonical_object_construction',{})
if obj.get('missing_optional_fields')!='encode_as_json_null': errors.append('mcp.yaml: missing optional fingerprint fields must be encoded as JSON null')

# Deployment/transport topology and state authority
dep=loaded['deployment.yaml']; transport=loaded.get('transport.yaml',{})
if dep['profiles']['personal_local'].get('state_authority')!='local_sqlite': errors.append('deployment.yaml: personal_local must declare local_sqlite authority')
if dep['profiles']['personal_local'].get('direct_public_inbound_to_workstation') is not False: errors.append('deployment.yaml: personal_local must not require public inbound workstation port')
if dep['profiles']['enterprise_user'].get('direct_cloud_access_to_desktop_sqlite') is not False: errors.append('deployment.yaml: cloud service must not open desktop SQLite')
if 'single_state_authority_per_workspace' not in transport.get('principles',[]): errors.append('transport.yaml: missing single state authority invariant')
if transport.get('topologies',{}).get('local_with_teams_relay',{}).get('public_inbound_to_workstation') is not False: errors.append('transport.yaml: local Teams relay must be outbound-only from workstation')

# Security presentation controls
sec=loaded['security.yaml']; rp=sec.get('approval_rendering_policy',{})
if not rp: errors.append('security.yaml: approval_rendering_policy required')
else:
    if rp.get('desktop',{}).get('block_data_uri') is not True: errors.append('security.yaml: desktop approval must block data URI')
    forbidden=set(rp.get('teams_adaptive_cards',{}).get('forbid_untrusted_elements',[]))
    if not {'Image','Media','BackgroundImage','Action.OpenUrl'}.issubset(forbidden): errors.append('security.yaml: Teams approval card must forbid untrusted remote-capable elements')

# Sync freshness/reconciliation
sync=loaded['sync.yaml']
require_contract(conc.get('material_plan_change'), 'apply_mutation_policy', 'approval.yaml:material_plan_change')
require_contract(sync.get('pre_side_effect_freshness'), {'applies_to': ['reply_or_forward_based_on_mutable_message',
                'send_existing_draft',
                'calendar_update_or_cancel',
                'side_effect_using_mutable_external_object'],
 'stored_tokens': ['connector_external_id', 'source_version_token', 'normalized_payload_hash'],
 'graph_mail_version_sources': ['changeKey', '@odata.etag_when_returned'],
 'rule': 'Immediately before material side effect, re-read/conditionally validate the source version. If the '
         'connector does not document a strong If-Match precondition for that operation, perform '
         'compare-before-execute and fail closed on mismatch.',
 'on_mismatch': ['block_execution',
                 'invalidate_stale_binding_per_approval_mutation_policy',
                 'build_material_revision_per_action_binding_contract'],
 'approval_mutation_policy_ref': 'spec/approval.yaml#concurrency.mutation_policy',
 'send_note': 'Microsoft Graph send-draft is POST and does not document If-Match on that operation; MAIA MUST NOT '
              'assume conditional-send support.',
 'build_order_ref': 'spec/action_binding.yaml#material_revision_build_order',
 'evidence_contract_ref': 'spec/execution.yaml#freshness'}, 'sync.yaml:pre_side_effect_freshness')
try:
    if resolve_source(sync['pre_side_effect_freshness']['approval_mutation_policy_ref']) != mut:
        errors.append('sync.yaml: mismatch handling must resolve to canonical approval mutation policy')
except (KeyError, TypeError, ValueError):
    errors.append('sync.yaml: unresolved approval mutation policy reference')
if 'pre_side_effect_freshness' not in sync: errors.append('sync.yaml: missing pre_side_effect_freshness')
if sync.get('side_effect_reconciliation',{}).get('run_state')!='outcome_unknown -> reconciling': errors.append('sync.yaml: ambiguous side effects must use outcome_unknown -> reconciling')

# Compliance tombstone safety
comp=loaded['compliance.yaml']; es=comp.get('third_party_personal_data',{}).get('erasure_strategy',{})
if 'MUST NOT be a deterministic hash' not in es.get('tombstone_id',''): errors.append('compliance.yaml: tombstone ID must explicitly forbid deterministic PII hashes')

# M0.1.1 architecture obligations. Production declarations still come only from YAML.
# An intentional contract evolution must update these fail-closed regression expectations.
m011_required = {'action_binding.yaml': {'schema_version': 1,
                         'scope': 'M0.1.1 canonical contract and reference conformance only; no production '
                                  'canonicalizers or resolvers.',
                         'revisions': {'action_identity': 'ActionId_is_logical_step',
                                       'revision_identity': ['action_id', 'action_version'],
                                       'plan_revision_identity': ['plan_id', 'plan_version'],
                                       'immutable_after': ['approval_binding', 'any_Run'],
                                       'immutable_fields_source': 'spec/action_binding.yaml#action_binding.fields',
                                       'mutable_exclusions': ['state', 'result_ref', 'input_ref'],
                                       'input_ref_change_cannot_change_input_bytes': True,
                                       'historical_revisions_reconstructible': True,
                                       'material_edit': 'next_version_exactly_once',
                                       'logical_identity_not_preservable': 'new_ActionId',
                                       'overflow': 'fail_closed_never_wrap_saturate_or_reuse'},
                         'canonicalizer': {'reference_type': 'CanonicalizerRef',
                                           'registry_key': ['action_type', 'id', 'version'],
                                           'material_identity_change': True,
                                           'material_failure': 'no_binding_no_execution',
                                           'failure_cases': ['missing_registration', 'canonicalization_failure'],
                                           'core_may_guess_normalization': False,
                                           'production_implementations': 'deferred'},
                         'canonical_input': {'name': 'CanonicalActionInput',
                                             'algorithm': 'sha256(utf8(jcs_rfc8785(canonical_action_input)))',
                                             'schema': 'action_type_specific_registered_contract',
                                             'number_domain': 'I-JSON_IEEE754_exact_semantics_required',
                                             'unsafe_integer_input': 'reject_unless_action_schema_defines_lossless_string_projection',
                                             'completeness_when_applicable': ['resolved_target_account_tenant_resource_identity',
                                                                              'effective_recipients_or_participants',
                                                                              'subject_body_content_or_content_addressed_references',
                                                                              'attachment_identity_and_content_hashes',
                                                                              'material_operation_options',
                                                                              'mcp_tool_definition_fingerprint',
                                                                              'every_value_changing_externally_observable_effect'],
                                             'input_ref_is_hash': False,
                                             'execution_recomputes_input_hash': True,
                                             'mutable_locator_mismatch': 'stale_fail_closed'},
                         'connector_selection': {'values': ['none', 'fixed', 'policy_routed'],
                                                 'material_risks': ['write_internal',
                                                                    'send_internal',
                                                                    'send_external',
                                                                    'destructive',
                                                                    'privileged'],
                                                 'none': {'meaning': 'connectorless_not_late_selection',
                                                          'null_fields': ['connector_profile_id',
                                                                          'connector_binding_hash']},
                                                 'fixed': {'required_fields': ['connector_profile_id',
                                                                               'connector_binding_hash'],
                                                           'actual_connector_must_match': True,
                                                           'current_binding_hash_must_match': True,
                                                           'mismatch': 'stale_fail_closed_rebuild_regate'},
                                                 'policy_routed': {'allowed_risks': ['read', 'analyze', 'draft'],
                                                                   'required': ['explicit',
                                                                                'policy_compliant',
                                                                                'audited',
                                                                                'requested_actual_Run_identity'],
                                                                   'scope_broadening': 'reevaluate_policy_and_resulting_gate'},
                                                 'material_connector_execution_requires': 'fixed',
                                                 'binding_field_change_always_requires_new_revision': True},
                         'connector_binding': {'name': 'ConnectorBindingV1',
                                               'algorithm': 'sha256(utf8(jcs_rfc8785(connector_binding_v1)))',
                                               'schema_tag': 'maia.connector-binding.v1',
                                               'fields': {'schema': 'literal_schema_tag',
                                                          'connector_type': 'NonEmptyString',
                                                          'connector_profile_id': 'ConnectorProfileId',
                                                          'authority': 'connector_specific_canonical_account_tenant_identity_object',
                                                          'resource_scope': 'connector_specific_canonical_resource_mailbox_scope_object',
                                                          'execution_identity': 'connector_specific_canonical_non_secret_identity_object'},
                                               'projection_contract': 'registered_connector_specific_schema_defines_all_identity_scope_material',
                                               'secrets_allowed': False,
                                               'runtime_resolvers': 'deferred'},
                         'mcp': {'invocation_requires_fingerprint': True,
                                 'non_mcp_fingerprint': None,
                                 'fingerprint_source': 'spec/mcp.yaml#tool_definition_pinning',
                                 'operation_kind_source': 'trusted_action_type_contract_not_untrusted_payload',
                                 'visible_field': 'tool_definition_fingerprint',
                                 'input_and_binding_fingerprints_must_agree': True,
                                 'current_tool_fingerprint_must_match': True},
                         'action_binding': {'name': 'ActionBindingV1',
                                            'algorithm': 'sha256(utf8(jcs_rfc8785(action_binding_v1)))',
                                            'schema_tag': 'maia.action-binding.v1',
                                            'fields': ['schema',
                                                       'action_id',
                                                       'action_version',
                                                       'plan_id',
                                                       'plan_version',
                                                       'ordinal',
                                                       'action_type',
                                                       'input_hash',
                                                       'input_canonicalizer',
                                                       'connector_selection',
                                                       'connector_profile_id',
                                                       'connector_binding_hash',
                                                       'tool_definition_fingerprint',
                                                       'risk_class',
                                                       'source_preconditions'],
                                            'excluded_fields': ['state',
                                                                'result_ref',
                                                                'input_ref',
                                                                'approval_state',
                                                                'approval_version',
                                                                'timestamps',
                                                                'surface',
                                                                'run_data',
                                                                'policy_decision'],
                                            'decimal_string_paths': ['action_version',
                                                                     'plan_version',
                                                                     'input_canonicalizer.version'],
                                            'decimal_string_rule': 'ASCII_digits_no_sign_no_leading_zero_Version_range_1_to_u64_max',
                                            'integer_paths': ['ordinal'],
                                            'nullable_paths': ['connector_profile_id',
                                                               'connector_binding_hash',
                                                               'tool_definition_fingerprint'],
                                            'null_encoding': 'explicit_JSON_null_never_omission',
                                            'canonicalizer_fields': ['id', 'version'],
                                            'source_precondition_fields': ['external_id',
                                                                           'source_version_token',
                                                                           'normalized_payload_hash'],
                                            'source_sort_keys': ['external_id',
                                                                 'source_version_token',
                                                                 'normalized_payload_hash'],
                                            'source_sort_order': 'raw_UTF8_bytes_lexicographic',
                                            'exact_duplicate_source_tuples': 'reject',
                                            'unicode_normalization': 'none',
                                            'unknown_fields': 'reject'},
                         'material_revision_build_order': ['preserve_or_replace_logical_ActionId',
                                                           'choose_next_Action_version_once',
                                                           'bind_exact_ExecutionPlan_revision',
                                                           'canonicalize_final_input',
                                                           'compute_input_hash',
                                                           'resolve_connector_selection_and_binding',
                                                           'bind_mcp_fingerprint_if_applicable',
                                                           'resolve_final_RiskClass',
                                                           'construct_ActionBindingV1',
                                                           'compute_action_hash',
                                                           'evaluate_policy',
                                                           'create_new_Approval_binding_unless_deny',
                                                           'execution_may_become_eligible']},
 'execution.yaml': {'schema_version': 1,
                     'scope': 'Pure contracts; no scheduler, policy engine, persistence implementation, connectors or runtime '
                              'execution in M0.2.1.',
                    'run': {'exact_binding_fields': ['action_id',
                                                     'action_version',
                                                     'action_hash',
                                                     'approval_id',
                                                     'approval_version'],
                            'result_affects': 'exact_action_revision_only',
                            'newer_revision_may_receive_historical_result': False,
                            'authorization_record_versions_reconstructible': True,
                            'eligibility_ref': 'spec/approval.yaml#execution_authorization',
                            'evidence_validation': 'match_run_id_action_id_action_version_action_hash'},
                    'attempts': {'scope': ['action_id', 'action_version'],
                                 'first': 1,
                                 'retry': 'new_RunId_previous_attempt_plus_one',
                                 'new_revision': 'reset_to_1',
                                 'new_action': 'reset_to_1',
                                 'initial_state': 'created',
                                 'overflow': 'fail_closed_no_new_Run',
                                 'prior_retryable_result_mutable': False,
                                 'retry_preconditions': ['current_policy_permits',
                                                         'current_execution_checks',
                                                         'prior_effect_resolved']},
                    'reconciliation': {'values': ['effect_confirmed', 'effect_not_executed', 'unresolved'],
                                       'start_edge': 'outcome_unknown_to_reconciling',
                                       'effect_confirmed': 'completed_no_retry_of_same_effect',
                                       'effect_not_executed': 'authoritative_non_execution_proof_before_considering_new_Run',
                                       'effect_not_executed_run_state': 'retryable_error',
                                       'unresolved': 'remain_reconciling_block_retry',
                                       'required_evidence': ['run_id',
                                                             'action_id',
                                                             'action_version',
                                                             'action_hash',
                                                             'effect_identity',
                                                             'source_ref',
                                                             'result'],
                                       'non_authoritative_search_absence_is_proof': False,
                                       'authoritative_proof_source': 'connector_specific_canonical_reconciliation_contract',
                                       'unresolved_blocks_semantically_equivalent_effect_across_ids_and_revisions': True,
                                       'connector_implementations': 'deferred'},
                    'freshness': {'values': ['matched', 'mismatched', 'unverifiable'],
                                  'enforcement': ['strong_precondition', 'compare_before_execute'],
                                  'required_evidence': ['run_id',
                                                        'action_id',
                                                        'action_version',
                                                        'action_hash',
                                                        'enforcement',
                                                        'precondition',
                                                        'observed_source_version_token',
                                                        'observed_payload_hash',
                                                        'result'],
                                  'matched': 'observed_version_and_hash_equal_bound_precondition',
                                  'mismatched': 'block_execution_rebuild_revision_binding_risk_policy_gate',
                                  'unverifiable_material': 'fail_closed',
                                  'missing_observation': 'unverifiable_not_matched',
                                  'strong_precondition': 'connector_atomically_binds_execution_to_source_version',
                                  'compare_before_execute': 'immediately_before_effect_does_not_eliminate_TOCTOU',
                                  'evidence_scope': 'single_RunId',
                                  'reuse_across_runs': False,
                                  'arbitrary_ttl_seconds': None,
                                  'applies_to_ref': 'spec/sync.yaml#pre_side_effect_freshness.applies_to'},
                    'pause': {'states_source': 'spec/states.yaml#pause_semantics.in_flight_states',
                              'scope': 'all_task_runs_including_superseded_plan_action_revisions',
                              'forbidden_after_request': ['create_run', 'schedule_run', 'created_to_starting'],
                              'paused_requires': 'zero_in_flight_runs',
                              'supersession_hides_in_flight': False,
                              'cancellability_source': 'explicit_operation_contract',
                              'non_cancellable_runs': 'drain_or_reconcile'}},
 'approval.yaml': {'policy_decision': {'values': ['allow', 'confirm', 'elevated_confirm', 'deny'],
                                       'restriction_order': ['allow', 'confirm', 'elevated_confirm', 'deny'],
                                       'composition': 'maximum_applicable_restriction',
                                       'deny_absorbing': True,
                                       'layers': ['tenant', 'workspace', 'user'],
                                       'lower_layer_may_weaken': False,
                                       'default_gate_mapping': {'allow': 'allow',
                                                                'confirm': 'confirm',
                                                                'elevated_confirm': 'elevated_confirm',
                                                                'policy': 'evaluate_policies_and_grants'},
                                       'policy_gate_without_explicit_autonomous_permission': 'confirm',
                                       'grant': {'requires_explicit_tenant_workspace_scope_permission': True,
                                                 'autonomous_reduction_only_for_gate': 'policy',
                                                 'cannot_auto_allow_gates': ['confirm', 'elevated_confirm'],
                                                 'relationship_authority': False},
                                       'snapshot': {'hash_field': 'Approval.policy_snapshot_hash',
                                                    'semantics': 'sha256_of_canonical_effective_policy_snapshot',
                                                    'administration_and_snapshot_materialization': 'deferred',
                                                    'reevaluate_before_material_execution': True},
                                       'decision_to_record': {'allow': 'not_required',
                                                              'confirm': 'pending',
                                                              'elevated_confirm': 'pending',
                                                              'deny': 'no_record_policy_evaluation_audited'}},
                   'assurance': {'values': ['none', 'confirm', 'elevated_confirm'],
                                 'order': ['none', 'confirm', 'elevated_confirm'],
                                 'required_by_decision': {'allow': 'none',
                                                          'confirm': 'confirm',
                                                          'elevated_confirm': 'elevated_confirm'},
                                 'technology_specific_evidence': 'future_authentication_and_audit_records'},
                   'record': {'new_id_per_binding': True,
                              'initial_version': 1,
                              'version_scope': 'Approval_record_CAS_not_Action_revision',
                              'immutable_fields': ['action_id',
                                                   'action_version',
                                                   'action_hash',
                                                   'policy_id',
                                                   'policy_snapshot_hash',
                                                   'origin_surface',
                                                   'required_assurance'],
                              'eligible_revision_requires_record': True,
                              'not_required': {'human_decision': False,
                                               'null_fields': ['decided_by', 'decided_at', 'decided_surface'],
                                               'achieved_assurance': 'none'},
                              'overflow': 'fail_closed_never_wrap_saturate_or_reuse',
                              'policy_decision': 'snapshot_of_binding_evaluation_not_current_policy',
                              'human_approved_requires': ['decided_by', 'decided_at', 'decided_surface'],
                              'human_approved_minimum_assurance': 'confirm',
                              'mutable_metadata': 'only_lifecycle_and_decision_metadata_per_legal_transition'},
                   'binding_validity': {'values': ['valid',
                                                   'stale_action',
                                                   'expired',
                                                   'policy_denied',
                                                   'actor_unauthorized',
                                                   'insufficient_assurance'],
                                        'valid_is_not_execution_authorization': True,
                                        'expiry': 'now_greater_than_or_equal_to_expires_at',
                                        'null_expiry': 'no_time_expiry',
                                        'approved_expiry': 'preserve_approved_history_block_execution',
                                        'pending_expiry': 'legal_pending_to_expired_increment_record_version',
                                        'actor_context': 'currently_authorized_actor_for_the_operation_being_checked',
                                        'material_mutation_policy_ref': 'spec/approval.yaml#concurrency.mutation_policy',
                                        'multiple_failures': 'all_failures_block_execution_no_priority_inferred'},
                   'execution_authorization': {'all_require': ['valid_binding',
                                                               'current_action_revision_and_hash',
                                                               'freshness_checks',
                                                               'routing_checks',
                                                               'tool_definition_checks'],
                                               'states_never_authorize': ['pending',
                                                                          'rejected',
                                                                          'revoked',
                                                                          'expired'],
                                               'matrix': {'allow': {'states': ['not_required', 'approved'],
                                                                    'minimum_assurance': 'none'},
                                                          'confirm': {'states': ['approved'],
                                                                      'minimum_assurance': 'confirm'},
                                                          'elevated_confirm': {'states': ['approved'],
                                                                               'minimum_assurance': 'elevated_confirm'},
                                                          'deny': {'states': [], 'minimum_assurance': None}},
                                               'uses_current_policy': True,
                                               'prior_approval_overrides_deny': False},
                   'recipient_boundary': {'values': ['internal', 'external', 'unknown'],
                                          'trusted_input_required': True,
                                          'untrusted_payload_can_self_declare_internal': False,
                                          'precedence': ['external', 'unknown', 'internal'],
                                          'risk_by_boundary': {'internal': 'send_internal',
                                                               'external': 'send_external',
                                                               'unknown': 'send_external'},
                                          'unresolved_group_membership': 'unknown_unless_trusted_resolver_proves_boundary'},
                   'dynamic_risk_classifiers': {'mail_recipient_boundary': {'inputs': ['to',
                                                                                       'cc',
                                                                                       'bcc',
                                                                                       'effective_recipient_boundaries',
                                                                                       'operation_risk_floor'],
                                                                            'outputs': ['send_internal',
                                                                                        'send_external',
                                                                                        'destructive',
                                                                                        'privileged'],
                                                                            'rule': 'Reject empty effective '
                                                                                    'recipients; external or '
                                                                                    'unknown => send_external, '
                                                                                    'all internal => '
                                                                                    'send_internal; preserve '
                                                                                    'explicit '
                                                                                    'destructive/privileged '
                                                                                    'operation floor.',
                                                                            'trusted_input_source': 'trusted_effective_recipient_boundary_resolver',
                                                                            'unknown_behavior': 'send_external_policy_may_deny',
                                                                            'operation_risk_floor': {'source': 'explicit_operation_contract',
                                                                                                     'preserve': ['destructive',
                                                                                                                  'privileged'],
                                                                                                     'global_risk_order_inferred': False},
                                                                            'boundary_outputs': ['send_internal',
                                                                                                 'send_external'],
                                                                            'empty_recipients': 'ValidationError_fail_closed'},
                                                'calendar_participant_boundary': {'inputs': ['organizer',
                                                                                             'attendees',
                                                                                             'effective_attendee_boundaries',
                                                                                             'operation_kind',
                                                                                             'private_event',
                                                                                             'operation_risk_floor'],
                                                                                  'outputs': ['write_internal',
                                                                                              'send_internal',
                                                                                              'send_external',
                                                                                              'destructive',
                                                                                              'privileged'],
                                                                                  'rule': 'Private no-attendee '
                                                                                          'event => '
                                                                                          'write_internal; '
                                                                                          'attendee '
                                                                                          'external/unknown => '
                                                                                          'send_external, all '
                                                                                          'internal => '
                                                                                          'send_internal; '
                                                                                          'preserve explicit '
                                                                                          'destructive/privileged '
                                                                                          'operation floor.',
                                                                                  'trusted_input_source': 'trusted_effective_recipient_boundary_resolver',
                                                                                  'unknown_behavior': 'send_external_policy_may_deny',
                                                                                  'operation_risk_floor': {'source': 'explicit_operation_contract',
                                                                                                           'preserve': ['destructive',
                                                                                                                        'privileged'],
                                                                                                           'global_risk_order_inferred': False},
                                                                                  'boundary_outputs': ['write_internal',
                                                                                                       'send_internal',
                                                                                                       'send_external'],
                                                                                  'incomplete_inputs': 'ValidationError_fail_closed_except_unknown_boundary_which_is_send_external'}},
                   'concurrency': {'decision_model': 'compare_and_swap',
                                   'required_match': ['approval_id',
                                                      'expected_approval_version',
                                                      'required_current_state',
                                                      'action_id',
                                                      'action_version',
                                                      'action_hash',
                                                      'current_executable_action_revision_and_hash'],
                                   'first_terminal_decision_wins': True,
                                   'stale_or_second_decision_error': 'ExternalConflict',
                                   'material_plan_change': 'apply_mutation_policy',
                                   'mutation_policy': {'on_payload_divergence': 'invalidate_binding_and_supersede_state_aware',
                                                       'old_binding_valid_for_new_payload_version': False,
                                                       'revocable_prior_states': ['pending', 'approved'],
                                                       'revocable_handling': {'transition_to': 'revoked',
                                                                              'reason': 'action_hash_divergence'},
                                                       'preserved_prior_states': ['not_required',
                                                                                  'rejected',
                                                                                  'expired',
                                                                                  'revoked'],
                                                       'preserved_handling': 'preserve_historical_state_without_transition',
                                                       'new_version_required': True,
                                                       'new_version_steps': ['build_material_revision_per_action_binding_contract',
                                                                             'create_new_approval_id_at_record_version_1_unless_policy_denies'],
                                                       'new_approval_state_source': 'spec/approval.yaml#policy_decision.decision_to_record',
                                                       'not_required_is_valid_gate_result': True,
                                                       'build_order_ref': 'spec/action_binding.yaml#material_revision_build_order'},
                                   'human_decision_state': 'pending',
                                   'human_decision_destinations': ['approved', 'rejected'],
                                   'additional_human_decision_checks': ['now_before_expires_at_if_present',
                                                                        'actor_currently_authorized',
                                                                        'assurance_satisfies_confirmation_requirement_for_approval'],
                                   'successful_mutation': {'increment_record_version': 1,
                                                           'immutable_binding_fields_preserved': True,
                                                           'metadata_matches_transition': True,
                                                           'overflow': 'fail_closed'},
                                    'atomicity': 'spec/persistence.yaml#atomic_transactions.T2_approval_decision_cas',
                                   'lifecycle_mutations': 'expiry_and_material_revocation_are_system_lifecycle_transitions_not_human_approve_reject_decisions'},
                   'gate_to_approval_state': {'allow': {'result': 'not_required'},
                                              'policy': {'if_policy_allows_without_confirmation': 'not_required',
                                                         'otherwise': 'pending',
                                                         'if_policy_denies': 'no_approval_record_execution_blocked'},
                                              'confirm': {'result': 'pending'},
                                              'elevated_confirm': {'result': 'pending'}}},
 'states.yaml': {'machines': {'RunState': {'states': ['created',
                                                      'starting',
                                                      'running',
                                                      'retryable_error',
                                                      'outcome_unknown',
                                                      'reconciling',
                                                      'completed',
                                                      'failed',
                                                      'canceled'],
                                           'transitions': {'created': ['starting', 'canceled'],
                                                           'starting': ['running',
                                                                        'retryable_error',
                                                                        'failed',
                                                                        'canceled'],
                                                           'running': ['retryable_error',
                                                                       'outcome_unknown',
                                                                       'completed',
                                                                       'failed',
                                                                       'canceled'],
                                                           'outcome_unknown': ['reconciling'],
                                                           'reconciling': ['completed',
                                                                           'retryable_error',
                                                                           'failed']},
                                           'semantics': 'one execution attempt; retryable_error is terminal; '
                                                        'retry creates a new RunId; unresolved outcomes require '
                                                        'reconciliation'}},
                 'pause_semantics': {'model': 'quiescent_non_preemptive',
                                     'task_transition': 'running -> pausing -> paused',
                                     'on_pause_request': ['stop_scheduling_new_actions_or_runs',
                                                          'request_cancel_only_for_in_flight_runs_whose_contract_is_cancellable',
                                                          'allow_non_cancellable_in_flight_runs_to_reach_terminal_or_outcome_unknown',
                                                          'enter_paused_only_when_no_in_flight_run_remains'],
                                     'action_state_paused_is_intentionally_absent': True,
                                     'rationale': 'A generic Action pause would falsely promise preemption for '
                                                  'non-cancellable side effects such as mail.send.',
                                     'in_flight_states': ['starting', 'running', 'outcome_unknown', 'reconciling'],
                                     'run_scope': 'all_task_runs_including_superseded_plan_action_revisions',
                                     'after_request_forbidden': ['create_run',
                                                                 'schedule_run',
                                                                 'created_to_starting']}}}
for filename, requirement in m011_required.items():
    require_contract(loaded.get(filename), requirement, filename)

# M0.1.2 corrective obligations.
m012_required = {'states.yaml': {'machines': {'RunState': {'terminal_states': ['retryable_error',
                                                               'completed',
                                                               'failed',
                                                               'canceled']}}},
 'approval.yaml': {'record': {'snapshot_state_compatibility': {'allow': ['not_required'],
                                                               'confirm': ['pending',
                                                                           'approved',
                                                                           'rejected',
                                                                           'expired',
                                                                           'revoked'],
                                                               'elevated_confirm': ['pending',
                                                                                    'approved',
                                                                                    'rejected',
                                                                                    'expired',
                                                                                    'revoked'],
                                                               'deny': []},
                              'pending': {'null_fields': ['decided_by',
                                                          'decided_at',
                                                          'decided_surface',
                                                          'decision_note'],
                                          'achieved_assurance': 'none'},
                              'human_rejected_requires': ['decided_by',
                                                          'decided_at',
                                                          'decided_surface'],
                              'human_rejected_minimum_assurance': 'confirm',
                              'rejection_requires_approval_level_assurance': False,
                              'rejected_is_human_decision_not_policy_deny': True},
                   'concurrency': {'decision_assurance_ref': 'spec/approval.yaml#record',
                                   'decision_timestamp': 'authoritative_now_not_request_timestamp'},
                   'recipient_boundary': {'input_validation_order': ['validate_complete_input',
                                                                     'resolve_boundary_risk',
                                                                     'preserve_operation_floor'],
                                          'invalid_input_with_floor': 'ValidationError_fail_closed'}}}
for filename, requirement in m012_required.items():
    require_contract(loaded.get(filename), requirement, filename)

# M0.2.1 authoritative persistence and audit contract closure. Historical
# 1.4.0 evidence lives in its changelog/ADR; the live product version may
# evolve in a later canonical milestone.
require_contract(loaded.get('domain.yaml', {}).get('execution_contracts', {}).get('AgentTask', {}).get('fields', {}).get('version'),
                 {'type': 'Version', 'required': True, 'nullable': False},
                 'domain.yaml:execution_contracts.AgentTask.fields.version')
require_contract(loaded.get('domain.yaml', {}).get('execution_contracts', {}).get('Run', {}).get('fields', {}).get('version'),
                 {'type': 'Version', 'required': True, 'nullable': False},
                 'domain.yaml:execution_contracts.Run.fields.version')
require_contract(loaded.get('domain.yaml', {}).get('execution_contracts', {}).get('AgentTask', {}).get('rules'),
                 {'version_semantics': 'mutable_record_cas_version',
                  'initial_version': 1,
                  'successful_mutation': 'expected_version_and_state_then_increment',
                  'overflow': 'fail_closed'},
                 'domain.yaml:execution_contracts.AgentTask.rules')
require_contract(loaded.get('domain.yaml', {}).get('execution_contracts', {}).get('Run', {}).get('rules'),
                 {'version_semantics': 'mutable_record_cas_version_not_attempt_or_action_revision',
                  'initial_version': 1,
                  'successful_mutation': 'expected_version_and_state_then_increment',
                  'overflow': 'fail_closed'},
                 'domain.yaml:execution_contracts.Run.rules')
require_contract(loaded.get('persistence.yaml'), {
    'authoritative_store': {
        'exactly_one_mutable_authority_per_workspace': True,
        'authority_changing_write_transactions_serialized': True,
        'local_authoritative_profile': {
            'one_writable_authority_process_per_workspace': True,
            'exclusive_os_level_file_lock_during_writable_authority': True,
            'lock_unavailable': 'read_only_or_fail_closed',
            'backup_is_never_parallel_writable_authority': True,
        },
    },
    'history': {
        'plan_action_approval_run_hard_delete_runtime_api': 'forbidden',
        'supersession_preserves_history': True,
    },
    'atomic_transactions': {
        'authority_boundary': 'serialized_authority_changing_write',
        'T1_material_action_revision': {'run_creation_included': False},
        'T3_run_creation': {'action_approval_creation_same_transaction_required': False},
    },
    'migrations': {
        'ledger_fields': ['migration_id', 'checksum', 'applied_at'],
        'monotonic_sequence': True,
        'applied_checksum_immutable': True,
        'backup_before_write': True,
        'newer_schema_than_binary': 'fail_closed',
        'checksum_mismatch': 'fail_closed',
        'silent_repair': 'forbidden',
    },
}, 'persistence.yaml:M0.2.1')
require_contract(loaded.get('audit.yaml'), {
    'record': {
        'append_only': True,
        'no_hard_delete_in_runtime': True,
        'identity': ['workspace_id', 'sequence'],
    },
    'hash': {
        'algorithm': 'SHA-256',
        'canonicalization': 'RFC8785-JCS',
        'excluded_fields': ['record_hash'],
        'prev_hash_in_projection': True,
        'record_hash_must_match_projection': True,
    },
    'chain': {
        'scope': 'per_workspace',
        'sequence': {
            'starts_at': 1,
            'allocated_atomically_by_authoritative_store': True,
            'monotonically_increases': True,
            'overflow': 'fail_closed',
        },
        'genesis': {
            'sequence': 1,
            'prev_hash': '0000000000000000000000000000000000000000000000000000000000000000',
        },
        'append': 'immutable',
    },
}, 'audit.yaml:M0.2.1')

# M0.6 bounded local immutable workspace evidence snapshot contract. Product
# version is intentionally not pinned here: later additive milestones preserve
# M0.6 semantics without retaining a historical product version forever.
require_contract(loaded.get('domain.yaml'), {
    'artifact_snapshot': {
        'artifact_kinds': ['imported_text_evidence'],
        'media_types': ['utf8_plain_text', 'markdown'],
        'content_representation': {
            'decoding': 'strict_UTF-8',
            'newline_normalization': 'forbidden',
            'Unicode_normalization': 'forbidden',
            'content_sha256': 'SHA-256_over_exact_imported_source_bytes',
            'byte_length': 'exact_imported_source_byte_count',
            'snapshot_content': 'MAIA-owned_immutable_UTF-8_text',
        },
        'knowledge_boundary': {
            'creates_memory_claim': False,
            'creates_workgraph_edges': False,
            'infers_semantics': False,
        },
    },
    'artifact_contracts': {
        'Artifact': {'rules': {
            'immutable_after_write': True,
            'content_is_MAIA_owned_snapshot': True,
            'source_locator_is_private_provenance': True,
            'source_modification_or_deletion_changes_snapshot': False,
            'semantic_extraction': 'forbidden',
        }},
        'EvidenceCitation': {'rules': {
            'external_source_locator_is_citation_identity': False,
            'line_positions_reference': 'immutable_MAIA_snapshot',
        }},
        'EvidenceImportRequest': {'rules': {
            'input_paths_are_host_boundary': True,
            'partial_success': 'forbidden',
        }},
    },
}, 'domain.yaml:M0.6')
require_contract(loaded.get('persistence.yaml'), {
    'atomic_transactions': {'T6_workspace_evidence_import': {
        'atomic_all_or_nothing': True,
        'partial_artifacts_on_failure': 'forbidden',
    }},
    'workspace_evidence_import': {
        'supported_media_types': ['utf8_plain_text', 'markdown'],
        'bounds': {
            'maximum_files_per_request': 8,
            'maximum_file_bytes': 262144,
            'maximum_total_bytes': 1048576,
        },
        'input_boundary': {
            'caller_supplies_each_path_explicitly': True,
            'local_regular_files_only': True,
            'UNC_or_network_paths': 'forbidden',
            'directory_input': 'forbidden',
            'recursive_discovery': 'forbidden',
            'symlink_junction_reparse_points': 'forbidden',
            'unsupported_encoding': 'fail_closed',
            'source_change_during_acquisition': 'fail_closed',
        },
        'snapshot_read': {
            'reads_MAIA_owned_content_not_external_source': True,
            'external_source_change_or_deletion_affects_existing_snapshot': False,
        },
    },
}, 'persistence.yaml:M0.6')
require_contract(loaded.get('audit.yaml'), {
    'evidence_snapshot_audit': {
        'event_types': ['workspace_evidence_imported', 'workspace_evidence_import_idempotent', 'workspace_evidence_import_refused'],
        'success_and_idempotent_subject_type': 'artifact',
        'operation_ref': 'EvidenceImportRequestId',
        'provenance_reference': 'source_locator_hash_only',
        'content_or_raw_locator_in_audit': 'forbidden',
    },
}, 'audit.yaml:M0.6')

# M0.7 execution intelligence contract. This is additive to the closed
# Task/Plan/Action/Run/Approval and M0.6 artifact contracts above. Product
# version is intentionally not pinned: later additive milestones preserve M0.7.
require_contract(loaded.get('product.yaml'), {
    'validation_hypotheses': {'initial_wedge': {
        'id': 'microsoft_365_execution_control',
        'replaceable': True,
        'does_not_define_core': True,
    }},
}, 'product.yaml:M0.7')
reasoning = loaded.get('reasoning.yaml', {})
levels = reasoning.get('assurance_levels', {})
expected_levels = {'A0': 'deterministic', 'A1': 'single_model', 'A2': 'verified_reasoning',
                   'A3': 'round_table', 'A4': 'human_decision_required'}
if set(levels) != set(expected_levels):
    errors.append('reasoning.yaml:M0.7 assurance levels must be exactly A0-A4')
else:
    for level, symbol in expected_levels.items():
        if levels[level].get('symbol') != symbol:
            errors.append(f'reasoning.yaml:M0.7 {level} must have symbol {symbol}')
if not {'assurance_level_never_grants_execution_permission', 'model_consensus_never_bypasses_approval_gate',
        'round_table_is_not_default'}.issubset(set(reasoning.get('hard_rules', []))):
    errors.append('reasoning.yaml:M0.7 missing assurance authority invariants')
round_table = reasoning.get('round_table', {})
if not {'A3', 'A4'}.issubset(set(round_table.get('associated_levels', []))):
    errors.append('reasoning.yaml:M0.7 Round Table must be associated with A3 and A4')
if not {'independent_reasoning_contributions', 'disagreement_record', 'evidence_comparison',
        'adjudication_result', 'usage_and_cost_recording'}.issubset(set(round_table.get('required_properties', []))):
    errors.append('reasoning.yaml:M0.7 Round Table properties incomplete')
if not {'direct_execution_privilege', 'approval_gate_bypass'}.issubset(set(round_table.get('forbidden_shortcuts', []))):
    errors.append('reasoning.yaml:M0.7 Round Table must not authorize execution or bypass ApprovalGate')
if reasoning.get('a4_human_confirmation', {}).get('substitutes_for_approval_gate') is not False:
    errors.append('reasoning.yaml:M0.7 A4 must not substitute for ApprovalGate')

outcomes = loaded.get('outcomes.yaml', {})
outcome_lifecycle = outcomes.get('lifecycle', {})
if outcome_lifecycle.get('statuses') != ['candidate', 'observed', 'verified', 'rejected', 'disputed', 'superseded']:
    errors.append('outcomes.yaml:M0.7 lifecycle statuses must be the approved ordered set')
outcome_rules = outcomes.get('rules', {})
if not (outcome_rules.get('observed_requires_source_or_evidence') is True and
        outcome_rules.get('verified_requires_verification_method') is True and
        outcome_rules.get('verified_requires_sufficient_evidence') is True and
        outcome_rules.get('model_confidence_alone_promotes_to_verified') is False and
        outcome_rules.get('business_outcome_is_not_run_outcome_certainty') is True):
    errors.append('outcomes.yaml:M0.7 Outcome evidence and Run separation invariants incomplete')
expected_entities = {'Tenant', 'Membership', 'Entitlement', 'Outcome', 'UsageRecord'}
if not expected_entities.issubset(set(domain.get('entities', {}))):
    errors.append('domain.yaml:M0.7 commercial and Outcome entity inventory incomplete')
outcome_contract = domain.get('outcome_contracts', {}).get('Outcome', {})
if outcome_contract.get('rules', {}).get('business_outcome_is_not_run_outcome_certainty') is not True:
    errors.append('domain.yaml:M0.7 Outcome must remain separate from Run OutcomeCertainty')
usage_contract = domain.get('outcome_contracts', {}).get('UsageRecord', {})
if usage_contract.get('rules', {}).get('unknown_cost_is_zero') is not False:
    errors.append('domain.yaml:M0.7 unknown UsageRecord cost must not be zero')

commercial = loaded.get('commercial.yaml', {})
scope = commercial.get('workspace_tenant_scope', {})
if scope.get('personal_local', {}).get('tenant_required') is not False:
    errors.append('commercial.yaml:M0.7 personal/local Workspace must not require Tenant')
organizational = scope.get('commercial_organizational', {})
if organizational.get('tenant_required') is not True or organizational.get('missing_tenant_behavior') != 'fail_closed':
    errors.append('commercial.yaml:M0.7 commercial organizational mode must require Tenant and fail closed')
if commercial.get('usage', {}).get('cost_rules', {}).get('unknown_forbids_implied_zero') is not True:
    errors.append('commercial.yaml:M0.7 unknown cost must not be treated as zero')
gates = commercial.get('commercial_readiness_gates', {})
if gates.get('G2', {}).get('name') != 'FORM COMPANY NOW' or gates.get('G2', {}).get('behavior') != 'stop_commercial_progression_and_alert_founder':
    errors.append('commercial.yaml:M0.7 G2 must be FORM COMPANY NOW founder alert')
if any('non_runtime' not in gates.get(gate, {}).get('kind', '') for gate in ('G3', 'G4')):
    errors.append('commercial.yaml:M0.7 G3/G4 must remain non-runtime gates')

metrics = loaded.get('metrics.yaml', {})
required_metrics = {'time_to_first_value', 'time_to_first_action', 'successful_outcome_rate',
                    'human_intervention_effort', 'cost_per_successful_outcome', 'repeatability_effort'}
if not required_metrics.issubset(set(metrics.get('metrics', {}))):
    errors.append('metrics.yaml:M0.7 required measured advantage metrics missing')
if metrics.get('metrics', {}).get('time_to_first_action', {}).get('approval_requirements_may_be_weakened_for_metric') is not False:
    errors.append('metrics.yaml:M0.7 TTFA must not weaken approval requirements')
if metrics.get('metrics', {}).get('cost_per_successful_outcome', {}).get('unknown_component_behavior') != 'preserve_unknown_never_assume_zero':
    errors.append('metrics.yaml:M0.7 unknown cost components must remain unknown')
benchmark = metrics.get('measured_advantage_rule', {}).get('benchmark_requirements', [])
if not {'named_baseline', 'case_set', 'verification_rule'}.issubset(set(benchmark)):
    errors.append('metrics.yaml:M0.7 Measured Advantage requires named benchmark baseline, cases and verification rule')
if metrics.get('privacy', {}).get('raw_customer_content_in_product_telemetry') != 'forbidden_by_default':
    errors.append('metrics.yaml:M0.7 raw customer telemetry content must be forbidden by default')

golden = set(loaded.get('testing.yaml', {}).get('golden_vectors', []))
required_golden = {'outcome_certainty_is_not_business_outcome', 'reasoning_assurance_cannot_lower_action_risk',
                   'round_table_cannot_authorize_execution', 'a4_human_reasoning_confirmation_cannot_substitute_for_approval_gate',
                   'unknown_cost_is_not_zero', 'unknown_human_intervention_is_not_zero', 'verified_outcome_requires_evidence',
                   'm365_wedge_does_not_become_core_boundary', 'personal_workspace_does_not_require_tenant',
                   'commercial_organizational_mode_requires_tenant', 'g2_precedes_paid_production_or_external_customer_production_data',
                   'g3_g4_are_not_task_or_action_fsm_states', 'diagnostic_or_noop_event_does_not_qualify_for_ttfa',
                   'generic_model_output_does_not_qualify_for_ttfv', 'm06_artifact_snapshot_contract_remains_unchanged'}
if not required_golden.issubset(golden):
    errors.append('testing.yaml:M0.7 required golden vectors missing')
acceptance_ids = {item['id'] for item in loaded.get('acceptance.yaml', {}).get('releases', {}).get('m0_7_execution_intelligence_contract', [])}
if acceptance_ids != {f'E{i:03d}' for i in range(1, 13)}:
    errors.append('acceptance.yaml:M0.7 must contain E001-E012 exactly once')

# M0.8 keeps the M0.7 authority boundary while defining a provider-neutral
# Round Table vertical slice. Credentials and live calls remain outside Core.
round_table_contract = loaded.get('round_table.yaml', {})
if loaded.get('product.yaml', {}).get('product', {}).get('version') != '1.9.0':
    errors.append('product.yaml:M0.10 product version must be 1.9.0')
required_participant_contracts = {'Participant', 'ParticipantId', 'ModelProvider', 'ModelRef', 'ParticipantRequest', 'ParticipantResponse', 'RoundTableSession', 'RoundTableDecision', 'Disagreement', 'EvidenceReference', 'UsageCostMetadata'}
participants_contract = round_table_contract.get('participants', {})
if not required_participant_contracts.issubset(set(participants_contract.get('required_contracts', []))):
    errors.append('round_table.yaml:M0.8 provider-neutral participant contract inventory incomplete')
if participants_contract.get('provider_neutral') is not True or participants_contract.get('model_configurable') is not True or participants_contract.get('hidden_provider_or_model_fallback') != 'forbidden':
    errors.append('round_table.yaml:M0.8 provider-neutrality or fallback boundary incomplete')
type_contracts = participants_contract.get('type_contracts', {})
if not required_participant_contracts.issubset(set(type_contracts)) or type_contracts.get('RoundTableDecision', {}).get('execution_authority_must_be_false') is not True:
    errors.append('round_table.yaml:M0.8 type or decision authority contract incomplete')
independence = round_table_contract.get('independence', {})
if independence.get('first_round_request_contains_other_participant_responses') is not False or independence.get('responses_persisted_or_retained_separately_before_adjudication') is not True or independence.get('adjudication_receives_independent_responses') is not True or independence.get('disagreement_explicit') is not True:
    errors.append('round_table.yaml:M0.8 first-round independence contract incomplete')
assurance = round_table_contract.get('assurance', {})
if not {'A3', 'A4'}.issubset(set(assurance.get('allowed_levels', []))) or assurance.get('a3_or_a4_increases_action_execution_authority') is not False or assurance.get('approval_gate_bypass') != 'forbidden':
    errors.append('round_table.yaml:M0.8 A3/A4 ApprovalGate boundary incomplete')
anthropic = round_table_contract.get('anthropic_adapter', {})
expected_anthropic = {'provider': 'anthropic', 'boundary': 'infra_adapter_not_core', 'api': 'messages', 'model_source': 'local_configuration', 'credential_source': 'local_secret_or_environment_boundary_only', 'organization_id_required': False, 'request_timeout_required': True, 'retries': 'safe_transient_failures_only', 'request_id_capture': True, 'usage_capture_when_returned': True, 'other_provider_fallback': 'forbidden'}
require_contract({'anthropic_adapter': anthropic}, {'anthropic_adapter': expected_anthropic}, 'round_table.yaml:M0.8')
live_smoke = round_table_contract.get('live_smoke', {})
if live_smoke.get('default') != 'skipped' or live_smoke.get('explicit_opt_in_environment') != 'M0_8_LIVE_ANTHROPIC=1' or live_smoke.get('credential_environment') != 'ANTHROPIC_API_KEY' or live_smoke.get('sensitive_prompt_or_credential_logging') != 'forbidden':
    errors.append('round_table.yaml:M0.8 live smoke must be explicit opt-in and credential-safe')
m08_golden = {'round_table_first_round_is_response_isolated', 'round_table_decision_never_authorizes_action', 'anthropic_adapter_has_no_hidden_provider_fallback', 'anthropic_live_smoke_is_opt_in_and_credential_gated'}
if not m08_golden.issubset(golden):
    errors.append('testing.yaml:M0.8 required golden vectors missing')
m08_acceptance_ids = {item['id'] for item in loaded.get('acceptance.yaml', {}).get('releases', {}).get('m0_8_round_table_vertical_slice', [])}
if m08_acceptance_ids != {f'R{i:03d}' for i in range(1, 6)}:
    errors.append('acceptance.yaml:M0.8 must contain R001-R005 exactly once')

# M0.9 routes assurance before any provider invocation. It may plan reasoning but
# cannot authorize execution, replace ApprovalGate, or silently lower a required tier.
routing = loaded.get('assurance_routing.yaml', {}).get('contract', {})
authority = routing.get('authority', {})
if authority != {'execution_authority': 'forbidden', 'approval_gate_substitution': 'forbidden', 'risk_class_substitution': 'forbidden'}:
    errors.append('assurance_routing.yaml:M0.9 authority boundary incomplete')
router = routing.get('router', {})
required_router_inputs = {'impact', 'ambiguity', 'uncertainty', 'evidence_conflict', 'novelty', 'reversibility', 'policy_required_assurance', 'estimated_reasoning_cost', 'session_budget_ceiling', 'explicit_human_escalation'}
if not required_router_inputs.issubset(set(router.get('inputs', []))) or router.get('outcomes') != ['selected', 'insufficient_assurance'] or router.get('policy_can_raise_but_not_lower') is not True or router.get('hidden_downgrade') != 'forbidden':
    errors.append('assurance_routing.yaml:M0.9 router contract incomplete')
if set(routing.get('paths', {})) != {'A0', 'A1', 'A2', 'A3', 'A4'}:
    errors.append('assurance_routing.yaml:M0.9 A0-A4 paths incomplete')
registry = routing.get('participant_registry', {})
registry_fields = {'id', 'provider', 'model_ref', 'role_capabilities', 'enabled', 'assurance_levels', 'cost_metadata_capability', 'availability_health'}
if registry.get('provider_neutral') is not True or not registry_fields.issubset(set(registry.get('required_fields', []))) or registry.get('provider_or_model_hardcoding') != 'forbidden':
    errors.append('assurance_routing.yaml:M0.9 participant registry incomplete')
budget = routing.get('budget', {})
if budget.get('estimate_before_reasoning') != 'required' or budget.get('unknown_cost') != 'unknown_not_zero' or budget.get('session_ceiling') != 'enforced_before_reasoning' or budget.get('exceeded_or_unestimable_required_budget') != 'insufficient_assurance':
    errors.append('assurance_routing.yaml:M0.9 budget contract incomplete')
failures = routing.get('failures', {})
if failures.get('silent_downgrade') != 'forbidden' or failures.get('result') != 'insufficient_assurance' or not {'participant_unavailable', 'quota_exhausted', 'timeout', 'verifier_unavailable', 'adjudication_failed', 'budget_exceeded', 'required_assurance_unachievable'}.issubset(set(failures.get('explicit_reason_codes', []))):
    errors.append('assurance_routing.yaml:M0.9 failure semantics incomplete')
m09_golden = {'assurance_router_policy_floor_never_lowers', 'assurance_router_budget_failure_is_insufficient_assurance', 'assurance_router_unknown_cost_is_not_zero', 'assurance_router_unavailable_participant_is_insufficient_assurance', 'assurance_router_a2_requires_independent_verifier', 'assurance_router_a3_requires_response_isolated_round_table', 'assurance_router_a4_requires_recorded_human_reasoning_acceptance', 'assurance_router_never_grants_execution_or_approval_authority'}
if not m09_golden.issubset(golden):
    errors.append('testing.yaml:M0.9 required golden vectors missing')
m09_acceptance_ids = {item['id'] for item in loaded.get('acceptance.yaml', {}).get('releases', {}).get('m0_9_assurance_routing', [])}
if m09_acceptance_ids != {f'Q00{i}' for i in range(1, 7)}:
    errors.append('acceptance.yaml:M0.9 must contain Q001-Q006 exactly once')

# M0.10 local briefing is a derived, non-executable result over immutable MAIA evidence.
briefing = loaded.get('briefing.yaml', {}).get('contract', {})
if briefing.get('locality') != 'local_only_loopback_provider' or briefing.get('external_egress') != 'forbidden' or briefing.get('execution_authority') is not False or briefing.get('automatic_promotion') != 'forbidden':
    errors.append('briefing.yaml:M0.10 local-only authority boundary incomplete')
packet = briefing.get('packet', {})
if packet.get('evidence_source') != 'MAIA_owned_immutable_artifacts_only' or packet.get('whole_workspace_default') != 'forbidden' or packet.get('provider_history') != 'non_canonical':
    errors.append('briefing.yaml:M0.10 packet boundary incomplete')
result = briefing.get('result', {})
if result.get('durable_entity') != 'BriefingResult' or result.get('citations_must_be_packet_scoped') is not True or result.get('unknown_or_out_of_packet_citation') != 'reject' or result.get('execution_authority') is not False:
    errors.append('briefing.yaml:M0.10 result/citation boundary incomplete')
identity = packet.get('identity', {})
packet_hash_fields = {'workspace_id', 'objective', 'evidence_references', 'evidence_hashes', 'citation_namespace', 'instructions', 'response_contract', 'privacy_classification', 'assurance_level', 'model_routing_constraints', 'instruction_template_version'}
if identity.get('version') != 'briefing_packet_v1' or not packet_hash_fields.issubset(set(identity.get('canonical_hash_binds', []))):
    errors.append('briefing.yaml:M0.10 packet identity incomplete')
if result.get('raw_provider_response') != 'untrusted_non_accepted' or result.get('accepted_result') != 'validated_before_persistence' or result.get('persistence') != 'atomic_packet_result_provenance_citations_audit' or result.get('immutability') != 'new_invocation_creates_new_result':
    errors.append('briefing.yaml:M0.10 validated persistence boundary incomplete')

# M0.12 binds the Claude Code subscription provider. It is an infra adapter with no
# execution authority, no API-key dependency, and no presence in a DEPLOYMENT_LOCKED build.
cc = loaded.get('claude_code_provider.yaml', {})
require_contract(cc.get('contract'), {
    'scope': 'M0.12_claude_code_subscription_provider_vertical_slice',
    'boundary': 'infra_adapter_not_core',
    'round_table_position': 'optional_external_module_ref_ADR_0046',
    'execution_authority': 'forbidden',
    'capability_profile': 'DEVELOPMENT_EVOLUTION'}, 'claude_code_provider.yaml:contract')
require_contract(cc.get('provider'), {
    'id': 'claude-code',
    'model_provider': 'anthropic',
    'transport': 'local_subprocess',
    'invocation_mode': 'non_interactive_print',
    'output_format': 'json',
    'shell_invocation': 'forbidden',
    'argument_safe_process_invocation': True,
    'prompt_delivery': 'child_stdin',
    'explicit_working_directory_required': True,
    'credential_source': 'local_subscription_session_only',
    'api_key_environment_injection': 'forbidden',
    'anthropic_api_key_dependency': 'forbidden',
    'other_provider_fallback': 'forbidden',
    'fabricated_success': 'forbidden',
    'failure_mode': 'fail_closed',
    'timeout_required': True,
    'cancellation_supported': True}, 'claude_code_provider.yaml:provider')
cc_env = cc.get('environment', {})
required_scrub = {'ANTHROPIC_API_KEY', 'ANTHROPIC_AUTH_TOKEN', 'ANTHROPIC_BASE_URL', 'ANTHROPIC_CUSTOM_HEADERS',
                  'CLAUDE_CODE_USE_BEDROCK', 'CLAUDE_CODE_USE_VERTEX', 'AWS_BEARER_TOKEN_BEDROCK'}
if not required_scrub.issubset(set(cc_env.get('scrubbed_variables', []))) or cc_env.get('scrub_is_unconditional') is not True:
    errors.append('claude_code_provider.yaml:M0.12 child environment credential scrub incomplete')
cc_perm = cc.get('permission_model', {})
if cc_perm.get('bypass_permissions') != 'forbidden' or cc_perm.get('empty_allowlist_is_deny_all') is not False or cc_perm.get('enforcement') != 'explicit_deny_list':
    errors.append('claude_code_provider.yaml:M0.12 permission model must deny-list explicitly and never bypass permissions')
required_denied = {'Bash', 'Edit', 'Write', 'NotebookEdit', 'WebFetch', 'WebSearch', 'Task'}
if not required_denied.issubset(set(cc_perm.get('denied_tools', []))) or cc_perm.get('egress_tools_always_denied') is not True:
    errors.append('claude_code_provider.yaml:M0.12 denied tool inventory incomplete')
cc_rec = cc.get('recursion_guard', {})
if cc_rec.get('environment_marker') != 'MAIA_CLAUDE_CODE_PROVIDER_ACTIVE' or cc_rec.get('nested_invocation') != 'forbidden':
    errors.append('claude_code_provider.yaml:M0.12 recursion guard incomplete')
cc_cap = cc.get('capture', {})
if not {'stdout', 'stderr', 'exit_code', 'duration_ms', 'cli_version', 'correlation_id', 'session_id'}.issubset(set(cc_cap.get('required_fields', []))) or cc_cap.get('raw_response_preserved_for_audit') is not True or cc_cap.get('cost_invention') != 'forbidden':
    errors.append('claude_code_provider.yaml:M0.12 execution capture contract incomplete')
cc_packet = cc.get('contracts', {}).get('consultation_packet', {})
if cc_packet.get('version') != 'maia.consultation_packet.v1' or not {'consultation_id', 'participant_id', 'role', 'task', 'context_references', 'evidence_requirements', 'constraints', 'timeout_seconds'}.issubset(set(cc_packet.get('required_fields', []))):
    errors.append('claude_code_provider.yaml:M0.12 ConsultationPacket v1 contract incomplete')
cc_result = cc.get('contracts', {}).get('consultation_result', {})
if cc_result.get('version') != 'maia.consultation_result.v1' or not {'response', 'findings', 'evidence', 'risks', 'disagreements', 'recommendation', 'confidence', 'execution'}.issubset(set(cc_result.get('required_fields', []))) or cc_result.get('validated_before_round_table_consumption') is not True or cc_result.get('consultation_id_must_match_request') is not True:
    errors.append('claude_code_provider.yaml:M0.12 ConsultationResult v1 contract incomplete')
cc_audit = cc.get('audit', {})
if cc_audit.get('raw_response_preserved') is not True or cc_audit.get('secret_material_logging') != 'forbidden' or cc_audit.get('write_failure_is_invocation_failure') is not True:
    errors.append('claude_code_provider.yaml:M0.12 audit contract incomplete')
cc_lock = cc.get('deployment_lock', {})
require_contract(cc_lock, {
    'profile_absent_in': 'DEPLOYMENT_LOCKED',
    'mechanism': 'build_graph_feature_absence_not_runtime_boolean',
    'cargo_feature': 'development-evolution',
    'default_enabled': False,
    'subprocess_code_compiled_without_feature': False,
    'runtime_reenable_by_instance': 'forbidden',
    'core_depends_on_provider': False}, 'claude_code_provider.yaml:deployment_lock')
cc_live = cc.get('live_smoke', {})
if cc_live.get('default') != 'skipped' or cc_live.get('explicit_opt_in_environment') != 'MAIA_LIVE_CLAUDE_CODE=1' or cc_live.get('read_only') is not True or cc_live.get('sensitive_prompt_or_credential_logging') != 'forbidden':
    errors.append('claude_code_provider.yaml:M0.12 live smoke must be opt-in, read-only and credential-safe')
m012_golden = {'claude_code_provider_never_depends_on_anthropic_api_key',
               'claude_code_provider_scrubs_api_key_from_child_environment',
               'claude_code_provider_never_uses_bypass_permissions',
               'claude_code_provider_refuses_nested_invocation',
               'claude_code_provider_schema_violation_fails_closed',
               'claude_code_provider_absent_without_development_evolution_feature',
               'claude_code_provider_audit_write_failure_fails_invocation',
               'claude_code_live_smoke_is_opt_in_and_read_only'}
if not m012_golden.issubset(golden):
    errors.append('testing.yaml:M0.12 required golden vectors missing')
m012_ids = {item['id'] for item in loaded.get('acceptance.yaml', {}).get('releases', {}).get('m0_12_claude_code_subscription_provider', [])}
if m012_ids != {f'C{i:03d}' for i in range(1, 9)}:
    errors.append('acceptance.yaml:M0.12 must contain C001-C008 exactly once')

# M0.13 makes the Round Table survive partial provider failure without ever
# converting isolation into a silent assurance downgrade.
orch = loaded.get('round_table.yaml', {}).get('orchestration', {})
require_contract(orch.get('contract_evolution'), {
    'm0_8_entry_point': 'preserved_as_compatibility_shim',
    'shim_delegates_to_new_implementation': True,
    'spec_precedes_implementation': True}, 'round_table.yaml:M0.13.contract_evolution')
outcomes = orch.get('participant_outcomes', {})
if (outcomes.get('collect_all_before_quorum_evaluation') is not True
        or outcomes.get('single_participant_failure_aborts_collection') is not False
        or outcomes.get('failed_contribution_retained_in_provenance') is not True
        or not {'responded', 'failed'}.issubset(set(outcomes.get('outcome_kinds', [])))):
    errors.append('round_table.yaml:M0.13 participant outcome isolation contract incomplete')
require_contract(orch.get('quorum'), {
    'evaluated_after_collection': True,
    'a3_minimum_independent_responses': 2,
    'below_minimum_result': 'insufficient_assurance',
    'below_minimum_reason_code': 'participant_unavailable',
    'silent_downgrade': 'forbidden',
    'single_participant_round_table': 'forbidden',
    'leader_substitutes_for_missing_first_round_response': 'forbidden',
    'degraded_panel_adjudication': 'forbidden'}, 'round_table.yaml:M0.13.quorum')
leader = orch.get('leader_selection', {})
if (leader.get('deterministic') is not True or leader.get('registry_driven') is not True
        or leader.get('recorded') is not True
        or leader.get('provider_or_model_hardcoding') != 'forbidden'
        or leader.get('independent_of_provider_resolution') is not True
        or not {'enabled', 'adjudicator_role_capability', 'supports_requested_assurance',
                'healthy_availability'}.issubset(set(leader.get('eligibility', [])))):
    errors.append('round_table.yaml:M0.13 leader selection contract incomplete')
require_contract(leader.get('authority'), {
    'scope': 'coordination_and_synthesis_within_session',
    'execution_authority': 'forbidden',
    'approval_authority': 'forbidden',
    'policy_authority': 'forbidden',
    'core_authority': 'forbidden',
    'persistence_ownership_outside_session_contract': 'forbidden'},
    'round_table.yaml:M0.13.leader_selection.authority')
require_contract(orch.get('participant_resolver'), {
    'role': 'maps_registration_to_live_participant',
    'core_depends_on_port_only': True,
    'provider_executable_model_or_credential_in_core': 'forbidden',
    'resolution_failure_is_isolated_and_classified': True,
    'silent_resolution_to_another_provider': 'forbidden'},
    'round_table.yaml:M0.13.participant_resolver')
prov = orch.get('provenance', {})
required_prov = {'participant_id', 'provider', 'model_ref_as_used', 'role', 'sequence',
                 'started_at', 'finished_at', 'provider_request_id', 'result',
                 'reason_code', 'first_round_isolated'}
if (not required_prov.issubset(set(prov.get('required_fields', [])))
        or prov.get('failed_attempts_recorded') is not True
        or prov.get('secret_or_credential_material') != 'forbidden'):
    errors.append('round_table.yaml:M0.13 contribution provenance contract incomplete')
require_contract(orch.get('session_store'), {
    'port_location': 'roundtable',
    'implementation_location': 'infra_adapter_not_core',
    'state_ownership': 'round_table_module_not_core',
    'reuse_existing_persistence_when_ownership_preserved': 'preferred',
    'records_versioned': True,
    'persists_failed_contributions': True}, 'round_table.yaml:M0.13.session_store')
require_contract(orch.get('core_independence'), {
    'core_depends_on_round_table_implementation': False,
    'shipped_application_depends_on_round_table_implementation': False,
    'verified_by': 'dependency_graph_not_workspace_layout'},
    'round_table.yaml:M0.13.core_independence')
m013_golden = {'round_table_participant_failure_does_not_abort_collection',
               'round_table_quorum_is_evaluated_after_all_outcomes',
               'round_table_below_quorum_is_insufficient_assurance_not_downgrade',
               'round_table_leader_never_substitutes_for_missing_quorum',
               'round_table_leader_selection_is_deterministic_and_unhardcoded',
               'round_table_failed_contribution_remains_in_provenance',
               'round_table_resolver_failure_never_falls_back_to_another_provider',
               'round_table_session_persistence_preserves_failed_contributions',
               'core_does_not_depend_on_round_table_implementation'}
if not m013_golden.issubset(golden):
    errors.append('testing.yaml:M0.13 required golden vectors missing')
m013_ids = {item['id'] for item in loaded.get('acceptance.yaml', {}).get('releases', {}).get('m0_13_round_table_live_orchestration', [])}
if m013_ids != {f'D{i:03d}' for i in range(1, 15)}:
    errors.append('acceptance.yaml:M0.13 must contain D001-D014 exactly once')

# Acceptance IDs unique
ids=[]
for stage,items in loaded['acceptance.yaml']['releases'].items(): ids += [x['id'] for x in items]
if len(ids)!=len(set(ids)): errors.append('acceptance.yaml: duplicate acceptance id')

errors.extend(validate_evolution_supervisor(loaded.get('evolution_supervisor.yaml')))

if errors:
    print('SPEC GUARD FAILED')
    for e in errors: print(' -',e)
    sys.exit(1)

# Executable RFC8785 reference self-test. Node is part of the project toolchain; absence fails closed for this security invariant.
node=shutil.which('node')
if not node:
    print('SPEC GUARD FAILED\n - Node.js required to run RFC8785 JCS reference vector')
    sys.exit(1)
vec=ROOT/'tests/spec/jcs_rfc8785_vectors.json'; script=ROOT/'tools/jcs_reference.mjs'
r=subprocess.run([node,str(script),'--self-test',str(vec)],capture_output=True,text=True)
if r.returncode:
    print('SPEC GUARD FAILED\n - RFC8785 JCS self-test failed')
    print(r.stdout+r.stderr); sys.exit(1)
print(r.stdout.strip())
binding_test = ROOT/'tests/spec/action_binding_reference.mjs'
r = subprocess.run([node,str(binding_test),'--self-test'],capture_output=True,text=True)
if r.returncode:
    print('SPEC GUARD FAILED\n - ActionBindingV1 conformance failed')
    print(r.stdout+r.stderr)
    sys.exit(1)
print(r.stdout.strip())
print(f'SPEC GUARD OK: {len(loaded)} YAML files parsed and contract checks passed.')
