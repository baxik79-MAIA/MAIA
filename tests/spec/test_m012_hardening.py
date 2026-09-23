"""F1-F4 regression evidence. No production policy engine, directory or store."""
from copy import deepcopy
from itertools import product
import unittest
import test_execution_contract_guard as guard
from test_m011_contracts import AP, SPEC, decision_cas, boundary_risk


def fixture(decision='confirm'):
    record = dict(approval_id='approval', version=1, state='pending', action_id='action',
                  action_version=3, action_hash='hash', expires_at=100, policy_id='policy',
                  policy_snapshot_hash='snapshot', policy_decision=decision, origin_surface='desktop',
                  required_assurance=decision, achieved_assurance='none', decided_by=None,
                  decided_at=None, decided_surface=None, decision_note=None)
    request = {k: record[k] for k in ('approval_id', 'action_id', 'action_version', 'action_hash')}
    request.update(expected_approval_version=1, required_current_state='pending',
                   decided_by='authorized-actor', decided_surface='teams', achieved_assurance=decision)
    current = {k: record[k] for k in ('action_id', 'action_version', 'action_hash')}
    return record, request, current


class HardeningTests(unittest.TestCase):
    def test_f1_validator_rejects_every_terminal_run_edge(self):
        checker = guard.ContractGuardTests()
        machine = SPEC['states.yaml']['machines']['RunState']
        for source, destination in product(machine['terminal_states'], machine['states']):
            with self.subTest(source=source, destination=destination):
                checker.check_rejected(lambda d, s=source, t=destination:
                    d['states.yaml']['machines']['RunState']['transitions'].update({s: [t]}))

    def test_f1_terminal_declaration_cannot_be_removed_or_weakened(self):
        checker = guard.ContractGuardTests()
        checker.check_rejected(lambda d: d['states.yaml']['machines']['RunState'].pop('terminal_states'))
        for value in ([], ['completed', 'failed', 'canceled'], ['retryable_error'] * 2, ['unknown'], None):
            with self.subTest(value=value):
                checker.check_rejected(lambda d: guard.set_at(d, 'states.yaml', 'machines.RunState.terminal_states', value))

    def test_f2_f3_f4_canonical_guards(self):
        checker = guard.ContractGuardTests()
        for path, value in [
            ('record.snapshot_state_compatibility.allow', ['not_required', 'approved']),
            ('record.snapshot_state_compatibility.confirm', ['not_required']),
            ('record.snapshot_state_compatibility.elevated_confirm', ['not_required']),
            ('record.snapshot_state_compatibility.deny', ['rejected']),
            ('record.pending.null_fields', []),
            ('record.pending.achieved_assurance', 'confirm'),
            ('record.human_rejected_requires', []),
            ('record.human_rejected_minimum_assurance', 'none'),
            ('record.rejection_requires_approval_level_assurance', True),
            ('concurrency.decision_timestamp', 'request_timestamp'),
            ('recipient_boundary.input_validation_order', ['preserve_operation_floor', 'validate_complete_input']),
            ('recipient_boundary.invalid_input_with_floor', 'accept'),
        ]:
            with self.subTest(path=path):
                checker.check_rejected(lambda d: guard.set_at(d, 'approval.yaml', path, value))

    def test_f3_assurance_and_complete_decision_metadata(self):
        for decision, destination, achieved in product(('confirm', 'elevated_confirm'),
                                                     ('approved', 'rejected'), ('none', 'confirm', 'elevated_confirm')):
            with self.subTest(decision=decision, destination=destination, achieved=achieved):
                record, request, current = fixture(decision)
                originals = deepcopy((record, request, current))
                request['achieved_assurance'] = achieved
                request['decided_at'] = 999  # The request cannot select its own audit timestamp.
                result = decision_cas(record, request, current, 99, True, destination)
                permitted = achieved != 'none' and (destination == 'rejected' or decision == 'confirm' or achieved == 'elevated_confirm')
                if permitted:
                    self.assertEqual(result['state'], destination)
                    self.assertEqual(result['version'], 2)
                    self.assertEqual(result['decided_at'], 99)
                    self.assertEqual(result['decided_by'], 'authorized-actor')
                    self.assertEqual(result['decided_surface'], 'teams')
                    self.assertEqual(result['achieved_assurance'], achieved)
                    mutable = {'state', 'version', 'decided_at', 'decided_by', 'decided_surface', 'achieved_assurance'}
                    self.assertEqual({k: v for k, v in result.items() if k not in mutable},
                                     {k: v for k, v in record.items() if k not in mutable})
                    self.assertEqual(decision_cas(result, request, current, 99, True, destination), 'ExternalConflict')
                else:
                    self.assertEqual(result, 'insufficient_assurance')
                self.assertEqual(record, originals[0])
                self.assertEqual(current, originals[2])

    def test_f3_negative_context_and_metadata_do_not_mutate_inputs(self):
        for destination in ('approved', 'rejected'):
            for field in ('approval_id', 'action_id', 'action_version', 'action_hash', 'expected_approval_version', 'required_current_state'):
                record, request, current = fixture()
                del request[field]
                before = deepcopy((record, request, current))
                self.assertEqual(decision_cas(record, request, current, 99, True, destination), 'ExternalConflict')
                self.assertEqual((record, request, current), before)
            for field in ('decided_by', 'decided_surface', 'achieved_assurance'):
                for value in (None, ''):
                    record, request, current = fixture()
                    request[field] = value
                    before = deepcopy((record, request, current))
                    expected = 'insufficient_assurance' if field == 'achieved_assurance' else 'InvalidApprovalMetadata'
                    self.assertEqual(decision_cas(record, request, current, 99, True, destination), expected)
                    self.assertEqual((record, request, current), before)
            for changes in ({'required_assurance': 'none'}, {'policy_decision': 'deny'},
                            {'policy_decision': 'allow'}, {'achieved_assurance': 'confirm'},
                            {'decided_at': 1}, {'decided_by': 'old-actor'}, {'decided_surface': 'old'}, {'decision_note': 'old'}):
                record, request, current = fixture()
                record.update(changes)
                before = deepcopy(record)
                self.assertEqual(decision_cas(record, request, current, 99, True, destination), 'InvalidApprovalMetadata')
                self.assertEqual(record, before)
            record, request, current = fixture()
            self.assertEqual(decision_cas(record, request, current, 99, False, destination), 'actor_unauthorized')
            for now in (100, 101):
                self.assertEqual(decision_cas(record, request, current, now, True, destination), 'ApprovalExpired')
            for field in current:
                self.assertEqual(decision_cas(record, request, {**current, field: 'stale'}, 99, True, destination), 'ExternalConflict')
            record['version'] = request['expected_approval_version'] = 2**64 - 1
            before = deepcopy(record)
            with self.assertRaises(ValueError):
                decision_cas(record, request, current, 99, True, destination)
            self.assertEqual(record, before)

    def test_f4_completeness_before_every_floor(self):
        for floor in (None, 'destructive', 'privileged'):
            for calendar, private in ((False, False), (False, True), (True, False)):
                with self.subTest(floor=floor, calendar=calendar, private=private):
                    with self.assertRaisesRegex(ValueError, 'empty_or_incomplete'):
                        boundary_risk([], private, calendar, floor)
            with self.assertRaisesRegex(ValueError, 'invalid_boundary'):
                boundary_risk(['untrusted'], floor=floor)
            self.assertEqual(boundary_risk([], True, True, floor), floor or 'write_internal')
            for boundary, expected in (('internal', 'send_internal'), ('unknown', 'send_external'), ('external', 'send_external')):
                self.assertEqual(boundary_risk([boundary], floor=floor), floor or expected)


if __name__ == '__main__':
    unittest.main()
