"""M0.3 structural guards for the OS-neutral store and SQLite adapter split."""

import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]


class M03StoreContractTests(unittest.TestCase):
    def test_store_has_no_adapter_dependency_or_sql(self):
        manifest = (ROOT / "core/store/Cargo.toml").read_text(encoding="utf-8")
        source = (ROOT / "core/store/src/lib.rs").read_text(encoding="utf-8")
        self.assertNotIn("rusqlite", manifest)
        self.assertNotIn("sqlite", source.lower())
        self.assertNotIn("std::fs", source)

    def test_sqlite_adapter_owns_allowed_dependencies(self):
        manifest = (ROOT / "infra/sqlite/Cargo.toml").read_text(encoding="utf-8")
        self.assertIn('rusqlite = { version = "0.32", features = ["bundled", "backup"] }', manifest)
        self.assertIn('fs2 = "0.4"', manifest)

    def test_schema_keeps_cas_and_audit_fields_as_columns(self):
        schema = (ROOT / "infra/sqlite/migrations/0001_initial.sql").read_text(encoding="utf-8")
        for field in ("version", "state", "action_hash", "attempt", "approval_version", "sequence", "prev_hash", "record_hash"):
            self.assertIn(field, schema)
        self.assertIn("UNIQUE (action_id, action_version, attempt)", schema)
        self.assertIn("migration_ledger", schema)

    def test_forbidden_runtime_layers_are_absent_from_m03_changes(self):
        changed = "\n".join(
            __import__("subprocess").check_output(
                ["git", "diff", "--name-only", "HEAD^"], cwd=ROOT, text=True
            ).splitlines()
        )
        self.assertNotRegex(changed.lower(), r"tauri|react|dashboard|\.css$")


if __name__ == "__main__":
    unittest.main()
