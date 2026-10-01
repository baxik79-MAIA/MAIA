# M0.16.5 R5V Runtime Evidence Review

**Verdict: OPEN / BLOCKED. M0.16.5 is not CLOSED_READY. M0.16.6 was not started.**

## Scope and decision basis

This review assessed the R5V completion report and original E1-E4 logs, the scratch launcher, the Rust/Clippy source checkout, its compatibility patch/build bundle, the controlling M0.16.3 and M0.16.4 specs/reports, and the current production host source.

The recorded E1-E4 runs prove a useful controlled LAB-host AppContainer experiment. They do not prove that production Tier-1 calls the AppContainer runner. The M0.16.4 readiness report requires a production restricted identity with filesystem/no-network policy and integration tests that verify actual OS capabilities before production Tier-1 is enabled. The M0.16.5 capability blocker explicitly says the production AppContainer Cargo launch and durable Gate attestation remain unavailable and that the milestone must stay fail-closed until the complete restricted verifier profile is demonstrated.

That limitation remains visible in current production code:

- ops/evolution-process-host/src/lib.rs documents Job Object containment only and disclaims network/filesystem/restricted-token isolation.
- ops/evolution-workspace-host/src/lib.rs Gate::tier1_isolation_ready() defaults to false; GitWorkspaceHost checks that gate before Tier-1.
- ops/evolution-workspace-host/src/tier1.rs FixedCargoTier1Verifier uses run_contained with a 120-second command bound. It does not create or attest an AppContainer identity.
- No durable protected Gate/approval attestation or attempt record from the scratch run was produced.

Therefore E1-E4 are qualification evidence for scratch tooling, not a production milestone acceptance. Keeping the default false Gate is the correct fail-closed outcome.

## E1 Clippy assessment

The stock staged Cargo and Clippy report `cargo 1.98.1` and `clippy 0.1.98 (48a229ceae 2026-09-01)`. The Clippy source provenance is rust-lang/rust commit `48a229ceaefd4985c50990b14116b6d856af0985`, corresponding to Rust 1.98.1. The local source checkout's HEAD matches that commit. The upstream Clippy package `conf.rs`, `main.rs`, and `driver.rs` source hashes match the standalone build source for these files.

The only source patch is `src/tools/clippy/clippy_config/src/conf.rs`. It changes the two path operations in `lookup_conf_file()` from `canonicalize()` to `std::path::absolute()`. The patch does not touch lint definitions, Clippy argument parsing, cargo-clippy argument forwarding, diagnostic construction, or exit-status propagation. The retained cargo-clippy and clippy-driver user-visible version strings match stock. Bundle hashes and the locked build inputs are in ops/evolution-workspace-host/tools/m0165-r5v/README.md and compat-clippy.patch.

The actual E1 Cargo arguments match the M0.16.3 fixed Tier-1 Clippy requirement: package maia-local-intelligence-host, all targets/features, locked, offline, and -D warnings. The absolute staged Cargo executable was invoked; PATH selected the compatibility cargo-clippy/clippy-driver bundle, and the runner set SYSROOT to the same staged Rust 1.98.1 toolchain. The scratch runner also set CLIPPY_CONF_DIR to the host-created candidate root and created an empty clippy.toml there.

E1 is consequently **PASS for this recorded fixed-path compatibility-Clippy invocation**, not a stock-Clippy run. Stock Clippy failed inside the AppContainer while canonicalizing the candidate configuration path (Win32 error 5). The compatible build passed the fixed package run in 55.91 seconds. The original report's successful host fixture compared an ordinary `len_zero` warning; this review added a focused host differential where clippy.toml lowers too-many-arguments-threshold to 2. Stock and compatibility Clippy emitted the same diagnostic text and both exited 101 under -D warnings.

The same focused test exposed the exact equivalence limit. With CLIPPY_CONF_DIR beneath a Windows junction and clippy.toml located only in an ancestor of the resolved target, stock Clippy found the config and exited 101; the compatibility binary retained the alias path, did not find that ancestor config, and exited 0. This is an actual config-discovery semantic difference caused by absolute() not resolving junctions. It did not occur in recorded E1 because that harness supplied its own ordinary candidate-root config file. The patch is suitable only as a narrowly qualified compatibility tool under that fixed host-owned config-path setup; it is not generally interchangeable with stock Clippy.

The compatibility run is useful evidence, but complete E1 equivalence is not established for arbitrary paths. A production adapter would need to preserve the fixed config-path invariant and test it, or use a supported way to run stock Clippy.

## E2-E4 evidence scope

### E2: component check

