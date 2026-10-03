# M0.16.5 Architecture Owner Review — Restricted Tier-1 Runtime

**Verdict: APPROVED_WITH_NONBLOCKING_FOLLOWUP.** The review found and fixed four blocking defects (§3). After
those fixes, M0.16.5 remains **CLOSED_READY** on the prepared LAB host, with the deployment constraints
stated in the closure audit. M0.16.6 was not started. Nothing was pushed or merged.

- Date: 2026-10-02 (Europe/Warsaw).
- Worktree: `C:\MAIA\public-export\.local\m0160-supervisor`, branch `codex/m0165-restricted-runtime`.
- Review start HEAD: `e21176ec1f944cacddcef9f92c3d5a5d64f5c849`. The pre-closure-session point was `3735f25`.
- New evidence: `C:\MAIA\reports\evolution\m0165-host-preparation\architecture-review-20261002\` (`REV\`).

The review did not accept any earlier decision just because its tests passed. Every claim below was
re-read in code, spec or evidence; where a test existed, I checked that it actually observed the property.

## 1. Inputs reviewed

- `M0165_HANDOFF.md`, `reports/evolution/M0_16_5_CLOSURE_AUDIT.md`, ADR-0048,
  `spec/evolution_protected_runtime.yaml` and its guard, `generated/spec_appendix.md`.
- Commit `6dcf043` and the related commits `03fc1fd`, `b198fe1`, `2847af1`.
- Runtime source: `ops/evolution-process-host` (Job, pipes, AppContainer, ACL, Null capability) and
  `ops/evolution-workspace-host` (Tier-1 verifier, config snapshot, Gate wiring).
- Tests, `.github/workflows/ci.yml`, the M0.16.4 readiness section, the M0.16.5 capability-blocker report,
  and the v5.1 directive (§3, §16).
- Evidence hashes re-verified (first 8 hex of SHA-256):
  - production journals `ABE06E4D` and `D24BE97D`;
  - E4 isolation `1133834B`, lifecycle `34BAF656`, Clippy differential `8A83ACBE`;
  - post-cleanup rescan `E8EBEDE1`, Null validation `0A924C5E`;
  - supervisor stdout/stderr `6815B765` / `4F4C3452`;
  - 34 orphan-cleanup records present.
  All match the closure report. The full 11-minute Tier-1 run was **not** repeated: no gap found touches
  the Cargo stages.

## 2. Assessment of commit 6dcf043 (canonical contract delta)

| Key | 6dcf043 | Assessment |
|---|---|---|
| `inherited_handles` | `explicit_handle_list_null_stdin_and_host_owned_output_pipes_only` | **Necessary and accurate.** The code passes exactly `[NUL stdin, stdout pipe, stderr pipe]` via `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`. Read ends are non-inheritable. Host write copies close after `CreateProcessW`. The pipe DACL names only the host user and the per-run package SID. The old value no longer described the code. |
| `output_capture` (new) | `host_drained_bounded_per_stream; persisted_outside_candidate_acl` | **Necessary and accurate.** 256 KiB is kept per stream and overflow is still drained, so the child never blocks. Readers join after the Job is empty. Captures go to `run-state\diagnostics\<run>`, which is never granted to the package SID. They are opaque bytes and never parsed. This does not weaken isolation. |
| `achieved_identity_claim` | `per_run_appcontainer_and_job_only_after_host_readiness_attestation; tier1_unavailable_otherwise` | **Necessary and accurate.** The old "same-user only" claim became false. Fail-closed holds: the Gate's `tier1_isolation_ready()` runs the host readiness probe, the verifier re-checks profile availability, and every run refuses without the exact Null ACE. Every child is verified before resume (Job membership, AppContainer, exact package SID, exactly one enabled capability equal to the Null SID). A failed verification terminates the Job. No caller selects the Job-only runner. |
| `stronger_isolation_capabilities` | rewritten as "available only after readiness probe **and production E4 verification**" | **Not acceptable as written; reverted** (`9915127`). No runtime path consumes E4 evidence, so the text overstated enforcement. The original M0.16.4 value (`unavailable_until_os_acl_and_no_network_are_verified`) is still accurate, because ACLs and the token are verified on every run. |

The net contract delta is now minimal: two values changed and one key added, each describing behavior
that is already implemented and tested. Unchanged and still enforced:
- startup defaults (fail closed);
- `network: appcontainer_without_network_capabilities_required`;
- the one-entry `resource_capabilities.exact_allowlist`;
- resource limits, journal semantics, outcomes and `authority_limits`.
The authority model and protected surfaces are **not** broadened: evolvable paths, Git authority,
promotion and provider access are unchanged. The only added runtime authority is null-stdin read and
host-owned diagnostics.

## 3. Defects found and fixed during review

1. **Null capability ACE check was not exact (fail-open on misconfiguration; blocking)** — `35c1c87`.
   - Spec says `exact`, ADR-0048 says "one exact, non-inheritable read ACE", and the elevated tool
     enforces `count == 1 && exact`. The runtime gate, however, accepted the DACL as soon as **one**
     matching ACE existed. A second, broader ACE for the same SID would therefore have passed, e.g.
     write or `WRITE_DAC` on the system-wide `\Device\Null`.
   - Fix: a pure `exact_null_capability_dacl` requires exactly one ACE naming the SID among allow, deny
     and callback types, and that ACE must be type allow, flags 0, mask `FILE_GENERIC_READ`.
   - Unit test on synthetic ACLs covers: exact accepted; empty, wrong SID, extra write mask, inherit
     flag, second allow, `WRITE_DAC` and an extra deny ACE all rejected. The old logic fails this test.
2. **Branch would fail CI (integration-blocking)** — `2854551`. CI runs
   `cargo clippy --workspace --all-targets --all-features -- -D warnings` on `windows-latest`. M0.16.5
   left nine process-host lints and two workspace-host lints. These were fixed without behavior change:
   `too_many_arguments` allow attributes (repo precedent), let-chains, removing a useless `format!` and
   an unneeded `&mut`.
3. **A default test required the LAB host (integration-blocking)** — `2854551`.
   `allowed_candidate_mutation_runs_tier1_and_does_not_promote_or_touch_baseline` uses the real verifier.
   With `LOCALAPPDATA` pointed at an empty folder, the verifier correctly fails closed (`InfraError`), and
   the test then failed its `Tier1Passed` assertion. Hosted runners are unprepared, so it is now ignored
   with an explicit reason, like the other prepared-host tests. The scripted-verifier tests keep CI
   coverage of the mechanics.
4. **Non-Windows build broken** — `2854551`. The crate defined `run_restricted` once under
   `cfg(not(windows))` and once ungated, so any non-Windows build saw two definitions. The Windows one is
   now gated, as the `Core is OS-neutral; adapters isolate platforms` invariant expects.

## 4. Questions the review had to answer

- **Exact capability, no network capability.** Verified per run on the real token (count = 1, equal to
  the Null SID, enabled). The E4 rerun shows outbound connections refused with WSAEACCES and loopback
  timing out. `DeriveCapabilitySidsFromName` yields a name-derived SID that any AppContainer could
  request, but its only grant is read on `\Device\Null`, so that is harmless.
- **Null ACE only non-inherited read-only.** Live DACL:
  `D:AI(A;;FR;;;<cap>)(A;;0x1201bf;;;WD)(A;;FA;;;SY)(A;;FA;;;BA)(A;;0x1200a9;;;RC)`. That is one ACE,
  flags 0, mask FR. This is now enforced exactly at runtime (fix 1).
- **No write outside target.**
  - The ACL grants are: Modify on `target` only; read on the candidate and the six resource roots plus
    the config snapshot; traverse-only (non-inheritable) on depot ancestors; read/execute on pinned files.
  - E4 shows workspace write/create, protected and canonical read/write, and vendor write are all denied.
  - **New probe** (`cc64e50`, `REV\production-e4-reparse-20261002.txt`): this was the most plausible
    escape. Candidate code in one stage could plant a link in `target`, and the host or the next stage's
    inheritable Modify grant could follow it. The AppContainer cannot create a junction to an outside
    directory (Win32 5), a directory or file symlink (1314), or a hardlink to an outside file (5).
  - Positive controls show the probe is valid: the same `FSCTL_SET_REPARSE_POINT` succeeds for an inner
    target directory and from the host.
  - A following stage, with a fresh SID and fresh grant, found no link to write through. The outside tree
    and its DACL stayed byte-identical.
- **Bounded capture does not weaken isolation** (see §2).
- **Job covers descendants and cleanup.** Breakaway is not allowed. `KILL_ON_JOB_CLOSE`, 64 processes,
  4 GiB and the 75% CPU cap are set. The process is assigned to the Job before resume. On root exit the
  Job is terminated and drained before readers join. Lifecycle E4 (re-run here) covers failure, timeout,
  cancellation and root exit with a live grandchild: each case had the descendant ended, exact DACLs,
  no package SID, and no profile mapping.
- **Host crash.**
  - `KILL_ON_JOB_CLOSE` kills the verifier tree when the host dies, and an abandoned slot mutex is
    accepted (`WAIT_ABANDONED`).
  - Temporary ACEs and the profile, however, **persist** and need the documented manual exact-SID recovery.
    The 61 stale ACEs cleaned in the closure audit came from exactly such interrupted and older runs.
  - These leftovers do not give any later run access, because every run uses a fresh random package SID,
    but recovery is manual and not automatic → follow-up F1.
- **Consistency of spec, ADR, code, tests and report.** After `9915127` they agree. The closure report is
  historical and gets an addendum instead of a rewrite (directive §16.12).
- **Explicit limits.** The short candidate-root path budget, the pinned proof-only compat Clippy (its
  junction-alias difference is neutralized because the snapshot always holds a `clippy.toml`) and
  LAB-host-only preparation are stated in closure §8 as constraints, never as general guarantees.

## 5. Non-blocking follow-ups (ordered)

- **F1 — Startup orphan reconciliation.** Readiness should detect leftover `maia-tier1-*` mappings and
  their package-SID ACEs from a crashed host. It should either reconcile them using the exact-SID
  procedure or fail closed with an operator message. Today that step is manual.
- **F2 — Host-side reparse guard under target.** Before the host writes `cargo-home`, `tmp` and `home`
  under `target`, it should reject reparse points itself, instead of relying solely on the OS sandbox
  mitigation proven above.
- **F3 — Path budget as an explicit check.** A deep candidate root currently ends as an MSVC `LNK1181`,
  which is classified as a candidate test failure. A host-side length bound should report `INFRA_ERROR`.
- **F4 — Timeout classification.** `evolution_mutation_tier1.yaml` maps `adapter_failure_or_timeout` to
  `INFRA_ERROR`, while `evolution_protected_runtime.yaml` treats the 900 s wall limit as a resource limit
  and the code maps `TimedOut` to `RESOURCE_LIMIT`. This predates M0.16.4 and fails closed either way,
  but the canonical spec should state one rule.
- **F5 — Durable Gate/approval attestation.** M0.16.4 recommended it for M0.16.5, but the canonical spec
  keeps protected state in host memory with a fail-closed restart. It is still not implemented and must
  be scheduled explicitly, not assumed.
- **F6 — Production use of the compat Clippy bundle** deserves its own ADR, recording provenance, the pin
  and its exit criteria. The bundle manifest still says "proof-only".
- **F7 — Diagnostics retention.** Capture is bounded per stream, but there is no retention policy for
  `run-state\diagnostics`.

## 6. Validation run in this review

- **Production probes on the prepared host** (`REV\`): E4 isolation (94.0 s), lifecycle (22.0 s), reparse
  containment (0.8 s) and host readiness (46.3 s) all PASS. They ran after the exact-ACE fix, so the fix
  works against the real device DACL.
- **Simulated hosted CI** (empty `LOCALAPPDATA`), evolution crates:
  - process-host 13 passed / 4 ignored;
  - supervisor 4 + 3 + 4 passed;
  - workspace-host lib 8 passed / 2 ignored;
  - allocation 20 passed / 2 ignored.
- **CI-equivalent run locally:**
  - workspace Clippy with `-D warnings`: clean;
  - `cargo test --workspace`: 604 passed, 0 failed, 15 ignored;
  - `--all-features`: 720 passed, 0 failed, 17 ignored;
  - `spec_guard` and `validate_spec`: PASS; generated-doc check OK; `cargo fmt --check` and `git diff --check` OK;
  - Python: 90 canonical tests and 56 governance tests OK.
- Not repeated: the full production Tier-1 run. The latest PASS is `e1740ad` (closure audit). None of the
  changes since then alter the Cargo stage commands, environment or ACL plan. The stricter Null check and
  the behavior-neutral refactors are exercised by the probes above.

Evidence files in `REV\` (SHA-256):
- `production_appcontainer_e4_isolation.txt` `4FE822B0D412615BBDF0998C1C5242FF2C462CB1AE7885910C0D768CDCFE2392`
- `production_appcontainer_e4_lifecycle_cleanup.txt` `1A77777AA077F606CDD5466CB36FE1EAFB0C471B1C8D2BFC5A0E2363477100FA`
- `production_appcontainer_e4_reparse_containment.txt` `5F1239FC9FFFC94F22A054942F0083D78710032EF6F768DF9F81CA9B98983CED`
  (`production-e4-reparse-20261002.txt` `AA0272BE…E5CB` is the first run of the final probe, before the ACE and lint commits)
- `prepared-host-readiness.txt` `9E6E9AA8152E728743456394F0A99D12415068A993E8A7CB2B13CF54B8B8B17D`
- `workspace-tests.txt` `423A82241FCB8FE1F7A4C7C2B2A4290A59D1A1960BC5FB5646E8F8885BCF6248`
- `workspace-tests-all-features.txt` `BFE3F86DD8A1BFEF20FE4485E024FA4980161B4864AB21DB3DDA3011D5EE6CEA`

## 7. Integration path

- **Branch shape.** `codex/m0165-restricted-runtime` is a linear stack: M0.16.0–M0.16.4 branches are all
  ancestors. It is ahead of `main` and **0 behind**; `origin/main` (`001f69e`) is the merge base.
  `origin/codex/m0165-restricted-runtime` is at `b0a7d9b`, which the local branch fast-forwards.
- **Steps, each requiring explicit owner authorization:**
  1. push the branch (fast-forward, no force);
  2. confirm a green GitHub Actions `windows-validation` run on the **exact pushed HEAD SHA**;
  3. open a PR to `main` presenting the whole M0.16.0–M0.16.5 stack, with this review and the closure
     audit linked;
  4. merge with a **merge commit or fast-forward — not squash or rebase**, because reports and evidence
     cite exact commit SHAs.
- **Do not merge blindly.** If `main` moves first, merge `main` into the branch, re-run CI and the
  prepared-host probes, and never rewrite published history.

## 8. Decision

**APPROVED_WITH_NONBLOCKING_FOLLOWUP.** Commit `6dcf043` is approved as amended by `9915127`. The runtime
corrections `35c1c87`, `2854551` and `cc64e50` are part of the approved M0.16.5 state. Follow-ups F1–F7
are not acceptance conditions for M0.16.5. F1 and F5 should be scheduled before any non-LAB deployment
of the restricted verifier.

## 9. Integration gate (2026-10-03)

**Qualified code:** commit `83365a5d052faa74fc647705dd770597f8bef489`, tree
`9cf9506967fa192489b429e7114bf898d183f767`. Later commits change only `reports/evolution/*.md` and
`M0165_HANDOFF.md`; `git diff 83365a5 HEAD` touches no other path. Evidence:
`C:\MAIA\reports\evolution\m0165-host-preparation\integration-gate-20261003\` (`GATE\`).

### Defect found at the gate and fixed (`83365a5`)

The final Tier-1 run on `4ab0746` passed. The post-run scan nevertheless found an explicit inheritable
Modify ACE on `run-state` for a deleted per-run package SID (`S-1-15-2-1151077204-…`). This is the same
pattern as the two unmapped `run-state` SIDs cleaned up in the closure audit.

- **Cause.** Temporary-ACE grant and restore is a read-modify-write of a shared host DACL. Concurrent
  readiness ACL probes interleaved: the workspace-host test threads call `host_isolation_ready()` in
  parallel. One owner restored a snapshot that still held the other's ACE, and both reported verified
  cleanup. `run_restricted_process` also changed shared resource-root DACLs before taking the
  cross-process verifier slot.
- **Reproduced.** A new test with four concurrent probers leaked grants on 3 of 3 runs before the fix.
- **Fix.** The cross-process verifier mutex is now taken before any profile or DACL change in
  `run_restricted_process`, and around the readiness probe (120 s bounded wait; fails closed if the slot
  is unavailable). The test passes on 5 of 5 isolated runs and the suite is stable at 14 passed. Re-running
  the parallel workspace-host suite on the prepared host left no new grant.
- **Host cleanup.** The leaked `run-state` ACE was removed with `-DeletedProfileSid` (1 explicit path,
  verified). The pre-fix reproduction had also left six `maia-tier1-{20552,19076}-*` profiles, which the
  probe deliberately keeps when its own restore check fails. Their temp folders were already deleted, and
  the six profiles were removed with the exact-SID tool (0 ACE paths each, `ORPHAN_CLEANUP=VERIFIED`).
  Records: `GATE\orphan-acl-cleanup\` (15 files).

Because the runtime changed, the "exactly one final Tier-1" instruction was applied to the corrected code.
The `4ab0746` run (PASS, journal `BD73C0FF…88D5`) is kept as superseded evidence.

### Final production Tier-1 on `83365a5`

- Run `15184-18db1e960543f424`.
- Stages: Rustfmt 168.1 s, component check 194.6 s, Clippy 167.2 s, targeted tests 192.2 s (25 passed).
- Every stage: `Exited(0)` and `cleanup_verified=true`, with 4/4 "Job empty", 4/4 "ACL restoration
  verified" and 4/4 "profile cleanup verified".
- Clippy ran with the host-owned snapshot `clippy.toml`, recorded as `absent-defaults`. That a candidate
  config is actually applied is proven by the closure-audit differential.
- Journal `GATE\production-supervisor-83365a5\production-supervisor-1791058899594694300.jsonl`, SHA-256
  `348AC3CE3E9DF7B96866847B87FF3EECF0A61FF8CAC6C2A51BB0D8FD90A448B2`. Independently verified: 6/6 entries
  in the chain, `TIER1PASSED`, six checks PASS, no promotion, `content_recorded=false`. Diagnostics are
  copied beside it.
- Logs: stdout `C82C9104…AAE3`, stderr `67841790…29DA`.

### Short regression checks (full E4 not repeated; the change touches only slot ordering)

- process-host unit suite: 14 passed, 4 ignored, including the exact Null ACE and concurrent-probe tests
  (`A3BA7DCA…AA26`);
- lifecycle E4 on `83365a5`: 4/4 cases with exact ACLs, 0 residual SIDs, 0 mappings (`DC93BD95…B1DD`).

### Final host state

- 0 verifier processes;
- 0 MAIA AppContainer mappings;
- 0 per-profile package-SID ACE lines under the depot, `%LOCALAPPDATA%\MAIA`, `C:\MAIA\scratch` and Temp
  (`final-host-state-83365a5.txt` `8DB4AFBA…2D16`, `final-host-state-after-profile-cleanup.txt`
  `D55CF7C4…FD68`);
- Null capability: exactly one non-inherited read-only ACE, unchanged (`0A924C5E…708A`);
- C: free space 12,235,431,936 bytes.

### CI corrections after the push (2026-10-04)

The first `windows-validation` run on `b050dcd` (runs 37153505221 push / 37153508192 pull_request) failed
for two concrete reasons:

1. **Toolchain change.** The hosted runner's stable toolchain moved to Rust 1.99.0, which deprecates
   `AtomicU8::fetch_update`. Workspace Clippy with `-D warnings` therefore failed in
   `infra/claude-code/src/runner.rs`, code that is unchanged since `main`. Fixed in `fac8b9d` with an
   equivalent `compare_exchange_weak` loop.
2. **Too-strict test comparison.** The new concurrent-probe test compared full SDDL. On the runner, the
   temp folder starts without the DACL auto-inherited control flag (`D:` → `D:AI` after any write) while
   the ACEs are identical; no grant leaked. Fixed in `7bbbe33` to compare the ACE list. The test also
   failed inside the Round Table absence step, which runs the workspace tests.

Verified locally with a pinned, additive `1.99.0` toolchain (default `stable` and the production
verifier's pinned toolchain were not touched) and an empty `LOCALAPPDATA`:
- workspace Clippy with `-D warnings`: clean (also clean on 1.98.1);
- `cargo test --workspace`: 605 passed / 0 failed;
- `--all-features`: 721 passed / 0 failed;
- `verify_round_table_absent.py`: PROVEN.

**Qualification scope.** The delta from qualified commit `83365a5` (tree `9cf95069…f767`) consists of one
`#[cfg(test)]` assertion in `ops/evolution-process-host` and `infra/claude-code`. `maia-claude-code` is not in
the dependency graph of `maia-local-intelligence-host`, `maia-evolution-workspace-host` or
`maia-evolution-process-host`. The restricted runtime and the Tier-1 candidate build are therefore identical
to the qualified tree, and the Tier-1 PASS on `83365a5` stands without a further run.
