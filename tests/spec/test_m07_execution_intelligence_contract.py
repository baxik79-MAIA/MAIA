"""M0.7 additive execution-intelligence contract regressions."""
from pathlib import Path
import unittest
import yaml


ROOT = Path(__file__).resolve().parents[2]


class M07ExecutionIntelligenceContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        def load(name):
            return yaml.safe_load((ROOT / "spec" / name).read_text(encoding="utf-8"))
        cls.product = load("product.yaml")
        cls.domain = load("domain.yaml")
        cls.reasoning = load("reasoning.yaml")
        cls.outcomes = load("outcomes.yaml")
        cls.commercial = load("commercial.yaml")
        cls.metrics = load("metrics.yaml")
        cls.testing = load("testing.yaml")

    def test_product_wedge_is_replaceable_and_not_core(self):
        wedge = self.product["validation_hypotheses"]["initial_wedge"]
        self.assertEqual(self.product["product"]["version"], "1.9.0")
        self.assertEqual(wedge["id"], "microsoft_365_execution_control")
        self.assertTrue(wedge["replaceable"])
        self.assertTrue(wedge["does_not_define_core"])

    def test_outcome_is_not_run_outcome_certainty_and_verified_needs_evidence(self):
        self.assertIn("Outcome", self.domain["entities"])
        self.assertEqual(
            self.outcomes["lifecycle"]["statuses"],
            ["candidate", "observed", "verified", "rejected", "disputed", "superseded"],
        )
        rules = self.outcomes["rules"]
        self.assertTrue(rules["business_outcome_is_not_run_outcome_certainty"])
        self.assertTrue(rules["verified_requires_sufficient_evidence"])
        self.assertFalse(rules["model_confidence_alone_promotes_to_verified"])
        self.assertIn("OutcomeCertainty", self.domain["value_objects"])

    def test_reasoning_assurance_is_orthogonal_and_cannot_authorize_execution(self):
        levels = self.reasoning["assurance_levels"]
        self.assertEqual(set(levels), {"A0", "A1", "A2", "A3", "A4"})
        self.assertEqual(levels["A3"]["symbol"], "round_table")
        self.assertIn("assurance_level_never_grants_execution_permission", self.reasoning["hard_rules"])
        self.assertIn("approval_gate_bypass", self.reasoning["round_table"]["forbidden_shortcuts"])
        self.assertFalse(self.reasoning["a4_human_confirmation"]["substitutes_for_approval_gate"])

    def test_unknown_cost_and_human_effort_are_not_zero(self):
        self.assertTrue(self.commercial["usage"]["cost_rules"]["unknown_forbids_implied_zero"])
        self.assertEqual(
            self.metrics["metrics"]["cost_per_successful_outcome"]["unknown_component_behavior"],
            "preserve_unknown_never_assume_zero",
        )
        self.assertTrue(self.commercial["human_intervention"]["observable_or_explicitly_recorded_only"])

    def test_local_workspace_and_commercial_governance_seams(self):
        scopes = self.commercial["workspace_tenant_scope"]
        self.assertFalse(scopes["personal_local"]["tenant_required"])
        self.assertTrue(scopes["commercial_organizational"]["tenant_required"])
        gates = self.commercial["commercial_readiness_gates"]
        self.assertEqual(gates["G2"]["name"], "FORM COMPANY NOW")
        self.assertIn("non_runtime", gates["G3"]["kind"])
        self.assertIn("non_runtime", gates["G4"]["kind"])

    def test_required_golden_vectors_protect_m07_and_m06_boundaries(self):
        expected = {
            "outcome_certainty_is_not_business_outcome",
            "reasoning_assurance_cannot_lower_action_risk",
            "round_table_cannot_authorize_execution",
            "a4_human_reasoning_confirmation_cannot_substitute_for_approval_gate",
            "unknown_cost_is_not_zero",
            "unknown_human_intervention_is_not_zero",
            "verified_outcome_requires_evidence",
            "m365_wedge_does_not_become_core_boundary",
            "personal_workspace_does_not_require_tenant",
            "commercial_organizational_mode_requires_tenant",
            "g2_precedes_paid_production_or_external_customer_production_data",
            "g3_g4_are_not_task_or_action_fsm_states",
            "diagnostic_or_noop_event_does_not_qualify_for_ttfa",
            "generic_model_output_does_not_qualify_for_ttfv",
            "m06_artifact_snapshot_contract_remains_unchanged",
        }
        self.assertTrue(expected.issubset(set(self.testing["golden_vectors"])))


if __name__ == "__main__":
    unittest.main()
