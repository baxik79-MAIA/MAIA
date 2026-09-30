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
- the compatibility bundle under compat\clippy-r5v-rust-1.98.1-48a229cea;
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

The launcher uses a fixed 900,000 ms wait for its Cargo child so a first compilation can finish. This is a scratch proof harness bound, not the production 120-second Tier-1 command deadline in spec/evolution_protected_runtime.yaml. The recorded E1, E2 and E3 Cargo-reported times were 55.91 s, 29.74 s and 39.81 s. The proof reports root-child Job membership and the configured 64-process/4 GiB/75% CPU Job limits. It does not substitute for testing production descendant cancellation, a fixed per-command deadline, or production evidence-journal behavior.

Original stdout/stderr, status files, E4 summary, restore record, and captured system evidence remain in C:\MAIA\reports\desk_outbox. They are deliberately outside this repository and were not modified or copied here. See reports/evolution/M0_16_5_R5V_RUNTIME_REVIEW.md for the evidence hashes and the exact scope decision.

## Acceptance boundary

The E1-E4 results establish a successful controlled Cargo/Clippy/build/test run in this scratch AppContainer profile and the specific observed filesystem/network boundary. They do not establish that the production FixedCargoTier1Verifier executes in AppContainer. In the reviewed production source, evolution-process-host remains Job Object process containment, and Gate::tier1_isolation_ready() defaults to false. No durable protected Gate/approval attestation or production AppContainer integration was exercised. Therefore this evidence does not satisfy M0.16.5 closure criteria and does not authorize promotion or M0.16.6.