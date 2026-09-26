"""Canonical M0.16.4 protected runtime and containment contract guard."""

EXPECTED = {
    "schema_version": 1,
    "milestone": "M0.16.4",
    "authority": "docs/project/directives/MAIA_Architecture_Amendment_Autonomous_Evolution_Deployment_Lock_v5.1.md",
    "placement": {
        "runtime_authority": "protected_host_process",
        "candidate_policy": "ops/evolution-supervisor",
        "windows_containment_adapter": "ops/evolution-process-host",
        "workspace_and_evidence_adapter": "ops/evolution-workspace-host",
        "core_dependency": "forbidden",
        "shipped_application_dependency": "forbidden",
    },
    "protected_state": {
        "owner": "protected_host_process",
        "authority_storage": "protected_host_process_memory",
        "restart_without_attestation": "fail_closed_locked_defaults",
        "fields": [
            "runtime_profile", "evolution_enabled", "kill_switch_state",
            "supervisor_identity", "candidate_identity", "generation_id",
            "parent_commit", "reservation_identity_and_expiry",
            "tier0_admission_identity", "protected_path_assumptions",
            "human_approval_identity_and_validity",
            "canonical_repository_identity_and_state", "state_version",
        ],
        "candidate_may_supply_or_change": False,
        "unknown_or_ambiguous": "fail_closed",
        "startup_defaults": {
            "runtime_profile": "DEPLOYMENT_LOCKED",
            "evolution_enabled": False,
            "kill_switch_state": "ON",
            "supervisor_integrity": "unverified",
            "tier1_isolation": "same_user_process_tree_contained",
            "explicit_host_attestation_required": True,
        },
        "deployment_locked": "deny_mutation_and_verification",
        "candidate_profile_switch": "forbidden",
    },
    "gate_decision": {
        "immutable_operation_snapshot": "required",
        "bind_fields": [
            "runtime_profile", "state_version", "supervisor_identity",
            "candidate_identity", "generation_id", "parent_commit",
            "reservation_identity_and_expiry", "tier0_admission_identity",
            "protected_path_assumptions",
            "human_approval_identity_and_validity",
            "canonical_repository_identity_and_state",
        ],
        "revalidate_before": ["mutation", "tier1_start"],
        "revalidate_after": ["tier1"],
        "invalidated_by": [
            "profile_change", "evolution_disable", "kill_switch_change",
            "supervisor_identity_change", "candidate_identity_change",
            "reservation_expiry", "approval_revocation", "tier0_evidence_change",
            "protected_path_change", "parent_change", "canonical_repository_drift",
        ],
    },
    "kill_switch": {
        "protected_owner_only": True,
        "on_or_disabled_denies_new_mutation": True,
        "disabled_denies_new_mutation": True,
        "disabled_denies_new_tier1": True,
        "running_verifier": "terminate_containment_job",
        "interruption_result": "CANCELLED",
        "candidate_operation": "discard_candidate",
        "preserve_and_flush_evidence": True,
        "unknown_state": "fail_closed",
    },
    "containment": {
        "platform": "windows",
        "process_tree": "job_object",
        "create_suspended_assign_before_resume": True,
        "breakaway": "forbidden",
        "kill_on_owner_close": True,
        "network": "appcontainer_without_network_capabilities_required; unavailable_fails_closed",
        "filesystem": "appcontainer_acl_read_only_workspace_toolchain_and_writable_candidate_target_required; unavailable_fails_closed",
        "capability_levels": ["same_user_process_tree_contained", "restricted_identity_network_and_filesystem"],
        "minimum_tier1_level": "restricted_identity_network_and_filesystem",
        "inherited_handles": "explicit_null_standard_handles_only",
        "candidate_or_environment_fallback": "forbidden",
        "unavailable_capability": "fail_closed",
        "achieved_identity_claim": "same_user_process_tree_containment_only",
        "stronger_isolation_capabilities": "unavailable_until_os_acl_and_no_network_are_verified",
    },
    "resource_limits": {
        "max_concurrent_verifier_trees": 1,
        "max_command_wall_seconds": 120,
        "cpu_rate_percent": 75,
        "job_memory_bytes": 4294967296,
        "active_process_limit": 64,
        "candidate_controls_limits": False,
        "resource_exhaustion_result": "RESOURCE_LIMIT",
        "job_resource_notifications": {
            "active_process_limit": "JOB_OBJECT_MSG_ACTIVE_PROCESS_LIMIT",
            "job_memory_limit": "JOB_OBJECT_MSG_JOB_MEMORY_LIMIT",
            "process_memory_limit": "JOB_OBJECT_MSG_PROCESS_MEMORY_LIMIT",
        },
        "recognized_memory_termination_status": [
            "STATUS_COMMITMENT_LIMIT", "STATUS_NO_MEMORY",
        ],
        "target_directory": "candidate_scoped",
        "cargo_network_mode": "offline",
        "environment": "fixed_allowlist",
    },
    "journal": {
        "path_owner": "protected_host",
        "path_candidate_supplied": False,
        "location": "outside_canonical_repository_and_candidate_root",
        "writer_lock": "operating_system_exclusive_whole_file",
        "lock_before_chain_validation": True,
        "second_writer": "INFRA_ERROR",
        "crash_releases_lock": True,
        "reopen_verifies_complete_hash_chain": True,
        "corruption": "fail_closed_without_append",
        "authority_transition_flush_before_success": True,
        "lock_or_storage_failure": "INFRA_ERROR",
    },
    "outcomes": {
        "verification": ["PASS", "TEST_FAILURE", "RESOURCE_LIMIT", "INFRA_ERROR", "CANCELLED", "ISOLATION_UNAVAILABLE"],
        "isolation_unavailable_operation": "discard_candidate",
        "isolation_unavailable_terminal_state": "ISOLATION_UNAVAILABLE",
        "cancellation_is_not_verification_failure": True,
        "candidate_failure_operation": "discard_candidate",
        "cancellation_is_test_failure": False,
        "tier1_pass_grants_promotion": False,
        "rollback_baseline_implied": False,
    },
    "authority_limits": {
        "evolvable_paths": ["apps/local-intelligence-host/src/lib.rs"],
        "arbitrary_filesystem_write": "forbidden",
        "arbitrary_subprocess": "forbidden",
        "provider_or_round_table_access": "forbidden",
        "git_commit_push_merge_or_protected_ref_change": "forbidden",
        "deployment_locked_mutation": "forbidden",
        "promotion_deployment_or_active_version_change": "forbidden",
    },
}


def validate(spec):
    errors = []

    def check(actual, expected, path):
        if isinstance(expected, dict):
            if not isinstance(actual, dict):
                errors.append(f"evolution_protected_runtime.yaml:{path}: mapping required")
                return
            for key, value in expected.items():
                check(actual.get(key), value, f"{path}.{key}" if path else key)
        elif type(actual) is not type(expected) or actual != expected:
            errors.append(
                f"evolution_protected_runtime.yaml:{path}: expected {expected!r}, got {actual!r}"
            )

    check(spec, EXPECTED, "")
    return errors
