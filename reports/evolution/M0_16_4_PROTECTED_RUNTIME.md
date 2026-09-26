# M0.16.4 Acceptance Report — Protected Runtime Gate, Execution Containment & Journal Ownership

## Status

Local required validation: **PASS**. The implementation and Windows CI portability repair are **ACCEPTED / CLOSED_READY** on tested commit `37265c9382354e495ecd4ad6f1f0beafbe113732`; GitHub Actions run `36236491417` completed **SUCCESS** on that exact SHA. This acceptance report is being committed as a documentation-only follow-up. The branch remains unmerged to `main`.

## Revision

- Starting accepted M0.16.3 HEAD: `677199724f3e508617b443f06541e9a680ed1c48`.
- Branch: `codex/m0164-protected-runtime`.
- Implementation commit: `af161385535a617104aeed4009fc649a1d08b983` (`Add protected runtime and verifier containment gate`).
- Windows CI portability repair: `37265c9382354e495ecd4ad6f1f0beafbe113732` (`Make Windows containment tests runner-portable`). This is the final tested implementation HEAD.
- Initial workflow: [GitHub Actions run 36234392536](https://github.com/baxik79-MAIA/MAIA/actions/runs/36234392536), **FAILURE**. Its Windows descendant tests depended on nested PowerShell launch timing.
- Successful implementation workflow: [GitHub Actions run 36236491417](https://github.com/baxik79-MAIA/MAIA/actions/runs/36236491417), **SUCCESS** on `37265c9382354e495ecd4ad6f1f0beafbe113732`.
- No merge to `main` was performed.

## Files changed

- `Cargo.toml`, `Cargo.lock`.
- `spec/evolution_protected_runtime.yaml`; generated `generated/spec_appendix.md` and `generated/spec_guard_report.txt`; `tools/validate_spec.py`; `tools/evolution_protected_runtime_contract.py`; `tests/spec/test_evolution_protected_runtime_contract.py`.
- `ops/evolution-supervisor/src/mutation.rs`, `ops/evolution-supervisor/src/workspace.rs`.
- New `ops/evolution-process-host/Cargo.toml` and `ops/evolution-process-host/src/lib.rs`.
- `ops/evolution-workspace-host/Cargo.toml`, `src/evidence.rs`, `src/lib.rs`, `src/tier1.rs`, `tests/allocation.rs`; new `src/protected_runtime.rs`.
- `roundtable/tests/core_independence.rs`.
- This acceptance report: `reports/evolution/M0_16_4_PROTECTED_RUNTIME.md`.

## Architecture decisions

- The policy/lifecycle remains in `ops/evolution-supervisor`. Host-owned runtime Gate state is held in `ops/evolution-workspace-host`; Windows process-tree mechanics are isolated in the new `ops/evolution-process-host` adapter. None of these crates is a Core or shipped-application dependency.
- The authoritative runtime Gate starts fail-closed and requires host attestation. Its profile, kill switch, enablement, Supervisor identity/integrity, admission, approval, and expiry live in protected host process memory. Candidate requests cannot create or mutate this state.
- Gate decisions are immutable operation snapshots bound to a version and the exact candidate, workspace, generation, parent commit, Tier-0 evidence, reservation, protected-path plan, and approval reference. The workspace adapter adds canonical repository path plus a clean-state fingerprint. The snapshot is checked again before the mutation write, before Tier-1 starts, and after Tier-1 completes.
- Profile, evolution-enable, kill-switch, Supervisor identity/integrity, approval revocation, admission replacement, and expiry invalidate pending operations. A protected profile/kill-switch change maps to `CANCELLED`; it is not recorded as a candidate code failure.
- New canonical contract: `spec/evolution_protected_runtime.yaml`. The spec guard validates the required state, containment, resource, journal, result, and forbidden-authority fields; generated contract fragments were regenerated.

## Protected state ownership and Gate semantics

The protected host owns the runtime profile, kill switch, Gate version and decisions, exact admission and reservation binding, approval reference and expiry, canonical repository identity/state, journal path and writer lock, and verifier containment authority. The Supervisor evaluates policy. Candidate workspaces own none of those authorities and receive no controller or Gate state handle.

Startup defaults are `DEPLOYMENT_LOCKED`, evolution disabled, kill switch ON, Supervisor integrity unverified, and same-user containment capability. The only current runtime API cannot promote the capability to restricted-identity/network/filesystem isolation. A host must explicitly attest DEVELOPMENT_EVOLUTION, enable evolution, clear the kill switch, and establish the expected Supervisor identity/integrity before bound operations can proceed. Any uncertainty denies the operation.

An OFF/disabled kill switch blocks new mutations and Tier-1 starts. A running verifier is terminated through its Job Object; the operation ends `CANCELLED`, the candidate is discarded, and transition evidence is flushed. It is not classified as test failure, candidate rejection, or rollback. Kill-switch changes advance the protected state version and invalidate prior snapshots.

## Isolation and containment achieved

### Process-tree containment

On Windows, the host adapter creates the fixed verifier process suspended, assigns it to a non-breakaway Job Object, and resumes it only after containment assignment and a final cancellation check. The Job Object uses kill-on-close, and its owner terminates the whole job on timeout, cancellation, resource-limit termination, and shutdown. This includes descendants and grandchildren even when the root process exits first. Standard handles are explicitly null; candidate input cannot select a command or inherit arbitrary handles. The verifier runs only host-fixed `rustfmt`/Cargo invocations in the candidate worktree.

The host limits concurrent verifier trees to one with an OS-named mutex and a process-local guard. Fixed resource ceilings are 120 seconds per command, a 75% hard CPU rate cap, 4 GiB Job memory, and 64 active processes. Cargo runs offline, with a fixed environment allowlist and candidate-scoped target directory. Job completion notifications and recognized Windows memory termination statuses are classified as `RESOURCE_LIMIT`, distinct from test failure and infrastructure failure.

### Filesystem/capability restriction

Candidate mutation remains limited to the existing positive EVOLVABLE surface, `apps/local-intelligence-host/src/lib.rs`, with fixed single-function replacement semantics. Tier-1 commands and paths are host-selected; no candidate-selected shell or arbitrary subprocess API was added. Canonical repository and protected state are verified around operation boundaries.

### OS identity and network isolation

The achieved OS boundary is **same-user process-tree containment only**. This host does not create a trustworthy restricted token/AppContainer, isolated filesystem ACL, or no-network identity. The Gate therefore fails closed for production Tier-1 execution because `RESTRICTED_IDENTITY_NETWORK_AND_FILESYSTEM` is not available. The implementation does not represent same-user containment as full sandbox isolation. Building and attesting a real restricted identity, no-network policy, and candidate/toolchain ACL is deferred to M0.16.5.

## Tier-1 and result/state taxonomy

Tier-1 evaluates the post-mutation candidate sequentially using the bounded offline checks already defined for M0.16.3: candidate identity, rustfmt parsing/format check, locked offline Clippy, component build/check, targeted package tests, and exact protected-surface Git status. Cancellation is checked between commands. Checks already begun can produce partial evidence, while the operation result preserves `CANCELLED`, `RESOURCE_LIMIT`, `ISOLATION_UNAVAILABLE`, `INFRA_ERROR`, `TEST_FAILURE`, or `PASS` truthfully.

Tier-1 PASS records evidence and leaves the candidate active. It grants no promotion, commit, push, merge, deployment, protected-ref, or active-version authority. Failed candidates use `discard_candidate`; they do not invoke `rollback_baseline`. M0.16.4 does not widen the EVOLVABLE allowlist or mutation operation.

## Evidence journal ownership

The content-free append-only SHA-256 hash-chained journal remains under the protected host's fixed path, outside both the canonical repository and candidate root. An OS-backed exclusive whole-file lock is acquired before reopen/chain validation and held for the writer lifetime. A second writer fails with infrastructure error; process termination releases the OS lock. Reopen validates the full existing chain and refuses corruption without appending. Each authority-transition write is flushed with `sync_all` before success is returned. Candidate input cannot select the journal path or acquire writer authority.

## Tests and validation

- `python -m unittest tests.spec.test_evolution_protected_runtime_contract`: **PASS**, 5 contract tests.
- `python -m unittest discover -s tests/spec -p "test*.py"`: **PASS**, 90 tests.
- `cargo test -p maia-evolution-process-host --lib -- --nocapture`: **PASS**, 9 Windows containment tests, including timeout, cancellation, descendant/grandchild termination, Job-owner close, single-writer verifier slot, candidate target traversal denial, and pre-start cancellation. The suite also passed three consecutive local runs.
- `cargo test -p maia-evolution-workspace-host --lib`: **PASS**, 7 protected Gate tests.
- `cargo test -p maia-evolution-workspace-host --test allocation --no-fail-fast`: **PASS**, 21 candidate allocation/mutation, kill-switch, journal, identity, and Tier-1 tests.
- `python tools/validate_spec.py`: **PASS**.
- `python tools/spec_guard.py`: **PASS**, 40 YAML contracts and generated checks.
- `cargo fmt --all -- --check`: **PASS**.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: **PASS**.
- `cargo test --workspace --locked --offline --no-fail-fast`: **PASS**.
- `cargo test --workspace --all-features --locked --offline --no-fail-fast`: **PASS**.
- `python tools/verify_round_table_absent.py`: **PASS**; all nine Round Table members removed from the copied workspace, with remaining packages building and testing successfully.
- `git diff --check`: **PASS**.
- An early concurrent validation attempt exposed a CRLF normalization regression in the host adapter. The original CRLF-preserving behavior was restored; the targeted allocation suite, reduced-workspace proof, and both final workspace suites passed afterward.
- Initial GitHub Actions run `36234392536` failed only in the process-host descendant tests: nested PowerShell launches did not reliably produce PID evidence before the short test deadlines on the hosted runner. Replaced those fixtures with a small self-spawning Rust test helper and made already-terminated descendants a valid completed result only when Windows reports the PID is gone. The repair is covered by three consecutive local runs and must be confirmed by the follow-up remote workflow before closure.
- No live provider calls were made. Provider- and local-runtime-dependent tests stayed opt-in/ignored where applicable.

## Remaining risks and M0.16.5 readiness

- Same-user process isolation is not a security boundary against another process running as that user or against an administrator. The production Gate intentionally blocks execution until a separately verified restricted identity, filesystem ACL, and no-network mechanism is available.
- Windows Job Objects strongly contain the verifier process tree and enforce the fixed limits. Some resource exhaustion causes can be observed from Job notifications/status, but Windows may not provide a distinct notification for every possible allocator/tool-level failure. Unclassified nonzero exits remain verifier/test outcomes rather than being overstated as proven resource exhaustion.
- Runtime protected facts are held in process memory and restart into closed defaults; durable Gate-state recovery/attestation is not part of this milestone. Human approval is an opaque host-bound reference with expiry/revocation, not yet an integration to a live approval service.
- The evidence journal is exclusive-writer and hash-chain validated, but is not a signed or administrator-proof distributed ledger.
- No promotion, autonomous push/merge, release, deployment, active-version replacement/rollback, provider access, Round Table execution, arbitrary shell, or mutation-authority expansion is introduced.

M0.16.5 should first deliver a verifiable Windows restricted identity/AppContainer or equivalent, enforce filesystem and no-network policy, bind durable host Gate and approval attestation, and add integration tests that prove the actual OS capabilities before enabling production Tier-1 execution. It should retain this milestone's Job Object boundary and fail-closed defaults.
