# Runbook: Teams / Microsoft 365 Agent Surface

1. Deploy/register a managed public HTTPS channel endpoint for production Teams/M365 traffic.
2. Treat Dev Tunnel/localhost exposure as development-only.
3. Resolve workspace state authority before accepting work: managed Core/store or local-authoritative Core.
4. For local-authoritative workspaces, establish an authenticated outbound-only relay from local Core/worker to the managed Channel Adapter; Azure Relay Hybrid Connections is the reference pattern, not a protocol requirement.
5. The Channel Adapter validates channel identity and emits a signed/authenticated MAIA envelope. It does not plan actions or bypass Core policy.
6. Envelopes are workspace/actor bound, idempotent, replay-protected and expiring.
7. Approval cards contain the canonical `approval_id`, `action_hash` and `expected_version`; final decision is CAS in authoritative Core.
8. If local Core is offline, queue only bounded/expiring envelopes. Never execute local-authority side effects in the cloud adapter.
9. Do not access/copy the workstation SQLite file from the cloud component.
10. For enterprise managed mode, state is held in the managed transactional store; desktop acts as thin client/capability worker as configured.
