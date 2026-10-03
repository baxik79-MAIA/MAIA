"""Adversarial checks for the M0.16.0 contract guard."""
import copy
import sys
import unittest
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
from evolution_supervisor_contract import validate  # noqa: E402


class ContractGuardTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.contract = yaml.safe_load(
            (ROOT / "spec" / "evolution_supervisor.yaml").read_text(encoding="utf-8")
        )

    def test_current_contract(self):
        self.assertEqual(validate(self.contract), [])

    def test_mutation_and_release_escalation_rejected(self):
        for path, value in [
            (("preflight", "mutation"), "allowed"),
            (("preflight", "success_grants_mutation_authority"), True),
            (("protected_surfaces", "evolvable_allowlist"), ["core/"]),
            (("release_boundary", "locked_artifact_can_reenable_evolution"), True),
            (("placement", "shipped_application_dependency"), "allowed"),
            (("authority", "supervisor_owns"), self.contract["authority"]["supervisor_owns"] + ["release_signing"]),
        ]:
            with self.subTest(path=path):
                contract = copy.deepcopy(self.contract)
                contract[path[0]][path[1]] = value
                self.assertTrue(validate(contract))


if __name__ == "__main__":
    unittest.main()
