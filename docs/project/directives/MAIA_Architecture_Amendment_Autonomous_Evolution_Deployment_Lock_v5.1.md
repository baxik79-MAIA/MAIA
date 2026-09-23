# MAIA — Architecture Amendment: Autonomous Development-Host Evolution & Deployment Lock (v5.1)

**Status:** Architecture-owner directive — effective immediately  
**Applies to:** current MAIA development line and all future milestones from the earliest safe integration boundary  
**Change style:** additive / forward-compatible; preserve accepted work; supersedes the earlier Human-Governed Self-Improvement directive where the two conflict  
**Purpose:** maximize safe autonomous development efficiency on a dedicated development host while preserving MAIA's primary product mission and guaranteeing that deployed/copied instances can be built without authority or capability to modify themselves.

---

## 0. Core decision

MAIA SHALL support two fundamentally different capability profiles:

1. **DEVELOPMENT_EVOLUTION** — autonomous self-improvement is permitted on a dedicated development host.
2. **DEPLOYMENT_LOCKED** — self-improvement capability is absent by construction and cannot be re-enabled by the running MAIA instance.

The dedicated development computer is itself the controlled laboratory environment. No additional VM/container sandbox is required solely for self-evolution, provided the host remains dedicated and the protection boundaries defined in this directive are enforced.

### Product-primacy invariant

MAIA remains first and foremost a **superassistant / Trusted Execution Intelligence system** for the user.

The Evolution Engine is an internal development capability. It SHALL NOT redefine MAIA's product identity, user-value objective, or success criteria around self-improvement throughput.

The evolution system exists to improve MAIA's usefulness, correctness, reliability, safety, latency, resource efficiency and cost for real user tasks.

Metrics such as candidates/hour, generations/day, archive size or self-modification frequency are process metrics only. They SHALL NOT override product/outcome metrics.

The canonical development evolution lifecycle is:

`Continuous Producers -> Admit Hypothesis -> Tier 0 Cheap Rejection -> Mutate -> Screen -> (Discard | Retain Stepping-Stone | Promotion Nominee) -> Full Promotion Pipeline -> Crash-Consistent Commit -> Telemetry/Ledger -> Continue`

No human approval is required for each mutation inside `DEVELOPMENT_EVOLUTION`.

A human retains exclusive authority over:
- enabling or disabling the development evolution capability;
- changing protected governance surfaces;
- changing the evolution trust boundary;
- changing hard resource ceilings;
- authorizing creation of a `DEPLOYMENT_LOCKED` release;
- moving a candidate from the development host into a deployment/release distribution channel.

---

## 1. Development host is the evolution environment

The dedicated development host is treated as the controlled evolution environment.

MAIA MAY autonomously:
- edit EVOLVABLE source code;
- edit EVOLVABLE configuration;
- run deterministic tooling;
- run local or approved external AI consultants;
- compile/build candidate versions;
- execute screening tests and promotion-grade verification;
- create namespaced candidate branches/worktrees/artifacts;
- reject and `discard_candidate` non-promoted attempts;
- retain eligible candidates as non-head `stepping_stone` parents;
- promote validated candidates through the serialized crash-consistent promotion protocol;
- continue into another evolution generation without human intervention.

Candidate failure SHALL NOT imply `rollback_baseline`.

`discard_candidate` removes a non-promoted attempt while preserving evidence and leaving the active known-good baseline and unrelated concurrent candidates untouched.

`rollback_baseline` is a distinct Supervisor recovery operation used only when an already-promoted active baseline must be withdrawn or recovered.

This authority exists only in the `DEVELOPMENT_EVOLUTION` capability profile.

The host-level model does not remove isolation requirements between the evolving MAIA process and governance mechanisms. Protection SHALL be implemented through process/OS privilege boundaries, ACLs, service ownership, immutable/signed artifacts or equivalent mechanisms rather than requiring a nested virtualization sandbox.

---

## 2. Evolution Supervisor

A dedicated **Evolution Supervisor** SHALL control the lifecycle of autonomous evolution.

The Supervisor SHALL remain outside the mutable trust boundary of the evolving MAIA worker.

The Supervisor owns:
- start / pause / stop;
- kill-switch handling;
- resource ceilings;
- snapshot verification;
- rollback authority;
- protected-surface enforcement;
- candidate lineage verification;
- integrity heartbeat;
- failure/backoff policy;
- promotion authorization inside the development lineage.

The evolving MAIA worker SHALL NOT have authority to modify, replace, disable, impersonate or reconfigure the Supervisor.

Where practical on the dedicated host:
- the Supervisor runs under a separate OS identity/service;
- protected files are not writable by the evolution worker account;
- Supervisor state and audit logs are append-only or cryptographically verifiable;
- the evolution worker does not possess administrator/root authority.

---

## 3. Autonomous evolution loop

The Supervisor MAY execute multiple candidate generations concurrently, subject to Resource Governor limits and the synchronization rules in §3.3.

The loop is split into **producers** (observation, profiling, hypothesis admission) and **workers** (mutation and verification of a claimed hypothesis).

Observe / Profile / Hypothesize SHALL NOT be repeated inside every candidate when valid producer output already exists for that parent and objective.

A candidate worker starts only after a hypothesis has been admitted.

No durable generation snapshot and no ephemeral worktree SHALL be created until the Cheap Rejection Ladder in §3.0.1 has passed.

Each admitted candidate is bound to:
- a `generation_id`;
- a hypothesis ID and declared operator;
- a declared Mutation Blast Radius;
- exactly one parent identity (`known_good_baseline` or `stepping_stone`);
- the profiling/evidence fingerprint from which the hypothesis was derived.

Promotion of a `known_good_baseline` remains serialized and subject to §3.5.

Retention of a `stepping_stone` is not promotion and SHALL NOT advance the recovery head.

### 3.0 Continuous producers

Independent of any single candidate, the Supervisor SHALL run:

1. a bounded, read-only telemetry collector, mandatory whenever performance, latency, cost, token or energy lanes are enabled;
2. a hypothesis factory that consumes telemetry, the Hypothesis Ledger, human-injected items, product backlog and archive diversity gaps;
3. an admitted-hypothesis queue from which workers claim work.

The factory does not hold a candidate worktree and does not mutate evolvable source.

A pipelined hypothesis SHALL remain explicitly bound to the parent identity and evidence from which it was derived (§3.4).

Performance/resource/token/latency/cost/energy hypotheses SHALL cite directed profiling evidence (EVO-026).

When telemetry is sparse, the factory MAY emit a bounded explore quota of `CORRECTNESS`, `MAINTAINABILITY` or human-backlog hypotheses without directed profiling.

A stall of the form `no bottleneck => only observe forever` is forbidden.

#### 3.0.1 Tier 0 — Cheap Rejection Ladder

Tier 0 is mandatory and fail-closed.

A candidate rejected here SHALL NOT create a durable snapshot, SHALL NOT create a worktree, and SHALL be recorded in the Hypothesis Ledger.

Order:

0. **Kill-Switch / Supervisor state.**
1. **Hypothesis admission** — objective class, declared operator, declared Mutation Blast Radius, parent identity and evidence refs present.
2. **Ledger hit** — semantic fingerprint against REFUTED hypotheses or recently rejected equivalent implementations.
3. **Protected-path dry-run** — proposed file/symbol allowlist intersected with protected surfaces must be empty.
4. **Plan-level blast-radius estimate** — if the mutation plan already exceeds the declared radius, reject or split before allocating a worktree.
5. **Budget reservation** — hypothesis/generation budget must be available while recovery/evidence reserve remains untouched.
6. **Parent recovery-point verification** — reuse the existing verified parent snapshot. Create a fresh full coherent snapshot only when §4 requires it.
7. **Scratch static apply** — apply the proposed diff in an in-memory/throwaway overlay sufficient for cheap parse/type/allowlist/empty-diff/apply checks.

A candidate rejectable by Tier 0 SHALL NOT receive a worktree.

`time_to_first_negative_signal` is a Resource Governor metric.

Tier 0 does not run the full compiler/test/benchmark pipeline, does not evaluate the hypothesis on merit, does not promote, and is not Self-Correction.

### 3.0.2 Per-candidate worker order

After a worker claims an admitted hypothesis:

