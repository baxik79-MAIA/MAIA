"""Rust generator regressions: canonical fixtures remain in memory, no Cargo needed."""
from copy import deepcopy
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('generate_rust_domain', ROOT / 'tools/generate_rust_domain.py')
GEN = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GEN)


class RustGeneratorTests(unittest.TestCase):
    def setUp(self):
        self.data = GEN.load()
        self.content = GEN.render(self.data)

    def test_checked_in_and_deterministic(self):
        self.assertEqual(self.content, GEN.render(deepcopy(self.data)))
        self.assertTrue(GEN.check(ROOT, self.content))
        self.assertTrue(self.content.startswith('// GENERATED / DO NOT EDIT'))
        self.assertNotIn(str(ROOT), self.content)

    def test_enum_drift(self):
        for file, key in [('approval.yaml', 'risk_classes'), ('approval.yaml', 'gate_types'), ('modes.yaml', 'privacy_classes')]:
            with self.subTest(file=file, key=key):
                data = deepcopy(self.data)
                data[file][key]['future_value'] = 'fixture'
                content = GEN.render(data)
                self.assertIn('FutureValue => "future_value"', content)
                self.assertFalse(GEN.check(ROOT, content))

    def test_risk_declaration_order(self):
        values = self.data['approval.yaml']['risk_classes']
        self.data['approval.yaml']['risk_classes'] = dict(reversed(list(values.items())))
        self.assertFalse(GEN.check(ROOT, GEN.render(self.data)))

    def test_state_and_edge_drift(self):
        for machine in ('AgentTaskState', 'ActionState', 'RunState', 'ApprovalState'):
            with self.subTest(machine=machine):
                data = deepcopy(self.data)
                fsm = data['states.yaml']['machines'][machine]
                fsm['states'].append('future_state')
                self.assertFalse(GEN.check(ROOT, GEN.render(data)))
                data = deepcopy(self.data)
                fsm = data['states.yaml']['machines'][machine]
                fsm['transitions'][fsm['states'][0]] = [fsm['states'][0]]
                self.assertFalse(GEN.check(ROOT, GEN.render(data)))

    def test_field_inventory_and_requiredness_drift(self):
        contracts = self.data['domain.yaml']['execution_contracts']
        for name, contract in contracts.items():
            for field in contract['fields']:
                with self.subTest(entity=name, field=field):
                    data = deepcopy(self.data)
                    del data['domain.yaml']['execution_contracts'][name]['fields'][field]
                    self.assertFalse(GEN.check(ROOT, GEN.render(data)))
        for key, value in [('required', False), ('nullable', True), ('type', 'OpaqueRef')]:
            data = deepcopy(self.data)
            data['domain.yaml']['execution_contracts']['AgentTask']['fields']['request_text'][key] = value
            self.assertFalse(GEN.check(ROOT, GEN.render(data)))

    def test_canonical_coupling_and_edges_emitted(self):
        coupling = self.data['domain.yaml']['value_objects']['OutcomeCertainty']['state_coupling']
        for state, certainty in coupling.items():
            self.assertIn(f'Self::{GEN.variant(state)} => OutcomeCertainty::{GEN.variant(certainty)},', self.content)
        for name, meta in self.data['domain.yaml']['primitives'].items():
            if 'transitions_source' not in meta:
                continue
            block = self.content.split(f'canonical_fsm!({name} {{\n')[1].split('});')[0]
            expected = [f'    {GEN.variant(src)} => {GEN.variant(dst)},' for src, dests in self.data['states.yaml']['machines'][name]['transitions'].items() for dst in dests]
            self.assertEqual(block.splitlines(), expected)
        data = deepcopy(self.data)
        data['domain.yaml']['value_objects']['OutcomeCertainty']['state_coupling']['created'] = 'known'
        self.assertFalse(GEN.check(ROOT, GEN.render(data)))

    def test_missing_generated_file_fails_closed(self):
        with patch.object(Path, 'exists', return_value=False):
            self.assertFalse(GEN.check(ROOT, self.content))

    def test_unknown_type_is_rejected(self):
        self.data['domain.yaml']['execution_contracts']['Run']['fields']['id']['type'] = 'Unknown'
        with self.assertRaises(ValueError):
            GEN.render(self.data)

    def test_execution_predicates_and_order_follow_canonical_sources(self):
        for file,path in [('approval.yaml',('policy_decision','values')),
                          ('approval.yaml',('assurance','values')),
                          ('execution.yaml',('freshness','values'))]:
            data=deepcopy(self.data)
            target=data[file]
            for key in path[:-1]: target=target[key]
            target[path[-1]]=list(reversed(target[path[-1]]))
            self.assertFalse(GEN.check(ROOT,GEN.render(data)))
        data=deepcopy(self.data)
        data['states.yaml']['pause_semantics']['in_flight_states'].append('created')
        self.assertIn('Self::Created)',GEN.render(data))
        self.assertFalse(GEN.check(ROOT,GEN.render(data)))


if __name__ == '__main__':
    unittest.main()
