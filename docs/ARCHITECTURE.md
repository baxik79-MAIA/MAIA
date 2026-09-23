# Architecture

MAIA is a local-first, human-governed execution-intelligence project. Its value chain is Signal -> Context -> Commitment / Decision / Risk -> Plan -> Action -> Run -> Outcome. Task, Plan, Action, Run and business Outcome are distinct concepts.

## Current workspace

| Location | Responsibility |
| --- | --- |
| `core/` | OS-neutral domain contracts, policy/orchestration, store ports, runtime/executor boundaries, assurance routing and briefing |
| `infra/` | SQLite and model/provider adapters; local intelligence health, diagnostics, hypothesis ledger and recorder |
| `composition/` | Explicit assembly and observability for optional consultation components |
| `roundtable/` | Optional multi-model reasoning module consuming public contracts |
| `apps/` | Experimental desktop, briefing/recorder hosts and Round Table viewer |
| `ops/` | Explicit development invocation tools |
| `spec/`, `docs/adr/` | Canonical contracts and architectural rationale |
| `tools/`, `tests/`, `generated/` | Validation/generation tooling, reference tests and derived artifacts |

Capability Mesh means explicit capability contracts and ownership within a modular monolith. It does not imply a completed distributed plugin marketplace. Core does not depend on concrete providers, platform adapters or the Round Table implementation. SQLite, network and process effects stay behind adapters. Round Table may depend on public MAIA contracts; the reverse dependency is prohibited by [ADR-0046](adr/ADR-0046.md).

Local Intelligence Runtime is consumed through its capability interface. The desktop and ordinary MAIA paths must remain usable without Round Table. CPU-first/office-PC-first is an architectural constraint, not a measured performance claim.

## Protected boundaries

Human approval, typed rendering of untrusted content, explicit egress, versioned approval CAS and exact execution binding remain required. Reasoning assurance is distinct from risk and approval assurance. `DEVELOPMENT_EVOLUTION` and `DEPLOYMENT_LOCKED` remain separate capability profiles; deployed instances cannot restore self-improvement themselves.

Executable guards include `roundtable/tests/core_independence.rs`, capability-mesh tests under the local intelligence adapters, and `tools/verify_round_table_absent.py`. Their results must be assessed on the actual candidate; their existence alone does not prove a release safe.

Canonical detail: [specifications](../spec/), [ADRs](adr/), [architecture invariants](project/03_ARCHITECTURE_INVARIANTS.md) and [deployment-lock directive](project/directives/MAIA_Architecture_Amendment_Autonomous_Evolution_Deployment_Lock_v5.1.md).

The diagrams in `assets/` are historical conceptual architecture illustrations; they are not an inventory of currently implemented connectors or product features.
