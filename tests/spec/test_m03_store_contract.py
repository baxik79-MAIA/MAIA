"""M0.3 structural guards for the OS-neutral store and SQLite adapter split."""

import pathlib
import tomllib
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

    def test_store_and_sqlite_trees_contain_no_ui_artifacts(self):
        # Inspect the current modules, including nested files, without Git.
        for module in ("core/store", "infra/sqlite"):
            for path in sorted((ROOT / module).rglob("*")):
                relative = path.relative_to(ROOT / module).as_posix().lower()
                with self.subTest(module=module, path=relative):
                    self.assertNotRegex(relative, r"tauri|react|dashboard")
                    self.assertNotIn(path.suffix.lower(), {
                        ".css", ".scss", ".sass", ".less", ".html",
                        ".js", ".jsx", ".ts", ".tsx", ".vue", ".svelte",
                    })
                    self.assertNotIn(path.name.lower(), {
                        "package.json", "package-lock.json", "yarn.lock",
                        "pnpm-lock.yaml", "node_modules",
                    })

    def test_store_and_sqlite_dependencies_preserve_headless_layer_boundaries(self):
        # ADR-0034: persistence stays below runtime and presentation. Briefing
        # contracts are also persisted now; runtime/executor/policy are allowed
        # only in the SQLite adapter's integration-test dependencies.
        store = {"maia-domain", "maia-briefing", "serde_json", "sha2"}
        sqlite = store | {"maia-store", "rusqlite", "fs2"}
        allowed = {
            "core/store": {"dependencies": store},
            "infra/sqlite": {
                "dependencies": sqlite,
                "dev-dependencies": {
                    "maia-executor", "maia-runtime", "maia-policy", "maia-briefing",
                },
            },
        }
        workspace = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
        for module, sections in allowed.items():
            directory = ROOT / module
            manifest = tomllib.loads((directory / "Cargo.toml").read_text(encoding="utf-8"))
            # Target-specific, optional and build dependencies cannot bypass
            # the boundary. An unlisted section has no allowed dependencies.
            tables = [("default", manifest), *manifest.get("target", {}).items()]
            for target, table in tables:
                for section in ("dependencies", "dev-dependencies", "build-dependencies"):
                    for alias, dependency in table.get(section, {}).items():
                        with self.subTest(module=module, target=target, section=section, dependency=alias):
                            details = dependency if isinstance(dependency, dict) else {}
                            base = directory
                            if details.get("workspace"):
                                dependency = workspace["workspace"]["dependencies"][alias]
                                details = dependency if isinstance(dependency, dict) else {}
                                base = ROOT
                            package = details.get("package", alias)
                            self.assertIn(package, sections.get(section, set()))
                            if package.startswith("maia-"):
                                self.assertIn("path", details)
                                self.assertEqual(
                                    (base / details["path"]).resolve(),
                                    (ROOT / "core" / package.removeprefix("maia-")).resolve(),
                                )
                            else:
                                self.assertNotIn("path", details)
                                self.assertNotIn("git", details)


if __name__ == "__main__":
    unittest.main()
