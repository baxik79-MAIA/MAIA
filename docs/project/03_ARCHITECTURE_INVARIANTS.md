# Architecture invariants

These invariants summarize accepted decisions. Canonical fields and enum
values remain defined by `spec/*.yaml`; this page does not create a parallel
contract.

## Core and authority

- MAIA Core is OS-neutral and provider-neutral.
- One local workspace has one authoritative mutable store and one writable
  authority process.
- The store does not evaluate policy, call connectors or own provider memory.
- Every authority mutation in T1â€“T5 includes its audit append in the same
  transaction.
- Audit sequence and hash-chain links are workspace-scoped and append-only.

## Identity, approval and execution

- Task, Plan, Action, Approval and Run are distinct aggregates.
- A material Action edit creates exactly one new immutable revision.
- Approval is bound to action id/version/hash, policy snapshot and required
  assurance. Current policy and actor authorization are checked again before
  execution.
- External sends, destructive actions and privileged actions pass ApprovalGate
  under the canonical policy.
- A Run binds one action revision and approval version. Retry creates a new
  RunId and requires current policy, execution checks and resolved prior effect.
- `outcome_unknown` is never silently converted to retryable failure; it goes
  to reconciliation before a semantically equivalent effect is retried.
- Task pause is quiescent and non-preemptive. New Run creation and
  `created -> starting` are forbidden after a pause request.

## Local first â€” external consultation when required

- MAIA remains useful with all external providers disabled.
- MAIA owns canonical memory, history, WorkGraph, permissions, actions and
  audit. Provider session history is not canonical memory.
- Models are replaceable compute. No domain component directly depends on
  OpenAI, Anthropic, Ollama, llama.cpp, LM Studio or another concrete provider.
- External consultation is controlled by MAIA's privacy/egress, policy and
  approval boundary.
- The Model Router evolves toward a provider-neutral Intelligence Fabric.
- Round Table is a reusable consultation subsystem, not MAIA's memory owner.
- Round Table is an optional external module, not part of MAIA Core
  (ADR-0046, a free-standing accepted Architecture Owner decision — not
  sourced from v5.1). MAIA Core never depends on the Round Table
  implementation to start, operate, brief, reason, manage memory, route
  providers or perform standard workflows; the dependency direction runs
  from Round Table toward public MAIA contracts only.

## Development-host evolution and deployment lock (MAIA v5.1)

- MAIA supports two capability profiles: `DEVELOPMENT_EVOLUTION` (autonomous
  self-improvement permitted on a dedicated development host, within defined
  protected-surface and Evolution Supervisor boundaries) and
  `DEPLOYMENT_LOCKED` (self-improvement capability absent by construction,
  not merely a runtime flag).
- A `DEPLOYMENT_LOCKED` instance cannot re-enable evolution for itself; only
  an external human-controlled development build/deployment action can.
- Autonomous development-host evolution does not weaken deployment-lock
  guarantees, and self-improvement throughput is never a substitute for
  MAIA's user-value/product-primacy mission.
- See `directives/MAIA_Architecture_Amendment_Autonomous_Evolution_Deployment_Lock_v5.1.md`
  for the full Evolution Supervisor, promotion-protocol and protected-surface
  model.

## Delivery and provenance

- No hidden connector, model or tenant fallback is permitted.
- External content is untrusted and must retain source references where the
  canonical contract requires provenance.
- UI, dashboard, Tauri, React, CSS, connectors and scheduler workers are not
  part of the headless M0.1â€“M0.5 foundations.
- A contradiction is reported to Architecture Desk; missing semantics are not
  invented locally in Rust or SQL.

## Product and commercial boundaries

- MAIA is one configurable product core, not separate personal, executive,
  technical or enterprise implementations.
- Capability, connector, policy, role and surface composition may tailor a
  workspace; it must not weaken approval, audit, privacy or authority rules.
- A third-party component is never adopted solely because it is mature or
  permissively licensed. Provenance, license obligations, security exposure,
  commercial fit, cost and coupling require recorded review.
- Competitive frameworks are benchmarks and research sources, not MAIA
  dependencies, canonical memory owners or authority substitutes unless a
  later explicit decision says otherwise.
- Generated DOCX/PDF documentation is non-canonical output. Its facts must be
  recoverable from canonical specifications, ADRs and versioned handbook text.

## Persona separation

- MAIA Core has no gender; persona is a presentation and interaction layer.
- The default feminine persona may affect wording, tone, warmth, humour,
  formality, verbosity, voice and avatar/name presentation only.
- Persona must never affect truthfulness, risk classification, approval,
  permissions, privacy, egress, audit, execution authorization, evidence,
  connector permissions or any security boundary.
- Persona configuration is MAIA-owned state, not provider session history. A
  model or provider change cannot silently change MAIA's persona or identity.
