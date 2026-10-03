import copy
import sys
import unittest
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
from evolution_mutation_tier1_contract import EXPECTED, validate  # noqa: E402


class EvolutionMutationTier1ContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.spec = yaml.safe_load(
            (ROOT / "spec/evolution_mutation_tier1.yaml").read_text(encoding="utf-8")
        )

    def test_contract_is_valid(self):
        self.assertEqual(validate(self.spec), [])

    def test_safety_boundaries_fail_closed(self):
        for path, value in [
            (("mutation", "tier0_outcome_required"), "ADMITTED_OR_UNKNOWN"),
            (("evolvable_surface", "paths"), ["."]),
            (("evolvable_surface", "absent_or_unclassified"), "allowed"),
            (("human_governance", "approval_verified_by_protected_host"), False),
            (("tier1", "success_grants_promotion"), True),
            (("authority_limits", "git_commit_push_merge_or_protected_ref_change"), "allowed"),
            (("state", "rollback_baseline_implied"), True),
        ]:
            altered = copy.deepcopy(self.spec)
            altered[path[0]][path[1]] = value
            with self.subTest(path=path):
                self.assertTrue(validate(altered))

    def test_spec_allowlist_is_single_explicit_product_source(self):
        self.assertEqual(self.spec["evolvable_surface"]["paths"], EXPECTED["evolvable_surface"]["paths"])
        self.assertNotIn("ops/", " ".join(self.spec["evolvable_surface"]["paths"]))


if __name__ == "__main__":
    unittest.main()
