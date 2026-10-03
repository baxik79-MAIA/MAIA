import copy
import sys
import unittest
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
from evolution_tier0_contract import validate

class Tier0ContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.spec = yaml.safe_load((ROOT / "spec/evolution_tier0.yaml").read_text())

    def test_canonical_contract_passes(self):
        self.assertEqual(validate(self.spec), [])

    def test_step_reordering_fails(self):
        altered = copy.deepcopy(self.spec)
        altered["ordered_checks"][0], altered["ordered_checks"][1] = (
            altered["ordered_checks"][1], altered["ordered_checks"][0]
        )
        self.assertTrue(validate(altered))

    def test_weakening_boundary_fails(self):
        for key, value in [
            ("candidate_worktree_before_admission", "allowed"),
            ("active_evolvable_allowlist", "worker_selected"),
            ("admitted_grants_mutation_authority", True),
        ]:
            with self.subTest(key=key):
                altered = copy.deepcopy(self.spec)
                altered["boundaries"][key] = value
                self.assertTrue(validate(altered))

    def test_missing_rejection_evidence_fails(self):
        altered = copy.deepcopy(self.spec)
        altered["rejection_record"]["evidence_refs"] = False
        self.assertTrue(validate(altered))

if __name__ == "__main__":
    unittest.main()