The locked/offline all-target/all-feature check for maia-local-intelligence-host passed in the scratch launcher; Cargo reported 29.74 seconds. The AppContainer token had zero capability SIDs and its launched root process was in the Job before resume. This is not an invocation of FixedCargoTier1Verifier.

### E3: targeted tests

The same package's all-target/all-feature tests passed: 25 tests, zero failures (7 library, 9 binary, 8 capability-mesh, 1 host smoke); Cargo reported 39.81 seconds. This verifies the copied scratch workspace contents used by the launcher. The E4 canonical-host snapshot shows that canonical HEAD, status, and selected protected file hashes did not change. It does not bind every file in the copied scratch workspace to an admitted candidate identity or prove the production adapter's candidate-identity check.

### E4: isolation and protected-surface probes

The root child was created suspended; before resume the launcher observed AppContainer identity, zero capability SIDs, and Job membership. The Job was configured for kill-on-close, 64 processes, 4 GiB job memory, and a 75% CPU hard cap. The recorded probes showed:

- candidate target write succeeded; candidate workspace-root write failed;
- canonical/protected/profile/restore-record reads and verifier-depot writes failed;
- vendor/config reads succeeded while vendor/config writes failed;
- the host-controlled loopback connection did not connect;
- no network capability SID was present.

This proves those tested paths and a loopback denial for that process. These logs do not test every possible network endpoint or prove production descendant-cancellation handling. Root Job membership and configured limits are not a substitute for production process-tree integration tests.

The launcher finally restored the original full NUL device descriptor and reports no residual Package SID ACE. E4 separately observed the same pre/post SDDL and ran a no-grant probe that received Win32 error 5 for NUL read and write. E1-E3 successful logs also report exact NUL restoration and temporary depot ACL cleanup. The initial non-elevated E2 attempt failed before changing system state; the elevated rerun passed.

The scratch Cargo launcher now waits up to 900,000 ms (15 minutes). This fixed the prior accidental 30-second truncation and allowed the recorded work to finish. It is an outer scratch-harness wait, not the production spec's 120-second per-command timeout. The actual recorded Cargo durations were under 120 seconds, but the scratch runner does not enforce the production deadline or create the production journal outcome when a command exceeds it.

## Reproducible artifacts committed with this report

- ops/evolution-workspace-host/tools/m0165-r5v/LadderR5V.cs preserves the tested launcher source and 15-minute Cargo wait.
- Build-R5VLauncher.ps1 preserves the verified .NET Framework C# compile command; no executable is committed.
- compat-clippy.patch, compat-clippy.Cargo.lock, and Build-CompatClippy.ps1 record exact Rust source provenance, patch, locked inputs, and the release build procedure; no binary, cache, Cargo home, or vendor tree is committed.
- Test-R5VHostResources.ps1 checks the staged 1.98.1 toolchain, MSVC/SDK header layout, vendor structure/count, and Cargo hash without rebuilding the vendor.
- README.md records host preparation, commands, evidence interpretation, and explicit compatibility/production limits.

The original logs and system reports remain at their C:\MAIA\reports\desk_outbox paths, including the E1-E4 logs and summary. Their hashes are preserved in the preceding desk_outbox completion report; this report does not replace or rewrite those original traces. Additional host differential traces remain outside Git: ordinary config case stock/compat outputs under C:\MAIA\scratch\m0165-clippy-config-diff (both exit 101; SHA-256 438A677981EE060F62336017097C17FD0AD70B69ADCE01062C0BF30EF8D95963 and 57B75C07AFC4CA51178E66F3D990EDB1FC82F9A2EBF8E648643A9310DE6CBFE0), junction case under C:\MAIA\scratch\m0165-clippy-symlink-diff2 (stock 101 / compat 0; SHA-256 530CF64760ECDE3305ADEC22932DAD36008A16DCD87763C58A977E5E1F896714 and 51758D3BB8D64DE191F97B9299AF5A579BFA832465885CE4C01DE1DCC4967979).

## Remaining blocker and milestone status

**Blocker:** production Tier-1 does not use the restricted AppContainer profile. The production process host still guarantees Job Object containment only; the Gate's isolation readiness default remains false; durable protected Gate/approval attestation and production OS-capability integration tests are absent. Also, the proof-only Clippy patch has a demonstrated junction-based config-search difference outside the exact fixed E1 path.

**Impact:** Tier-1 remains fail-closed/unavailable through the production host. Scratch E1-E4 must not be reported as production acceptance or general stock-Clippy equivalence. M0.16.5 remains OPEN / BLOCKED; do not begin M0.16.6, promote, or enable production Tier-1 on this evidence alone.

