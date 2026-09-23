# MAIA 1.3.0 — Execution Binding & Authorization Contract Closure

M0.1.1 intentionally evolves the 1.2.1 contracts. It is not a backward-compatible patch.

- Add immutable Action/Plan revision semantics, canonical input identity, connector
  selection/binding and visible MCP definition binding.
- Specify independent ActionBindingV1 JCS/SHA-256 with lossless u64 decimal strings,
  deterministic UTF-8 source sorting and duplicate rejection.
- Separate Approval record CAS version from Action revision. Add policy snapshot,
  policy decision and assurance, expiry/validity, and execution authorization matrix.
- Bind every Run to exact Action and Approval versions. Retry creates a new Run;
  retryable_error has no outgoing Run edges.
- Define authoritative reconciliation, per-Run freshness and pause across superseded
  revisions. Specify trusted conservative recipient classification and operation floors.
- Add ADR-0032, direct amendment notices, fail-closed guard obligations, independent
  golden vectors, test-only contract oracles and mechanical Rust conformance.

No policy runtime, database/CAS store, production canonicalizers, connectors,
directory expansion, scheduler, UI, Tauri/React or model/MCP runtime is implemented.
No runtime migration is needed in this repository; future consumers must adopt the
new required fields and FSM semantics explicitly.
