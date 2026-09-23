"""Executable contract oracles, not the M0.2 policy engine or store implementation.

Fixtures model trusted boundary observations. They do not implement directory lookup,
connector reconciliation, atomic persistence, or semantic effect equivalence discovery.
"""
from copy import deepcopy
from itertools import product, permutations
from pathlib import Path
import subprocess
import unittest
import yaml
import test_execution_contract_guard as guard

ROOT = Path(__file__).resolve().parents[2]
SPEC = {p.name: yaml.safe_load(p.read_text(encoding='utf-8')) for p in (ROOT / 'spec').glob('*.yaml')}
AP = SPEC['approval.yaml']
EX = SPEC['execution.yaml']


def compose(*decisions):
    order = AP['policy_decision']['restriction_order']
    return max(decisions, key=order.index)


def gate_result(gate, explicit_autonomous=False, restrictions=()):
    mapping = AP['policy_decision']['default_gate_mapping']
    decision = mapping[gate]
    if gate == 'policy':
        decision = 'allow' if explicit_autonomous else AP['policy_decision']['policy_gate_without_explicit_autonomous_permission']
    return compose(decision, *restrictions)


def authorize(decision, state, achieved, validity='valid', preconditions=True):
    row = AP['execution_authorization']['matrix'][decision]
    if validity != 'valid' or not preconditions or state not in row['states']:
        return False
    order = AP['assurance']['order']
    return order.index(achieved) >= order.index(row['minimum_assurance'])


def expired(now, expires_at):
    return expires_at is not None and now >= expires_at


def checked_increment(value, maximum):
    if value >= maximum:
        raise ValueError('overflow')
    return value + 1


def decision_cas(record, request, current, now, actor_allowed, next_state):
    """Single snapshot oracle. Decision identity/assurance are trusted test context;
    atomic persistence and real authentication remain outside this milestone.
    """
    if any(request.get(k) != record[k] for k in ('approval_id', 'action_id', 'action_version', 'action_hash')):
        return 'ExternalConflict'
    if (request.get('expected_approval_version') != record['version']
            or request.get('required_current_state') != 'pending' or record['state'] != 'pending'):
        return 'ExternalConflict'
    if any(current.get(k) != record[k] for k in ('action_id', 'action_version', 'action_hash')):
        return 'ExternalConflict'
    if expired(now, record['expires_at']):
        return 'ApprovalExpired'
    if not actor_allowed:
        return 'actor_unauthorized'
    if next_state not in AP['concurrency']['human_decision_destinations']:
        return 'InvalidStateTransition'
    if (record.get('policy_decision') not in ('confirm', 'elevated_confirm')
            or record.get('required_assurance') != AP['assurance']['required_by_decision'].get(record.get('policy_decision'))
            or record.get('achieved_assurance') != 'none'
            or any(k not in record or record[k] is not None for k in AP['record']['pending']['null_fields'])):
        return 'InvalidApprovalMetadata'
    if not request.get('decided_by') or not request.get('decided_surface'):
        return 'InvalidApprovalMetadata'
    order = AP['assurance']['order']
    achieved = request.get('achieved_assurance')
    minimum = record['required_assurance'] if next_state == 'approved' else AP['record']['human_rejected_minimum_assurance']
    if achieved not in order or order.index(achieved) < order.index(minimum):
        return 'insufficient_assurance'
    updated = deepcopy(record)
    updated['version'] = checked_increment(record['version'], 2**64 - 1)
    updated.update(state=next_state, decided_by=request['decided_by'], decided_at=now,
                   decided_surface=request['decided_surface'], achieved_assurance=achieved)
    return updated