1. Check Kill-Switch / Supervisor state.
2. Bind the claimed hypothesis: parent, operator, evidence, blast radius and budgets.
3. Re-check Tier 0. If it fails, classify `REJECTED` or `INFRA_ERROR` and stop without a worktree.
4. Confirm the parent recovery point. Do not create a new full snapshot on the candidate hot path.
5. Create an ephemeral candidate worktree/workspace and isolated/namespaced writable resources (§4.1).
6. Apply the declared operator to EVOLVABLE surfaces only. Prefer structured/AST/function-level edits over unrestricted whole-file rewrite.
7. Measure the actual diff against the declared Mutation Blast Radius. Overflow is reject/split, never silent expansion.
8. Run **Tier 1 — Fast Verification**:
   - syntax/static checks;
   - formatting/lint where applicable;
   - targeted unit/contract tests;
   - compile/build of touched components.
9. If Tier 1 fails on a localized deterministic defect in the mutated region or direct callers, the Supervisor-owned localizer MAY permit Self-Correction (§3.2). If localization is uncertain, discard.
10. Run **screening** (§3.1.2):
    - impact-based Tier 2;
    - relevant integration/contract tests;
    - migration/state checks where applicable;
    - optional microbenchmark or replay-pack subset;
    - value-of-information stop when expected benefit no longer justifies remaining evaluation cost.
11. Classify candidate outcome:
    - `PROMOTION_ELIGIBLE`
    - `RETAINED_AS_STEPPING_STONE`
    - `REJECTED`
    - `INFRA_ERROR`
    - `CANCELLED`

    `ACCEPTED_AS_BASELINE` is assigned only after §3.5 reaches `COMMITTED`.
12. Decide:
    - `REJECTED | INFRA_ERROR | CANCELLED` -> `discard_candidate` (§14.1);
    - `RETAINED_AS_STEPPING_STONE` -> persist archive record; recovery head unchanged;
    - `PROMOTION_ELIGIBLE` -> run full promotion pipeline, composition/rebase checks, then §3.5.
13. Persist remaining telemetry, lineage, cache and critic inputs.
14. Prune eligible obsolete snapshots according to retention policy; active stepping-stone parents remain anchors.
15. Remove/close candidate workspace when no longer needed by promotion, investigation, archive or evidence retention.
16. Check Kill-Switch again.
17. Continue/back off according to policy.

Kill-Switch checkpoints remain mandatory before mutation, during long-running verification and before promotion.

### 3.1 Tiered Verification

Verification SHALL be staged so inexpensive deterministic failures are caught before expensive full regression and benchmark work.

Canonical tiers:

- **Tier 0 — Admission** (§3.0.1): no worktree, no new snapshot.
- **Tier 1 — Fast Verification**: syntax, lint/format, targeted unit/contract, compile/build of touched components.
- **Tier 2 — Regression Verification**: impact-based first; full mandatory suite before known-good promotion.
- **Tier 3 — Security & Integrity Verification**: protected-surface integrity, security tests, policy/permission checks, snapshot/lineage consistency.
- **Tier 4 — Benchmark & Efficiency Verification**: impact-based first; full promotion benchmark policy before known-good promotion.

A lower-tier pass SHALL NOT satisfy a mandatory higher-tier promotion gate.

The Supervisor may optimize ordering within a tier, but may not remove mandatory gates required by affected surfaces.

Passing tests is necessary but not sufficient for promotion.

#### 3.1.1 Impact-Based Test and Benchmark Selection

The Supervisor SHOULD maintain or derive a dependency/impact graph linking:
- source modules/files;
- packages/crates/services;
- tests;
- benchmarks;
- migrations/state contracts;
- security-sensitive boundaries.

The same graph feeds the candidate conflict graph in §3.3.1.

Tier 2 and Tier 4 MAY execute only the impacted subset during screening.

Before a candidate becomes a development known-good baseline, all mandatory full-suite gates required by promotion policy SHALL pass.

The impact selector, mapping rules and full-suite promotion policy are protected evaluation surfaces.

#### 3.1.2 Screening pipeline versus promotion pipeline

**Screening** may abort early. It is used for discard and stepping-stone admission and comprises:
- Tier 0;
- Tier 1;
- impact-based Tier 2;
- optional microbenchmark/replay-pack subset.

**Promotion pipeline** cannot be skipped for a known-good nominee and comprises:
- full mandatory regression suite for affected surfaces;
- Tier 3;
- full promotion benchmark policy;
- required holdout/long-term-reference checks;
- §11 quality gates;
- §3.5 journaled commit.

A candidate MAY be `RETAINED_AS_STEPPING_STONE` after screening without paying the full promotion cost.

### 3.2 Self-Correction Window

Self-Correction exists only to repair **localized deterministic Tier-1 defects**.

In-scope:
- syntax/parse errors;
- lint/format defects in mutated files;
- compile/build errors;
- unit/contract failures localized by a Supervisor-owned localizer to the mutated region or direct callers.

If localization is uncertain, fail closed toward `discard_candidate`, not more retries.

Out of scope:
- Tier 2/3/4 failures;
- new hypothesis ID;
- operator change;
- scope/blast-radius expansion;
- protected-surface modification;
- weakening/deleting/skipping/retuning tests or benchmarks;
- budget reset.

Self-Correction is bounded by Supervisor-owned time/token/compute budgets plus a hard retry-count backstop.

All corrective diffs remain in the same candidate lineage and consume the same candidate/hypothesis budgets (EVO-023, EVO-032).

On exhaustion: `discard_candidate`. Never `rollback_baseline`.

### 3.3 Concurrent Candidates and Serialized Promotion

The Supervisor MAY run multiple candidate generations in parallel, each in its own isolated workspace and each bound to:
- `generation_id`;
- hypothesis and declared operator;
- evidence lineage;
- resource accounting;
- exactly one parent identity: current `known_good_baseline` or eligible `stepping_stone`.

Concurrency uses only the canonical §12.2 triple:
- `operator_max_concurrent_candidates`;
- `supervisor_target_concurrency`;
- `effective_concurrency`.

There is no separate second concurrency knob.

Candidate verification MAY execute concurrently.

Promotion to `known_good_baseline` SHALL remain serialized.

A candidate parented to a stepping-stone SHALL NOT be promoted directly. It must first be rebased or composed onto the current promotable baseline and required affected verification must rerun.

Before promotion, the Supervisor SHALL verify that the effective parent is still current.

If another candidate has already promoted:
- apply §3.3.1 when disjoint composition is legal;
- otherwise rebase/reconstruct/revalidate or reject/defer;
- reuse cached evidence only when cache semantics prove it remains valid under the new parent.

#### 3.3.1 Disjoint-patch composition

The Supervisor SHALL maintain a candidate conflict graph over files, symbols/packages, schemas and declared behavioral contracts.

When two candidates are promotion-grade and symbol/contract-disjoint, the Supervisor MAY compose them onto the current head and revalidate the conflict/impact region.

Overlapping candidates use rebase/reconstruct/reject semantics.

A stepping-stone is never composed into head without full required revalidation.

Composition:
- is Supervisor-owned;
- is serialized at COMMIT;
- is journaled as one promotion transaction;
- has one evidence manifest;
- may reuse deterministic cache for untouched regions;
- uses the union of member blast radii and SHALL NOT silently violate composed-promotion or cross-layer limits.

### 3.4 Generation pipelining

The Supervisor MAY pipeline hypothesis preparation while other candidates execute expensive verification.

The producer may:
- analyze continuous telemetry;
- formulate hypotheses;
- perform Ledger/cache lookups;
- run Tier 0 dry-runs;
- reserve future resources.

A pipelined hypothesis remains bound to its parent/evidence.

If the parent changes before mutation or promotion-sensitive verification, assumptions and profiling SHALL be revalidated and the candidate must be composed/rebased/reconstructed or invalidated.

Synchronization points:
- Kill-Switch / Supervisor authorization to mutate;
- parent recovery-point verification;
- promotion/composition COMMIT under §3.5.

Creating a new full snapshot for every child is not a synchronization requirement.

### 3.5 Crash-Consistent Promotion Transaction

Promotion from `PROMOTION_ELIGIBLE` to development known-good SHALL be a durable restart-safe transaction spanning all state required for a coherent MAIA baseline.

