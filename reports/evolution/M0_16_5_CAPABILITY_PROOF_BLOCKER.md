# M0.16.5 Capability Proof and Blocker

## Status

**BLOCKED at the executable capability proof; not ACCEPTED or CLOSED_READY.** Tier-1 remains unavailable under the M0.16.4 fail-closed Gate. No weaker execution fallback was enabled.

## Repository state

- Starting accepted M0.16.4 HEAD: `c3bbb1c1d227975c09fc4665433a7d1cfe6f758f`
- Branch: `codex/m0165-restricted-runtime`
- Implementation changes: none retained
- Main: untouched

## Decision

Windows AppContainer is available on the LAB host and was proven to provide a distinct restricted verifier identity, deny network access without network capabilities, and enforce filesystem access through explicit package-SID ACL grants. However, the host could not execute the required Cargo/Rust verifier toolchain under that identity without adding access across the user-owned toolchain and Cargo cache trees. A test using those grants still failed to execute `rustc.exe` with Windows `Access denied` after approximately 382 seconds.

That failure is a deployment-boundary blocker. This milestone stops here rather than making broad user-profile ACL changes, claiming an unproven Tier-1 profile, or falling back to same-user Job Object containment plus Cargo offline mode.

## Executable proof observations

The Windows host is Windows 10 Pro build 19045. An exploratory native launcher created a Windows AppContainer profile with no network capabilities, launched the probe suspended, verified `TokenIsAppContainer=1` and the expected package SID, assigned the child to the existing Job Object before resume, then queried the child’s actual behavior. The probe reported:

```text
created AppContainer token=1
AppContainer launch=True; exit=0
candidate_write=true
workspace_write=false
protected_read=false
network_connected=false
```

The network check used a host-controlled loopback TCP listener. The protected-read check targeted the canonical M0.16.4 checkout’s `README.md`. Filesystem grants were scoped to disposable probe directories and restored during cleanup. This proves the AppContainer primitive and a narrow candidate filesystem/network boundary; it does not prove the complete Cargo verifier can execute inside that boundary.

The follow-up integration attempt used a synthetic dependency-free Cargo workspace, fixed host-selected Cargo/Rust executables, offline mode, and the same restricted profile. Cargo failed before compilation:

```text
error: could not execute process `...\\rustc.exe -vV` (never executed)
Caused by: Access denied (os error 5)
```

Scoped read/execute grants to the toolchain and Cargo registry, including inherited access under their user-owned paths, did not resolve the failure. The attempt took approximately 382 seconds. No broad machine ACLs were changed; the test’s captured original DACLs were restored. No Tier-1 production path or fallback was changed.

## Capability matrix

| Capability | Evidence | M0.16.5 result |
|---|---|---|
| `PROCESS_TREE_CONTAINMENT` | M0.16.4 Windows Job Object implementation and accepted validation; the probe assigned the AppContainer child to that Job before resume | Existing capability retained; this does not itself restrict identity or filesystem/network access |
| `RESTRICTED_OS_IDENTITY` | Child token reported AppContainer identity and the expected package SID before resume | Primitive proven on LAB host |
| `FILESYSTEM_ISOLATION` | Explicit candidate-target write succeeded; workspace-root write, protected sentinel read, and canonical checkout read failed | Narrow proof passed; full toolchain-readable verifier layout not proven |
| `NETWORK_EGRESS_DENIAL` | Connection to host-controlled loopback listener failed from the AppContainer with no network capabilities | Probe passed |
| `DURABLE_GATE_ATTESTATION` | No durable Gate or approval attestation implementation was attempted | Not implemented / unavailable |
| Complete production Tier-1 profile | Cargo could not launch Rustc within the restricted profile | **Unavailable; Tier-1 must remain fail-closed** |

## Threat model and limits

- **Against the restricted candidate:** the observed AppContainer token and package ACLs blocked the tested protected paths and loopback networking. The probe does not establish a complete filesystem policy for all verifier dependencies.
- **Against another normal process under the same user:** no protection is claimed. No durable protected Gate store was created.
- **Against an administrator:** no protection is claimed.

The canonical development host, Supervisor Gate state, journal, and user profile were not made accessible to candidate code by this work. Candidate mutation authority, EVOLVABLE paths, approval authority, Git authority, promotion, deployment, and active-version authority were not expanded.

## Deferred work

No durable Gate persistence, approval attestation, capability matrix implementation, production AppContainer Cargo launch, or production Tier-1 admission was implemented. Those depend on an approved host deployment boundary that makes fixed verifier/toolchain dependencies safely readable to the restricted identity without broad or expensive ACL propagation. The Architecture Owner should decide whether to provide such a host-owned toolchain/runtime layout or select another supported isolation mechanism.

No spec was added because the milestone could not establish the required executable deployment contract. No code or tests are retained from the exploratory implementation attempt. The existing M0.16.4 Gate behavior remains authoritative and fail-closed.

## Architecture Desk disposition requested

Please review the proven AppContainer primitive and the Cargo/Rust access failure above, then decide the acceptable host-owned verifier/toolchain layout or alternate isolation mechanism. Until that boundary exists and the full restricted verifier profile is demonstrated, M0.16.5 must not be marked CLOSED_READY and Tier-1 must not execute.
