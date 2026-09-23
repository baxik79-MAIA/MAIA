"""M0.8 provider-neutral Round Table contract regressions."""
from pathlib import Path
import unittest
import yaml


ROOT = Path(__file__).resolve().parents[2]


class M08RoundTableContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.contract = yaml.safe_load(
            (ROOT / "spec" / "round_table.yaml").read_text(encoding="utf-8")
        )
        cls.testing = yaml.safe_load(
            (ROOT / "spec" / "testing.yaml").read_text(encoding="utf-8")
        )
        cls.acceptance = yaml.safe_load(
            (ROOT / "spec" / "acceptance.yaml").read_text(encoding="utf-8")
        )

    def test_provider_neutral_contract_has_no_core_provider_or_credential(self):
        participants = self.contract["participants"]
        self.assertTrue(participants["provider_neutral"])
        self.assertTrue(participants["model_configurable"])
        self.assertEqual(participants["hidden_provider_or_model_fallback"], "forbidden")
        self.assertIn("Participant", participants["type_contracts"])
        self.assertIn("ModelProvider", participants["type_contracts"])

    def test_independence_and_execution_authority_are_explicit(self):
        independence = self.contract["independence"]
        self.assertFalse(independence["first_round_request_contains_other_participant_responses"])
        self.assertTrue(independence["adjudication_receives_independent_responses"])
        self.assertTrue(independence["disagreement_explicit"])
        self.assertFalse(self.contract["assurance"]["a3_or_a4_increases_action_execution_authority"])
        self.assertTrue(
            self.contract["participants"]["type_contracts"]["RoundTableDecision"]
            ["execution_authority_must_be_false"]
        )

    def test_anthropic_boundary_and_live_smoke_are_local_and_opt_in(self):
        adapter = self.contract["anthropic_adapter"]
        self.assertEqual(adapter["boundary"], "infra_adapter_not_core")
        self.assertEqual(adapter["credential_source"], "local_secret_or_environment_boundary_only")
        self.assertFalse(adapter["organization_id_required"])
        self.assertEqual(adapter["other_provider_fallback"], "forbidden")
        smoke = self.contract["live_smoke"]
        self.assertEqual(smoke["default"], "skipped")
        self.assertEqual(smoke["credential_environment"], "ANTHROPIC_API_KEY")

    def test_acceptance_and_golden_vectors_cover_the_vertical_slice(self):
        expected = {
            "round_table_first_round_is_response_isolated",
            "round_table_decision_never_authorizes_action",
            "anthropic_adapter_has_no_hidden_provider_fallback",
            "anthropic_live_smoke_is_opt_in_and_credential_gated",
        }
        self.assertTrue(expected.issubset(set(self.testing["golden_vectors"])))
        ids = {
            item["id"]
            for item in self.acceptance["releases"]["m0_8_round_table_vertical_slice"]
        }
        self.assertEqual(ids, {"R001", "R002", "R003", "R004", "R005"})


if __name__ == "__main__":
    unittest.main()