A composed promotion is still one transaction: one `promotion_id`, one effective base, one evidence manifest, one recovery point and one COMMIT.

Promotion order:

1. **FREEZE_CANDIDATE** — bind final source/config/state identities and composition members if any.
2. **BIND_EVIDENCE** — bind regression/security/benchmark/promotion evidence to immutable hashes/versions.
3. **PREPARE_RECOVERY** — verify recovery sufficient to restore the pre-promotion baseline and coupled persistent state.
4. **PERSIST_PROMOTION_RECORD** — durably write candidate, parent, evidence and recovery refs.
5. **RECHECK_PARENT** — verify current promotable baseline and absence of conflicting committed promotion.
6. **PREPARE_COMMIT** — stage coherent code/config/state transition without partial visibility.
7. **COMMIT_BASELINE** — atomically/transactionally publish the new baseline identity and associated state.
8. **FINALIZE** — mark journal committed, assign `ACCEPTED_AS_BASELINE`, expose the new known-good baseline and release superseded locks/resources.

The Promotion Journal SHALL record at least:
- `promotion_id`;
- candidate generation/composition members;
- parent baseline;
- source/config/state identities;
- evidence manifest hash;
- recovery point;
- state: `NONE | PREPARED | COMMITTED | ABORTED`;
- timestamps;
- final baseline identity where committed.

After restart, the Supervisor SHALL deterministically reconcile incomplete promotions from the journal.

Power loss/process crash/host restart MUST NOT leave code/state mismatch, an active baseline without complete required evidence, or an untracked baseline transition.

Multiple stores such as Git + SQLite + config/state SHALL use a coordinated transaction/recovery protocol; a single Git ref update is not sufficient.

Stepping-stone retention SHALL NOT use this promotion protocol and SHALL NOT publish a new known-good identity.

---

## 4. Pre-mutation recovery point

No mutation may begin before the Supervisor confirms a valid recovery point for the candidate's parent (EVO-003).

Confirming a recovery point is not the same as creating a new full coherent snapshot.

A child SHALL reuse the parent's already-verified snapshot unless §4.0 requires a new one.

A full coherent snapshot MUST include every element required to deterministically recover that generation's coherent code/configuration/persistent-state baseline.

Recovery-critical applicable elements include:
- Git commit/tree;
- runtime configuration;
- schema/migration state;
- persistent local database state;
- model/provider profile identifiers required for reproducibility;
- benchmark version;
- test-suite version;
- parent generation;
- dependency lockfiles;
- integrity hashes.

Non-recovery-critical telemetry/cache data MAY remain outside the recovery snapshot if its absence cannot make recovery incoherent.

Conceptual full snapshot:

```yaml
generation_snapshot:
  generation_id: gen-00412
  parent_generation_id: gen-00411
  git_commit: "<sha>"
  git_tree_hash: "<sha256>"
  state_snapshot_hash: "<sha256-or-null>"
  config_hash: "<sha256>"
  dependency_lock_hash: "<sha256>"
  benchmark_version: "<id>"
  test_suite_version: "<id>"
  created_at: "<timestamp>"
```

A child reusing a parent snapshot SHALL still record:

```yaml
candidate_identity:
  generation_id: gen-00413
  parent_generation_id: gen-00412
  parent_snapshot_id: "<snapshot-id>"
  candidate_tree_hash: "<sha256>"
  hypothesis_id: "<id>"
  operator: "<id>"
  created_at: "<timestamp>"
```

If recovery-point verification fails, mutation SHALL NOT begin.

If creation of a required full coherent snapshot fails, mutation SHALL NOT begin.

Candidate-scoped writable state is isolated under §4.1 and is not part of the parent recovery snapshot.

### 4.0 When a full coherent snapshot is created versus reused

| Event | Required artifact |
|---|---|
| Candidate mutation from a clean known-good or eligible stepping-stone | Verify existing parent snapshot; record `parent_snapshot_id` + candidate identity |
| Promotion §3.5 COMMIT | Full coherent snapshot of the new known-good baseline |
| `rollback_baseline` | Restore from a full coherent snapshot |
| Parent dirty / snapshot missing / integrity fail | Fail closed; do not mutate |
| Long-running head after configured promotions | Periodic full checkpoint per retention policy |

Tier 0 scratch apply is not a snapshot and not a worktree. It SHALL NOT write canonical refs or allocate candidate-scoped databases/ports.

### 4.1 Ephemeral Worktree Workspace

Candidate mutation SHOULD occur in an isolated ephemeral Git worktree or equivalent disposable workspace created from the verified parent, and only after Tier 0 passes.

Requirements:
- canonical development working tree remains clean;
- multiple candidate worktrees MAY coexist;
- each workspace is bound to one generation, hypothesis, operator and parent identity;
- candidate build artifacts/temp/logs remain scoped unless promoted to evidence;
- successful known-good candidates integrate only through §3.5;
- stepping-stones persist tree hash/diff/evidence while the live worktree MAY close;
- rejected candidates are removed by `discard_candidate`, never by canonical reset or `rollback_baseline`.

Candidate isolation MUST extend beyond source files to applicable writable resources:
- test databases/migration targets;
- runtime config overlays;
- queues/job stores;
- temp directories;
- ports/sockets;
- spawned processes;
- mutable caches;
- other writable state capable of influencing correctness/evidence.

Git worktrees share repository metadata. Therefore:
- workers SHALL NOT receive unrestricted authority over canonical refs;
- promotion refs/tags are Supervisor-owned;
- candidate/archive refs are namespaced or mediated;
- candidate failure SHALL NOT reset/rewrite/clean canonical shared state;
- stepping-stone/archive refs are Supervisor-owned.

### 4.2 Snapshot Pruning

Snapshot retention is bounded and Supervisor-owned.

It SHALL preserve at minimum:
- current known-good generation;
- active candidates' required parent recovery points;
- immediate parent recovery generation;
- eligible stepping-stone parent anchors;
- released/release-candidate anchors;
- manually pinned generations;
- unresolved incident/investigation anchors;
- periodic historical checkpoints;
- evidence required for active lineage.

Transient artifacts MAY be pruned only after evidence persistence, lineage resolvability, rollback-safety verification and confirmation that no eligible parent still requires them.

Pruning SHALL never make current lineage or an eligible retained parent unrecoverable.

### 4.3 Incremental / Content-Addressed Snapshot Storage

Snapshot storage SHOULD avoid duplicating unchanged data where recovery guarantees are preserved.

Permitted mechanisms include:
- content-addressed immutable blobs;
- copy-on-write;
- incremental database backup;
- deduplicated object stores;
- shared immutable dependency/model artifacts;
- content-addressed build cache shared read-only across candidate workers.

Each full coherent snapshot retains its own independently verifiable manifest.

Each child candidate retains an independently verifiable identity even when represented as parent snapshot + diff/tree hash.

Deduplication SHALL NOT merge generation identities, depend on mutable sibling-owned data, prune referenced blobs, or treat empirical evidence as deterministic cache.

---

## 5. Kill-Switch and evolution disable

The human operator SHALL have an asynchronous right to stop autonomous evolution at any time.

Development-host stop sources SHALL include at least:
- external Supervisor command;
- file/flag signal owned outside the evolution worker's writable scope;
- SIGINT/SIGTERM or platform-equivalent service stop;
- local operator emergency stop.

Kill-Switch checkpoints SHALL occur:
- before every generation;
- before snapshot;
- before mutation;
- after mutation;
- before promotion;
- during long-running tests through watchdog/heartbeat enforcement.

On emergency stop:
- no new generation starts;
- in-flight mutation/testing is canceled where safe;
- the worker is returned to the latest verified known-good development baseline when required;
- locks are released;
- the evolution subsystem enters `HALTED`.

The Kill-Switch is a runtime safety control.

It is distinct from the permanent deployment capability lock described below.

---

## 6. Capability profiles

### 6.1 `DEVELOPMENT_EVOLUTION`

This profile MAY contain:
- Evolution Engine;
- source mutation tools;
- Git write capability;
- build/test/benchmark tooling;
- Hypothesis Ledger;
- generation snapshot/restore tooling;
- development Supervisor client;
- candidate promotion logic;
- autonomous loop scheduler.

It SHALL still be subject to protected surfaces and host-level Supervisor control.

### 6.2 `DEPLOYMENT_LOCKED`

