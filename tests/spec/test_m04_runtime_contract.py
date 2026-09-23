"""M0.4 guards for the pure runtime/store composition boundary."""

import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]


class M04RuntimeContractTests(unittest.TestCase):
    def test_runtime_depends_on_ports_not_sqlite(self):
        manifest = (ROOT / "core/runtime/Cargo.toml").read_text(encoding="utf-8")
        source = "\n".join(
            path.read_text(encoding="utf-8")
            for path in (ROOT / "core/runtime/src").glob("*.rs")
        )
        self.assertIn('maia-domain = { path = "../domain" }', manifest)
        self.assertIn('maia-policy = { path = "../policy" }', manifest)
        self.assertIn('maia-orchestrator = { path = "../orchestrator" }', manifest)
        self.assertIn('maia-store = { path = "../store" }', manifest)
        self.assertNotIn("rusqlite", manifest)
        self.assertNotIn("infra/sqlite", source)
        self.assertNotIn("std::fs", source)

    def test_runtime_is_registered_as_workspace_layer(self):
        workspace = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
        self.assertIn('"core/runtime"', workspace)

    def test_no_side_effect_runtime_shortcuts(self):
        source = "\n".join(
            path.read_text(encoding="utf-8")
            for path in (ROOT / "core/runtime/src").glob("*.rs")
        ).lower()
        for forbidden in ("std::process", "tokio", "std::net", "std::thread"):
            self.assertNotIn(forbidden, source)


if __name__ == "__main__":
    unittest.main()
