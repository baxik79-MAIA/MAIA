import sys
import unittest
from copy import deepcopy
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
from evolution_workspace_contract import validate  # noqa: E402


class EvolutionWorkspaceContractTests(unittest.TestCase):
    def setUp(self):
        self.spec = yaml.safe_load((ROOT / "spec/evolution_workspace.yaml").read_text(encoding="utf-8"))

    def test_contract_is_valid(self):
        self.assertEqual(validate(self.spec), [])

    def test_admission_and_isolation_cannot_be_weakened(self):
        for path, value in [
            (("admission", "required_outcome"), "REJECTED"),
            (("admission", "exact_plan_binding_revalidated_by_protected_host"), False),
            (("host", "candidate_scoped_cleanup"), "global"),
            (("authority_limits", "deployment_locked_allocation"), "allowed"),
            (("lifecycle", "rollback_baseline_implied"), True),
        ]:
            changed = deepcopy(self.spec)
            changed[path[0]][path[1]] = value
            with self.subTest(path=path):
                self.assertTrue(validate(changed))
