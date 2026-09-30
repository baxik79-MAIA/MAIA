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