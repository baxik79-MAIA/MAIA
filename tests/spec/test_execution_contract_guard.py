"""Negative Spec Guard tests; fixtures stay in memory and canonical files are untouched."""
from copy import deepcopy
from pathlib import Path
import contextlib
import io
import runpy
import unittest
from unittest.mock import patch

import yaml

ROOT = Path(__file__).resolve().parents[2]
BASE = {p.name: yaml.safe_load(p.read_text(encoding='utf-8')) for p in (ROOT / 'spec').glob('*.yaml')}
REAL_READ = Path.read_text


def set_at(data, file, path, value):
    target = data[file]
    keys = path.split('.')
    for key in keys[:-1]:
        target = target[key]
    target[keys[-1]] = value


class ContractGuardTests(unittest.TestCase):
    def check_rejected(self, mutate):
        data = deepcopy(BASE)
        mutate(data)

        def read(path, *args, **kwargs):
            if path.parent == ROOT / 'spec' and path.name in data:
                return yaml.safe_dump(data[path.name], sort_keys=False)
            return REAL_READ(path, *args, **kwargs)

        output = io.StringIO()
        with patch.object(Path, 'read_text', read), contextlib.redirect_stdout(output):
            with self.assertRaises(SystemExit) as result:
                runpy.run_path(str(ROOT / 'tools/validate_spec.py'), run_name='__main__')
        self.assertEqual(result.exception.code, 1)
        self.assertIn('SPEC GUARD FAILED', output.getvalue())

    def test_contract_regressions(self):
        cases = [
            ('domain.yaml', 'execution_contracts', {}),
            ('domain.yaml', 'execution_contracts.Action.fields.input_ref.type', 'MissingType'),
            ('domain.yaml', 'execution_contracts.AgentTask.fields.request_text.required', False),
            ('domain.yaml', 'execution_contracts.ExecutionPlan.fields.estimated_cost.nullable', False),
            ('domain.yaml', 'primitives.WorkspaceId.uuid_version', 4),
            ('domain.yaml', 'primitives.ActionId.rust_type', 'generic_id'),
            ('domain.yaml', 'primitives.PolicyId.uuid_required', True),
            ('domain.yaml', 'primitives.OpaqueRef.normalization', 'uri_normalize'),
            ('domain.yaml', 'primitives.SurfaceId.registry', 'closed'),
            ('domain.yaml', 'primitives.ActionType.pattern', '.*'),
            ('domain.yaml', 'primitives.Version.minimum', 0),
            ('domain.yaml', 'primitives.Ordinal.minimum', 1),
            ('domain.yaml', 'primitives.Attempt.minimum', 0),
            ('domain.yaml', 'primitives.Timestamp.canonical_serialization', None),
            ('domain.yaml', 'primitives.Sha256Hex.pattern', '^[A-F0-9]{64}$'),
            ('domain.yaml', 'primitives.PrivacyClass.enum_source', 'spec/modes.yaml#missing'),
            ('domain.yaml', 'primitives.RunState.transitions_source', 'spec/states.yaml#missing'),
            ('domain.yaml', 'value_objects.CostEstimate.floating_point_allowed', True),
            ('domain.yaml', 'primitives.AmountMicros.wire_type', 'float'),
            ('domain.yaml', 'value_objects.SourcePrecondition.fields', {}),
            ('domain.yaml', 'execution_contracts.Action.rules.nonempty_source_preconditions_requires', None),
            ('domain.yaml', 'value_objects.OutcomeCertainty.values', ['known', 'unknown']),
            ('domain.yaml', 'value_objects.OutcomeCertainty.state_coupling.created', 'known'),
            ('domain.yaml', 'value_objects.OutcomeCertainty.state_coupling', {}),
            ('domain.yaml', 'value_objects.RiskSummary.items', 'GateType'),
            ('domain.yaml', 'value_objects.RiskSummary.ordering', 'alphabetical'),
            ('domain.yaml', 'value_objects.RiskSummary.unique_items', False),
            ('approval.yaml', 'concurrency.mutation_policy.revocable_prior_states', ['pending', 'approved', 'expired']),
            ('approval.yaml', 'concurrency.mutation_policy.preserved_prior_states', ['rejected']),
            ('approval.yaml', 'concurrency.mutation_policy.preserved_handling', 'transition_to_revoked'),
            ('approval.yaml', 'concurrency.mutation_policy.old_binding_valid_for_new_payload_version', True),
            ('approval.yaml', 'concurrency.mutation_policy.new_approval_state_source', 'pending'),
            ('states.yaml', 'machines.ApprovalState.transitions.approved', []),
            ('states.yaml', 'machines.ApprovalState.transitions.rejected', ['revoked']),
            ('sync.yaml', 'pre_side_effect_freshness.approval_mutation_policy_ref', 'spec/approval.yaml#missing'),
            ('sync.yaml', 'pre_side_effect_freshness.on_mismatch', ['revoke_stale_approval']),
        ]
        for file, path, value in cases:
            with self.subTest(file=file, path=path):
                self.check_rejected(lambda data: set_at(data, file, path, value))

    def test_unknown_run_state_requires_certainty_coupling(self):
        self.check_rejected(lambda data: data['states.yaml']['machines']['RunState']['states'].append('new_state'))

    def test_extra_unconditional_sync_step_is_rejected(self):
        self.check_rejected(lambda data: data['sync.yaml']['pre_side_effect_freshness']['on_mismatch'].append('revoke_stale_approval'))

    def test_missing_required_field_is_rejected(self):
        self.check_rejected(lambda data: data['domain.yaml']['execution_contracts']['Approval']['fields'].pop('action_hash'))


if __name__ == '__main__':
    unittest.main()
