# M0.16.5 handoff to Claude Code (2026-10-02, Europe/Warsaw)

## Checkout and stop point

- Worktree: `C:\MAIA\public-export\.local\m0160-supervisor`
- Branch: `codex/m0165-restricted-runtime`
- HEAD before this handoff file: `3735f2594b0c80a94c02b4ee3d134a1cd32c82dc`.
- `git status --short` was empty immediately before writing this file. This handoff file is the only intended uncommitted change (`?? M0165_HANDOFF.md`); verify that state on receipt. No work was reverted to obtain a clean status.
- Do not start M0.16.6. No new long test is required for the handoff.

## Verified work and production decision

- `c552142`: added `ops/evolution-workspace-host/tools/m0165-r5v/Recover-OrphanTier1Profile.ps1`. It inspects an exact profile/package SID and specified roots, removes only explicit ACEs for that SID, checks unrelated ACL rules and residual SID access, then deletes the profile. A synthetic orphan recovery passed.
- `567e179`: added actual production E4 isolation test, narrowed an unneeded ancestor ACL grant, documented crash recovery, and regenerated `generated/spec_appendix.md` from the canonical generator. E4 passed in 94.34 s: candidate target write allowed; workspace write, protected/canonical read, vendor write, and loopback network denied; vendor read and Null read allowed. Job drain, exact ACL restoration and profile deletion passed.
- `530fe0c`: production supervisor fixture copies its durable journal outside Temp before normal fixture cleanup.
- `3735f25`: shortened the production fixture root so MSVC linker object paths remain below 260 characters, and moved the PASS assertion after journal preservation. The prior deep Temp fixture produced `LNK1181` on `.obj` (one object path was 274 characters); its genuine failed decision and diagnostics were retained. The short-path production run passed all four restricted stages and its integration test passed in 717.89 s.
- Final production supervisor journal: `C:\MAIA\reports\evolution\m0165-host-preparation\production-evidence-20261002\production-supervisor-1790919589078478800.jsonl`, SHA-256 `ABE06E4DF0F3B03B92A328B61A6106FB5C4C3EB07E5E11C103634EAEEA21BE45`. Six hash-chain-readable events end with a real `TIER1PASSED` mutation event and all six checks PASS. Candidate remains active, without promotion; the canonical repository HEAD and status were unchanged by the test. No synthetic terminal event was appended.
- The final production Rustfmt, component check, Clippy, and targeted tests each exited 0 with `cleanup_verified=true`; the Job was empty and the temporary ACL/profile cleanup was logged as verified after each stage. The test fixture root was removed.
- `python tools/validate_spec.py`, `python tools/spec_guard.py`, `cargo fmt --all -- --check`, and `git diff --check` passed after the generated appendix update. Component check passed for `maia-evolution-process-host`, `maia-evolution-workspace-host`, and `maia-local-intelligence-host` with all targets/features, locked/offline. Effective product Clippy passed with `CLIPPY_CONF_DIR` pointing at the recorded empty external config directory and `-D warnings`.
- Release targeted tests passed: process host 12/12, workspace host library 7/7, allocation integration 21/21; ignored host qualification tests were run separately. The production E4 and final supervisor tests also passed separately. These need no repeat merely to prepare the handoff.

## Recovery and host cleanup