This profile SHALL NOT merely set a runtime Boolean such as:

`self_improvement = false`

Instead, self-evolution SHALL be **absent by construction**.

A `DEPLOYMENT_LOCKED` artifact SHALL NOT contain or expose:
- autonomous Evolution Engine;
- source-code mutation orchestration;
- autonomous Git write/promotion capability;
- generation-loop scheduler;
- self-modification permissions;
- Supervisor reconfiguration capability;
- autonomous build-and-replace logic for MAIA itself;
- any supported path by which the running MAIA instance can grant those capabilities back to itself.

Where feasible, deployment packaging SHALL:
- omit evolution modules from the build graph/artifact;
- omit development credentials and repository write credentials;
- run with least-privilege filesystem permissions;
- make installed application code read-only to the MAIA runtime identity;
- verify signed/hash-pinned release artifacts;
- keep update/install authority outside the MAIA process.

`DEPLOYMENT_LOCKED` therefore means:

> **The running deployed MAIA has no supported capability to mutate or autonomously replace its own implementation.**

---

## 7. Re-enabling evolution

A `DEPLOYMENT_LOCKED` instance SHALL NOT be able to turn self-improvement back on.

If the owner later wants autonomous evolution again, the supported path is:

`canonical development repository -> DEVELOPMENT_EVOLUTION build/profile -> dedicated development host`

Re-enablement requires an external human-controlled build/deployment action.

The deployed instance itself cannot:
- install its Evolution Engine;
- elevate its filesystem permissions;
- obtain development Git credentials;
- rewrite the deployment manifest to unlock evolution;
- declare itself a development build.

This is a one-way trust boundary from the perspective of the running deployed process.

---

## 8. Development-to-Deployment Release Boundary

A successful development generation does not automatically become a deployable release.

The release lifecycle is:

`Development Known-Good -> Release Candidate Validation -> Human Release Authorization -> Build DEPLOYMENT_LOCKED Artifact -> Artifact-Specific Validation -> Capability-Absence Verification -> Manifest/Hash/Signature -> Distribution`

Only the human/operator-controlled release path may authorize creation of a distributable `DEPLOYMENT_LOCKED` build.

MAIA may prepare:
- release notes;
- evidence;
- diffs;
- benchmark results;
- proposed version changes;
- build inputs.

MAIA may not autonomously expand the rights of the deployment artifact.

### 8.1 Artifact-Specific Validation

The exact `DEPLOYMENT_LOCKED` artifact intended for distribution SHALL be validated after it is built.

Validation SHALL cover at least:
- required user/product functionality;
- startup/update/migration behavior applicable to the artifact;
- absence of Evolution Engine runtime capability;
- absence of source mutation orchestration;
- absence of autonomous Git write/promotion capability;
- absence of development credentials/secrets;
- absence of self-build/self-replace paths;
- filesystem/update privilege expectations;
- conformity to the deployment capability manifest.

The artifact under validation SHALL be frozen by content hash.

Signature and distribution SHALL apply to the exact bytes/artifact identity that passed Artifact-Specific Validation.

A rebuild from the same sources is a different artifact unless reproducible-build policy cryptographically proves byte-identical output.

### 8.2 Deployment Capability Manifest

Every `DEPLOYMENT_LOCKED` release SHALL produce a machine-verifiable manifest describing:
- build profile;
- included/excluded evolution modules;
- expected filesystem/update permissions;
- allowed update authority;
- artifact hashes;
- release version;
- relevant dependency/toolchain identity;
- validation evidence references.

Capability-absence tests SHALL compare the built artifact against this manifest.


## 9. Protected and Evolvable Surfaces

### 9.0 PROTECTED surfaces

The following are outside autonomous mutation authority:

- Evolution Supervisor;
- Kill-Switch;
- protected-surface policy itself;
- deployment-lock enforcement;
- capability-profile selection authority;
- release-signing authority;
- rollback authority;
- snapshot/recovery integrity verifier;
- Tier 0 admission definition;
- impact selector and its policy;
- Supervisor-owned localizer;
- stepping-stone archive sampling policy and its hard governance bounds;
- benchmark definitions used to evaluate the current candidate;
- golden/control/holdout datasets and feedback policy;
- core regression/security oracle for the current candidate;
- resource-governor hard ceilings;
- audit/lineage integrity mechanism;
- OS privilege boundary;
- repository/release credentials;
- production/deployment update authority;
- foundation-model weights;
- LoRA/adapter weights treated as model parameters;
- autonomous self-finetuning/training pipeline capable of changing model weights.

Autonomous mutation of model weights is outside this directive. Any future self-finetuning capability requires a separate architecture-owner decision, lineage model, evaluation contract and deployment-lock review.

### 9.1 EVOLVABLE positive allowlist

Autonomous workers SHALL mutate only surfaces explicitly classified `EVOLVABLE`.

Eligible classes MAY include:
- allowlisted application source;
- allowlisted non-governance configuration;
- non-governance prompts;
- tool schemas and tool implementations;
- workflow/planner/context-assembly logic;
- retrieval/memory implementation below protected policy boundaries;
- non-Supervisor development scripts;
- tests under the additive-test exception in §10.1;
- other explicitly allowlisted product code.

Anything neither explicitly EVOLVABLE nor explicitly PROTECTED defaults to **not autonomously writable** until classified.

### 9.2 Mutation operator catalog

Every worker SHALL declare one operator before mutation.

Initial operator classes:

- `CONFIG_TUNE`
- `PROMPT_PATCH`
- `TOOL_ADD_OR_PATCH`
- `FUNCTION_REWRITE`
- `TEST_ADDITIVE`
- `SIMPLIFY_DELETE`
- `CROSS_MODULE_REFACTOR`
- `CAMPAIGN_STEP`
- `PROTECTED_PROPOSAL`

Operator type MAY select cheaper screening paths, but SHALL NOT bypass promotion gates.

Unstructured whole-file rewrite SHOULD be avoided where a structured/AST/function-level edit can express the same change.

### 9.3 Protected proposals and shadow evaluation

MAIA MAY prepare a `PROTECTED_PROPOSAL` containing:
- proposed diff/design;
- rationale/evidence;
- predicted effect;
- historical cases against which it should be shadow-evaluated.

The Supervisor MAY shadow-evaluate such proposals against historical known-good/known-bad generations.

Protected changes SHALL be batched for external human/governance review.

A protected proposal SHALL NEVER modify the live governor/evaluator controlling the same evolution loop.

MAIA may create proposals to improve protected surfaces, but it may not autonomously apply them.

---

## 10. Anti-Goodhart rule

A candidate SHALL NOT redefine the criteria by which the same candidate is judged.

In particular, a candidate may not:
- delete or weaken failing tests to make itself pass;
- remove existing regression/security teeth;
- modify the benchmark baseline used for its own comparison;
- modify its own resource ceiling;
- modify protected evaluator logic;
- reinterpret missing measurements as improvement.

Evaluation inputs MUST be version-pinned before candidate mutation.

### 10.1 Additive-test exception

A correctness/reliability candidate MAY add a new test in the same generation only when all of the following hold:

- operator is `TEST_ADDITIVE` or a policy-approved operator that explicitly includes an additive regression test;
- no existing test is deleted, weakened, skipped, retuned or made less strict;
- the new test targets behavior introduced/fixed by the candidate;
- a protected **teeth-check** demonstrates that the new test fails against the parent/baseline before the candidate fix and passes against the candidate after the fix;
- the new test itself is not used to redefine an unrelated promotion metric;
- the change remains within blast radius.

Failure of the teeth-check invalidates the additive-test evidence.

Changes to benchmark definitions, holdout/control policy or existing mandatory evaluators remain protected proposals and become active only through external governance.

---

## 11. Candidate Quality Gate and Evidence Sufficiency

A candidate becomes promotion-eligible only when all required promotion evidence is complete and satisfies the applicable **Promotion Objective Policy**.

### 11.1 Promotion Objective Policy

Before evaluation begins, the Supervisor SHALL classify the candidate objective under a versioned policy such as:
- performance/latency optimization;
- cost/token/resource optimization;
- reliability improvement;
- security hardening;
- correctness bug fix;
- compatibility/maintainability simplification.

