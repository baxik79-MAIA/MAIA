# MAIA v1.2.0 - Topology & Concurrency Hardening

1. Explicit Teams production topology; cloud Channel Adapter never opens local SQLite.
2. Azure Relay Hybrid Connections documented as a reference outbound-only relay pattern, not as a fictional M365 relay product.
3. Task pause changed to `running -> pausing -> paused`; generic `ActionState.paused` intentionally rejected for non-cancellable side effects.
4. Added RunState and Action reconciliation for ambiguous non-idempotent outcomes.
5. MCP tool fingerprints standardized on RFC 8785 JCS + SHA-256 with executable golden vector.
6. Approval payload divergence automatically revokes/supersedes stale approval.
7. Approval rendering uses typed/sanitized fields; remote/data-URI exfiltration elements are blocked.
8. Outlook Classic runbook adds dedicated STA, IMessageFilter/busy retry handling and ambiguous-send reconciliation.
9. Graph/source freshness precondition added before material side effects; no unsupported If-Match assumption on send-draft.
10. WorkGraph inferred edges are visibly non-authoritative until confirmed/policy-promoted.
11. Third-party erasure uses random opaque tombstones when retention requires structure; deterministic PII hashes are forbidden.
12. Commitment extraction adds PL modality/aspect cues as confidence signals only; free text never auto-confirms a commitment.
13. Approval-fatigue mitigation uses explicit scoped grants/templates, never RelationshipProfile familiarity as permission.
