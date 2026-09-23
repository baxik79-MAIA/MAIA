# Decision index

This index points to the authoritative reason for a decision. It does not
duplicate whole ADRs or canonical YAML.

## Accepted ADR families

- ADR-0001Ă˘â‚¬â€śADR-0031: foundational product, security, connector, privacy and
  topology decisions; read the individual ADR for scope and status.
- [ADR-0032](../adr/ADR-0032.md): M0.1.1 execution binding and authorization
  contract closure.
- [ADR-0033](../adr/ADR-0033.md): M0.2.1 persistence and audit contract
  closure.
- [ADR-0034](../adr/ADR-0034.md): M0.3 authoritative local store and
  SQLite adapter.
- [ADR-0035](../adr/ADR-0035.md): M0.4 transactional runtime coordinator.
- [ADR-0036](../adr/ADR-0036.md): M0.5 one-shot executor boundary and run
  driver.
- ADR-0043: reasoning-assurance routing before participant invocation.
- ADR-0044: M0.10 local-only source-cited evidence briefing.
- ADR-0045: M0.11 uses native eframe for the first MAIA desktop surface.
- ADR-0046: Round Table leaves MAIA Core as an optional external module
  (free-standing accepted Architecture Owner decision; see
  `docs/adr/ADR-0046.md` — not sourced from v5.1).

Canonical implementation contracts referenced by these decisions live in
`spec/product.yaml`, `spec/domain.yaml`, `spec/approval.yaml`,
`spec/action_binding.yaml`, `spec/execution.yaml`, `spec/persistence.yaml`,
`spec/audit.yaml` and related spec files.

## Directives and amendments

Private development directives and operational reports are excluded. Public architecture contracts remain in YAML, ADRs and the retained deployment-lock directive.

## Supersession map

Architecture Directive v5.1 supersedes the earlier Human-Governed
Self-Improvement directive where the two conflict; the earlier directive's
exact source is not held locally (see `directives/README.md`).

M0.1.1 superseded the earlier 1.2.1 execution binding shape. M0.1.2 hardened
that contract without changing the milestone boundary. M0.2 implemented the
accepted policy/orchestration semantics. M0.2.1 closed persistence/audit
contracts, M0.3 implemented them, M0.4 composed them, and M0.5 added the
one-shot executor boundary. The operational DeskBridge commits affect return
workflow only and do not supersede canonical product decisions.

## Current owner instructions

- `directives/MAIA_Architecture_Amendment_Autonomous_Evolution_Deployment_Lock_v5.1.md`
  (v5.1, ACTIVE / CANONICAL): defines the `DEVELOPMENT_EVOLUTION` (dedicated
  development host, autonomous evolution permitted within defined
  boundaries) and `DEPLOYMENT_LOCKED` (self-improvement capability absent by
  construction) capability profiles, the Evolution Supervisor, and the
  promotion and protected-surface model. Supersedes the earlier
  Human-Governed Self-Improvement directive where the two conflict; that
  earlier directive is not held locally as an exact-text copy. Product
  primacy and CPU-first/office-PC-first remain mandatory; deployment-lock
  semantics are not weakened by autonomous development-host evolution. v5.1
  does not define, and is not the source of, the Round Table external-module
  decision (see ADR-0046, above) or Capability Mesh v1.0 (below); the three
  are compatible, and none supersedes another.
- `directives/MAIA_Product_Direction_Universal_Commercial_Benchmark_Victor.md`:
  forward-only product direction, commercial readiness and benchmark boundary.
- Capability Mesh & Decomposition Directive v1.0 (ACTIVE / MANDATORY per
  Architecture Owner declaration during M0.11): establishes capability
  boundaries (public responsibility vs private implementation, state
  ownership, permissions/authority, resource profile, metrics, dependencies,
  lifecycle) as the canonical target shape for MAIA's subsystems, including
  the emerging Local Intelligence Runtime capability. Its exact source text
  is **pending import**; this page records only that it is active and does
  not recreate the directive, consistent with the pending-import convention
  used above for the M0.5 LocalFirst/RoundTable amendment.

## Public development references

- `13_COMMERCIAL_READINESS.md`: licensing and reuse boundaries.
- `14_PRODUCT_VISION_AND_USER_PROFILES.md`: universal product objectives.
- `15_COMPETITIVE_BENCHMARK_VICTOR.md`: research and attribution boundaries.
- `16_PERSONA_AND_INTERACTION_MODEL.md`: presentation-layer separation.

ADRs remain under `docs/adr/`. Private milestone reports and local capability
telemetry are excluded; current source and tests establish behavior.
