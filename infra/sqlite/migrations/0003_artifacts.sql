CREATE TABLE IF NOT EXISTS artifact_contents (
    workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
    content_sha256 TEXT NOT NULL,
    content TEXT NOT NULL,
    byte_length TEXT NOT NULL,
    PRIMARY KEY(workspace_id, content_sha256)
);
CREATE TABLE IF NOT EXISTS artifacts (
    artifact_id TEXT PRIMARY KEY NOT NULL,
    workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    media_type TEXT NOT NULL,
    content_sha256 TEXT NOT NULL,
    byte_length TEXT NOT NULL,
    source_locator TEXT NOT NULL,
    source_locator_hash TEXT NOT NULL,
    import_request_id TEXT NOT NULL,
    trust TEXT NOT NULL,
    classification TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(workspace_id, content_sha256) REFERENCES artifact_contents(workspace_id, content_sha256),
    UNIQUE(workspace_id, source_locator_hash, content_sha256)
);
