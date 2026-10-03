import copy
import sys
import unittest
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
from evolution_protected_runtime_contract import EXPECTED, validate  # noqa: E402


class EvolutionProtectedRuntimeContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.spec = yaml.safe_load(
            (ROOT / "spec/evolution_protected_runtime.yaml").read_text(encoding="utf-8")
        )

    def test_contract_is_valid(self):
        self.assertEqual(validate(self.spec), [])

    def test_protected_boundary_mutations_are_rejected(self):
        for path, value in [
            (("protected_state", "candidate_may_supply_or_change"), True),
            (("protected_state", "deployment_locked"), "allow"),
            (("protected_state", "startup_defaults"), {}),
            (("kill_switch", "running_verifier"), "continue"),
            (("containment", "breakaway"), "allowed"),
            (("containment", "network"), "host_network"),
            (("resource_limits", "candidate_controls_limits"), True),
            (("journal", "lock_before_chain_validation"), False),
            (("outcomes", "cancellation_is_test_failure"), True),
            (("outcomes", "isolation_unavailable_terminal_state"), "REJECTED"),
            (("authority_limits", "git_commit_push_merge_or_protected_ref_change"), "allowed"),
        ]:
            altered = copy.deepcopy(self.spec)
            altered[path[0]][path[1]] = value
            with self.subTest(path=path):
                self.assertTrue(validate(altered))

    def test_resource_and_evolvable_limits_are_exact(self):
        self.assertEqual(self.spec["resource_limits"], EXPECTED["resource_limits"])
        self.assertEqual(
            self.spec["authority_limits"]["evolvable_paths"],
            ["apps/local-intelligence-host/src/lib.rs"],
        )

    def test_startup_and_missing_isolation_fail_closed(self):
        self.assertEqual(
            self.spec["protected_state"]["startup_defaults"],
            EXPECTED["protected_state"]["startup_defaults"],
        )
        self.assertEqual(
            self.spec["containment"]["minimum_tier1_level"],
            "restricted_identity_network_and_filesystem",
        )
        self.assertFalse(EXPECTED["containment"]["minimum_tier1_level"] == "same_user_process_tree_contained")

    def test_protected_decision_rechecks_cover_all_required_phases(self):
        self.assertEqual(self.spec["gate_decision"]["revalidate_before"], ["mutation", "tier1_start"])
        self.assertEqual(self.spec["gate_decision"]["revalidate_after"], ["tier1"])
        self.assertIn("kill_switch_change", self.spec["gate_decision"]["invalidated_by"])


if __name__ == "__main__":
    unittest.main()
