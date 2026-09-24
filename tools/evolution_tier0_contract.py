"""Canonical M0.16.1 Tier 0 boundary guard."""
ORDER = [
    "kill_switch_and_supervisor_state",
    "hypothesis_admission_completeness",
    "refuted_or_recently_rejected_ledger_hit",
    "protected_path_dry_run",
    "plan_blast_radius",
    "budget_reservation",
    "parent_recovery_point_verification",
    "scratch_static_apply",
]
BOUNDARIES = {
    "unknown_check": "fail_closed",
    "candidate_worktree_before_admission": "forbidden",
    "candidate_snapshot_before_admission": "forbidden",
    "real_source_mutation": "forbidden",
    "git_commit_push_promotion": "forbidden",
    "provider_and_round_table_calls": "forbidden",
    "arbitrary_subprocess": "forbidden",
    "deployment_locked_mutation": "forbidden",
    "supervisor_outside_worker_mutable_boundary": True,
    "active_evolvable_allowlist": "empty_in_m0_16_1",
    "admitted_grants_mutation_authority": False,
}
def validate(spec):
    if not isinstance(spec, dict):
        return ["evolution_tier0.yaml: missing mapping"]
    errors = []
    def check(path, actual, expected):
        if type(actual) is not type(expected) or actual != expected:
            errors.append(f"evolution_tier0.yaml:{path}: expected {expected!r}, got {actual!r}")
    check("schema_version", spec.get("schema_version"), 1)
    check("milestone", spec.get("milestone"), "M0.16.1")
    check("authority", spec.get("authority"), "docs/project/directives/MAIA_Architecture_Amendment_Autonomous_Evolution_Deployment_Lock_v5.1.md#3.0.1")
    check("phase", spec.get("phase"), "admission_only_non_mutating")
    check("placement", spec.get("placement"), "ops/evolution-supervisor")
    check("ordered_checks", spec.get("ordered_checks"), ORDER)
    check("outcomes", spec.get("outcomes"), ["ADMITTED", "REJECTED", "INFRA_ERROR", "HALT_REQUIRED"])
    for key, value in BOUNDARIES.items():
        check("boundaries." + key, spec.get("boundaries", {}).get(key), value)
    for key, value in {
        "required": True, "machine_readable_reason": True,
        "evidence_refs": True, "recording_failure": "INFRA_ERROR",
    }.items():
        check("rejection_record." + key, spec.get("rejection_record", {}).get(key), value)
    return errors
