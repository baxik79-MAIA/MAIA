"""M0.6 canonical guards for bounded immutable workspace evidence."""
from pathlib import Path
import unittest
import yaml


ROOT = Path(__file__).resolve().parents[2]


class M06EvidenceContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.domain = yaml.safe_load((ROOT / "spec/domain.yaml").read_text(encoding="utf-8"))
        cls.persistence = yaml.safe_load((ROOT / "spec/persistence.yaml").read_text(encoding="utf-8"))
        cls.audit = yaml.safe_load((ROOT / "spec/audit.yaml").read_text(encoding="utf-8"))
        cls.product = yaml.safe_load((ROOT / "spec/product.yaml").read_text(encoding="utf-8"))

    def test_product_and_artifact_contract_are_additive_and_typed(self):
        self.assertEqual(self.product["product"]["version"], "1.9.0")
        fields = self.domain["artifact_contracts"]["Artifact"]["fields"]
        self.assertEqual(
            set(fields),
            {"id", "workspace_id", "kind", "name", "media_type", "content",
             "content_sha256", "byte_length", "source_locator", "source_locator_hash",
             "import_request_id", "trust", "classification", "created_at"},
        )
        self.assertEqual(self.domain["artifact_snapshot"]["artifact_kinds"], ["imported_text_evidence"])
        self.assertEqual(self.domain["artifact_snapshot"]["media_types"], ["utf8_plain_text", "markdown"])

    def test_snapshot_and_citation_never_depend_on_later_source_state(self):
        representation = self.domain["artifact_snapshot"]["content_representation"]
        self.assertEqual(representation["decoding"], "strict_UTF-8")
        self.assertEqual(representation["newline_normalization"], "forbidden")
        artifact_rules = self.domain["artifact_contracts"]["Artifact"]["rules"]
        self.assertTrue(artifact_rules["content_is_MAIA_owned_snapshot"])
        self.assertFalse(artifact_rules["source_modification_or_deletion_changes_snapshot"])
        citation = self.domain["artifact_contracts"]["EvidenceCitation"]["rules"]
        self.assertFalse(citation["external_source_locator_is_citation_identity"])
        self.assertEqual(citation["line_positions_reference"], "immutable_MAIA_snapshot")

    def test_t6_is_bounded_atomic_and_does_not_create_derived_knowledge(self):
        contract = self.persistence["workspace_evidence_import"]
        self.assertEqual(contract["bounds"], {
            "maximum_files_per_request": 8,
            "maximum_file_bytes": 262144,
            "maximum_total_bytes": 1048576,
        })
        self.assertTrue(contract["input_boundary"]["caller_supplies_each_path_explicitly"])
        self.assertEqual(contract["input_boundary"]["UNC_or_network_paths"], "forbidden")
        self.assertTrue(contract["input_boundary"]["symlink_junction_reparse_points"] == "forbidden")
        t6 = self.persistence["atomic_transactions"]["T6_workspace_evidence_import"]
        self.assertTrue(t6["atomic_all_or_nothing"])
        self.assertEqual(t6["partial_artifacts_on_failure"], "forbidden")
        boundary = self.persistence["aggregate_boundaries"]["workspace_evidence_snapshot"]
        self.assertFalse(boundary["creates_memory_claims"])
        self.assertFalse(boundary["creates_workgraph_edges"])

    def test_audit_excludes_snapshot_text_and_raw_locator(self):
        forbidden = self.audit["record"]["privacy"]["forbidden_by_default"]
        self.assertIn("evidence_snapshot_content", forbidden)
        self.assertIn("source_locator", forbidden)
        events = self.audit["evidence_snapshot_audit"]["event_types"]
        self.assertEqual(events, [
            "workspace_evidence_imported",
            "workspace_evidence_import_idempotent",
            "workspace_evidence_import_refused",
        ])


if __name__ == "__main__":
    unittest.main()