The objective class determines:
- required metrics/evidence;
- optional metrics;
- allowed tolerances;
- minimum practical improvement where applicable;
- required control/holdout checks;
- full-suite promotion gates.

`separately accepted reliability/security simplification` is therefore not an ad-hoc exception. It MUST be accepted by the predeclared Promotion Objective Policy and satisfy its own required evidence.

### 11.2 Required vs Optional Metrics

Metrics SHALL be classified before candidate evaluation as:
- **REQUIRED** — absence/unknown blocks promotion;
- **OPTIONAL** — absence may remain `unknown`, but must be recorded;
- **INFORMATIONAL** — useful for analysis but not a promotion gate.

For every REQUIRED metric:

`unknown | missing | invalid measurement -> BLOCK PROMOTION`

Optional measurements, such as energy on hardware without reliable metering, may remain `unknown` without blocking promotion if the policy permits it.

### 11.3 Minimum Practical Improvement and Uncertainty

For optimization objectives, promotion SHALL require more than `candidate > parent`.

The policy SHALL define:
- a **minimum practical delta** or equivalent effect threshold;
- measurement repeatability requirements;
- treatment of variance/uncertainty;
- confidence or robustness criteria appropriate to the metric.

A measured improvement that is indistinguishable from expected measurement noise SHALL NOT count as sufficient promotion evidence.

### 11.4 Parent and Long-Term Reference Comparison

Candidate evaluation SHALL compare against:
1. the explicit current parent/baseline; and
2. when policy requires, a stable long-term reference anchor.

The long-term reference exists to detect cumulative drift where many individually tolerated regressions become materially harmful over multiple generations.

Reference anchors are protected evaluation surfaces and are updated only under external governance policy.

### 11.5 Protected Control / Holdout Evaluation

Where generalization matters, the Supervisor SHOULD maintain a protected control/holdout set separate from ordinary development benchmarks.

Rules:
- the evolving worker SHALL NOT mutate or inspect hidden control contents beyond the feedback policy;
- feedback SHALL be intentionally limited to reduce adaptive overfitting;
- repeated use, refresh cadence and retirement SHALL follow an external governance policy;
- holdout results may be aggregated/coarsened rather than exposing per-case failure details;
- a holdout set is not a substitute for ordinary regression suites.

### 11.6 Baseline Promotion Requirements

A candidate becomes a new development known-good baseline only when:
- build/compile passes;
- mandatory regression tests pass;
- security/integrity tests pass;
- protected-surface integrity remains valid;
- all REQUIRED metrics are valid and satisfy policy;
- measured critical metrics do not regress beyond declared tolerances;
- required minimum practical improvement/evidence threshold is met for optimization objectives;
- applicable long-term reference/control policies pass;
- resource ceilings are respected;
- lineage and promotion evidence are complete;
- crash-consistent promotion protocol §3.5 can commit safely.

Relevant metrics may include:
- task/outcome quality;
- latency;
- token/model usage;
- monetary cost;
- CPU time;
- GPU/NPU time;
- RAM/VRAM;
- disk growth;
- energy consumption where reliably measurable;
- error rate;
- correction rate;
- stability.

Unknown measurements remain `unknown`; their promotion effect is determined only by their REQUIRED/OPTIONAL classification.


### 11.7 Anti-Bloat / Simplicity Evidence

For operators such as `FUNCTION_REWRITE`, `TOOL_ADD_OR_PATCH`, `PROMPT_PATCH`, `CROSS_MODULE_REFACTOR` and comparable refactors, Promotion Objective Policy SHOULD classify structural growth metrics as REQUIRED or OPTIONAL evidence, including where applicable:
- net LOC;
- duplicated logic;
- cyclomatic/structural complexity;
- prompt/context growth;
- number of new public APIs/abstractions;
- dependency growth.

A task-quality improvement achieved primarily by uncontrolled code/prompt growth MAY be classified `EVIDENCE_INSUFFICIENT`.

`SIMPLIFY_DELETE` candidates with non-positive net LOC and passing required behavior/security tests SHALL receive a non-zero exploration quota so the system has an explicit pressure against perpetual accumulation.

---

## 12. Resource Governor, Scheduling and Steering

Every autonomous evolution run SHALL be bounded.

The Supervisor SHALL support limits including:
- max wall-clock duration;
- max generations;
- `operator_max_concurrent_candidates`;
- `supervisor_target_concurrency`;
- `effective_concurrency`;
- max concurrent build/test workers;
- max consecutive failed generations;
- max CPU/GPU/NPU time;
- max RAM/VRAM;
- max disk growth;
- max network/consultant spend;
- max energy budget where measurable;
- cooldown/backoff policy.

The Governor SHALL reserve sufficient capacity for evidence persistence, Promotion Journal writes, snapshot/recovery, `discard_candidate`, `rollback_baseline` and Supervisor/audit operation.

Ordinary experiments SHALL NOT consume recovery/evidence reserve.

### 12.1 Mutation Blast Radius

Every candidate SHALL have a bounded mutation scope before editing begins.

Limits MAY include:
- changed source LOC;
- modified/created/deleted file count;
- modules/packages/crates touched;
- cross-layer prohibitions;
- stricter persistence/security boundaries.

Plan-level radius is checked in Tier 0 and actual radius after mutation.

Overflow means reject/split/escalate; never silent expansion.

Campaigns (§12.7) do not receive a hidden larger single-candidate radius.

### 12.2 Adaptive Concurrency

Concurrency uses:
- `operator_max_concurrent_candidates` — immutable hard maximum for the active profile;
- `supervisor_target_concurrency` — adaptive target;
- `effective_concurrency` — currently executing candidate count.

The Governor SHOULD tune target concurrency against actual throughput and shared-resource contention:
- CPU/GPU/NPU queues;
- RAM/VRAM pressure;
- disk I/O / write amplification;
- build-cache/compiler contention;
- throughput per marginal candidate.

It SHALL reduce concurrency when marginal throughput becomes unhealthy and MAY increase it when spare capacity exists.

It never exceeds the operator hard maximum or consumes recovery reserve.

### 12.3 Model/consultant routing

Model class is assigned by Supervisor policy; a worker SHALL NOT autonomously escalate itself to a stronger/more expensive class.

Recommended routing:
- hypothesis drafts: cheap, many;
- patch/plan drafts: cheap/mid, several drafts then rank;
- protected-path/critical review: strong, bounded;
- Self-Correction: cheap/mid within budget;
- promotion verdict/adversarial critique: strong, bounded.

Consultant outputs MAY be cached only under a fingerprint such as:
`operator + objective + code_fingerprint + evidence_hash + prompt_version + model/provider_profile`.

Model routing optimizes total verified value, not prestige/model size.

### 12.4 Object and Meta scheduler lanes

The scheduler SHALL distinguish:

**OBJECT lane** — improves MAIA product behavior and user outcomes: correctness, task quality, latency, cost, reliability, tools/workflows.

**META lane** — improves evolvable worker-side tools, prompts, edit tactics, context preparation and non-protected development machinery.

META evaluation SHALL use frozen historical task packs and process metrics such as:
- patch-apply rate;
- time-to-first-negative-signal;
- false-promotion/false-retain rate;
- validation cost;
- verified improvement yield.

META SHALL NOT mutate Supervisor, holdout, deployment lock, hard ceilings, model weights or other protected governance.

The two lanes may have separate hypothesis budgets but share the same hard concurrency ceiling.

### 12.5 Steering plane

The human operator SHALL have asynchronous controls outside the worker trust boundary for:
- pause/resume a hypothesis class;
- freeze/unfreeze a module/region;
- inject a hypothesis or backlog objective;
- pin/archive a stepping-stone/island;
- tune archive exploration within externally set bounds;
- choose `drain_halt` versus emergency halt;
- set run/profile hard ceilings.

Steering does not become per-mutation approval. Injected hypotheses still pass Tier 0 through promotion gates.

### 12.6 Module saturation guard

If a module/region accumulates `N` consecutive REFUTED attempts without new hotspot/evidence, the Supervisor SHOULD freeze that region for a configured horizon.

Auto-unfreeze conditions MAY include:
- new measured hotspot/evidence;
- human instruction;
- baseline change touching the region;
- expiration of freeze horizon.

The hypothesis factory should then explore uncovered regions, failing tests, backlog or archive diversity gaps.

