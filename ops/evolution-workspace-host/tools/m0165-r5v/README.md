# M0.16.5 R5V restricted-runtime proof tools

This folder preserves the exact scratch launcher source and host-side preparation needed to reproduce the recorded R5V E1-E4 package checks on the original LAB host. These tools are qualification artifacts. They are not the production Tier-1 adapter and do not by themselves close M0.16.5.

## Provenance and limits

The Clippy source is the rust-lang/rust repository at commit 48a229ceaefd4985c50990b14116b6d856af0985, the Rust 1.98.1 release commit. The commit exists in the local source checkout at C:\MAIA\scratch\m0165-rust-1.98.1-clippy. The Clippy package source copied into the standalone build was hash-compared against that checkout for conf.rs, main.rs, and driver.rs. The two executable source files matched byte-for-byte.

compat-clippy.patch changes exactly one source file, src/tools/clippy/clippy_config/src/conf.rs, replacing two canonicalize() path operations in lookup_conf_file() with std::path::absolute(). The patch diff has three insertions and three deletions, changing exactly two canonicalize() operations. It does not change lint definitions, Clippy command-line parsing, Cargo arguments, diagnostics logic, or exit-status propagation. cargo-clippy and clippy-driver report clippy 0.1.98 (48a229ceae 2026-09-01), the same user-facing version as the staged stock tool.

This is not general semantic equivalence. A local config differential with an explicit ordinary CLIPPY_CONF_DIR, a config setting that changes the too_many_arguments lint, identical diagnostics, and exit 101 passed for stock and compatibility binaries. A second local fixture with CLIPPY_CONF_DIR traversing a Windows directory junction found a real path-search difference: stock Clippy resolved the junction and found clippy.toml in the target's ancestor (exit 101); the compatibility build kept the alias path and did not find that ancestor config (exit 0). Thus the patch changes config discovery when a config search crosses symlinks/junctions. The recorded E1 run explicitly set CLIPPY_CONF_DIR to the freshly created candidate root and created an empty clippy.toml there, so it did not exercise that divergent case. Do not describe the compatibility binary as stock Clippy or use it with an uncontrolled config search path.

The Clippy compatibility source, patch, and locked dependency list are reproducible without committing generated executables or caches:

1. Obtain https://github.com/rust-lang/rust.git and checkout the exact commit above.
2. From a clean checkout, apply compat-clippy.patch at the repository root. Build-CompatClippy.ps1 checks the commit, clean starting tree, patch scope, and resulting config-source hash before building.
3. Supply the staged Rust 1.98.1 MSVC toolchain, including rustc-dev runtime files. Run Build-CompatClippy.ps1 with RustSourceRoot and a new OutputRoot. Host build dependency downloads, if needed, happen outside AppContainer and outside the test command. The script stages the Clippy package source separately, uses the committed compat-clippy.Cargo.lock, sets RUSTC_BOOTSTRAP=1 for rustc_private, builds cargo-clippy and clippy-driver in release mode, and assembles a hash manifest with the matching rustc_driver/std DLLs. The generated source copy, target directory, Cargo home, and bundle must stay outside Git.
4. Build the launcher from this folder with Build-R5VLauncher.ps1. It compiles the retained C# source using the Windows .NET Framework 4 C# compiler. The compile recipe was verified against the scratch source. The launcher executable is not committed.

## Host preparation

The harness is intentionally bound to the original LAB host paths in LadderR5V.cs. It expects:

- C:\MAIA\restricted-verifier-depot\toolchains\stable-x86_64-pc-windows-msvc with Cargo, rustc, rustdoc, rustfmt, and the matching compiler runtime DLLs;
- the already-generated and validated 388-package offline vendor depot at C:\MAIA\restricted-verifier-depot\vendor\m0165-r5v-generated-20260928;
- staged x64 MSVC cl.exe/link.exe and the MSVC/UCRT/Windows SDK headers beneath the msvc and windows-sdk folders;
- the compatibility source bundle under compat\clippy-r5v-rust-1.98.1-48a229cea; the host installer stages runtime files under %LOCALAPPDATA%\MAIA\RestrictedVerifierHost\resources\clippy-r5;
- a host-created disposable candidate workspace under C:\MAIA\candidate-workspaces-r2 and the recorded scratch workspace fixture under C:\MAIA\scratch\m0165-appcontainer-probe\tier1-workspace-r3.

