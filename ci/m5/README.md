# Exact internal RC hosted controls

This input is the unchanged Windows-built unsigned development/staging RC1:
244307 bytes, 52 members, SHA256
`572fa19a023a2cf4ab7c4005ee63916b4ffb816332f97dc1e4be9c98157a9244`.
It is a reviewed acceptance-binding fixture for this finite CI job. It is not
an installer, deployment package, operational database or raw session archive.

`INPUT_MANIFEST.json` binds the exact ZIP, unchanged trusted `m5_rc.py`, the
minimal accepted M4 publication receipt and the CI adapter. The same M4 receipt
is already inside the immutable ZIP; the separately pinned bytes let the
unchanged helper verify that exact archive/member/receipt join before staging.
The receipt contains only project path identifiers, digests, acceptance/scope
flags and a publication timestamp. It contains no owner secrets, private raw
prompts, customer data, machine state contents or runtime logs. No other local
operational report is committed as an input.

The unchanged helper's `validate()` has fixed LABtop Windows runtime pins.
Those pins remain untouched. The explicit `hosted_rc.py` adapter instead uses
the helper's unchanged archive verification, staging, suite-summary parser and
owned-return functions, while invoking only the four already-reviewed pure
suites. It records the hosted Python/Node file SHA before/after controls. On
Windows it also performs AST5 and the existing memory-only wire19 controls and
records PowerShell file SHA. Both finite PowerShell scripts suppress only
progress display through `$ProgressPreference='SilentlyContinue'`. They retain
`ErrorActionPreference='Stop'`, strict empty-stderr/count checks, and the same
script policy; the wire wrapper propagates script failures and nonzero
`LASTEXITCODE`. Returned child exit code and raw-log hashes are retained before
checking the verdict, including a failing verdict. No CLIXML/error stream is
ignored or filtered. These are hosted byte inventories, not the
LABtop's Capsule/RuntimePins, policy check, executed-image, native process-tree
or durability qualification. OS-owned hardlinks are inventoried only for the
PowerShell runtime; all fixture/source inputs require one ordinary link.

The pure suites must report exactly23 M5,64 M3,98 M4 and1 controlled-DOM test,
zero skips and zero process errors. Native M4 effect matrices, installed
Supervisor, Store claims, production SQLite/schema migration, deployment,
Apply and release signing are not invoked. All four global flags and full M5
acceptance remain false even when both hosted jobs pass. A coordinator must
join actual job/run/source-commit URLs, input digest, raw logs and artifact
receipts before recording the hosted CI gate.

Source/raw test stdout and stderr from this job are synthetic portable-control
evidence. Job evidence contains their exact bytes and hashes plus fresh
copy/return receipts. No private model conversation, historical raw runtime log
or 70MB master archive is uploaded. Input fixture bytes are protected against
Git newline conversion by the scoped `.gitattributes`.

All four third-party actions are pinned to official full commit SHA, checked
against their official release links on2026-11-09. CPython3.13.15 and Node24.19.0
versions are fixed; GitHub-hosted image labels are Ubuntu24.04/Windows2022.
Actual runner image versions and installed runtime SHA are reported. This
does not claim an immutable full host image or a release signature.

The workflow is confined to `codex/m5-rc-ci-20261109` path changes or explicit
dispatch, has read-only contents permission, stores no checkout credentials,
uses no dependency/cache install and has a10-minute job limit. Public-repository
standard hosted runner usage is covered by GitHub's published free usage;
repository visibility and policy still have to be confirmed by the coordinator.

## Local checks of this adapter

Input-only check (no test execution or staging):

```sh
python -I -S -B ci/m5/hosted_rc.py --manifest-sha256 d8de229a1074c4907f4b79c2f467e3ca49bb5c20fc4b9b232874f3ef66d42928 --preflight-only
```

One Linux-only local validation of the new adapter uses an explicitly fresh
root, never GitHub environment impersonation:

```sh
python -I -S -B ci/m5/hosted_rc.py --manifest-sha256 d8de229a1074c4907f4b79c2f467e3ca49bb5c20fc4b9b232874f3ef66d42928 --local-validation --local-root /absolute/fresh/adapter-check
```

That mode reports `hosted_ci_job_observed:false`, requires no GitHub environment,
accepts local CPython3.12+ and records the actual local Node version. It does
not replace either required hosted job. A local Windows exception is not
provided. A partial/terminal namespace is preserved and never automatically
retried to improve its result.
