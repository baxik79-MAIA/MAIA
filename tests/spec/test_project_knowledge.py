from pathlib import Path
import subprocess
import unittest
import importlib.util
import tempfile
import hashlib
import json
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
PROJECT = ROOT / "docs" / "project"
HANDBOOK = [f"{index:02d}_{name}.md" for index, name in enumerate([
    "START_HERE", "CURRENT_STATE", "ARCHITECTURE", "ARCHITECTURE_INVARIANTS",
    "MILESTONE_LEDGER", "CURRENT_ROADMAP", "AI_WORKFLOW", "PARALLEL_DEVELOPMENT",
    "INTELLIGENCE_FABRIC", "ROUND_TABLE", "PRIVACY_AND_EGRESS", "DEV_ENVIRONMENT",
    "DECISION_INDEX", "COMMERCIAL_READINESS", "PRODUCT_VISION_AND_USER_PROFILES",
    "COMPETITIVE_BENCHMARK_VICTOR", "PERSONA_AND_INTERACTION_MODEL",
])]


spec = importlib.util.spec_from_file_location("knowledge", ROOT / "tools/update_project_knowledge.py")
knowledge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(knowledge)


class ProjectKnowledgeTests(unittest.TestCase):
    def test_required_handbook_and_reference_files_exist(self):
        for name in HANDBOOK:
            self.assertTrue((PROJECT / name).is_file(), name)
        self.assertTrue((PROJECT / "directives" / "README.md").is_file())
        self.assertFalse((PROJECT / "milestone-reports").exists())

    def test_updater_check_passes(self):
        result = subprocess.run(
            ["python", "tools/update_project_knowledge.py", "--checkout-check"],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_checkout_check_rejects_integration_sync(self):
        result = subprocess.run(
            ["python", "tools/update_project_knowledge.py", "--checkout-check", "--sync-shared"],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("cannot sync integration mirrors", result.stderr)

    def test_integration_mirror_validation_detects_stale_content(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            shared, export = root / "shared", root / "export"
            archive = root / "MAIA_Project_Knowledge.zip"
            with patch.object(knowledge, "SHARED_ROOT", shared), \
                 patch.object(knowledge, "EXPORT_ROOT", export), \
                 patch.object(knowledge, "EXPORT_ZIP", archive):
                knowledge.sync_tree(shared)
                knowledge.sync_export()
                self.assertEqual(knowledge.check_tree(shared), [])
                self.assertEqual(knowledge.check_tree(export), [])
                self.assertEqual(knowledge.check_zip(), [])
                (shared / "00_START_HERE.md").write_bytes(b"stale")
                self.assertIn(f"mirror differs: {shared / '00_START_HERE.md'}", knowledge.check_tree(shared))

    def test_public_source_excludes_private_import_provenance(self):
        # Private import provenance is excluded from the public baseline.
        self.assertFalse((PROJECT / "reference/source_provenance.json").exists())
        self.assertTrue((ROOT / "spec/product.yaml").is_file())
        self.assertTrue((ROOT / "docs/adr/ADR-0046.md").is_file())

    def test_snapshot_drift_and_unknown_file_preservation(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            note = root / "user-note.txt"
            note.write_text("keep me")
            with patch.object(knowledge, "expected_files", return_value={Path("state.md"): b"head A"}):
                knowledge.sync_tree(root)
                self.assertEqual(knowledge.check_tree(root), [])
            with patch.object(knowledge, "expected_files", return_value={Path("state.md"): b"head B"}):
                self.assertTrue(knowledge.check_tree(root))
                knowledge.sync_tree(root)
                self.assertEqual(knowledge.check_tree(root), [])
            self.assertEqual(note.read_text(), "keep me")
            with self.assertRaises(SystemExit):
                knowledge.safe_target(root, Path("../escape"))

    def test_generator_does_not_rewrite_tracked_sources(self):
        before = {p: p.read_bytes() for p in PROJECT.rglob("*") if p.is_file()}
        with tempfile.TemporaryDirectory() as tmp:
            knowledge.sync_tree(Path(tmp).resolve())
        self.assertEqual(before, {p: p.read_bytes() for p in before})

    def test_start_here_authority_and_local_first_rules(self):
        text = (PROJECT / "00_START_HERE.md").read_text(encoding="utf-8")
        self.assertIn("spec/*.yaml", text)
        self.assertIn("Provider chat/session history is never canonical project knowledge", text)
        self.assertIn("contradiction", text)
        invariants = (PROJECT / "03_ARCHITECTURE_INVARIANTS.md").read_text(encoding="utf-8")
        self.assertIn("MAIA remains useful with all external providers disabled", invariants)
        self.assertIn("Models are replaceable compute", invariants)

    def test_round_table_is_subsystem_and_deferred(self):
        text = (PROJECT / "09_ROUND_TABLE.md").read_text(encoding="utf-8")
        self.assertIn("reusable multi-model consultation engine", text)
        self.assertIn("subsystem", text)
        self.assertIn("DEFERRED", text)

    def test_canonical_authority_order_is_explicit(self):
        text = (PROJECT / "00_START_HERE.md").read_text(encoding="utf-8")
        self.assertLess(text.index("spec/*.yaml"), text.index("docs/adr"))
        self.assertLess(text.index("docs/adr"), text.index("generated artifacts"))
        self.assertIn("narrative project documentation", text)


if __name__ == "__main__":
    unittest.main()