Test-R5VHostResources.ps1 verifies the staged files, exact Cargo hash, and the existing vendor package/checksum count. It does not re-vendor or re-hash every archive; the full vendor validation remains in the preserved R5V preparation report. If host MSVC/SDK files must be staged again, copy only the required resources from an installed x64 Visual Studio Build Tools and Windows SDK into the depot. Do not grant AppContainer access to the original installed tool paths. The Cargo commands remain offline and the AppContainer receives no network capability.

The NUL device DACL is checked against the captured R2 baseline before a grant. The launcher grants only its temporary AppContainer Package SID, snapshots depot ACLs before adding the SID, restores them in finally, and writes a standalone restore record. Every run must end with RESTORE_NULL_DESCRIPTOR=PASS and VERIFIER_DEPOT_ACL_RESTORE=PASS with no residual package SID ACE. Keep the existing standalone restoration helper and captured baseline available when running the original scratch harness. The post-restore no-grant probe should return 9 and report nul_read=open-fail=5 and nul_write=open-fail=5.

## Replaying the recorded checks

Run host preparation and launcher compilation from an elevated PowerShell session with SeSecurityPrivilege assigned. The AppContainer itself is still created with zero capabilities; elevation is only for the host's temporary device/ACL preparation. Do not continue if the pre-probe NUL SDDL differs from the captured baseline.

The fixed Cargo arguments used were:

- E1: cargo clippy --manifest-path "{TARGET}\workspace\Cargo.toml" -p maia-local-intelligence-host --all-targets --all-features --locked --offline -- -D warnings
- E2: cargo check --manifest-path "{TARGET}\workspace\Cargo.toml" -p maia-local-intelligence-host --all-targets --all-features --locked --offline
- E3: cargo test --manifest-path "{TARGET}\workspace\Cargo.toml" -p maia-local-intelligence-host --all-targets --all-features --locked --offline
- E4: invoke the launcher with --nulprobe; require exit 0, zero capability SIDs, Job membership before resume, denied protected/workspace/depot writes, writable candidate target, and network_connected=False. Then invoke --nulprobe-no-grant and require exit 9 plus NUL read/write error 5.

The launcher uses a fixed 900,000 ms wait for its Cargo child so a first compilation can finish. This is a scratch proof harness bound, the production 900-second Tier-1 command deadline in spec/evolution_protected_runtime.yaml. The recorded E1, E2 and E3 Cargo-reported times were 55.91 s, 29.74 s and 39.81 s. The proof reports root-child Job membership and the configured 64-process/4 GiB/75% CPU Job limits. It does not substitute for testing production descendant cancellation, a fixed per-command deadline, or production evidence-journal behavior.

Original stdout/stderr, status files, E4 summary, restore record, and captured system evidence remain in C:\MAIA\reports\desk_outbox. They are deliberately outside this repository and were not modified or copied here. See reports/evolution/M0_16_5_R5V_RUNTIME_REVIEW.md for the evidence hashes and the exact scope decision.

## Production integration status (2026-09-30)

The worktree now wires FixedCargoTier1Verifier through run_restricted. The Windows process host creates a unique AppContainer with zero capability SIDs, creates the root process suspended, assigns it to a kill-on-close Job before resume, verifies the root token and Job membership, and grants temporary read access to host resources/workspace plus write access only to the candidate target. It waits for and terminates remaining Job members on root exit, timeout, cancellation, resource limit, or setup failure. Each observed production attempt reported exact temporary ACL restoration and AppContainer profile deletion.

Gate::tier1_isolation_ready() delegates to host resource validation, AppContainer profile creation/deletion, and a reversible ACL probe; it is not hard-coded true. Install-R5VHostResources.ps1 validates the existing toolchain/MSVC/SDK/vendor, stages the pinned Rustfmt executable and compatible Clippy bundle to a protected per-user resources directory, and registers the host resource root. This host preparation is non-elevated and does not rebuild or copy the vendor depot.

