# Runbook: Outlook Classic / TrustedBridge

1. Detect Classic Outlook availability and current user session.
2. Health-check COM access without sending or modifying mail.
3. Restrict the connector to declared mailbox/folder scope.
4. Normalize messages into MAIA domain objects; raw COM objects never cross the connector boundary.
5. Draft creation may be autonomous under policy; send always passes ApprovalGate by default.
6. On client/COM failure, mark connector degraded and do not silently switch to another mailbox connector for a pending side effect.
7. Migration to Graph preserves domain IDs via external reference mapping where possible.
8. **STA apartment:** all Outlook object-model calls execute on a dedicated COM STA thread initialized with `COINIT_APARTMENTTHREADED`. Worker threads communicate with this executor through a queue; raw COM proxies are not passed across arbitrary worker threads.
9. **Busy/modal Outlook:** register/implement `IMessageFilter` (or equivalent COM interop retry handling). `SERVERCALL_RETRYLATER`, `RPC_E_SERVERCALL_RETRYLATER` and `RPC_E_CALL_REJECTED` are treated as bounded transient busy conditions, not `InternalError`. Use bounded increasing retry delay; the default MAIA busy budget is 10 seconds and is configuration/policy data.
10. If the busy budget is exhausted before the external call is accepted, publish `ConnectorHealth=degraded` and return a retryable Run result. Do not hang the sidecar.
11. **Non-idempotent ambiguity:** if a send/move call may have crossed the side-effect boundary but the response is lost/ambiguous, set `RunState=outcome_unknown` and `ActionState=reconciling`. Never retry automatically until reconciliation proves whether the effect occurred.
12. For retry/reconciliation, prefer stable external IDs and an MAIA action marker where the connector safely supports one. If the outcome cannot be proven, require explicit human resolution rather than risking a duplicate send.
