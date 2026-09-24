"""Canonical M0.16.2 candidate allocation boundary guard."""
FIELDS = ["candidate_workspace_id", "generation_id", "hypothesis_id", "parent_id", "parent_kind", "parent_snapshot_id", "parent_source_commit", "declared_operator", "proposed_paths", "proposed_symbols", "estimated_changed_lines", "max_changed_lines", "max_files", "semantic_fingerprint", "implementation_fingerprint", "profiling_fingerprint", "evidence_refs", "reservation_id", "created_sequence"]
EXPECTED = {
    "schema_version": 1,
    "milestone": "M0.16.2",
    "authority": "docs/project/directives/MAIA_Architecture_Amendment_Autonomous_Evolution_Deployment_Lock_v5.1.md#3.0.2",
    "placement": {"policy": "ops/evolution-supervisor", "host_adapter": "ops/evolution-workspace-host", "core_dependency": "forbidden", "shipped_application_dependency": "forbidden"},
    "admission": {"required_outcome": "ADMITTED", "exact_plan_binding_revalidated_by_protected_host": True, "supervisor_ready_at_allocation": True, "parent_identity_and_source_reverified": True, "reservation_live_and_bound": True, "protected_paths_rechecked": True, "unknown_or_adapter_error": "fail_closed"},
    "identity_fields": FIELDS,
    "lifecycle": {"states": ["REQUESTED", "ALLOCATED", "ACTIVE", "CLOSED"], "transitions": {"REQUESTED": ["ALLOCATED", "CLOSED"], "ALLOCATED": ["ACTIVE", "CLOSED"], "ACTIVE": ["CLOSED"], "CLOSED": []}, "terminal_outcomes": ["REJECTED", "CANCELLED", "INFRA_ERROR"], "candidate_failure_operation": "discard_candidate", "rollback_baseline_implied": False},
    "host": {"workspace_root": "dedicated_maia_controlled_candidate_root", "path_or_identity_collision": "reject", "canonical_main_and_other_candidates": "untouched", "candidate_scoped_cleanup": "deterministic_idempotent", "evidence_preserved_before_cleanup": True, "no_worker_execution_in_milestone": True},
    "authority_limits": {"protected_surface_write": "forbidden", "supervisor_write": "forbidden", "git_commit_push_promotion": "forbidden", "release_authority": "forbidden", "deployment_locked_allocation": "forbidden", "provider_calls": "forbidden", "source_mutation_logic": "forbidden"},
}
def validate(spec):
    errors = []
    def check(actual, expected, path):
        if isinstance(expected, dict):
            if not isinstance(actual, dict):
                errors.append(f"evolution_workspace.yaml:{path}: mapping required")
                return
            for key, value in expected.items():
                check(actual.get(key), value, f"{path}.{key}" if path else key)
        elif type(actual) is not type(expected) or actual != expected:
            errors.append(f"evolution_workspace.yaml:{path}: expected {expected!r}, got {actual!r}")
    check(spec, EXPECTED, "")
    return errors
