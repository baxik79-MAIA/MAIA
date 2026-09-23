from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "core" / "executor" / "Cargo.toml"
SOURCE = ROOT / "core" / "executor" / "src"


class M05ExecutorContractTests(unittest.TestCase):
    def test_manifest_has_only_allowed_core_dependencies(self):
        text = MANIFEST.read_text(encoding="utf-8")
        for dep in ("maia-domain", "maia-orchestrator", "maia-policy", "maia-runtime", "maia-store"):
            self.assertIn(dep, text)
        self.assertNotIn("infra/sqlite", text)
        self.assertNotIn("rusqlite", text)

    def test_executor_source_is_headless_and_one_shot(self):
        text = "\n".join(path.read_text(encoding="utf-8") for path in SOURCE.rglob("*.rs"))
        for forbidden in ("std::fs", "std::net", "std::process", "tokio", "rusqlite", "while true", "loop {"):
            self.assertNotIn(forbidden, text)
        for symbol in ("ActionExecutor", "ExecutionRequest", "EffectSucceeded", "EffectFailedFinal", "EffectFailedRetryable", "OutcomeUnknown", "recover_incomplete_runs"):
            self.assertIn(symbol, text)

    def test_workspace_includes_executor(self):
        text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
        self.assertIn('"core/executor"', text)


if __name__ == "__main__":
    unittest.main()
