#!/usr/bin/env python3
"""M0.15.3 - prove MAIA builds and operates with the Round Table physically absent.

`roundtable/tests/core_independence.rs` (D009) reads manifests. This does
the stronger thing: it copies the workspace, deletes every Round Table crate
from the copy, and builds and tests what is left with cargo's own resolver. If
Core or a shipped app secretly needed the Round Table, this fails to compile.

It also checks that doing so brings no Round Table storage into existence.

Nothing in the real worktree is modified. The copy and its target directory live
under the system temp directory (override with MAIA_ABSENT_WORKDIR).

Usage: python tools/verify_round_table_absent.py [--keep]
Exit 0 = proven, 1 = a check failed, 2 = could not run.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# Round Table implementation crates, by workspace member path. Must stay in step
# with ROUND_TABLE_CRATES in roundtable/tests/core_independence.rs; the
# check below fails if a member that looks like Round Table is not listed here.
ROUND_TABLE_MEMBERS = [
    "roundtable",
    "infra/anthropic",
    "infra/claude-code",
    "infra/local-model-roundtable",
    "infra/roundtable-store",
    "composition/roundtable-composition",
    "composition/roundtable-observability",
    "apps/roundtable-viewer",
    "ops/roundtable-invoke",
]
ROUND_TABLE_PACKAGES = {
    "maia-roundtable",
    "maia-anthropic",
    "maia-claude-code",
    "maia-local-model-roundtable",
    "maia-roundtable-store",
    "maia-roundtable-composition",
    "maia-roundtable-observability",
    "maia-roundtable-viewer",
    "maia-roundtable-invoke",
}
# Directory names that must not appear in the copy after building and testing.
STORAGE_MARKERS = ("roundtable", "round_table", "round-table")
SKIP_COPY = {"target", ".git", "node_modules", "__pycache__"}


def fail(msg: str) -> None:
    print(f"FAIL: {msg}")
    sys.exit(1)


def members_of(manifest: Path) -> list[str]:
    text = manifest.read_text(encoding="utf-8")
    start = text.index("[", text.index("members"))
    end = text.index("]", start)
    return [m.strip().strip('"') for m in text[start + 1 : end].split(",") if m.strip()]


def snapshot(root: Path) -> set[str]:
    out: set[str] = set()
    for p in root.rglob("*"):
        rel = p.relative_to(root)
        if rel.parts and rel.parts[0] == "target":
            continue
        out.add(rel.as_posix())
    return out


def copy_workspace(dst: Path) -> None:
    shutil.copytree(
        REPO, dst,
        ignore=lambda _d, names: [n for n in names if n in SKIP_COPY],
    )


def cargo(args: list[str], cwd: Path, target: Path) -> subprocess.CompletedProcess[str]:
    env = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_TERM_COLOR="never")
    return subprocess.run(
        ["cargo", *args, "--offline"],
        cwd=cwd, env=env, text=True, capture_output=True,
    )


def main() -> int:
    keep = "--keep" in sys.argv
    members = members_of(REPO / "Cargo.toml")

    for m in members:
        if "roundtable" in m and m not in ROUND_TABLE_MEMBERS:
            fail(f"workspace member `{m}` looks like Round Table but is not in "
                 "ROUND_TABLE_MEMBERS; the absence proof would silently keep it")
    missing = [m for m in ROUND_TABLE_MEMBERS if m not in members]
    if missing:
        fail(f"ROUND_TABLE_MEMBERS names members the workspace does not have: {missing}")

    remaining = [m for m in members if m not in ROUND_TABLE_MEMBERS]

    base = Path(os.environ.get("MAIA_ABSENT_WORKDIR") or tempfile.mkdtemp(prefix="maia-absent-"))
    copy = base / "workspace"
    target = base / "target"
    if copy.exists():
        shutil.rmtree(copy)
    try:
        copy_workspace(copy)
        for m in ROUND_TABLE_MEMBERS:
            shutil.rmtree(copy / m)
        # `composition/` may now be empty; remove empty parents so nothing hints at it.
        comp = copy / "composition"
        if comp.exists() and not any(comp.iterdir()):
            comp.rmdir()
        (copy / "Cargo.toml").write_text(
            "[workspace]\nmembers = ["
            + ", ".join(f'"{m}"' for m in remaining)
            + '] \nresolver = "3"\n',
            encoding="utf-8",
        )
        print(f"workspace copy: {copy}")
        print(f"members kept  : {len(remaining)}; Round Table removed: {len(ROUND_TABLE_MEMBERS)}")

        for m in ROUND_TABLE_MEMBERS:
            if (copy / m).exists():
                fail(f"`{m}` still present in the copy")

        # 1. cargo's own resolved graph, not our parsing of it.
        meta = cargo(["metadata", "--format-version", "1"], copy, target)
        if meta.returncode != 0:
            print(meta.stderr)
            fail("cargo metadata failed on the Round-Table-absent workspace")
        packages = {p["name"] for p in json.loads(meta.stdout)["packages"]}
        leaked = packages & ROUND_TABLE_PACKAGES
        if leaked:
            fail(f"Round Table packages present in the resolved graph: {sorted(leaked)}")
        print(f"resolved graph: {len(packages)} packages, none Round Table")

        before = snapshot(copy)

        # 2. It builds. 3. Its own tests pass. Both are what "operates" means here.
        for label, args in (
            ("build", ["build", "--workspace", "--all-targets"]),
            ("test", ["test", "--workspace"]),
        ):
            r = cargo(args, copy, target)
            if r.returncode != 0:
                print(r.stdout[-4000:])
                print(r.stderr[-4000:])
                fail(f"cargo {label} failed with the Round Table absent")
            print(f"cargo {label}: ok")

        # 4. Nothing Round Table-shaped appeared. Cargo may rewrite Cargo.lock.
        created = sorted(snapshot(copy) - before - {"Cargo.lock"})
        stray = [p for p in created if any(k in p.lower() for k in STORAGE_MARKERS)]
        if stray:
            fail(f"Round Table storage appeared without being asked for: {stray}")
        print(f"post-run tree: {len(created)} new paths, none Round Table storage")

        print("PROVEN: MAIA Core and shipped apps build, test and run with the "
              "Round Table absent, and create no Round Table storage.")
        return 0
    except FileNotFoundError as e:
        print(f"could not run: {e}")
        return 2
    finally:
        if not keep and not os.environ.get("MAIA_ABSENT_WORKDIR"):
            shutil.rmtree(base, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
