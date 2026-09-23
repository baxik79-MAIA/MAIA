# Runbook: Sync recovery

1. Detect expired subscription, invalid delta cursor or missed notification signal.
2. Pause derived proactive actions that depend on incomplete state.
3. Attempt subscription renewal or safe cursor continuation.
4. If cursor is invalid, run bounded resynchronization for affected folders/resources.
5. Deduplicate by connector/external ID/version.
6. Recompute derived entities (threads, commitments, WorkGraph edges) idempotently.
7. Record recovery event and completeness status.