- The original interrupted run (PID 7684) had a five-entry journal ending `APPLIED`; preserved outside Temp at `C:\MAIA\reports\evolution\m0165-host-preparation\recovery-20261001\interrupted-evolution.jsonl`, SHA-256 `1ADBD7144D4473D3F974497B40284AC7C38207AE760EF8FD0B57BFA1EDBA2D67`. PID 7684 and Cargo PID 5788 were absent. Its profile `maia-tier1-7684-18da826c993781e4-4` mapped to package SID `S-1-15-2-1943362053-194520890-855057922-2981893542-1459643958-2579069714-1255573247`. Ten explicit ACEs for that SID were removed from exact known paths, unrelated ACEs were unchanged, SID rescans over the relevant roots returned zero, and `DeleteAppContainerProfile` returned success. Do not alter that original journal.
- A later stopped debug test left a separate PID 1588 profile and one explicit run-state ACE. It was recovered with the exact-SID tool; before/after evidence is under `...\recovery-20261002\`. Two debug supervisor attempts stopped at `APPLIED` before profile creation; their journals were preserved under that same recovery directory. The deep-path failed release test's seven-entry `TIER1FAILED`/`REJECTED` journal and four diagnostics are in `...\production-evidence-20261002\` and must remain historical evidence.
- The final successful run used PID 1940 / run ID `1940-18da9fe244da10d8`. At handoff, the test process and descendants were absent; its run-state directory and fixture root `C:\MAIA\scratch\m0165p-1790919589078478800` were absent. No registry mapping matching `maia-tier1-1940-*` remained. Its four stage logs each state Job empty, temporary ACL restoration verified, and AppContainer profile cleanup verified. No further cleanup of this run is indicated.
- Registry enumeration still showed two **other** old profile mappings, `maia-tier1-6876-18da33c353bd9664-6` and `maia-tier1-12176-18da7b59c09a0974-4`. They were not linked to the recovered PID 7684 or final PID 1940 and were not touched. Do not remove them without a separate exact-SID/grant audit.
- Persistent capability `maia.evolution.tier1.null.stdin` is separate from temporary package grants. `Prepare-M0165NullStdinCapability.ps1 -ValidateOnly` exited 0 on 2026-10-02: SID `S-1-15-3-1024-1988635889-3063999656-362765618-2578588777-2081098023-3982749461-1818700090-2051019908` has exactly one noninherited read-only ACE on `\Device\Null`. Validation output is `...\production-evidence-20261002\null-capability-validate-20261002.txt`. Do not remove it as orphan cleanup.
- About 2.397 GB was reclaimed only from unused old build `target` directories; vendor and cache were preserved. C: had 16,995,794,944 bytes free at the final measurement (~15.83 GiB); measure again before reporting current free space.

## Evidence, commands, and next step

External evidence base: `C:\MAIA\reports\evolution\m0165-host-preparation`.

- Final production supervisor: `production-supervisor-tier1-shortpath-20261002.stdout.txt`, `.stderr.txt`; journal and its four `*.meta.txt`, stdout/stderr captures in `production-evidence-20261002\` and `C:\MAIA\restricted-verifier-depot\run-state\diagnostics\1940-18da9fe244da10d8\`.
- Prior failed release attempt: `production-supervisor-tier1-release-20261002.stdout.txt`, `.stderr.txt`; `production-evidence-20261002\production-supervisor-12032-failed.jsonl` and `diagnostics-12032\`.
- Other gates: `component-check-20261001.txt`, `clippy-product-effective-20261001.txt`, `clippy-effective-20261001.config.txt`, `targeted-release-tests-20261002.txt`, `production-e4-20261002.txt`; recovery snapshots and `space-reclamation.txt` under `recovery-20261001\` and `recovery-20261002\`.
- Actual final supervisor invocation (already passed; do not rerun for handoff):

```powershell
$env:MAIA_M0165_EVIDENCE_DIR='C:\MAIA\reports\evolution\m0165-host-preparation\production-evidence-20261002'
cargo test --release -p maia-evolution-workspace-host --test allocation production_supervisor_tier1_runs_all_stages_in_appcontainer_and_journals_decision --locked --offline -- --ignored --exact --nocapture
```

- Host capability validation: `& .\ops\evolution-workspace-host\tools\m0165-r5v\Prepare-M0165NullStdinCapability.ps1 -ValidateOnly`.
- Quick handoff audit: `git status --short; git rev-parse HEAD; Get-PSDrive C`; inspect the preserved final journal and stage metadata. The integration test called `FileEvidenceJournal::read_events`, which validated the final journal chain. Verify the PID 1940 process tree and mapping remain absent if host state may have changed.

No known functional blocker remains for the measured short-path production fixture. The next concrete step is a read-only acceptance audit of the final journal, E4 log, stage metadata, host cleanup and canonical criteria, followed by a concise M0.16.5 qualification report/evidence index and a commit of coherent verified documentation. Decide `CLOSED_READY` only after that audit; do not infer general support for arbitrary long candidate paths from the short-path pass. If the product permits deeper candidate roots, treat that path-budget issue as an explicit remaining deployment constraint or implement a separately validated bound. Do not start M0.16.6.

## Closure audit checkpoint 1 (2026-10-02, Claude Code session)

- Start state verified: HEAD `3735f25`, only `?? M0165_HANDOFF.md`; final journal SHA-256 unchanged; PID 1940 absent; no `maia-tier1-1940-*` mapping.
- Final journal hash chain independently recomputed (Python, sorted compact JSON = serde_json `Value`): 6/6 entries valid, terminal event `TIER1PASSED`, all six checks PASS. Failed release journal 12032: 7/7 valid.
- Gap found: production Clippy evidence was `config=absent-defaults`; config application was unproven and, with no candidate config, the compat Clippy search would continue to host ancestors of `CLIPPY_CONF_DIR`. Fix (uncommitted at this checkpoint): `ConfigSnapshot` always writes an empty host-owned `clippy.toml`. Real-runtime proof: `tier1::tests::production_clippy_applies_only_the_host_config_snapshot` PASS (candidate threshold 2 -> exit 101 `too_many_arguments`; absent candidate + planted run-state ancestor config -> PASS).
- E4 gaps found: lifecycle cleanup/descendant tests used Job-only runner; protected/canonical writes and non-loopback network were unprobed. Added `production_appcontainer_e4_lifecycle_cleanup` (failure/timeout/cancel/root-exit-with-descendant through `run_restricted_process`) and extended E4 probes. Both PASS. Evidence: `C:\MAIA\reports\evolution\m0165-host-preparation\closure-audit-20261002\`.
- Host finding: explicit ACEs for four old per-run package SIDs remain on synthetic Temp fixture dirs (`maia-m0163-{12176,1588,6876}-*`, `maia-m0165-proof-*`). Includes PID 1588, whose earlier recovery did not scan Temp. Exact-SID cleanup in progress; `Recover-OrphanTier1Profile.ps1` gained an ACE-only `-DeletedProfileSid` mode.

## Closure audit checkpoint 2 (2026-10-02 ~09:40 Europe/Warsaw)

- Committed: `03fc1fd` (Clippy snapshot fix + tests), `b198fe1` (E4 extended + lifecycle probes), `6dcf043` (spec/guard/appendix/ADR-0048 alignment: handle list with output pipes; identity claim only after readiness attestation).
- Full `C:\MAIA` + per-user + Temp scan (note: Git Bash rewrites `/t`; use `MSYS_NO_PATHCONV=1` or PowerShell) found 61 explicit ACE paths for 11 stale per-run package SIDs on depot toolchain/vendor/msvc/sdk/compat/run-state, `C:\MAIA`, per-user clippy-r5/rustfmt-r5 and Temp fixtures. None belongs to the final run 1940. Includes the 1588 SID whose earlier recovery scanned only run-state, two unmapped SIDs with inheritable Modify on `run-state`, and a third MAIA mapping `MAIA.M0165.Probe.ae0ef8af...` (R5V scratch launcher).
- Recovery tool extended (uncommitted until cleanup verifies): `-DeletedProfileSid` ACE-only mode and R5V probe profile names. Inspect records: `EVID\closure-audit-20261002\orphan-acl-cleanup\*-before.json`; apply log `orphan-acl-apply.txt`; wrapper `orphan-cleanup-wrapper.ps1`. Apply running in background at this checkpoint.
- Draft report: `reports/evolution/M0_16_5_CLOSURE_AUDIT.md` (sections 4 and 7 pending). Remaining: finish cleanup, rescan, commit tool, rerun production supervisor on new HEAD (code under test changed), finalize report and verdict.
