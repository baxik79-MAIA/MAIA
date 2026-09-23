# MAIA 1.5.0 — Read-Only Workspace Evidence Snapshot Contract

M0.6 Phase A adds an additive canonical contract for bounded, caller-selected
local UTF-8 text/Markdown evidence snapshots. It extends the existing Artifact
concept with workspace binding, immutable MAIA-owned content identity, private
source provenance, import-request identity and stable citations.

- Define strict decoding/hash/byte semantics and small explicit import bounds.
- Define atomic all-or-nothing T6 import, duplicate behavior and restart-safe
  snapshot reads.
- Keep raw content and source locators out of the audit chain while retaining
  artifact/request/provenance-hash references.
- Forbid semantic extraction, MemoryClaim/WorkGraph creation, product models,
  connectors, external network, external side effects and parser dependencies.

This is a canonical minor-version evolution from 1.4.0 because it adds required
product contract fields and a new authoritative transaction. Rust/SQLite/host
implementation remains pending Architecture Desk review of Phase A.
