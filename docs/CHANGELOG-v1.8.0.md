# MAIA 1.8.0 — M0.9 Assurance Routing and Round Table Orchestration

M0.9 adds a provider-neutral assurance router that produces a non-executable A0–A4 orchestration plan or an explicit `InsufficientAssurance` outcome. It introduces a participant registry seam, pre-invocation session budget checks, named failure handling and audit metadata for request IDs, usage, cost and A4 human reasoning acceptance.

The router never lowers a policy-required assurance level, never hides a provider/model fallback, and never replaces Risk classification or ApprovalGate. A3 remains the existing response-isolated Round Table; A4 adds recorded human acceptance of the reasoning conclusion only.
