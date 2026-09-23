# MAIA architecture

MAIA is a local-first, provider-neutral, human-governed executive agent. The
canonical contracts in `spec/*.yaml` define the product semantics. ADRs explain
why accepted boundaries exist. This handbook records the current shape without
claiming deferred implementation.

## Current layers

- `maia-domain` contains OS-neutral canonical value types, state machines and
  structural validation generated from the spec.
- `maia-policy` composes policy decisions and evaluates approval bindings,
  assurance and execution authorization from already-resolved facts.
- `maia-orchestrator` evaluates execution eligibility, routing, freshness,
  reconciliation, retry and quiescent pause semantics without I/O.
- `core/store` defines small OS-neutral repository and atomic T1Ă˘â‚¬â€śT5 ports.
- `infra/sqlite` is the current local authoritative adapter. It owns SQLite,
  migrations, WAL, authority locking, CAS, evidence and audit-chain writes.
- `core/runtime` composes policy, orchestration and store ports into pure
  transactional use cases, including Run creation and persisted transitions.
- `core/executor` is the one-shot boundary. It revalidates a persisted Run,
  claims it through T4, invokes one injected executor and persists a classified
  outcome. It has no concrete connector or scheduler.

## Aggregate distinctions

`AgentTask`, `ExecutionPlan`, `Action`, `Approval` and `Run` are different
records with different identity and version semantics. Action revisions are
immutable. Approval records bind an exact action hash and approval version.
Runs bind one action revision and one approval version; retry creates a new
RunId and never reuses the old attempt.

## Authority boundaries

Policy decides whether current facts authorize an operation. Approval records
the human decision and assurance. The authoritative store enforces CAS,
uniqueness, immutable history and atomic mutation-plus-audit. Runtime composes
these decisions. The executor port receives a bounded request. This API is not a sandbox: an
injected implementation could retain other capabilities, so adapter review is
required. Concrete connectors, provider adapters,
schedulers and UI remain outside the core.

## Target direction

The architecture is local-first and survives disappearance of any single
external provider. MAIA owns canonical memory, history, WorkGraph, permissions,
actions and audit. External models are replaceable consultants. The future
Model Router evolves toward an Intelligence Fabric and a reusable Round Table
consultation subsystem; both are documented as target architecture, not as
implemented M0.5 behavior.

## Universal product composition

**ACCEPTED ARCHITECTURE.** MAIA has one common core and composes a workspace
experience from capabilities, connector adapters, policy, roles and product
surfaces. Personal, professional, technical and team workspaces can therefore
select different capabilities without creating separate MAIA cores or changing
the authority model. Communication, people, commitments, cases and projects
are intended as first-class product concepts when their canonical contracts are
approved; M0.5 does not yet implement those product layers.

**PLANNED / DEFERRED.** Connector implementations, workspace profile storage,
user experience surfaces, people/relationship context, commitment extraction,
case history and product packaging require their own contracts and milestones.

## Persona and interaction layer

**ACCEPTED ARCHITECTURE.** MAIA Core has no gender. Persona is a separately
configured, MAIA-owned presentation and interaction layer. The default persona
uses a feminine presentation and may control wording, tone, warmth, humour,
formality, verbosity, voice and visual presentation. It persists independently
of a provider session so changing model or provider cannot change MAIA's
identity.

**PLANNED / DEFERRED.** There is no Persona Engine, persona storage schema,
voice/avatar implementation or UI in M0.5. Persona configuration must not be
implemented by embedding identity in a model prompt alone.
