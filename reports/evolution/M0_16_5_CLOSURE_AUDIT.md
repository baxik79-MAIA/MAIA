# M0.16.5 Closure Audit — Production Restricted Tier-1

**Verdict: PENDING (audit in progress).**

Date: 2026-10-02 (Europe/Warsaw). Worktree `C:\MAIA\public-export\.local\m0160-supervisor`, branch
`codex/m0165-restricted-runtime`, audit start HEAD `3735f2594b0c80a94c02b4ee3d134a1cd32c82dc`.
External evidence base: `C:\MAIA\reports\evolution\m0165-host-preparation\` (abbreviated `EVID\`).
Audit additions are under `EVID\closure-audit-20261002\`. M0.16.6 was not started.

## 1. Production supervisor Tier-1 PASS (run 1940-18da9fe244da10d8)

Journal `EVID\production-evidence-20261002\production-supervisor-1790919589078478800.jsonl`, SHA-256
`ABE06E4DF0F3B03B92A328B61A6106FB5C4C3EB07E5E11C103634EAEEA21BE45`, 4,866 bytes.

- The hash chain was recomputed independently of `FileEvidenceJournal`, using SHA-256 over compact
  sorted-key JSON of each `body` (the serde_json `Value` encoding). Sequence 1..6 is contiguous, every
  `previous_sha256` links, and every `entry_sha256` matches. The failed release journal
  `production-supervisor-12032-failed.jsonl` (7 entries, `REJECTED`) also verifies and is kept as history.
- Terminal state: `REQUESTED → ALLOCATED → ACTIVE`, then mutation attempt `REQUESTED → APPLIED → TIER1PASSED`.
  `tier1.outcome=PASSED`, `verifier_identity=fixed-cargo-tier1-v1`. All six checks are PASS:
  candidate identity, syntax/static (Rustfmt), component build, formatting/lint (Clippy), targeted tests,
  and protected-surface integrity (`git-status:exact-single-allowlisted-worktree-change`). There is no
  terminal or promotion event, so the candidate stayed `ACTIVE`, as the contract requires.
  `content_recorded=false`; the replacement text is absent from the journal.
- Four restricted stages, from the diagnostics at
  `C:\MAIA\restricted-verifier-depot\run-state\diagnostics\1940-18da9fe244da10d8\`:

| Stage | Completion | cleanup_verified | Duration | Output |
|---|---|---|---|---|
| Rustfmt | Exited(0) | true | 153.9 s | none |
| Component check | Exited(0) | true | 187.7 s | stderr 3,121 B (10 crates checked) |
| Clippy (`-D warnings`) | Exited(0) | true | 181.5 s | stderr 1,915 B (clippy-driver ran over all 10 workspace crates) |
| Targeted tests | Exited(0) | true | 186.5 s | 25 passed (7+9+8+1), 0 failed |

  For every stage, the supervisor stderr records Job empty after root exit, `temporary ACL restoration:
  verified` and `AppContainer profile cleanup: verified`. No stream was truncated.
- Protected surfaces: the test asserts canonical `HEAD` and an empty `git status` after the run, and the
  journal check proves the only worktree change was the allowlisted file.

## 2. Clippy configuration — finding and fix

**Finding.** The PASS run's Clippy evidence hashes to `cargo-clippy:compat-clippy;config=absent-defaults`
(SHA-256 `d7f10285…`). The candidate had no `clippy.toml`, so the host snapshot directory
(`CLIPPY_CONF_DIR`) was empty. Clippy's config lookup walks from `CLIPPY_CONF_DIR` through every ancestor
(`run-state`, the depot, `C:\MAIA`, `C:\`). A config file placed in any of those host folders would
therefore have applied silently. The PASS run also never showed that a config is applied at all.

**Fix.** `ConfigSnapshot::create` now writes an empty host-owned `clippy.toml` when the candidate has none,
as it already did for Rustfmt. Clippy's lookup stops at the first directory that holds a config.

**Verification.**
- Unit test `tier1::tests::config_snapshot_always_holds_host_owned_clippy_and_rustfmt_files`: absent →
  empty defaults; one candidate file → byte-exact copy plus SHA evidence; two candidate files → fail closed.
- Ignored production test `tier1::tests::production_clippy_applies_only_the_host_config_snapshot` ran
  through the real `run()` path (pinned compat Clippy, AppContainer, Job, temporary ACLs):
  - With candidate `clippy.toml` `too-many-arguments-threshold = 2`, Clippy reported
    `this function has too many arguments (3/2)`, `-D clippy::too-many-arguments`, and exit 101
    (`Failed(101)`). Evidence was `sha256:8955228B…`.
  - With no candidate config and the same strict config planted at `run-state\clippy.toml` (the parent of
    `CLIPPY_CONF_DIR`), Clippy passed with `absent-defaults`. The planted file was removed afterwards.
  - Both runs had ACL restoration and profile cleanup verified. Log: `EVID\closure-audit-20261002\production-clippy-config-20261002.txt`
    (SHA-256 `8A83ACBE…F730`); diagnostics copied to `EVID\closure-audit-20261002\clippy-diagnostics\`.

## 3. Production E4 — completeness

Before this audit, production E4 (`production_appcontainer_e4_isolation`, 94.34 s) covered target write,
workspace write, protected and canonical reads, vendor read/write, NUL read and loopback. Three gaps:
1. no protected or canonical **write** probe, no new-file creation in the workspace, and no non-loopback network probe;
2. the timeout, cancellation and descendant tests ran only through the Job-only runner (`run_process`,
   no AppContainer), so cleanup after failure, timeout or cancellation was never shown in the restricted runtime;
3. ACL restoration was checked only by the runner itself, never independently.

Both probes below use the real `run_restricted_process`: a unique AppContainer profile, the null-stdin
capability verified on the token, temporary ACLs, and a Job whose processes are assigned before resume.

- **Extended isolation** (`production_appcontainer_e4_isolation`, 101.39 s, PASS):
  `target_write=true workspace_write=false workspace_create=false protected_read=false protected_write=false
  canonical_read=false canonical_write=false vendor_read=true vendor_write=false null_read=true
  network_connected=false loopback=TimedOut external=PermissionDenied/10013`. Loopback timed out against a
  live host listener. The TEST-NET-1 (192.0.2.1:443) connect failed immediately with WSAEACCES, which is
  the AppContainer network denial and not a routing timeout. Log: `production-e4-isolation-extended-20261002.txt`
  (SHA-256 `1133834B…EBB8`).
- **Lifecycle** (`production_appcontainer_e4_lifecycle_cleanup`, 21.29 s, PASS):

| Case | Completion | Descendant ended | Exact DACLs (tree + exe dir/file) | Residual package SID | Profile mapping |
|---|---|---|---|---|---|
| failure (exit 3) | Exited(3) | n/a | yes | 0 | 0 |
| timeout (20 s, live grandchild) | TimedOut | yes | yes | 0 | 0 |
| cancellation after grandchild start | Cancelled | yes | yes | 0 | 0 |
| root exits, grandchild alive | Exited(0) | yes | yes | 0 | 0 |

  Log: `production-e4-lifecycle-20261002.txt` (SHA-256 `34BAF656…25A4`).

The cross-process slot, kill-on-close, active-process limit and resource-limit notification paths are
unchanged and remain covered by the existing unit tests. Production E4 is now complete for the M0.16.5
criteria: network denial, protected read/write denial, target-only writes, descendant containment, and
cleanup after success, failure, timeout and cancellation.

## 4. Host state — orphan per-run ACEs and profiles

**Final run.** PID 1940 and its descendants are gone. No `maia-tier1-1940-*` mapping exists, and the
run's package SID appears nowhere in the scanned roots. Its fixture root and run-state snapshot directory
are gone. The four stage diagnostics are kept.

**Finding — stale grants from earlier runs.** A full `icacls /t` scan found 61 explicit ACE paths for 11
stale per-run package SIDs. None belongs to the final run. Most are inheritable `RX`/`M` grants on the
depot `toolchains`, `vendor`, `msvc`, `windows-sdk` and `compat` folders, plus the per-user `clippy-r5` and
`rustfmt-r5` resources and four synthetic Temp fixtures. There is also one traverse ACE on `C:\MAIA` and
on the depot root, and two inheritable `Modify` grants on `run-state`. All of them came from interrupted
or older runs:

| SID (prefix) | Origin | Explicit paths | Action |
|---|---|---|---|
| S-1-15-2-4092322014 | profile `maia-tier1-12176-…-4`, owner absent | 14 | ACEs removed, profile deleted |
| S-1-15-2-2373618257 | profile `maia-tier1-6876-…-6`, owner absent | 10 | ACEs removed, profile deleted |
| S-1-15-2-3098141911 | profile `MAIA.M0165.Probe.ae0ef8af…` (R5V scratch launcher) | 2 | ACEs removed, profile deleted |
| S-1-15-2-3075060834 | PID 1588 (profile deleted 2026-10-01) | 8 | ACEs removed |
| S-1-15-2-1002002296, -3046712473, -3544704036, -25362997, -570903151 | unmapped (R5V/compat-era runs) | 5 each | ACEs removed |
| S-1-15-2-1826953743, -1745619640 | unmapped; inheritable Modify on `run-state` | 1 each | ACEs removed |

The PID 1588 recovery on 2026-10-01 had scanned only `run-state`. It deleted that profile while its grants
remained on the toolchain, vendor, MSVC, SDK, Clippy/Rustfmt resources and a Temp fixture. The earlier
statement that the two remaining mappings were "not linked" was correct, but the handoff scan of
`maia-tier1-*` names missed the third MAIA mapping and the unmapped SIDs. Earlier depot scans run from Git
Bash were also unreliable: MSYS path conversion rewrites `icacls … /t` into a file path. Scans must run
from PowerShell or with `MSYS_NO_PATHCONV=1`.

**Cleanup.** `Recover-OrphanTier1Profile.ps1` gained two modes. `-DeletedProfileSid` removes ACEs only, for
a SID whose mapping is already gone, and it refuses a mapped SID. It also now accepts the R5V
`MAIA.M0165.Probe.<guid>` profile name. The first apply pass stopped safely on a nested explicit path:
removing the parent's ACE had rewritten the child's inherited entries, which the full-SDDL drift check
flagged. The drift check now compares explicit (non-inherited) rules only, and the pass was re-run. Each SID
was scanned across every depot entry except `prep` (an archive with two over-long paths that no run ever
granted), plus the per-user `MAIA` folder, its Temp fixture, `C:\MAIA`, the depot root and the depot
top-level files. Each removal kept every unrelated rule, and every rescan returned zero for that SID. Three
profile deletions returned `HRESULT 0` with no mapping left. In total, 61 explicit ACE paths were removed
(13 in the stopped pass, 48 in the re-run), which equals the inspection total. Records:
`EVID\closure-audit-20261002\orphan-acl-cleanup\` (inspect/before/after JSON), `orphan-acl-inspect.txt`,
`orphan-acl-apply.txt` (stopped pass), `orphan-acl-apply-2.txt`, `orphan-cleanup-wrapper.ps1`.

**Independent rescan** (plain `icacls /t /c` from a separate command, 2026-10-02T08:29Z):
`C:\MAIA`, `%LOCALAPPDATA%\MAIA` and `%LOCALAPPDATA%\Temp` contain 0 explicit or inherited
per-profile package-SID ACE lines, and HKCU AppContainer mappings contain 0 MAIA monikers
(`post-cleanup-rescan.txt`, SHA-256 `E8EBEDE120BA186F4CC789277F17D434D319500D8CEDDF6A7138AD13EE510355`).

**Not removed.** The synthetic Temp fixture folders (under 350 KB each) and old run-state snapshot folders
(`12176-…`, `6876-…`, `7684-…`, `1588-…`) are historical test state. They are no longer reachable by any
AppContainer SID and were left in place. No vendor, toolchain, model, repository or evidence data was changed.

## 5. Persistent Null capability

`Prepare-M0165NullStdinCapability.ps1 -ValidateOnly` (2026-10-02, during the audit): capability SID
`S-1-15-3-1024-1988635889-3063999656-362765618-2578588777-2081098023-3982749461-1818700090-2051019908`,
`ace_count_for_sid=1`, `exact_read_only_noninherited_ace=True`, DACL
`D:AI(A;;FR;;;<sid>)(A;;0x1201bf;;;WD)(A;;FA;;;SY)(A;;FA;;;BA)(A;;0x1200a9;;;RC)`. This is byte-identical
to the earlier validation (`null-capability-validate-closure.txt`, SHA-256 `0A924C5E…708A`). It is a
deliberate persistent host resource and is not orphan state.

## 6. Contract alignment

The canonical `spec/evolution_protected_runtime.yaml` still described the pre-M0.16.5 runtime. It was
updated first, followed by its guard (`tools/evolution_protected_runtime_contract.py`) and the generated
appendix:
- `inherited_handles`: `explicit_null_standard_handles_only` →
  `explicit_handle_list_null_stdin_and_host_owned_output_pipes_only`. A new key `output_capture`
  records bounded, host-drained output persisted outside the candidate ACL.
- `achieved_identity_claim` → `per_run_appcontainer_and_job_only_after_host_readiness_attestation;
  tier1_unavailable_otherwise`. The `stronger_isolation_capabilities` claim is now available only after the
  host readiness probe and production E4 verification, and fails closed otherwise.
- Unchanged: startup defaults (fail closed), network without capabilities, the one-entry resource
  capability allowlist, resource limits and journal semantics. ADR-0048 records the qualification outcome.
  This is a contract change and needs Architecture Owner review before merge.

## 7. Verification run during the audit

VERIFICATION_PENDING

## 8. Remaining constraints (not blockers)

- **Path budget.** MSVC object paths must stay under 260 characters. The production pass used the short
  candidate root `C:\MAIA\scratch\m0165p-<nonce>\evolution-candidates\<workspace>`. A deeper root failed
  with `LNK1181` (release run 12032, kept as history). Deployment must use a candidate root of comparable
  depth. No general support for long roots is claimed.
- **Compat Clippy.** The Clippy bundle is the pinned proof-only compatibility build. Its junction-alias
  config-discovery difference (R5V review) stays outside the fixed host-owned snapshot path, which is now
  always populated.
- **LAB host only.** The qualification applies to this prepared LAB host (resources, vendor of 388 packages,
  and the admin-installed Null capability). Another host must repeat the readiness probe, E4 and the
  supervisor run.
- **Per-stage overhead.** Each stage spends about 150 s granting and restoring ACLs over the read-only
  resource trees; that overhead fits the 900 s per-command bound.
