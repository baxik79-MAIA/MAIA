# M0.16.3 Acceptance Report — Candidate Mutation + Tier-1 Verification

## Status

Local implementation and required local validation: **PASS**. Remote GitHub Actions: **pending push and completion**. This report does not claim CLOSED_READY until the remote workflow passes and the branch is clean.

## Revision

- Starting accepted M0.16.2 HEAD: `f94b3ae9176435f4481c686d234498e2ad9bec7e`
- Branch: `codex/m0163-mutation-tier1`
- Parent architecture: M0.16.0 Supervisor policy remains in `ops/evolution-supervisor`; host filesystem and Git mechanics remain in `ops/evolution-workspace-host`.

## Architecture decision

The Supervisor owns the pure admission-to-mutation policy and Tier-1 result state. The development host owns canonical path resolution, candidate file I/O, Git worktree identity checks, evidence persistence, and the fixed verification adapter. Core and shipped applications do not depend on either operations crate.

The existing Hypothesis Ledger stores hypothesis observations and qualification evidence, not candidate mutation lifecycle records. Mutation attempts therefore use a separate append-only host evidence journal rather than changing the ledger's domain or authority. The journal is outside both the canonical repository and the candidate root, emits content-free JSONL records, and validates a SHA-256 hash chain when reopened.

The user-required human approval is represented by an approval reference and revalidated through the protected-host Gate before any candidate file changes. A missing, stale, or unverifiable approval denies mutation.

## Mutation authority and isolation

- Positive EVOLVABLE allowlist: only `apps/local-intelligence-host/src/lib.rs` with operator `FUNCTION_REWRITE`.
- The operation is one unique expected-text replacement, bound to the current file SHA-256, with a 64 KiB replacement bound. It cannot create, delete, rename, or select arbitrary files.
- Patch comparison normalizes LF/CRLF and preserves the candidate file's consistent newline style. Mixed or lone carriage-return files fail closed.
- Mutation requires an ACTIVE candidate, its exact workspace/generation/hypothesis identity, exact admitted Tier-0 evidence, current protected-host approval, the DEVELOPMENT_EVOLUTION profile, and a healthy Supervisor gate.
- Paths must be candidate-relative normal components. Absolute paths, traversal, symlinks, canonical-host aliases, and hardlinks to the canonical file or another candidate are refused.
- Before mutation and after Tier-1, the host verifies that the workspace is still the allocated Git worktree at the admitted parent commit, and that canonical HEAD and canonical tracked/untracked status still match the admitted clean parent.
- Git calls are limited to fixed worktree allocation/removal and read-only identity/status queries. There is no candidate path to commit, push, merge, change refs, rewrite history, or promote.
- A Tier-1 pass leaves the candidate ACTIVE and records evidence. Tier-0 denial, Tier-1 failure, or infrastructure failure does not roll back the active baseline: it records the candidate terminal reason and discards only that candidate workspace.

## Tier-1 checks

The fixed offline adapter evaluates the post-mutation candidate using:

1. candidate worktree identity and admitted parent commit;
2. `rustfmt --check` on the allowlisted Rust file (also parses the Rust source);
3. locked, offline Clippy for `maia-local-intelligence-host`, all targets and features, with warnings denied;
4. locked, offline all-target component build/check for that package;
5. locked, offline targeted package tests;
6. exact Git status check requiring only the one allowlisted modified file, plus a post-verification canonical-host integrity recheck.

The adapter owns the executable, arguments, working directory, environment allowlist, offline mode, target directory, and per-command 120-second bound. Candidate data cannot supply commands. A pass is verification evidence only; it does not mean screening completion, promotion approval, deployment, or merge authorization.

## Durable evidence

Every attempt is recorded before policy execution and updated at material transitions. Records identify the candidate/workspace, generation, hypothesis, approval reference, Tier-0 outcome and evidence references, requested path/operator, permitted or rejected result and reason, before/after file digests and changed-line count, verifier identity and per-check outcomes, and any terminal candidate state/reason. Replacement text and command output are not persisted. Unsafe or oversized tokens are represented by a digest. Writes are flushed before success is returned; a journal write failure quarantines the candidate and returns infrastructure failure.

## Tests and validation

- Contract unit tests: **PASS**, 3 tests.
- Supervisor and workspace-host tests: **PASS**, 4 Supervisor unit tests, 3 dependency-boundary tests, 4 safety-boundary tests, and 14 allocation/mutation/Tier-1 integration tests.
- Required adversarial cases covered: admitted allowlisted rewrite; allowlist denial; traversal and absolute path denial; canonical hardlink denial; Tier-0 and identity mismatch short-circuit; incomplete Tier-1 evidence fails closed; Tier-1 failed and infrastructure results discard only candidate; verifier unexpected candidate protected-file write is rejected; evidence chain integrity; baseline and sibling remain unchanged; no promotion or Git history mutation.
- `python tools/validate_spec.py`: **PASS**.
- `python tools/spec_guard.py`: **PASS**.
- `cargo fmt --all -- --check`: **PASS**.
- Targeted and workspace `cargo clippy --all-targets --all-features -- -D warnings`: **PASS**.
- `cargo test --workspace --locked --offline --no-fail-fast`: **PASS**.
- `cargo test --workspace --all-features --locked --offline --no-fail-fast`: **PASS**.
- `python -m unittest discover -s tests/spec -p "test*.py"`: **PASS**, 85 tests.
- `python tools/verify_round_table_absent.py`: **PASS**; 341 packages resolved, workspace build and tests passed with all nine Round Table members physically removed.
- `git diff --check`: **PASS**.
- Provider-dependent tests remained opt-in/ignored where credentials or a local Ollama service are required; no provider calls were made for this milestone.
- GitHub Actions runs `36125313264` and `36127462534` exposed Windows mutation attempts rejected before file-write or Tier-1 evidence. The diagnostic run `36128951721` confirmed `MutationRejected` with no changed-file/Tier-1 evidence. A synthetic repository with `core.autocrlf=true` reproduced the cause: candidate worktree source was CRLF while the approved patch text used LF. The adapter now normalizes for matching and preserves the candidate style; the synthetic test asserts CRLF is present and the rewrite succeeds. Remote validation of this correction is pending.

## Deliberate non-goals

No autonomous change origination, unrestricted source editing, worker process, arbitrary shell, provider call, Round Table dependency, Git commit/push/merge/ref operation, candidate promotion, release, deployment, DEPLOYMENT_LOCKED mutation, active-version rollback, or M0.16.4 capability is introduced.

## Remaining risks and M0.16.4 readiness

This is a controlled host library boundary and testable vertical slice, not yet a production-connected evolution service. A protected production Gate must still bind kill-switch state, Supervisor integrity, DEVELOPMENT_EVOLUTION versus DEPLOYMENT_LOCKED profile, exact Tier-0 admission, parent, reservation, protected-path assumptions, and human approval to authoritative runtime state. The host and candidate currently run under the same operating-system identity; software path checks do not defend against a compromised same-user process or administrator. The fixed Cargo timeout kills the direct command process; process-tree/job containment remains to be established before running less-trusted code. The evidence journal is single-writer and hash-chain tamper-evident, not an administrator-proof signed ledger.

M0.16.4 should not expand mutation authority until the production Gate is wired to protected state, OS-level candidate isolation and process-tree termination are proven, journal ownership/locking is specified, and the Tier-1 surface and resource bounds are reviewed. M0.16.3 provides the first candidate-only mutation path but does not claim those operational controls are complete.
