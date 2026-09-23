# Round Table consultation subsystem

Round Table is a reusable multi-model consultation engine that can remain
independently useful outside MAIA. It is a consultant subsystem, not MAIA's
memory owner and not a forced repository merge.

## Architecture update: optional external module (ADR-0046)

**ACCEPTED ARCHITECTURE OWNER DECISION.** Round Table SHALL NOT remain part
of MAIA Core; it is an optional external module. The dependency direction is
Round Table -> public MAIA contracts / Intelligence Fabric interfaces, never
the reverse. MAIA Core must build, run and perform all standard workflows
with Round Table entirely absent, disabled, uninstalled or offline, with no
hidden fallback and no required DB migration on that account. Round Table
never bypasses Privacy, Policy, Approval, `execution_authority`, egress or
audit. The provider-neutral Intelligence Fabric itself stays in MAIA Core;
only the Round Table implementation (participant-role logic, debate
orchestration, transcripts/state, its own synthesis) is extracted. This is a
free-standing accepted Architecture Owner decision, not sourced from v5.1
(a separate, compatible directive covering `DEVELOPMENT_EVOLUTION` /
`DEPLOYMENT_LOCKED`). See `docs/adr/ADR-0046.md` for full rationale,
invariants and the three-phase (analyze / define contract / extract)
migration strategy. **Physical extraction completed at M0.15.8**: the crate
formerly at `core/roundtable` now lives at `roundtable/` (top-level, package
name `maia-roundtable` unchanged); no dependency edge changed, only path
strings did. Core independence continues to be proven by the dependency
graph (`roundtable/tests/core_independence.rs`, `verify_round_table_absent.py`),
not by workspace layout — the physical move changes only where the crate
sits, not the invariant that already held.

## Target shape

Participants have explicit roles such as proposer, critic, verifier, domain
specialist and chair. A bounded debate lifecycle uses a Case Dossier, bounded
participant count and bounded rounds, then records a conclusion and dissent.
MAIA may act as Chair when the consultation is invoked from a governed task.

The Case Dossier contains the purpose, reduced evidence, constraints,
questions, participant outputs and provenance needed for the bounded case. A
consultant receives reduced context rather than raw entire history. MAIA keeps
the local source-of-truth task, action, approval, result and audit state.

## Implemented versus DEFERRED

The source includes provider-neutral orchestration, participant resolution,
quorum, adjudication, dissent preservation, session persistence and fail-closed
provider adapters. Deterministic tests cover these contracts. No private live
session evidence is bundled or required to reproduce public validation.

Product readiness, universal provider support, production-grade process-tree
containment, atomic create-only SessionStore semantics and more precise local
provider failure reporting remain separate work. Case Dossier persistence,
chair policy and ergonomic presentation are deferred product work.

Reasoning workflows preserve execution_authority=false and unknown costs.
Round Table cannot bypass privacy, policy, approval or reconciliation.
