# M0.16.5-R1 Restricted Rust Toolchain Compatibility Proof

## Status

**M0.16.5 remains OPEN / BLOCKED.** The dedicated staged Rust toolchain can execute directly in the restricted AppContainer, but Cargo's first Rustc probe fails before the Rustc process starts. Investigation stopped at ladder Step E. No weaker verifier mode was enabled.

## Checkpoint and repository state

- R1 starting commit: `73db03d12baf944fa649a3d22e935545c99a8c5d`
- Branch: `codex/m0165-restricted-runtime`
- The checkpoint branch was absent from origin before this work. It was pushed without modification, and origin was verified at that exact SHA before experiments.
- Original blocker report: [M0_16_5_CAPABILITY_PROOF_BLOCKER.md](M0_16_5_CAPABILITY_PROOF_BLOCKER.md)
- Original blocker report SHA-256 before and after R1: `D4761CB7DC39ABE822EC2F3C7FD18DD7B91CB7E504ECAC896F285B5E787042B7`
- No Supervisor, Gate, Tier-1, canonical source, or spec implementation was changed during R1.

## Host and staged toolchain

- Host: Windows 10 Pro build `19045.0`, x64
- Rust: `rustc 1.98.1 (48a229cea 2026-09-01)`, commit `48a229ceaefd4985c50990b14116b6d856af0985`
- Cargo: `1.98.1`, commit `797e8a9bc276c1c9f9f738d2a20f484fa4eea9d`
- MSVC linker: `14.44.35228.0`
- Windows SDK libraries: `10.0.26100.0`
- Host-selected staged depot: `C:\MAIA\restricted-verifier-depot`
- Depot logical size at stop: `2,268,255,694` bytes across 70,253 files
- C: free space: `8,212,856,832` bytes before staging and `7,505,739,776` bytes at stop

The depot contains copies of the active Rust toolchain, MSVC x64 linker/bin and libraries, and Windows SDK x64 UM/UCRT libraries. Nothing was granted on the original `.rustup`, `.cargo`, profile, canonical checkout, or machine-wide filesystem. During each probe, only the depot root received a temporary AppContainer Package SID read/execute ACE; the candidate workspace root received read/execute and its candidate target received write access. The depot write probe failed as expected.

Critical staged file hashes:

| File | SHA-256 |
|---|---|
| `toolchains/stable-x86_64-pc-windows-msvc/bin/rustc.exe` | `CA9988AF88B1463F6857FDFE909299FEE50A63260B3E5F7DBCB5B4D3318B27BC` |
| `toolchains/stable-x86_64-pc-windows-msvc/bin/cargo.exe` | `C37545EC61D48D31BDEFCE53280ECAB61C4EA54EAACE372FAC6A0316C6E165D9` |
| `msvc/bin/Hostx64/x64/link.exe` | `CA11E6C45DEBD34BF652DFE984C5360A531A005ED78BF72852330C9C2590CF0D` |
| compiled `hello.exe` result | 134,656 bytes; `18F504A5FDD0A7529B251101EAEAD38D803811818E4405ED8B9D816D987B12CD` |

The temporary proof launcher and helper source hashes are `Ladder.cs` `1AD75DF7D9CE3C232377AF2806943AC41BFDC2F946D09DB4C975FDC4BC79E8ED` and `spawnprobe.rs` `3645EA65D42417300661194ABED79C80C375CD1E38C5E52C0D520B30D24D71C3`. They are retained under `C:\MAIA\scratch\m0165-appcontainer-probe` for local inspection. They are diagnostic artifacts, not production verifier components.

## Restricted launch configuration

Each child was created suspended with a host-created AppContainer profile and zero requested capabilities. The launcher assigned it to a Windows Job Object before resuming it. The Job used kill-on-close, a 64-process cap, a 4 GiB job memory cap, and a 75% hard CPU cap. Before resume, the launcher verified `TokenIsAppContainer=1`, read the child package SID, queried `TokenCapabilities`, and verified `IsProcessInJob`.

On the Step E failure run, the token evidence was:

```text
appcontainer_sid=S-1-15-2-2886214934-1414181939-2738900050-1905209736-2122247246-296526792-2059759011
capability_count=0
capability_sids=[]
network_capabilities=[]
job_membership=True
```

The package SID is generated per AppContainer profile. No Internet or private-network capability SID was present. A live host-controlled loopback listener was unreachable from the restricted child. The combined filesystem/network probe reported:

```text
candidate_target_write=true
workspace_root_write=false
canonical_repo_read=false
unrelated_profile_read=false
toolchain_depot_write=false
loopback_connected=false
```

