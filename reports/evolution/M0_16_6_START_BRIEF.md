# M0.16.6 Start Brief (not started)

Status: **brief only**. No M0.16.6 spec, code or host change exists. Starting M0.16.6 requires a separately
authorized session. M0.16.5 should first be integrated (see `M0_16_5_ARCHITECTURE_REVIEW.md` §7).

## Grounding

- The repository has **no canonical M0.16.6 definition**: no `spec/*.yaml` has `milestone: M0.16.6`,
  and no roadmap lists it. Its scope must therefore be fixed by the Architecture Owner, then encoded in
  canonical YAML first (directive §16).
- Built so far: M0.16.0 supervisor contract, .1 Tier 0, .2 candidate workspace, .3 mutation and Tier 1,
  .4 protected runtime, and .5 restricted Tier-1 runtime. In the directive's per-candidate worker order
  (§3.0.2), the next canonical step after Tier 1 is **screening** (§3.1.2):
  - impact-based **Tier 2** regression (§3.1, §3.1.1);
  - relevant integration and contract tests;
  - classification of the candidate outcome.
  Self-correction (§3.2), promotion (§3.5) and recovery points (§4) come later or in parallel and need
  more authority than M0.16.5 granted.

## Recommended scope: "M0.16.6 — Impact-based Tier-2 screening in the restricted runtime"

1. **Canonical first.** Add `spec/evolution_tier2_screening.yaml` (`milestone: M0.16.6`) with:
   - the impact-selection contract (protected evaluation surface, §3.1.1): source → crate → test mapping
     for the single EVOLVABLE path, a fixed allowlisted test set, and no candidate-supplied selection;
   - the screening outcomes `PROMOTION_ELIGIBLE | RETAINED_AS_STEPPING_STONE | REJECTED | INFRA_ERROR | CANCELLED`,
     where `PROMOTION_ELIGIBLE` grants nothing (promotion stays forbidden);
   - the rule that a lower-tier pass never satisfies a higher-tier gate.
   Extend the guard and validators, regenerate derived artifacts, and record the next free ADR number
   (**ADR-0049**).
2. **Runtime.** Reuse the M0.16.5 restricted runner unchanged: same AppContainer, Null capability, Job,
   ACL plan and fixed environment. The only addition is a fixed, host-selected Tier-2 command list. Do not
   add new capabilities, network access, writable roots or Git authority.
3. **Evidence.** Journal Tier-2 results in the existing hash chain, and prove the boundary with
   real-runtime tests. A full Tier-2 run should happen only after the targeted probes pass.
4. **Out of scope:** promotion and §3.5 commit, self-correction, snapshots and recovery points,
   stepping-stone archive persistence beyond the journal, provider or Round Table access, and deployment.

## Entry conditions

- M0.16.5 is integrated via a green CI run on the exact SHA and a merge commit (or fast-forward) to `main`.
- The prepared LAB host is re-validated with `Prepare-M0165NullStdinCapability.ps1 -ValidateOnly`, the
  readiness probe and the E4 probes.
- The Architecture Owner confirms the scope above, or names a different one (e.g. F5 durable Gate
  attestation, or F1 orphan reconciliation, if non-LAB deployment comes first).

## First actions of the M0.16.6 session

1. Run `git status`, check HEAD, and fetch. Confirm M0.16.5 is on `main` or on the agreed base.
2. Re-read the v5.1 directive §3.0.2–§3.1.2, §9 and §11, `spec/evolution_mutation_tier1.yaml`,
   `spec/evolution_protected_runtime.yaml`, ADR-0048 and the M0.16.5 review follow-ups F1–F7.
3. Produce a read-only plan and draft the canonical YAML. Write no runtime code before the YAML exists
   and validates.
