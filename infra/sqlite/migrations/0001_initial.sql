CREATE TABLE IF NOT EXISTS schema_meta (
    schema_version INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS migration_ledger (
    migration_id TEXT PRIMARY KEY NOT NULL,
    checksum TEXT NOT NULL,
    applied_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS workspaces (
    workspace_id TEXT PRIMARY KEY NOT NULL,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS agent_tasks (
    id TEXT PRIMARY KEY NOT NULL,
    version TEXT NOT NULL,
    workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
    request_text TEXT NOT NULL,
    origin_surface TEXT NOT NULL,
    origin_ref TEXT,
    state TEXT NOT NULL,
    privacy_class TEXT NOT NULL,
    requested_by TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS execution_plans (
    id TEXT NOT NULL,
    version TEXT NOT NULL,
    task_id TEXT NOT NULL REFERENCES agent_tasks(id),
    risk_summary TEXT NOT NULL,
    estimated_amount_micros TEXT,
    estimated_currency TEXT,
    approval_requirement TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (id, version)
);
CREATE TABLE IF NOT EXISTS actions (
    id TEXT NOT NULL,
    version TEXT NOT NULL,
    plan_id TEXT NOT NULL,
    plan_version TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    action_type TEXT NOT NULL,
    connector_profile_id TEXT,
    risk_class TEXT NOT NULL,
    state TEXT NOT NULL,
    input_ref TEXT NOT NULL,
    source_preconditions_json TEXT NOT NULL,
    result_ref TEXT,
    input_hash TEXT NOT NULL,
    input_canonicalizer_id TEXT NOT NULL,
    input_canonicalizer_version TEXT NOT NULL,
    connector_selection TEXT NOT NULL,
    connector_binding_hash TEXT,
    tool_definition_fingerprint TEXT,
    action_hash TEXT NOT NULL,
    PRIMARY KEY (id, version),
    FOREIGN KEY (plan_id, plan_version) REFERENCES execution_plans(id, version)
);
CREATE TABLE IF NOT EXISTS approvals (
    id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NOT NULL REFERENCES agent_tasks(id),
    action_id TEXT NOT NULL,
    action_version TEXT NOT NULL,
    policy_id TEXT NOT NULL,
    state TEXT NOT NULL,
    requested_at TEXT NOT NULL,
    decided_at TEXT,
    decided_by TEXT,
    decision_note TEXT,
    action_hash TEXT NOT NULL,
    version TEXT NOT NULL,
    expires_at TEXT,
    origin_surface TEXT NOT NULL,
    decided_surface TEXT,
    policy_snapshot_hash TEXT NOT NULL,
    policy_decision TEXT NOT NULL,
    required_assurance TEXT NOT NULL,
    achieved_assurance TEXT NOT NULL,
    FOREIGN KEY (action_id, action_version) REFERENCES actions(id, version)
);
CREATE TABLE IF NOT EXISTS runs (
    id TEXT PRIMARY KEY NOT NULL,
    version TEXT NOT NULL,
    action_id TEXT NOT NULL,
    action_version TEXT NOT NULL,
    attempt TEXT NOT NULL,
    requested_connector_id TEXT,
    actual_connector_id TEXT,
    requested_model_id TEXT,
    actual_model_id TEXT,
    state TEXT NOT NULL,
    outcome_certainty TEXT NOT NULL,
    reconciliation_ref TEXT,
    started_at TEXT,
    ended_at TEXT,
    action_hash TEXT NOT NULL,
    approval_id TEXT NOT NULL REFERENCES approvals(id),
    approval_version TEXT NOT NULL,
    FOREIGN KEY (action_id, action_version) REFERENCES actions(id, version),
    UNIQUE (action_id, action_version, attempt)
);
CREATE TABLE IF NOT EXISTS freshness_evidence (
    run_id TEXT PRIMARY KEY NOT NULL REFERENCES runs(id),
    action_id TEXT NOT NULL,
    action_version TEXT NOT NULL,
    action_hash TEXT NOT NULL,
    enforcement TEXT NOT NULL,
    precondition_external_id TEXT NOT NULL,
    precondition_source_version_token TEXT NOT NULL,
    precondition_payload_hash TEXT NOT NULL,
    observed_source_version_token TEXT,
    observed_payload_hash TEXT,
    result TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS reconciliation_evidence (
    run_id TEXT PRIMARY KEY NOT NULL REFERENCES runs(id),
    action_id TEXT NOT NULL,
    action_version TEXT NOT NULL,
    action_hash TEXT NOT NULL,
    effect_identity TEXT NOT NULL,
    source_ref TEXT NOT NULL,
    result TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS audit_heads (
    workspace_id TEXT PRIMARY KEY NOT NULL REFERENCES workspaces(workspace_id),
    next_sequence TEXT NOT NULL,
    last_hash TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS audit_records (
    workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
    sequence TEXT NOT NULL,
    audit_id TEXT PRIMARY KEY NOT NULL,
    recorded_at TEXT NOT NULL,
    actor_ref TEXT,
    event_type TEXT NOT NULL,
    subject_type TEXT NOT NULL,
    subject_ref TEXT,
    operation_ref TEXT,
    decision_ref TEXT,
    metadata_hash TEXT,
    prev_hash TEXT NOT NULL,
    record_hash TEXT NOT NULL,
    UNIQUE (workspace_id, sequence)
);
