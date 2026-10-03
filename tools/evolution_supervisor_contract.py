"""M0.16.0 fail-closed checks for the canonical Supervisor boundary."""
from collections.abc import Mapping


def validate(contract):
    errors = []
    if not isinstance(contract, Mapping):
        return ["evolution_supervisor.yaml: contract is required"]

    def field(path):
        value = contract
        for part in path.split("."):
            if not isinstance(value, Mapping):
                return None
            value = value.get(part)
        return value

    expected = {
        "schema_version": 1,
        "milestone": "M0.16.0",
        "phase": "contract_first_non_mutating",
        "placement.subsystem": "development_operations",
        "placement.crate": "ops/evolution-supervisor",
        "placement.core_dependency": "forbidden",
        "placement.shipped_application_dependency": "forbidden",
        "placement.hypothesis_ledger_generator_qualification_authority": "advisory_only",
        "trust_boundary.supervisor": "outside_evolving_worker_mutable_boundary",
        "trust_boundary.worker_may_control_supervisor": False,
        "trust_boundary.worker_may_modify_supervisor_or_kill_switch": False,
        "trust_boundary.host_isolation_required_before_mutation": True,
        "trust_boundary.unknown_integrity_or_isolation": "halt",
        "capability_profiles.development_evolution": "contract_review_only",
        "capability_profiles.deployment_locked": "supervisor_absent_from_shipped_build_graph",
        "capability_profiles.runtime_reenable_by_deployed_instance": "forbidden",
        "authority.hypothesis_and_qualification_grant_mutation_authority": False,
        "authority.worker_may_self_classify_promotion_eligible": False,
        "preflight.result": "advisory_contract_review_only",
        "preflight.success_grants_mutation_authority": False,
        "preflight.unknown_or_missing": "reject",
        "preflight.tier0_required_before_worktree_or_candidate_snapshot": True,
        "preflight.tier0_complete_in_this_milestone": False,
        "preflight.candidate_worktree_creation": "forbidden",
        "preflight.candidate_snapshot_creation": "forbidden",
        "preflight.mutation": "forbidden",
        "preflight.promotion": "forbidden",
        "protected_surfaces.unclassified": "non_writable",
        "protected_surfaces.worker_may_edit_policy_or_evaluator": False,
        "protected_surfaces.worker_may_edit_resource_hard_ceiling": False,
        "protected_surfaces.evolvable_allowlist": [],
        "kill_switch.unreadable_or_unknown": "halt",
        "kill_switch.worker_may_reset": False,
        "kill_switch.emergency_result": "HALTED",
        "release_boundary.development_promotion_is_release": False,
        "release_boundary.release_authority": "external_human_controlled",
        "release_boundary.locked_artifact_can_reenable_evolution": False,
    }
    for path, value in expected.items():
        if field(path) != value:
            errors.append(f"evolution_supervisor.yaml:{path} must be {value!r}")

    required_evidence = {
        "development_evolution_profile", "supervisor_integrity", "host_isolation",
        "kill_switch_operational", "positive_evolvable_allowlist",
        "parent_recovery_point", "resource_budget_and_recovery_reserve",
        "protected_path_dry_run", "declared_operator", "declared_blast_radius",
        "hypothesis_evidence_refs",
    }
    if not required_evidence.issubset(set(field("preflight.required_evidence") or [])):
        errors.append("evolution_supervisor.yaml:preflight.required_evidence incomplete")
    required_authority = {
        "start_pause_stop", "kill_switch", "resource_ceilings",
        "snapshot_verification", "rollback_baseline",
        "protected_surface_enforcement", "candidate_lineage_verification",
        "integrity_heartbeat", "failure_backoff",
        "development_lineage_promotion",
    }
    if set(field("authority.supervisor_owns") or []) != required_authority:
        errors.append("evolution_supervisor.yaml:authority.supervisor_owns incomplete")
    return errors
