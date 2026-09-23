"""M0.9 assurance-routing contract regressions."""
from pathlib import Path
import unittest
import yaml

ROOT = Path(__file__).resolve().parents[2]
class M09AssuranceRoutingContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.contract = yaml.safe_load((ROOT / "spec" / "assurance_routing.yaml").read_text(encoding="utf-8"))["contract"]
        cls.testing = yaml.safe_load((ROOT / "spec" / "testing.yaml").read_text(encoding="utf-8"))
        cls.acceptance = yaml.safe_load((ROOT / "spec" / "acceptance.yaml").read_text(encoding="utf-8"))
    def test_authority_and_router_invariants(self):
        self.assertEqual(self.contract["authority"]["execution_authority"], "forbidden")
        self.assertTrue(self.contract["router"]["policy_can_raise_but_not_lower"])
        self.assertEqual(self.contract["router"]["hidden_downgrade"], "forbidden")
    def test_paths_registry_budget_and_failures_are_complete(self):
        self.assertEqual(set(self.contract["paths"]), {"A0", "A1", "A2", "A3", "A4"})
        self.assertTrue(self.contract["participant_registry"]["provider_neutral"])
        self.assertEqual(self.contract["budget"]["unknown_cost"], "unknown_not_zero")
        self.assertEqual(self.contract["failures"]["silent_downgrade"], "forbidden")
        self.assertEqual(self.contract["failures"]["result"], "insufficient_assurance")
    def test_acceptance_and_golden_vectors_cover_routing(self):
        self.assertEqual({x["id"] for x in self.acceptance["releases"]["m0_9_assurance_routing"]}, {f"Q00{i}" for i in range(1, 7)})
        self.assertIn("assurance_router_budget_failure_is_insufficient_assurance", self.testing["golden_vectors"])
if __name__ == "__main__": unittest.main()