def boundary_risk(boundaries, private_event=False, calendar=False, floor=None):
    if any(b not in AP['recipient_boundary']['values'] for b in boundaries):
        raise ValueError('invalid_boundary')
    if not boundaries and not (calendar and private_event):
        raise ValueError('empty_or_incomplete_recipient_context')
    if not boundaries:
        risk = 'write_internal'
    else:
        risk = next(AP['recipient_boundary']['risk_by_boundary'][boundary]
                    for boundary in AP['recipient_boundary']['precedence'] if boundary in boundaries)
    if floor in ('destructive', 'privileged'):
        return floor
    return risk


def next_run(previous, action_id, action_version, new_run_id, unresolved_equivalent=False):
    if unresolved_equivalent:
        raise ValueError('unresolved_effect')
    if new_run_id == previous['run_id']:
        raise ValueError('retry_requires_new_RunId')
    same = (action_id, action_version) == (previous['action_id'], previous['action_version'])
    attempt = checked_increment(previous['attempt'], 2**32 - 1) if same else EX['attempts']['first']
    return dict(run_id=new_run_id, action_id=action_id, action_version=action_version, attempt=attempt, state='created')


class ExecutionContractTests(unittest.TestCase):
    def test_binding_golden_and_negative_vectors(self):
        result = subprocess.run(['node', str(ROOT/'tests/spec/action_binding_reference.mjs'), '--self-test'], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_policy_composition_lattice(self):
        expected = ['allow', 'confirm', 'elevated_confirm', 'deny']
        for choices in product(expected, repeat=3):
            answer = expected[max(expected.index(d) for d in choices)]
            for ordering in permutations(choices):
                self.assertEqual(compose(*ordering), answer)
            self.assertEqual(compose(*choices, 'deny'), 'deny')
        self.assertEqual(gate_result('policy'), 'confirm')
        self.assertEqual(gate_result('policy', True), 'allow')
        for gate in ('confirm', 'elevated_confirm'):
            self.assertEqual(gate_result(gate, True), gate)
        self.assertEqual(gate_result('policy', True, ('deny',)), 'deny')
        self.assertEqual(gate_result('allow', False, ('elevated_confirm',)), 'elevated_confirm')

    def test_authorization_matrix_and_tightening(self):
        for state in ('pending', 'rejected', 'revoked', 'expired'):
            for decision in AP['policy_decision']['values']:
                self.assertFalse(authorize(decision, state, 'elevated_confirm'))
        self.assertTrue(authorize('allow', 'not_required', 'none'))
        self.assertTrue(authorize('confirm', 'approved', 'confirm'))
        self.assertFalse(authorize('elevated_confirm', 'approved', 'confirm'))
        self.assertTrue(authorize('elevated_confirm', 'approved', 'elevated_confirm'))
        self.assertFalse(authorize('deny', 'approved', 'elevated_confirm'))
        for invalid in AP['binding_validity']['values'][1:]:
            self.assertFalse(authorize('allow', 'approved', 'elevated_confirm', invalid))
        self.assertFalse(authorize('allow', 'approved', 'confirm', preconditions=False))
        self.assertFalse(authorize('confirm', 'not_required', 'none'))

    def test_expiry_history_and_material_mutation(self):
        for now, expected in [(99, False), (100, True), (101, True)]:
            self.assertEqual(expired(now, 100), expected)
        self.assertFalse(expired(999, None))
        record = {'state':'approved', 'expires_at':100}
        self.assertTrue(expired(100, record['expires_at']))
        self.assertEqual(record['state'], 'approved')
        transitions=SPEC['states.yaml']['machines']['ApprovalState']['transitions']
        self.assertNotIn('expired', transitions['approved'])
        policy=AP['concurrency']['mutation_policy']
        for state in SPEC['states.yaml']['machines']['ApprovalState']['states']:
            result='revoked' if state in policy['revocable_prior_states'] else state
            self.assertEqual(result, 'revoked' if state in ('pending','approved') else state)
        self.assertEqual(AP['record']['not_required']['null_fields'], ['decided_by','decided_at','decided_surface'])

    def test_cas_exact_revision_and_history(self):
        record=dict(approval_id='approval',version=1,state='pending',action_id='action',action_version=3,action_hash='hash',expires_at=100,
                    policy_id='policy',policy_snapshot_hash='snapshot',origin_surface='desktop',required_assurance='confirm',
                    policy_decision='confirm',achieved_assurance='none',decided_by=None,decided_at=None,decided_surface=None,decision_note=None)
        request={k:record[k] for k in ('approval_id','action_id','action_version','action_hash')}
        request.update(expected_approval_version=1,required_current_state='pending',decided_by='actor',decided_surface='desktop',achieved_assurance='confirm')
        current={k:record[k] for k in ('action_id','action_version','action_hash')}
        updated=decision_cas(record,request,current,99,True,'approved')
        self.assertEqual(updated['version'],2)
        self.assertEqual(record['state'],'pending')
        for field in AP['record']['immutable_fields']:
            self.assertEqual(updated[field],record[field])
        self.assertEqual(decision_cas(updated,request,current,99,True,'rejected'),'ExternalConflict')
        for field in ('approval_id','action_id','action_version','action_hash','expected_approval_version','required_current_state'):
            bad={**request,field:'different'}
            self.assertEqual(decision_cas(record,bad,current,99,True,'approved'),'ExternalConflict')
        for field in current:
            self.assertEqual(decision_cas(record,request,{**current,field:'different'},99,True,'approved'),'ExternalConflict')
        self.assertEqual(decision_cas(record,request,current,100,True,'approved'),'ApprovalExpired')
        self.assertEqual(decision_cas(record,request,current,99,False,'approved'),'actor_unauthorized')
        maximum={**record,'version':2**64-1}
        with self.assertRaises(ValueError):
            decision_cas(maximum,{**request,'expected_approval_version':2**64-1},current,99,True,'approved')

    def test_retry_identity_scope_and_terminal_state(self):
        previous=dict(run_id='old',action_id='a',action_version=3,attempt=2,state='retryable_error')
        self.assertEqual(next_run(previous,'a',3,'new')['attempt'],3)
        self.assertEqual(next_run(previous,'a',4,'new')['attempt'],1)
        self.assertEqual(next_run(previous,'b',1,'new')['attempt'],1)
        self.assertEqual(next_run(previous,'a',3,'new')['state'],'created')
        self.assertEqual(previous['state'],'retryable_error')
        with self.assertRaises(ValueError): next_run(previous,'a',3,'old')
        with self.assertRaises(ValueError): next_run({**previous,'attempt':2**32-1},'a',3,'new')
        for action,version in [('a',3),('a',4),('b',1)]:
            with self.assertRaises(ValueError): next_run(previous,action,version,'new',unresolved_equivalent=True)
        self.assertEqual(SPEC['states.yaml']['machines']['RunState']['transitions'].get('retryable_error',[]),[])
        self.assertEqual(EX['reconciliation']['values'],['effect_confirmed','effect_not_executed','unresolved'])
        self.assertFalse(EX['reconciliation']['non_authoritative_search_absence_is_proof'])

    def test_freshness_scope_and_superseded_pause(self):
        run=dict(run_id='run1',action_id='a',action_version=1,action_hash='hash')
        evidence={**run,'result':'matched'}
        for field in run:
            other={**run,field:'different'}
            self.assertFalse(all(evidence[k]==other[k] for k in run))
        self.assertEqual(EX['freshness']['unverifiable_material'],'fail_closed')
        self.assertFalse(EX['freshness']['reuse_across_runs'])
        self.assertIsNone(EX['freshness']['arbitrary_ttl_seconds'])
        in_flight=SPEC['states.yaml']['pause_semantics']['in_flight_states']
        runs=[dict(state='completed',superseded=False),dict(state='reconciling',superseded=True)]
        self.assertFalse(all(r['state'] not in in_flight for r in runs))
        self.assertEqual(set(in_flight),{'starting','running','outcome_unknown','reconciling'})
        self.assertIn('created_to_starting',EX['pause']['forbidden_after_request'])
        self.assertEqual(EX['pause']['scope'],'all_task_runs_including_superseded_plan_action_revisions')

    def test_recipient_boundary_and_operation_floors(self):
        self.assertEqual(boundary_risk(['internal']),'send_internal')
        for boundaries in [('internal','unknown'),('unknown',),('internal','external')]:
            self.assertEqual(boundary_risk(boundaries),'send_external')
        with self.assertRaises(ValueError): boundary_risk([])
        self.assertEqual(boundary_risk([],True,True),'write_internal')
        self.assertEqual(boundary_risk(['internal'],True,True),'send_internal')
        with self.assertRaises(ValueError): boundary_risk([],False,True)
        for floor in ('destructive','privileged'):
            self.assertEqual(boundary_risk(['internal'],floor=floor),floor)
        self.assertFalse(AP['recipient_boundary']['untrusted_payload_can_self_declare_internal'])

    def test_build_order_hashes_final_versions_and_risk(self):
        steps=SPEC['action_binding.yaml']['material_revision_build_order']
        self.assertLess(steps.index('choose_next_Action_version_once'),steps.index('compute_action_hash'))
        self.assertLess(steps.index('resolve_final_RiskClass'),steps.index('compute_action_hash'))
        self.assertEqual(steps.count('choose_next_Action_version_once'),1)
        self.assertLess(steps.index('compute_action_hash'),steps.index('create_new_Approval_binding_unless_deny'))

    def test_new_contract_negative_regressions(self):
        checker=guard.ContractGuardTests()
        # Every new typed field must fail closed if removed or made optional.
        additions={'Action':['version','plan_version','input_hash','input_canonicalizer','connector_selection','connector_binding_hash','tool_definition_fingerprint'],
                   'Approval':['action_version','policy_snapshot_hash','policy_decision','required_assurance','achieved_assurance'],
                   'Run':['action_version','action_hash','approval_id','approval_version']}
        for entity,fields in additions.items():
            for field in fields:
                with self.subTest(entity=entity,field=field):
                    checker.check_rejected(lambda d,e=entity,f=field:d['domain.yaml']['execution_contracts'][e]['fields'].pop(f))
                    if SPEC['domain.yaml']['execution_contracts'][entity]['fields'][field]['required']:
                        checker.check_rejected(lambda d,e=entity,f=field:d['domain.yaml']['execution_contracts'][e]['fields'][f].update(required=False))
        cases=[('action_binding.yaml','action_binding.decimal_string_paths',[]),
               ('action_binding.yaml','action_binding.exact_duplicate_source_tuples','deduplicate'),
               ('action_binding.yaml','action_binding.null_encoding','omit'),
               ('action_binding.yaml','canonicalizer.material_failure','execute'),
               ('action_binding.yaml','connector_binding.secrets_allowed',True),
               ('action_binding.yaml','mcp.invocation_requires_fingerprint',False),
               ('approval.yaml','policy_decision.restriction_order',['deny','allow','confirm','elevated_confirm']),
               ('approval.yaml','execution_authorization.matrix.deny.states',['approved']),
               ('approval.yaml','record.not_required.human_decision',True),
               ('approval.yaml','concurrency.required_match',['approval_id']),
               ('approval.yaml','binding_validity.expiry','now_greater_than_expires_at'),
               ('execution.yaml','freshness.reuse_across_runs',True),
               ('execution.yaml','reconciliation.non_authoritative_search_absence_is_proof',True),
               ('execution.yaml','run.newer_revision_may_receive_historical_result',True),
               ('states.yaml','pause_semantics.run_scope','current_actions_only'),
               ('approval.yaml','recipient_boundary.risk_by_boundary.unknown','send_internal')]
        for file,path,value in cases:
            with self.subTest(file=file,path=path):
                checker.check_rejected(lambda d:guard.set_at(d,file,path,value))


if __name__ == '__main__':
    unittest.main()
