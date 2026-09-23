# MAIA 1.3.1 — Approval & Execution Conformance Hardening

M0.1.2 is a corrective conformance/guard patch to 1.3.0. Commit 472ff64 and its
milestone history remain unchanged.

- F1: declare terminal Run states and reject every outgoing edge in the validator.
- F2: enforce snapshot policy/state/assurance coherence, including autonomous records.
- F3: require attributable human rejection at confirm assurance; pending has no
  human-decision metadata. Strengthen the snapshot CAS oracle and negative tests.
- F4: validate input completeness before applying operation risk floors.

No new entity fields, wire enums or FSM edges. No runtime, persistence, connectors,
UI or Rust dependencies. Previously accepted inconsistent in-memory snapshots now
fail validation; there is no implemented store requiring a database migration.