## Host measurement and repository state

- C: free space was 8767434752 bytes at 2026-09-30 07:14:50 UTC (measured once after validation, before commit). The 2026-09-29 figure in the prior R5V completion report is historical.
- Worktree: C:\MAIA\public-export\.local\m0160-supervisor, branch codex/m0165-restricted-runtime. The C:\MAIA\repo public-release worktree contains unrelated existing changes and was left untouched.
- No runtime binaries, vendor depot, local caches, credentials, or host snapshots are committed.

## Production runtime integration attempt (2026-09-30)

This section supersedes the earlier statement that production FixedCargoTier1Verifier still used Job-only containment. The worktree now calls the AppContainer runner from the production verifier and uses the host resource registration installed by ops/evolution-workspace-host/tools/m0165-r5v/Install-R5VHostResources.ps1. The installer revalidated the existing resources (Cargo SHA C37545EC61D48D31BDEFCE53280ECAB61C4EA54EAACE372FAC6A0316C6E165D9; vendor 388 packages and 388 checksums), staged the pinned Clippy compatibility runtime under %LOCALAPPDATA%\MAIA\RestrictedVerifierHost\resources\clippy-r5, staged Rustfmt under the corresponding rustfmt-r5 directory, and did not rebuild or copy the vendor depot. The installer is non-elevated.

The focused production entry-point test was:
cargo test --manifest-path ops/evolution-workspace-host/Cargo.toml --offline --test allocation allowed_candidate_mutation_runs_tier1_and_does_not_promote_or_touch_baseline -- --nocapture

It ran all four fixed verifier stages through GitWorkspaceHost and FixedCargoTier1Verifier. Rustfmt exited 0 in AppContainer after 164.2 seconds. Clippy exited 101 after 158.5 seconds, component check exited 101 after 160.1 seconds, and targeted tests exited 101 after 159.4 seconds. Each Cargo stage failed before compilation with Windows Access denied (os error 5) while trying to execute the staged clippy-driver.exe / host rustc.exe -vV. The complete test took 676.46 seconds and failed its Tier1Passed assertion. This is not a production E1-E3 PASS.

For every observed process run, the root was created suspended, assigned to the configured Job before resume, and verified with the expected AppContainer SID and zero capability SIDs. The Job was observed empty after root exit, including when Cargo returned 101. Temporary ACL restoration reported verified and the unique AppContainer profile deletion reported verified after each stage. The production runner does not change the NUL device descriptor. The production integration test did not reach the package compile/test workloads, did not run production network or protected-surface probes, and did not produce a durable journal result. The ignored production journal test remains unrun.

An additional host-only rustc staging probe was rejected and reverted: a copied rustc.exe under per-user resources reported that directory as its inferred sysroot, and a no-dependency Cargo check then failed to find std. No such rustc copy remains staged or referenced. The production runner continues to use the pinned toolchain rustc path.

### Updated scope and milestone status

Scratch E1-E4 remain separate qualification results. Scratch E4 showed the selected loopback connection was denied and the tested filesystem reads/writes had the recorded outcomes. Production runtime currently proves root AppContainer identity, zero capability SIDs, Job membership and drain, and restoration after execution. It does not yet prove production no-network or protected-surface denial under a successful compiler run.

M0.16.5 remains OPEN / BLOCKED. The current exact blocker is child-process creation from the production AppContainer: Cargo cannot execute the pinned Clippy driver and toolchain rustc because Windows reports access denied. That prevents production E1, E2, and E3 from completing and leaves production E4 and the durable Gate/approval journal result unverified. Do not label the milestone CLOSED_READY or start M0.16.6.

### Production E2E follow-up (2026-09-30)

The host adapter now also validates the two SHA256-pinned Rust runtime DLLs in the toolchain and temporarily grants read/execute on those exact files. It adds temporary traverse-only ACEs for immediate read-root parents, the candidate target parent, and their exact prior ACL snapshots are restored after the run. These ACL refinements were made after the earlier Cargo launch failure; they do not broaden the AppContainer token or grant network capability.

The repeated production GitWorkspaceHost E2E completed Rustfmt with exit 0, then Clippy, component check, and targeted tests each exited 101. Each root process was attested in the AppContainer and Job, each Job drained to zero, and the host logged verified temporary ACL restoration and AppContainer profile cleanup. Total test duration was 717.53 seconds. The test rejected the candidate as Tier1Failed. Because production stdout/stderr are explicit NUL handles, the remaining Cargo 101 cause is not present in the durable evidence. Do not infer a specific compiler or filesystem error from the exit status.

