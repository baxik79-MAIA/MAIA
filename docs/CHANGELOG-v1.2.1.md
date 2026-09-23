# MAIA 1.2.1 — Canonical Contract Closure

M0.0.1 is a specification-only, backward-compatible patch before Rust M0.1.

- Preserve the domain inventory while defining typed AgentTask, ExecutionPlan,
  Action, Run and Approval fields, requiredness and nullability.
- Specify UUIDv7 strong IDs, opaque references, extensible surfaces, action symbols,
  canonical UTC millisecond timestamps and bounded integer primitives.
- Define integer-only CostEstimate, SourcePrecondition, OutcomeCertainty coupling
  and ordered unique RiskSummary; reference existing enum and FSM sources.
- Invalidate all stale approval bindings while revoking only pending/approved and
  preserving other historical states. Re-gate the new action/approval version,
  allowing not_required where the canonical gate mapping permits it.
- Bind source-freshness mismatch handling to that policy. ADR-0031 amends only
  ADR-0028's approval-state clause and retains its source-freshness decision.
- Harden Spec Guard and update acceptance M014, golden vectors and generated
  fragments to enforce the clarified contracts.

No runtime implementation, Rust workspace, database migration or narrative master
rewrite is included. Security behavior continues to require independent per-Action
risk/gate evaluation and prevents stale bindings from authorizing changed payloads.