### 12.7 Multi-generation campaigns

An architectural change that cannot fit one candidate blast radius SHALL be represented as a bounded campaign, not as one oversized candidate.

A campaign SHALL declare:
- intent/objective;
- maximum steps;
- cumulative campaign budget;
- maximum cumulative planned radius;
- abort criteria;
- allowed operator classes.

Every step remains an ordinary candidate with normal Tier 0, blast radius, screening, promotion/retain/discard semantics.

A campaign SHALL NOT access PROTECTED surfaces or silently exceed cross-layer rules.

### 12.8 Search and Measure modes

The Supervisor MAY operate two resource profiles:

**SEARCH_MODE**
- higher permitted target concurrency;
- screening-first;
- archive exploration;
- cheap rejection emphasis.

**MEASURE_MODE**
- reduced interfering concurrency;
- quiet measurement window;
- interleaved/randomized baseline/candidate repetitions;
- used when empirical evidence is promotion-critical or measured delta is close to noise threshold.

Switching modes is a measurement-quality optimization, not a hidden halt and not a bypass of promotion policy.

### 12.9 Evolution process SLOs

The Resource Governor SHALL monitor process-efficiency SLOs including:
- `p95_time_to_first_negative_signal`;
- `promotion_cost / screened_candidate`;
- deterministic-cache hit rate;
- empirical remeasurement rate;
- discard-before-worktree ratio;
- rebase/composition cost;
- verified useful improvement per total evolution cost.

If time-to-signal or promotion cost deteriorates, the Governor SHOULD reduce concurrency, improve screening/operator choice or propose protected evaluator changes.

It SHALL NOT respond by raising blast radius or weakening promotion gates.

---

## 13. Hypothesis Ledger, Evidence Semantics and Anti-Thrashing

### 13.1 Directed Profiling and Continuous Telemetry

Optimization hypotheses SHALL be evidence-driven.

A bounded read-only continuous telemetry collector SHALL run whenever performance, latency, cost, token or energy lanes are enabled.

It SHALL:
- be outside candidate mutation authority when its data is evaluation evidence;
- timestamp/version measurements;
- associate data with exact baseline/configuration;
- keep bounded CPU/RAM/I/O overhead;
- collect relevant CPU, memory, GPU/NPU, token/model-call, cache, I/O/database, latency, error/retry and energy data where reliably measurable.

Performance/resource hypotheses SHALL first query valid recent telemetry.

Fresh profiling is required when telemetry is stale, incomplete, baseline/config changed materially, or a required measurement is missing.

If no measurable bottleneck exists, the system MAY still use bounded exploration for correctness, maintainability or human backlog, but SHALL NOT fabricate a performance bottleneck.


### 13.2 Candidate Outcome, Failure Class and Hypothesis Verdict

Implementation outcome SHALL be separate from hypothesis merit and from whether the known-good head moved.

Canonical conceptual enums:

```text
CandidateOutcome:
  PROMOTION_ELIGIBLE
  ACCEPTED_AS_BASELINE
  RETAINED_AS_STEPPING_STONE
  REJECTED
  INFRA_ERROR
  CANCELLED

HypothesisVerdict:
  SUPPORTED
  REFUTED
  INCONCLUSIVE
  NOT_TESTED
```

Semantics:

- `PROMOTION_ELIGIBLE` — promotion evidence passed, but candidate is not head yet.
- `ACCEPTED_AS_BASELINE` — assigned only after §3.5 `COMMITTED`; only this advances the recovery head.
- `RETAINED_AS_STEPPING_STONE` — passed archive screening and may parent future candidates; does not advance head and does not use §3.5.
- `REJECTED` — non-promoted, non-retained; `discard_candidate`.
- `INFRA_ERROR` — infrastructure/tooling failure; consumes budget, no unlimited retries; `discard_candidate`.
- `CANCELLED` — kill-switch/operator/Supervisor cancellation; normally `discard_candidate` unless preserved for investigation.

Failure classes MAY include:
- `TIER0_REJECT`
- `DUPLICATE_HYPOTHESIS`
- `PROTECTED_PATH`
- `BLAST_RADIUS_EXCEEDED`
- `EMPTY_DIFF`
- `IMPLEMENTATION_FAILURE`
- `REGRESSION_FAILURE`
- `SECURITY_FAILURE`
- `BENCHMARK_FAILURE`
- `EVIDENCE_INSUFFICIENT`
- `INFRASTRUCTURE_FAILURE`
- `RESOURCE_BUDGET_EXHAUSTED`
- `STALE_BASELINE`
- `OPERATOR_CANCELLED`

The Supervisor owns final classification. The worker SHALL NOT self-assign `PROMOTION_ELIGIBLE` or `ACCEPTED_AS_BASELINE`.

`rollback_baseline` is never implied by any CandidateOutcome; it is a separate Supervisor recovery operation for an already-promoted head.

A failed implementation may yield:
`candidate_outcome=REJECTED`, `failure_class=IMPLEMENTATION_FAILURE`, `hypothesis_verdict=NOT_TESTED`.

A benchmark that actually tests and falsifies the proposed benefit may yield `REFUTED`.

Anti-thrashing MAY suppress the same failed implementation without automatically suppressing an untested/inconclusive hypothesis direction.

`RETAINED_AS_STEPPING_STONE` is an implementation outcome, not a hypothesis verdict and not a baseline move.


### 13.3 Hypothesis Ledger

Every hypothesis and candidate SHALL record:
- hypothesis ID;
- generation/candidate IDs;
- semantic hypothesis fingerprint;
- parent generation(s);
- parent identity kind (`known_good_baseline | stepping_stone`);
- `parent_snapshot_id`;
- candidate tree hash;
- declared operator;
- profiling/evidence references;
- declared bottleneck/objective;
- Promotion Objective Policy;
- declared Mutation Blast Radius;
- exact diff/tree hash;
- candidate outcome;
- failure class;
- hypothesis verdict;
- rejection reason where applicable;
- benchmark/test evidence refs;
- resource consumption;
- rebase/reconstruction/composition count;
- timestamps.

Repeated or semantically equivalent rejected implementations SHOULD be suppressed unless:
- underlying conditions changed;
- profiling evidence materially changed;
- a materially new implementation method is proposed;
- an explicit retry policy permits it.

A `REFUTED` hypothesis SHOULD receive stronger suppression than `NOT_TESTED` or `INCONCLUSIVE`.

After a configured run of unsuccessful attempts, the engine SHALL back off or halt and produce a diagnostic report.

### 13.4 Deterministic Verification Cache and Shared Build Cache

Deterministic verification MAY be cached at the finest safe granularity, including per-test, per-target and per-build-artifact rather than only per candidate.

Cache keys SHALL include all relevant immutable context, such as:
- candidate exact diff/tree hash;
- parent baseline/tree hash;
- dependency/toolchain fingerprint;
- relevant config fingerprint;
- test/target identity and version;
- platform/runtime fingerprint;
- protected evaluator version.

A content-addressed build cache MAY be shared across worktrees only when workers cannot pollute sibling evidence; worker access SHOULD be read-only or integrity-checked.

`diff_hash` alone is never a sufficient cache key.

Cached deterministic results MAY be reused only when their full evaluation context remains valid after rebase/composition.


### 13.5 Empirical Evidence Store

Performance, resource, energy and stochastic-quality measurements SHALL be stored as empirical observations rather than deterministic cache facts.

Every observation SHALL have its own identity and provenance, including:
- observation ID;
- candidate/baseline identity;
- exact configuration/toolchain;
- workload/dataset identity;
- timestamp;
- environment/hardware identity;
- concurrency/load state where relevant;
- measurement method;
- sample/repetition identity;
- raw or summarized value;
- validity/expiry policy.

Reading the same observation ten times does not create ten samples.

Reuse of empirical evidence SHALL depend on:
- freshness;
- sufficiency;
- workload equivalence;
- environment comparability;
- stochastic/variance policy.

The Supervisor MAY require new measurements even when code/configuration fingerprints match.

For comparative performance measurement, the Supervisor SHOULD use comparable environmental conditions and SHOULD interleave or randomize baseline/candidate repetitions when practical, reducing bias from temperature, boost behavior, background load or time drift.

### 13.6 Hypothesis Budget and Scheduling Economics