The actual production integration test allowed_candidate_mutation_runs_tier1_and_does_not_promote_or_touch_baseline ran through the GitWorkspaceHost entry point. Rustfmt exited 0 in AppContainer (164.2 s including ACL setup/restoration). Cargo Clippy, component check, and targeted tests each exited 101 before compilation because Windows returned Access denied while Cargo tried to execute clippy-driver.exe / rustc.exe -vV. The complete fixture test took 676.46 s. This is a production integration failure, not PASS. The ignored production journal test was not run; durable Gate/approval journal behavior is therefore not yet evidenced by this integration attempt.

The prior scratch E1-E4 logs remain qualification evidence only. Scratch E1 used the fixed host-owned candidate config path and compatible Clippy; the junction differential demonstrated a real configuration-discovery difference. Scratch E4 observed the tested loopback denial and tested filesystem results. Those scratch observations do not prove the production verifier's specific filesystem/network behavior, descendant cancellation, or durable journal result. The production run did verify the root token/AppContainer SID, zero capability SIDs, Job membership, root-exit Job drain, and cleanup, but it did not reach compiler work or production network/protected-surface probes.

M0.16.5 remains OPEN / BLOCKED. The precise remaining blocker is that the production AppContainer Cargo process cannot execute the staged Clippy driver and host rustc.exe from the protected resource paths (Win32 access denied). Consequently E1-E3 have not passed through the production adapter, and production E4 isolation probes plus the production journal outcome remain unverified. Do not treat scratch evidence as production acceptance, enable promotion, or begin M0.16.6.

### Production E2E follow-up (2026-09-30)

The host adapter now also validates the two SHA256-pinned Rust runtime DLLs in the toolchain and temporarily grants read/execute on those exact files. It adds temporary traverse-only ACEs for immediate read-root parents, the candidate target parent, and their exact prior ACL snapshots are restored after the run. These ACL refinements were made after the earlier Cargo launch failure; they do not broaden the AppContainer token or grant network capability.

The repeated production GitWorkspaceHost E2E completed Rustfmt with exit 0, then Clippy, component check, and targeted tests each exited 101. Each root process was attested in the AppContainer and Job, each Job drained to zero, and the host logged verified temporary ACL restoration and AppContainer profile cleanup. Total test duration was 717.53 seconds. The test rejected the candidate as Tier1Failed. Because production stdout/stderr are explicit NUL handles, the remaining Cargo 101 cause is not present in the durable evidence. Do not infer a specific compiler or filesystem error from the exit status.

The allocation suite was adjusted so the unavailable-isolation case injects a deterministic false host capability, and the durable-journal mechanics test uses its existing scripted verifier. The real production verifier remains separately exercised by the E2E above; it still does not pass. Its journal assertion and all production E4 network/protected-surface probes remain unverified.

M0.16.5 remains OPEN / BLOCKED. Production Tier-1 integration is wired and containment/cleanup are observed, but production E1-E3 fail with exit 101 and no diagnostic output; production E4 isolation outcomes and durable decision journaling have not been demonstrated. The scratch E1-E4 report remains evidence only for its own harness.

### Host-side fixture control (2026-09-30)

Using the same minimal allocation-test manifest, lockfile, pinned Cargo/Rust toolchain, offline vendor configuration and fixed command flags in the ordinary host context, cargo check exited 0 in 0.64 s. This rules out the synthetic manifest, lockfile and Cargo TOML as causes in the host context; it does not identify the remaining AppContainer-specific exit 101. Production child output remains NUL by design, so the exact restricted-token diagnostic is still unavailable.

## Production runner output capture follow-up (2026-10-01)

The production `FixedCargoTier1Verifier` now stores bounded stdout/stderr per fixed command under the host-only `run-state/diagnostics/<run-id>/` directory. Each stream is drained concurrently and storage is capped at 256 KiB while excess output is discarded after draining. The host writes captures only after the restricted process tree exits and ACL/profile cleanup has been verified.

