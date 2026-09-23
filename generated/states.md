**AgentTaskState**

- **Stany:** draft, preflight, awaiting_approval, queued, running, pausing, paused, partially_completed, completed, canceled, failed

- **Przejścia:** {"awaiting_approval": ["queued", "canceled", "failed"], "draft": ["preflight", "canceled"], "partially_completed": ["queued", "completed", "failed", "canceled"], "paused": ["queued", "canceled"], "pausing": ["paused", "partially_completed", "completed", "failed", "canceled"], "preflight": ["awaiting_approval", "queued", "failed", "canceled"], "queued": ["running", "canceled", "failed"], "running": ["pausing", "partially_completed", "completed", "failed", "canceled"]}

- **Semantyka:** domain state machine

**ActionState**

- **Stany:** planned, gated, awaiting_approval, queued, running, reconciling, retryable_error, completed, skipped, failed, canceled

- **Przejścia:** {"awaiting_approval": ["queued", "canceled", "failed"], "gated": ["awaiting_approval", "queued", "skipped", "failed"], "planned": ["gated", "canceled"], "queued": ["running", "canceled"], "reconciling": ["completed", "retryable_error", "failed", "canceled"], "retryable_error": ["queued", "failed", "canceled"], "running": ["reconciling", "retryable_error", "completed", "failed", "canceled"]}

- **Semantyka:** domain state machine

**ApprovalState**

- **Stany:** not_required, pending, approved, rejected, expired, revoked

- **Przejścia:** {"approved": ["revoked"], "pending": ["approved", "rejected", "expired", "revoked"]}

- **Semantyka:** domain state machine

**CommitmentStatus**

- **Stany:** candidate, proposed, confirmed, in_progress, fulfilled, overdue, canceled, disputed

- **Przejścia:** {"candidate": ["proposed", "confirmed", "canceled"], "confirmed": ["in_progress", "fulfilled", "overdue", "canceled", "disputed"], "in_progress": ["fulfilled", "overdue", "canceled", "disputed"], "overdue": ["fulfilled", "canceled", "disputed"], "proposed": ["confirmed", "canceled", "disputed"]}

- **Semantyka:** domain state machine

**ConnectorHealth**

- **Stany:** unconfigured, needs_auth, connecting, healthy, degraded, rate_limited, blocked, error

- **Przejścia:** {"blocked": ["needs_auth", "connecting", "error", "unconfigured"], "connecting": ["healthy", "degraded", "rate_limited", "blocked", "error", "needs_auth", "unconfigured"], "degraded": ["healthy", "connecting", "rate_limited", "blocked", "error", "needs_auth", "unconfigured"], "error": ["connecting", "healthy", "degraded", "needs_auth", "blocked", "unconfigured"], "healthy": ["degraded", "rate_limited", "blocked", "error", "needs_auth", "unconfigured"], "needs_auth": ["connecting", "blocked", "error", "unconfigured"], "rate_limited": ["connecting", "healthy", "degraded", "blocked", "error", "needs_auth", "unconfigured"], "unconfigured": ["needs_auth", "connecting", "blocked"]}

- **Semantyka:** observed_health_state_with_explicit_allowed_transitions

**RunState**

- **Stany:** created, starting, running, retryable_error, outcome_unknown, reconciling, completed, failed, canceled

- **Przejścia:** {"created": ["starting", "canceled"], "outcome_unknown": ["reconciling"], "reconciling": ["completed", "retryable_error", "failed"], "running": ["retryable_error", "outcome_unknown", "completed", "failed", "canceled"], "starting": ["running", "retryable_error", "failed", "canceled"]}

- **Semantyka:** one execution attempt; retryable_error is terminal; retry creates a new RunId; unresolved outcomes require reconciliation
