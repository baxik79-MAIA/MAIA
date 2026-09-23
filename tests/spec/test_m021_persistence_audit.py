"""M0.2.1 persistence/audit contract oracles and store-layer boundary checks."""
from pathlib import Path
import importlib.util
import unittest

import yaml

ROOT = Path(__file__).resolve().parents[2]
SPEC = ROOT / "spec"


def load(name):
    return yaml.safe_load((SPEC / name).read_text(encoding="utf-8"))


class PersistenceAuditContractTests(unittest.TestCase):
    def test_versioned_runtime_records_are_distinct_from_revisions(self):
        domain = load("domain.yaml")
        task = domain["execution_contracts"]["AgentTask"]
        run = domain["execution_contracts"]["Run"]
        self.assertEqual(task["fields"]["version"]["type"], "Version")
        self.assertEqual(run["fields"]["version"]["type"], "Version")
        self.assertEqual(task["rules"]["version_semantics"], "mutable_record_cas_version")
        self.assertIn("not_attempt_or_action_revision", run["rules"]["version_semantics"])
        self.assertEqual(domain["execution_contracts"]["Action"]["fields"]["version"]["type"], "Version")

    def test_t1_to_t5_and_authority_guarantees_are_canonical(self):
        persistence = load("persistence.yaml")
        authority = persistence["authoritative_store"]
        self.assertTrue(authority["exactly_one_mutable_authority_per_workspace"])
        self.assertTrue(authority["authority_changing_write_transactions_serialized"])
        tx = persistence["atomic_transactions"]
        self.assertEqual(set(tx), {"authority_boundary", "T1_material_action_revision", "T2_approval_decision_cas", "T3_run_creation", "T4_run_transition_evidence", "T5_task_pause", "T6_workspace_evidence_import"})
        self.assertFalse(tx["T1_material_action_revision"]["run_creation_included"])
        self.assertFalse(tx["T3_run_creation"]["action_approval_creation_same_transaction_required"])
        self.assertEqual(tx["T5_task_pause"]["race_guards"], [
            "run_creation_rechecks_current_task_state",
            "created_to_starting_rechecks_current_task_state",
        ])

    def test_audit_genesis_and_hash_projection_contract(self):
        audit = load("audit.yaml")
        self.assertEqual(audit["record"]["identity"], ["workspace_id", "sequence"])
        self.assertTrue(audit["record"]["append_only"])
        self.assertEqual(audit["hash"]["algorithm"], "SHA-256")
        self.assertEqual(audit["hash"]["canonicalization"], "RFC8785-JCS")
        self.assertEqual(audit["hash"]["excluded_fields"], ["record_hash"])
        genesis = audit["chain"]["genesis"]
        self.assertEqual(genesis["sequence"], 1)
        self.assertEqual(genesis["prev_hash"], "0" * 64)
        self.assertTrue(audit["chain"]["sequence"]["allocated_atomically_by_authoritative_store"])

    def test_generated_rust_contains_audit_record_and_cas_fields(self):
        generated = (ROOT / "core/domain/src/generated/contracts.rs").read_text(encoding="utf-8")
        self.assertIn("integer_type!(AuditSequence, u64, 1);", generated)
        self.assertIn("domain_struct!(AuditRecord {", generated)
        self.assertIn("domain_struct!(AgentTask {", generated)
        self.assertIn("domain_struct!(Run {", generated)
        self.assertIn("version: Version,", generated)

    def test_store_contract_stays_os_neutral_and_adapter_is_explicit(self):
        store_manifest = (ROOT / "core/store/Cargo.toml").read_text(encoding="utf-8")
        self.assertNotIn("rusqlite", store_manifest)
        self.assertNotIn("filesystem", store_manifest.lower())
        adapter_manifest = (ROOT / "infra/sqlite/Cargo.toml").read_text(encoding="utf-8")
        self.assertIn("rusqlite", adapter_manifest)


if __name__ == "__main__":
    unittest.main()
