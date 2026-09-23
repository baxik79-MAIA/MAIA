# Runbook: Ambiguous side-effect reconciliation

Use when a connector reports timeout/disconnect/unknown outcome after a non-idempotent operation such as mail send, calendar invitation, external post or destructive action.

1. Set the current Run to `outcome_unknown` and Action to `reconciling`; block automatic retry.
2. Record the exact `action_hash`, connector identity, external target, timestamps and any idempotency/action marker.
3. Query the authoritative external system for evidence that the side effect occurred.
4. If execution is proven, mark Run/Action completed and store the external result reference.
5. If non-execution is proven, transition to retryable state and perform a fresh policy/source-version check before retry.
6. If outcome remains ambiguous, keep the action blocked and request human resolution; do not guess.
7. Any material source/payload change during reconciliation invalidates previous approval.