The allocation suite was adjusted so the unavailable-isolation case injects a deterministic false host capability, and the durable-journal mechanics test uses its existing scripted verifier. The real production verifier remains separately exercised by the E2E above; it still does not pass. Its journal assertion and all production E4 network/protected-surface probes remain unverified.

M0.16.5 remains OPEN / BLOCKED. Production Tier-1 integration is wired and containment/cleanup are observed, but production E1-E3 fail with exit 101 and no diagnostic output; production E4 isolation outcomes and durable decision journaling have not been demonstrated. The scratch E1-E4 report remains evidence only for its own harness.

### Host-side fixture control (2026-09-30)

Using the same minimal allocation-test manifest, lockfile, pinned Cargo/Rust toolchain, offline vendor configuration and fixed command flags in the ordinary host context, cargo check exited 0 in 0.64 s. This rules out the synthetic manifest, lockfile and Cargo TOML as causes in the host context; it does not identify the remaining AppContainer-specific exit 101. Production child output remains NUL by design, so the exact restricted-token diagnostic is still unavailable.

## Production follow-up from checkpoint 6caf8b5 (2026-10-01)

This follow-up supersedes the earlier snapshot above wherever it described the current production runner. `FixedCargoTier1Verifier` now calls the restricted AppContainer/Job runner and host-captures stdout/stderr. The fixed worker remains closed on any failed check; no weakened profile or alternate command was enabled.

### Captured production result

The real production-entry integration test `allowed_candidate_mutation_runs_tier1_and_does_not_promote_or_touch_baseline` was run serially with the prepared host resources. It invoked the actual supervisor mutation path and ran all four fixed Tier-1 stages:

| Stage | Result | Evidence |
|---|---|---|
| Rustfmt | exit 0 | AppContainer root resumed only after Job assignment; capture completed; ACL restoration and profile cleanup verified |
| Clippy | exit 101 | stderr: Cargo could not execute `clippy-driver.exe ... rustc.exe -vV` (never executed), Windows error 5, Access denied |
| Component check | exit 101 | stderr: Cargo could not execute staged `rustc.exe -vV` (never executed), Windows error 5, Access denied |
| Targeted tests | exit 101 | same `rustc.exe -vV` / Windows error 5 before rustc execution |
| Protected surface integrity | passed | exact allowed candidate change was the only worktree change |

Each process stage reported `cleanup_verified=true`; runner output confirms the Job was empty before return, and the host reported `temporary ACL restoration: verified` and `AppContainer profile cleanup: verified`. Rustfmt, Clippy, component-check and targeted-test captures are under `C:\MAIA\restricted-verifier-depot\run-state\diagnostics\13564-18da7b9923501d1c`, outside candidate ACLs. The component failure is independently preserved at `...\probe-520\component-build.stderr.bin` with metadata `Exited(101)`, 189 stderr bytes, no truncation, and `cleanup_verified=true`.

The supervisor result was `Rejected` with `Tier1Failed`; the individual evidence was FormattingLint=Failed, ComponentBuild=Failed, TargetedTests=Failed, and ProtectedSurfaceIntegrity=Passed. The test's assertion expected a successful mutation and therefore the test process exited failed after recording that accurate rejection. This run is not E1-E3 PASS and does not close M0.16.5.

### Diagnosis and boundary

The original production diagnostic was unavailable because the host sent both streams to NUL. The runner now creates separate pipes with a host-user and per-run AppContainer SID DACL, passes only stdin/stdout/stderr through the process attribute handle list, closes parent writer copies after process creation, drains both pipes concurrently, retains at most 256 KiB per stream while continuing to drain, and joins the readers after the Job becomes empty. Captures are persisted by the host outside the candidate workspace. The fixed Rustfmt command now passes through this path.