The current prepared LAB host can run fixed Rustfmt in the AppContainer, but Clippy, component check, and tests fail before rustc starts: Cargo reports Windows error 5 when attempting the captured child launch (`rustc -vV`, never executed). This is a production Tier-1 failure and keeps the milestone OPEN / BLOCKED. The scratch E1-E4 results do not replace the production test. See `reports/evolution/M0_16_5_R5V_RUNTIME_REVIEW.md` for the current evidence and limits.

### AppContainer null-stdin diagnosis and host resource (2026-10-01)

The production runner's captured stdout/stderr pipes are not the denied resource. A short ignored test in `maia-evolution-process-host` invoked through the exact production AppContainer/Job path compared inherited streams, Win32 `CreatePipe`, Rust `Command::spawn` variants, `Command::output()`, and the three NT calls in the pinned Rust `child_pipe.rs`. Inherited stdio, Win32 `CreatePipe`, `Command::spawn` with inherited stdin, `Command::spawn` with piped output, `NtOpenFile("\\Device\\NamedPipe\\")`, `NtCreateNamedPipeFile`, and `NtOpenFile` on the anonymous peer all succeeded. `CreateFileW("NUL", GENERIC_READ)` failed immediately with Win32 5; `Command::spawn` with `stdin=Stdio::null()` and `Command::output()` failed with OS error 5. The denial is the default null stdin open before the captured pipe path.

The pinned Rust 1.98.1 std source confirms `Stdio::Null` opens `\\.\NUL`; the pinned `child_pipe.rs` NT sequence itself succeeded in the probe. No Cargo, Clippy, or Rust toolchain rebuild is indicated by this evidence.

The canonical contract now allowlists exactly one non-network resource capability, `maia.evolution.tier1.null.stdin`. Each process retains its unique random AppContainer package SID and receives that one extra capability. The capability gets a persistent, non-inheritable `FILE_GENERIC_READ` ACE only on `\\Device\Null`; it is never added to filesystem, registry, NamedPipe, or other ACLs. No network capability is granted. Host preparation and exact-SID rollback are separate in `Prepare-M0165NullStdinCapability.ps1`; `-ValidateOnly` is unprivileged, while install/remove require an elevated administrator shell and write a durable SDDL report under `C:\MAIA\reports\desk_outbox`. Production readiness and process start fail closed unless the exact ACE is present. Earlier scratch E1-E4 used zero capability SIDs and remain separate evidence.

The current LAB host has no ACE for the derived capability SID. Its DACL probe returned `D:(A;;0x1201bf;;;WD)(A;;FA;;;SY)(A;;FA;;;BA)(A;;0x1200a9;;;RC)`, capability SID `S-1-15-3-1024-1988635889-3063999656-362765618-2578588777-2081098023-3982749461-1818700090-2051019908`, and `ace_count_for_sid=0`. Host preparation has not been applied. The code path and tests compile, but the positive post-preparation AppContainer probe, component check, Clippy, targeted tests, production E4 probes, and production journal result remain to be run. M0.16.5 remains OPEN / BLOCKED.

### Crash recovery for a per-run AppContainer

An ordinary cancellation returns through `run_restricted_process`, which drains the Job, restores temporary ACLs, and deletes the profile. Abrupt host termination skips those in-process cleanup objects. Before another production attempt, copy the interrupted journal and logs outside Temp, establish that the owner and verifier descendants have ended, and identify the exact profile name, package SID, candidate root, run-state directory, and granted host roots. `Recover-OrphanTier1Profile.ps1` inspects those roots and writes a before record. With `-Apply`, it removes only explicit allow ACEs for that unique package SID, rescans for residual SID access, checks unrelated ACL rules, and then calls `DeleteAppContainerProfile`. Include every relevant host and candidate root in `-ScanRoots` and any granted ancestor in `-ExactPaths`; an incomplete path set is not proof of whole-host cleanup. The script never changes the persistent `maia.evolution.tier1.null.stdin` device ACE. Retain its before/after JSON and the original journal; do not append a guessed terminal event to an interrupted journal.
