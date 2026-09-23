CREATE TABLE IF NOT EXISTS briefing_results (
    result_id TEXT PRIMARY KEY NOT NULL,
    workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
    packet_id TEXT NOT NULL,
    packet_hash TEXT NOT NULL,
    requested_provider TEXT NOT NULL,
    requested_model TEXT NOT NULL,
    actual_provider TEXT,
    actual_model TEXT,
    assurance TEXT NOT NULL,
    result_json TEXT NOT NULL,
    result_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    execution_authority INTEGER NOT NULL CHECK(execution_authority=0)
);
CREATE TABLE IF NOT EXISTS briefing_citations (
    result_id TEXT NOT NULL REFERENCES briefing_results(result_id),
    citation_id TEXT NOT NULL,
    PRIMARY KEY(result_id, citation_id)
);