A focused AppContainer helper probe showed that `CreatePipe` itself succeeds inside the AppContainer, while `std::process::Command::output()` attempting a child with captured streams returns `Access denied (os error 5)`; launching the same child with inherited standard streams succeeds. The staged Rust 1.98.1 source at `library/std/src/sys/process/windows/child_pipe.rs` shows that captured process output takes a different path: it opens `\Device\NamedPipe\` with `NtOpenFile`, then uses `NtCreateNamedPipeFile` and `NtOpenFile` to build the overlapped child pipes. This source plus the paired probe localizes the failure to that NT named-pipe/captured-child path, although no syscall trace identifies which of those calls returns error 5. A one-off token-default-DACL adjustment did not resolve the Cargo run and was removed. No broad ACL, AppContainer capability, network, or candidate-write grant was added; changing the global named-pipe device ACL would broaden local IPC access and was not attempted. A lasting workaround would need a reviewed/rebuilt Cargo and Clippy toolchain using an AppContainer-compatible captured-pipe backend, then a repeated production run; the pinned binaries were not silently replaced.

### Verification and status

- `maia-evolution-process-host`: all 11 unit tests passed, including simultaneous stdout/stderr capture over the storage limit, timeout and cancellation with descendants, and exact nested ACL restoration.
- `maia-evolution-workspace-host` library tests: 7 passed, 1 ignored (prepared host required).
- Allocation integration tests excluding the two long cases: 19 passed, 1 ignored, 2 filtered. The production-entry case was then run alone and exercised the actual supervisor as described above; it failed its PASS expectation with the captured Tier-1 rejection. The durable journal test was run separately and passed (synthetic Tier-1 verifier), confirming queryability/hash-chain/disjointness for that adapter path; it does not verify a durable journal record from the production AppContainer run.
- `cargo check --offline` passed for both host crates. A local `cargo clippy --all-targets -- -D warnings` exploration is not an E1 result and reported existing host-crate lints including `too_many_arguments` and `collapsible_if`; it did not run the restricted candidate command.
- Production durable-journal verification and the explicit E4 network/protected-surface adversarial probes remain unverified. Scratch E4 evidence remains scoped to its own launcher and is not substituted for these checks.
- System changes from the completed production run were restored: all three process stages report exact ACL restoration and AppContainer profile cleanup; exact nested-DACL restore tests passed. No DeskBridge action was performed.

**Current verdict: OPEN / BLOCKED, not CLOSED_READY.** The concrete remaining blocker is the repeated Windows error 5 when Cargo/Clippy's child process is launched with captured stdout/stderr inside the AppContainer. It prevents Clippy, component check, and targeted tests from executing their rustc child, so the production Tier-1 verdict is rejection. E4 durable journal and adversarial isolation acceptance have not been established. M0.16.6 was not started.

Disk measurement: C: had 3,195,781,120 bytes free at approximately 2026-10-01 18:53 UTC (20:53 Europe/Warsaw). Worktree: `C:\MAIA\public-export\.local\m0160-supervisor`; branch `codex/m0165-restricted-runtime`; starting HEAD `6caf8b55df5ed51c26287329b7971002e3b46211`.

## Durable production-supervisor journal follow-up (2026-10-01)

The ignored integration fixture originally failed before Tier-1 because it named its candidate root `candidates`, while the production host requires the exact `evolution-candidates` root name. This test-fixture-only mismatch was corrected in commit `0b0f4e4`; no acceptance check was relaxed. The test then ran from the clean committed checkout through the real `ProtectedRuntimeGate`, `GitWorkspaceHost`, `FixedCargoTier1Verifier`, restricted process runner, and `FileEvidenceJournal`.

The production result was `Rejected / Tier1Failed`, as expected from the captured Clippy/component/test failures. Rustfmt passed; Clippy, component check, and targeted tests failed before rustc execution. The result included `CandidateIdentity=PASS`, `SyntaxStatic=PASS`, and `ProtectedSurfaceIntegrity=PASS`. The test's existing assertion expects all Tier-1 stages to pass and therefore the test process itself ended failed at that assertion; the failure is recorded honestly and is not an acceptance PASS.

The durable journal was preserved at:

`C:\Users\PS\AppData\Local\Temp\maia-m0165-production-4360-1790881223336353600\evidence\evolution.jsonl`

It is 7,622 bytes, SHA-256 `E66AC37935CC0A81BC502C5796CD75DB44342F46A9BD335885B84372C789659D`. All 9 entries passed sequence, previous-hash, and entry-digest verification against the `FileEvidenceJournal` chain algorithm. The recorded decision is `TIER1FAILED` / `REJECTED`; checks are identity PASS, syntax PASS, formatting/lint FAIL, component build FAIL, targeted tests FAIL, protected-surface integrity PASS. The journal records `candidate_terminal=REJECTED`, `operation=discard_candidate`, followed by `state=CLOSED`. The candidate directory is absent after rejection. This verifies durable journaling and discard through the real supervisor failure path; it does not turn the Tier-1 result into PASS.

The production-entry test does not run separate adversarial network or protected-path probes. Those E4 claims therefore remain limited to the earlier scratch launcher evidence. No production PASS or full E4 isolation acceptance is claimed.

Final disk sample: C: had 3,191,877,632 bytes free at approximately 2026-10-01 19:21 UTC (21:21 Europe/Warsaw). Repository worktree was clean at HEAD `0b0f4e497f8c2d5b5f1115fd8ec5f652951b7e91` on `codex/m0165-restricted-runtime`.