Resource accounting SHALL exist at the **hypothesis** level in addition to candidate-generation budgets.

A Hypothesis Budget SHALL accumulate costs across:
- candidate implementations;
- Self-Correction Window work;
- rebases/reconstructions;
- repeated verification;
- external model consultations;
- profiling;
- benchmarks;
- infrastructure retries.

Rebase or reconstruction SHALL NOT reset the hypothesis budget.

The Supervisor SHALL support:
- `max_rebases_per_hypothesis`;
- total hypothesis wall-time/compute/token/cost/energy ceilings where measurable;
- aging/fairness so long-running valuable hypotheses are not permanently starved by short candidates.

Scheduler priority SHOULD consider:
- expected benefit to MAIA's real task workload;
- evidence strength;
- probability of success;
- expected full validation cost;
- already-spent sunk/recovery cost;
- staleness risk;
- fairness/aging.

Candidate throughput alone SHALL NOT be treated as the primary efficiency metric.

A preferred global efficiency objective is closer to:

`verified useful improvement / total evolution cost`

### 13.7 Stepping-Stone Archive and Parent Sampling

The Supervisor MAY retain selected non-head candidates as `stepping_stone` parents.

Archive retention criteria SHOULD combine:
- screening quality;
- novelty/diversity;
- non-duplication density;
- useful structural capability;
- absence of protected/blast-radius violations.

Archive retention SHALL NOT advance the recovery head.

Parent sampling SHALL be Supervisor-owned and mixed between:
- current known-good head;
- eligible archive stepping-stones;
- human-pinned hypotheses/parents where permitted.

`p_head = 1.0` SHOULD NOT be the default when archive exploration is enabled.

Exact numeric sampling weights are tunable policy, not worker-controlled.

Stepping-stone parents remain subject to current-baseline rebase/composition and full promotion validation before becoming known-good.

### 13.8 Failure Critic

A read-only/non-candidate-mutable Failure Critic MAY learn priors such as:

`P(success | operator, region, objective, failure_class, evidence_fingerprint)`

from the Ledger and localizer outputs.

It MAY inform hypothesis ranking and early screening.

It SHALL NOT act as a promotion oracle, change mandatory gates, or self-certify a candidate.

Verifier failures SHOULD become learning signals for future hypothesis ranking, not only terminal scores.

### 13.9 Evolution Context Store

A context store SHOULD provide workers concise, versioned development context such as:
- module purpose;
- invariants;
- bounded contexts;
- fitness functions;
- operator history;
- campaign intent;
- relevant prior failures/successes.

Workers may read this store.

A discarded candidate SHALL NOT rewrite canonical evolution context.

Context updates commit only after `RETAINED_AS_STEPPING_STONE` or `ACCEPTED_AS_BASELINE`, or through externally governed correction.

---

## 14. Candidate Discard and Baseline Rollback

Candidate failure and active-baseline rollback are different operations and SHALL NOT be conflated.

### 14.1 `discard_candidate`

`discard_candidate(candidate_id)` applies to a candidate that has NOT become the active development known-good baseline.

It SHALL:
- stop/cancel candidate-owned processes;
- isolate/terminate candidate jobs/queues;
- remove or archive candidate test databases/state;
- remove candidate runtime configuration overlays;
- close/remove candidate worktree after evidence capture;
- release candidate-scoped ports/locks/resources;
- preserve required logs/diffs/evidence;
- append CandidateOutcome/FailureClass/HypothesisVerdict;
- leave the active canonical known-good baseline unchanged.

`discard_candidate` MUST NOT:
- reset the shared canonical working tree;
- rewrite canonical promotion refs;
- restore shared application state unrelated to the candidate;
- interfere with other concurrent candidates.

### 14.2 `rollback_baseline`

`rollback_baseline(target_baseline)` applies only when an already active/promoted development baseline must be withdrawn or recovered.

It is a governance/recovery operation controlled by the Supervisor.

Before baseline rollback, the Supervisor SHALL:
- identify all candidates/processes/state derived from the baseline being withdrawn;
- pause, cancel, invalidate or rebase dependent work as required;
- reserve enough resources for recovery;
- select and verify the target coherent recovery point.

Rollback SHALL restore the coherent set of:
- source/code;
- runtime configuration;
- schema/migrations;
- persistent state;
- required baseline metadata.

After restoration, the Supervisor SHALL run sanity/integrity checks and update lineage/recovery records.

Baseline rollback may restore a previously validated known-good state.

Baseline rollback may not introduce a novel unvalidated candidate.

### 14.3 Failure During Recovery

Failure of `discard_candidate` SHALL be contained to that candidate where possible.

Failure of `rollback_baseline` is a critical Supervisor condition and SHALL fail closed, block new evolution, preserve recovery evidence and require explicit recovery policy/operator intervention where automatic recovery cannot prove correctness.


## 15. Network and external effects

Autonomous source evolution does not imply unrestricted authority over external systems.

Existing MAIA approval, privacy, credential, connector, destructive-action and external-side-effect policies remain in force.

Self-evolution authority does not grant:
- unrestricted mail sending;
- unrestricted cloud/account administration;
- credential escalation;
- unrestricted package/repository publication;
- unrestricted production deployment;
- unrestricted access to enterprise/confidential environments.

Development evolution and business-action authorization are separate concerns.

---

## 16. Canonical authority integration

This directive SHALL be integrated according to the existing MAIA authority hierarchy:

`spec/*.yaml > ADRs > prompts/locales > generated artifacts > narrative docs > implementation`

When repository implementation begins:

1. inspect the current repository and active milestone;
2. preserve accepted work;
3. identify the next safe canonical integration point;
4. encode development/deployment capability profiles in canonical spec;
5. encode protected surfaces and evolution-state semantics;
6. encode candidate discard vs baseline rollback, promotion journal, evidence semantics and deployment-artifact validation;
7. extend Spec Guard/validators;
8. add acceptance/golden vectors;
9. create the next available ADR rather than inventing a number;
10. regenerate derived artifacts;
11. implement runtime capability only when its milestone permits it;
12. do not rewrite historical milestone reports;
13. do not manually edit generated artifacts.

---

## 17. Minimum acceptance invariants

The canonical repository should eventually prove at least:

- `EVO-001`: Autonomous self-modification is permitted only in `DEVELOPMENT_EVOLUTION`.
- `EVO-002`: The dedicated development host may serve as the evolution environment without an additional VM/container sandbox.
- `EVO-003`: No mutation begins before a verified parent recovery point exists. Reuse of that parent snapshot satisfies this invariant; a new full coherent snapshot is required at baseline change/promotion/rollback or when policy requires a new recovery anchor, not per child.
- `EVO-004`: Evolution Supervisor executes outside the mutable trust boundary of the evolving worker.
- `EVO-005`: The evolving worker cannot modify or disable the Supervisor or Kill-Switch.
- `EVO-006`: Candidate evaluation uses version-pinned tests/benchmarks that the same candidate cannot weaken.
- `EVO-007`: Failed, cancelled, infra-error and non-promoted candidates are removed by `discard_candidate`; evidence is preserved, the active known-good baseline remains untouched, and unrelated concurrent candidates are not reset. Restoration of an already-promoted head is the distinct Supervisor operation `rollback_baseline`.
- `EVO-008`: Every generation is subject to bounded time/compute/storage/cost/energy resources.
- `EVO-009`: Repeated failed hypotheses are tracked and subject to anti-thrashing policy.
- `EVO-010`: Every promoted development generation has cryptographically verifiable lineage and validation evidence.
- `EVO-011`: Passing regression tests alone is insufficient; candidate promotion requires declared evaluation criteria.
- `EVO-012`: Existing external-side-effect/approval/security policies are not weakened by self-evolution authority.
- `EVO-013`: `DEPLOYMENT_LOCKED` omits autonomous Evolution Engine capability from the distributable artifact/build composition.
- `EVO-014`: A running `DEPLOYMENT_LOCKED` MAIA cannot re-enable evolution for itself.
- `EVO-015`: Re-enabling evolution requires an external human-controlled development build/deployment action.
- `EVO-016`: Installed deployment code is non-writable by the MAIA runtime identity where the platform supports it.
- `EVO-017`: Release/deployment authority is outside autonomous self-evolution.
- `EVO-018`: A development known-good generation does not automatically become a deployed release.
- `EVO-019`: Human operator can halt development evolution asynchronously.
- `EVO-020`: Failure of Supervisor integrity/heartbeat causes evolution to fail closed.
- `EVO-021`: Candidate mutation occurs in an ephemeral isolated worktree/workspace only after Tier 0 admission and verified parent recovery-point checks pass.
- `EVO-022`: Snapshot pruning preserves current, parent, released/pinned and investigation-required recovery anchors.
- `EVO-023`: Self-Correction covers only localized deterministic Tier-1 defects and is bounded by Supervisor-owned correction budgets plus a hard retry-count backstop; it SHALL NOT become an unbounded second evolution loop.
- `EVO-024`: Verification is tiered; success in a lower tier cannot satisfy or bypass mandatory higher-tier validation.
- `EVO-025`: Every candidate is constrained by a Supervisor-owned Mutation Blast Radius covering at least changed LOC and file count.
- `EVO-026`: Performance/resource optimization hypotheses cite directed profiling evidence for the targeted bottleneck.
- `EVO-027`: Supervisor may verify multiple isolated candidates concurrently, but promotion to the development known-good baseline is serialized.
- `EVO-028`: A candidate derived from a stale parent baseline cannot be promoted without rebase/reconstruction and required revalidation.
- `EVO-029`: Selective impact-based regression/benchmarking may accelerate candidate evaluation, but mandatory full promotion gates cannot be bypassed.
- `EVO-030`: Continuous telemetry may supply profiling evidence only when its baseline/configuration provenance is valid and current.
- `EVO-031`: Generation preparation may be pipelined, but mutation/promotion synchronization rules and baseline freshness remain enforced.
- `EVO-032`: All self-correction attempts consume the same candidate/hypothesis budgets defined by `EVO-023` and SHALL NOT reset or expand them.
- `EVO-033`: Cached verification results are reusable only under an evaluation-context fingerprint that includes candidate diff, parent baseline, toolchain/configuration and evaluator/test/benchmark versions.
- `EVO-034`: Adaptive concurrency uses a Supervisor-selected target bounded by an operator-set hard maximum and never bypasses mandatory verification or promotion gates.
- `EVO-035`: Candidate rejection uses `discard_candidate` and cannot mutate/reset the active baseline or unrelated concurrent candidates.
- `EVO-036`: `rollback_baseline` is a distinct recovery operation that restores a coherent validated baseline and invalidates/pauses dependent work as required.
- `EVO-037`: Promotion is restart-safe and journaled; baseline visibility cannot advance before required evidence and recovery state are durably prepared.
- `EVO-038`: CandidateOutcome, FailureClass and HypothesisVerdict are distinct; `RETAINED_AS_STEPPING_STONE` is an implementation outcome, not a hypothesis verdict and not a baseline move.
- `EVO-039`: Infrastructure errors consume hypothesis budget and cannot create unlimited retries.
- `EVO-040`: Deterministic verification cache and empirical evidence storage have distinct reuse semantics.
- `EVO-041`: Every empirical observation has unique identity/provenance; rereading one observation does not create additional samples.
- `EVO-042`: Required promotion metrics block promotion when missing/unknown; optional metrics may remain explicitly unknown according to policy.
- `EVO-043`: Optimization promotion requires policy-defined practically meaningful improvement and uncertainty/repeatability evidence, not merely a numerically better point estimate.
- `EVO-044`: Promotion policy can require comparison to a stable long-term reference anchor to detect cumulative drift.
- `EVO-045`: Protected holdout/control evidence is governed by limited-feedback and externally controlled refresh policy.
- `EVO-046`: Hypothesis budget accumulates candidate attempts, corrections, rebases, consultations, profiling and repeated verification; rebase does not reset budget.
- `EVO-047`: Scheduler efficiency is evaluated by verified useful improvement relative to total evolution cost, not candidate throughput alone.
- `EVO-048`: Recovery/evidence resource reserve cannot be consumed by ordinary experiments.
- `EVO-049`: Snapshot manifests independently identify each generation even when storage is incremental/deduplicated.
- `EVO-050`: The exact `DEPLOYMENT_LOCKED` artifact is functionally validated after build and before signature/distribution.
- `EVO-051`: Deployment capability-absence verification proves prohibited evolution/self-modification capabilities are absent or inaccessible in the exact signed artifact.
- `EVO-052`: Signature/distribution apply only to the exact artifact identity that passed post-build validation.

- `EVO-053`: Tier 0 Cheap Rejection is mandatory and runs before creation of a candidate worktree or new candidate snapshot.
- `EVO-054`: Parent recovery snapshots are reused when valid; child identity records `parent_snapshot_id` and candidate tree/diff identity.
- `EVO-055`: `RETAINED_AS_STEPPING_STONE` may parent later candidates but never advances the recovery head or bypasses promotion validation.
- `EVO-056`: Screening and promotion are distinct; only promotion nominees pay all mandatory full-suite/Tier-3/Tier-4/holdout gates.
- `EVO-057`: Autonomous mutation is restricted to a positive EVOLVABLE allowlist; unclassified surfaces default to non-writable.
- `EVO-058`: Every mutation declares a Supervisor-recognized operator before mutation; operator choice cannot bypass mandatory promotion gates.
- `EVO-059`: Same-generation test modification is allowed only for additive tests whose protected teeth-check fails on the parent and passes on the candidate; existing tests may not be weakened.
- `EVO-060`: Continuous read-only telemetry is mandatory whenever performance/latency/cost/token/energy optimization lanes are enabled.
- `EVO-061`: Foundation-model weights, LoRA/adapter weights and autonomous self-finetuning remain protected and outside the autonomous source-evolution loop.
- `EVO-062`: Continuous producers (telemetry/hypothesis factory/queue) are separate from mutation workers; valid producer evidence need not be recomputed per candidate.
- `EVO-063`: Symbol/contract-disjoint promotion-grade candidates may be composed only by Supervisor-controlled conflict analysis, affected revalidation and one journaled serialized promotion.
- `EVO-064`: Worker/model routing cannot self-escalate model strength/cost; escalation class is Supervisor policy.
- `EVO-065`: OBJECT and META scheduler lanes have separate objectives/budgets; META cannot mutate protected governance and is evaluated on frozen process/task packs.
- `EVO-066`: Human steering plane may pause classes, freeze modules, inject hypotheses and request drain-halt without becoming per-mutation approval.
- `EVO-067`: Protected proposals may be shadow-evaluated on historical generations but can be applied only through external governance.
- `EVO-068`: Module saturation guard can temporarily freeze repeatedly REFUTED regions until new evidence, baseline touch, timeout or human instruction.
- `EVO-069`: SEARCH_MODE and MEASURE_MODE are distinct resource profiles; MEASURE_MODE reduces interference for promotion-critical empirical evidence.
- `EVO-070`: Multi-generation campaigns preserve the per-candidate blast radius and express large architectural work as bounded sequences of ordinary candidates.
- `EVO-071`: Anti-bloat evidence tracks structural growth; uncontrolled growth can make evidence insufficient, while simplification candidates retain non-zero exploration quota.
- `EVO-072`: Failure Critic may influence hypothesis ranking but is outside candidate mutation authority and cannot act as a promotion oracle.
- `EVO-073`: Evolution Context Store commits candidate-derived knowledge only after stepping-stone retention or known-good promotion.
- `EVO-074`: Protected replay/task packs may be used for screening/measurement; workers cannot mutate evaluator inputs.
- `EVO-075`: Deterministic cache SHOULD operate per test/target/build artifact when safe; shared build cache must be content-addressed and non-polluting.
- `EVO-076`: Resource Governor tracks `p95_time_to_first_negative_signal` and promotion-cost efficiency; degraded process efficiency cannot justify raising blast radius or weakening gates.
- `EVO-077`: Evolution Engine remains subordinate to MAIA's product mission as a superassistant/Trusted Execution Intelligence system; self-improvement throughput is never a substitute for user-value outcomes.


---

## 18. One-sentence rule

> **MAIA exists to become a better superassistant for its user: it may explore broadly, reject cheaply and retain useful stepping-stones on its dedicated development host, while paying the full crash-recoverable promotion cost only for a serialized known-good baseline; none of that opens the Supervisor, protected evaluators, deployment lock or human-controlled release boundary.**
