# Security model

This is an overview of canonical boundaries, not a security certification or a claim of production readiness. `spec/*.yaml` remains authoritative.

- The human user is the highest authority for material actions. External sends, destructive and privileged actions pass the applicable policy and ApprovalGate.
- RiskClass, ApprovalAssurance and ReasoningAssuranceLevel are distinct. Round Table output and A4 reasoning confirmation do not authorize an Action.
- Runs bind an exact immutable Action revision/hash and authorization version. Approval decisions use compare-and-swap; stale decisions must fail explicitly.
- Non-idempotent unknown outcomes require reconciliation before retry. Task pause is quiescent; it does not pretend to stop a non-cancellable effect mid-flight.
- Mail, messages, documents and tool/model outputs are untrusted. Approval rendering cannot introduce remote-loading content or bypass typed rendering.
- Tool-definition trust is pinned to canonical RFC 8785 fingerprints. ActionBindingV1 has its own JCS contract and decimal-string version encoding.
- One workspace has one authoritative mutable store. Platform, SQL, filesystem, network and process concerns remain behind adapters.
- Credentials must not appear in stores, logs, fixtures, exports or provenance. Egress is explicit, minimal and policy-controlled; no hidden provider, connector, tenant or model fallback is allowed.
- `DEPLOYMENT_LOCKED` excludes evolution capabilities by construction. An all-features development test run is not evidence that a distributed build meets this boundary.

These APIs are not an operating-system sandbox. Adapter review, process behavior, storage integrity, permissions and release composition require independent verification. Audit/provenance records may themselves contain private information and must remain local unless explicitly reviewed and authorized for disclosure.

See [SECURITY.md](../SECURITY.md) for reporting, and [architecture](ARCHITECTURE.md) for executable guards. Publication blockers and current validation limits are tracked in [development status](DEVELOPMENT_STATUS.md).