The unrelated profile check attempted to read `C:\Users\PS\.codex\memories\MEMORY.md` and did not read its contents. The candidate could write only in its explicitly granted target. Canonical checkout and host profile reads failed. The verifier depot remained non-writable to the AppContainer.

## Ordered execution ladder

| Step | Result | Evidence |
|---|---|---|
| A. Trusted trivial executable | **PASS** | `C:\Windows\System32\whoami.exe` launched in the AppContainer Job and exited `0`. AppContainer identity, package SID, and Job membership were checked before resume. |
| B. `rustc.exe --version` at the normal user toolchain path | **FAIL, diagnosed** | `CreateProcessW` succeeded, but the child exited before its runtime with `0xC0000135` (`STATUS_DLL_NOT_FOUND`). No missing DLL was named by the loader. The difference is associated with image/dependency loading from the user-owned path; no ACL was changed there. |
| B. `rustc.exe --version` from staged depot | **PASS** | Staged Rustc ran and returned `rustc 1.98.1`. This isolates the initial failure to loading the user-path toolchain/dependencies under the restricted identity. |
| C. Direct Rustc compile | **PASS** | A fixed host-owned, dependency-free `hello.rs` compiled directly to the candidate target using staged Rustc, staged `link.exe`, and staged MSVC/Windows SDK library paths. The output executable was hashed before probe cleanup. |
| D. `cargo.exe --version` | **PASS** | Staged Cargo ran and returned `cargo 1.98.1`. |
| E. Locked/offline tiny Cargo test, no external dependencies | **FAIL, diagnosed** | Cargo started, then reported it could not execute staged `rustc.exe -vV` (`never executed`), Windows error 5 (`Access denied`). The failure occurs before compilation and dependency resolution. |
| F. Locked/offline dependency fixture | **NOT RUN** | Step E failed; the ordered ladder stops here. |
| G. Actual Tier-1 verifier | **NOT RUN** | Step E failed; no production verifier path exists or was invoked. |

Direct Rust subprocess probes isolated the Step E cause further:

- `std::process::Command::status()` spawned the same staged Rustc successfully.
- `status()` with stdout and stderr pipes also spawned Rustc successfully.
- `status()` with `stdin(Stdio::null())` failed with `PermissionDenied`, raw OS error `5`.
- `Command::output()` failed with the same `PermissionDenied`, raw OS error `5`.
- Inside the same AppContainer, direct `CreateFileW("NUL", GENERIC_READ, ..., OPEN_EXISTING)` failed and `GetLastError()` returned `5`.

Cargo's `rustc -vV` probe reports `never executed`, matching failure while preparing the null standard-input handle before a Rustc process is created. The direct null-device check and the contrasting successful child-spawn modes identify the denied Windows resource: AppContainer access to the `NUL` device. The staged Rust toolchain and linker themselves execute successfully; Cargo's process-launch behavior is the remaining blocker.

## ACL cleanup evidence

The first diagnostic launcher attempted to restore the depot root DACL from a saved .NET `DirectorySecurity` object, but inspection showed that 19 temporary explicit package-SID read/execute ACEs had accumulated on the investigation-created depot root. No such ACEs were added to other paths. I removed those explicit package-SID ACEs from that exact depot root and verified zero AppContainer SID ACEs on:

- `C:\MAIA`
- the depot root, staged Rustc, and staged linker
- the original user `.rustup` and `.cargo`
- the canonical M0.16.4 checkout

Representative staged child ACLs also had zero remaining package-SID ACEs after parent cleanup. No processes remained from the probes, and the launcher deleted its temporary candidate roots and profiles. The attempted DACL restoration defect is retained here as evidence; the diagnostic launcher must not be reused without fixing its cleanup logic.

## Decision and stopping point

A host-owned restricted verifier depot solves Rustc image loading and direct compilation without altering the normal development toolchain or broad ACLs. It does **not** solve Cargo's null-stdin access: `NUL` is outside the permitted depot, and changing its device security would exceed the approved ACL scope. Patching/rebuilding Cargo or Rust standard library, changing Windows device ACLs, or using another isolation technology would be a new security/platform decision and was not attempted.

R1 stops at Step E. Steps F and G were not run. No Cargo offline setting was treated as a network boundary, and no same-user fallback was enabled. M0.16.5 remains OPEN / BLOCKED; Tier-1 remains fail-closed. Architecture Desk review is requested to decide whether an approved host-owned Cargo/toolchain compatibility mechanism exists without changing the isolation contract.
