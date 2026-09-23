#!/usr/bin/env python3
"""Negative controls for tools/verify_round_table_absent.py (M0.15.3).

A proof that cannot fail proves nothing. These show the absence check does fail
when Core secretly needs the Round Table, and when a Round Table-looking
workspace member is not accounted for. Both fail before any full build, so this
is quick.

Run: python tools/tests/test_round_table_absent.py
"""

import contextlib
import io
import pathlib
import shutil
import sys
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import verify_round_table_absent as v  # noqa: E402

FAILURES = []


def check(name, ok, detail=""):
    print(("PASS " if ok else "FAIL ") + name + (f" - {detail}" if detail and not ok else ""))
    if not ok:
        FAILURES.append(name)


def run_with_copy_hook(hook):
    """Run the proof, applying `hook(copy_dir)` after the workspace is copied."""
    real = v.copy_workspace
    work = pathlib.Path(tempfile.mkdtemp(prefix="maia-absent-neg-"))

    def patched(dst):
        real(dst)
        hook(pathlib.Path(dst))

    v.copy_workspace = patched
    import os
    os.environ["MAIA_ABSENT_WORKDIR"] = str(work)
    out = io.StringIO()
    try:
        with contextlib.redirect_stdout(out):
            try:
                code = v.main()
            except SystemExit as e:
                code = e.code
    finally:
        v.copy_workspace = real
        os.environ.pop("MAIA_ABSENT_WORKDIR", None)
        shutil.rmtree(work, ignore_errors=True)
    return code, out.getvalue()


def inject_core_dependency(copy):
    manifest = copy / "core" / "briefing" / "Cargo.toml"
    text = manifest.read_text(encoding="utf-8")
    manifest.write_text(
        text.replace("[dependencies]", '[dependencies]\nmaia-roundtable = { path = "../roundtable" }', 1),
        encoding="utf-8",
    )


def main():
    code, out = run_with_copy_hook(inject_core_dependency)
    check("a Core crate needing the Round Table fails the absence proof", code == 1, f"exit={code}\n{out}")
    check("...and is not reported as proven", "PROVEN" not in out)
    
    original = list(v.ROUND_TABLE_MEMBERS)
    v.ROUND_TABLE_MEMBERS.remove("infra/roundtable-store")
    try:
        code, out = run_with_copy_hook(lambda _copy: None)
    finally:
        v.ROUND_TABLE_MEMBERS[:] = original
    check("an unaccounted Round Table member fails rather than being kept", code == 1, f"exit={code}\n{out}")
    check("...naming the member", "infra/roundtable-store" in out)
    
    print()
    print("FAILED: " + ", ".join(FAILURES) if FAILURES else "all negative controls passed")
    return 1 if FAILURES else 0


if __name__ == "__main__":
    sys.exit(main())
