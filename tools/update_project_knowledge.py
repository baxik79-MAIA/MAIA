#!/usr/bin/env python3
"""Materialize and verify the portable MAIA project knowledge set.

The repository handbook is the editable source. This tool reads live Git and
spec/product.yaml facts, renders live current-state templates into ignored snapshots, and can
sync the handbook plus references to the shared mirror and Library export.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import zipfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
PROJECT_ROOT = REPO_ROOT / "docs" / "project"
SHARED_ROOT = REPO_ROOT.parent / (REPO_ROOT.name + "-knowledge")
EXPORT_ROOT = REPO_ROOT.parent / (REPO_ROOT.name + "-library")
EXPORT_ZIP = EXPORT_ROOT.parent / "MAIA_Project_Knowledge.zip"
DOCUMENTATION_ROOT = REPO_ROOT / ".local/exports/documentation"
HANDBOOK_FILES = [
    "00_START_HERE.md",
    "01_CURRENT_STATE.md",
    "02_ARCHITECTURE.md",
    "03_ARCHITECTURE_INVARIANTS.md",
    "04_MILESTONE_LEDGER.md",
    "05_CURRENT_ROADMAP.md",
    "06_AI_WORKFLOW.md",
    "07_PARALLEL_DEVELOPMENT.md",
    "08_INTELLIGENCE_FABRIC.md",
    "09_ROUND_TABLE.md",
    "10_PRIVACY_AND_EGRESS.md",
    "11_DEV_ENVIRONMENT.md",
    "12_DECISION_INDEX.md",
    "13_COMMERCIAL_READINESS.md",
    "14_PRODUCT_VISION_AND_USER_PROFILES.md",
    "15_COMPETITIVE_BENCHMARK_VICTOR.md",
    "16_PERSONA_AND_INTERACTION_MODEL.md",
]
REPORT_NAMES = []  # Private operational reports are not public checkout inputs.
DIRECTIVE_PLACEHOLDER = PROJECT_ROOT / "directives" / "README.md"
BEGIN = "<!-- BEGIN GENERATED CURRENT STATE -->"
END = "<!-- END GENERATED CURRENT STATE -->"
CHECKPOINT_BEGIN = "<!-- BEGIN GENERATED CHECKPOINT -->"
CHECKPOINT_END = "<!-- END GENERATED CHECKPOINT -->"


def fail(message: str) -> "NoReturn":
    print(f"knowledge updater: ERROR: {message}", file=sys.stderr)
    raise SystemExit(1)


def git(*args: str) -> str:
    try:
        result = subprocess.run(
            ["git", *args],
            cwd=REPO_ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as exc:
        fail(f"git {' '.join(args)} failed: {exc}")
    return result.stdout.strip()


def product_version() -> str:
    text = (REPO_ROOT / "spec" / "product.yaml").read_text(encoding="utf-8")
    match = re.search(r"(?ms)^product:\s*\n.*?^\s+version:\s*([^\s#]+)", text)
    if not match:
        fail("product.version not found in spec/product.yaml")
    return match.group(1)


def metadata() -> dict[str, str]:
    path = PROJECT_ROOT / "knowledge_metadata.json"
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"cannot read {path}: {exc}")
    required = {"latest_accepted_milestone", "next_approved_step"}
    if not required.issubset(value):
        fail(f"metadata is missing {sorted(required - set(value))}")
    return value


def facts() -> dict[str, str]:
    status = git("status", "--porcelain=v1")
    return {
        "PRODUCT_VERSION": product_version(),
        "BRANCH": git("branch", "--show-current"),
        "HEAD": git("rev-parse", "HEAD"),
        "WORKING_TREE": "clean" if not status else "dirty",
        "RECENT_COMMITS": git("log", "-8", "--format=%h %s"),
        "MAIN_HEAD": git("rev-parse", "refs/heads/main"),
        "LATEST_ACCEPTED_MILESTONE": metadata()["latest_accepted_milestone"],
        "NEXT_APPROVED_STEP": metadata()["next_approved_step"],
    }


def generated_current_state(values: dict[str, str]) -> str:
    return "\n".join(
        [
            BEGIN,
            f"- Branch: `{values['BRANCH']}`",
            f"- HEAD: `{values['HEAD']}`",
            f"- Working tree: **{values['WORKING_TREE']}**",
            f"- Canonical product/spec version: **{values['PRODUCT_VERSION']}**",
            f"- Latest accepted milestone: **{values['LATEST_ACCEPTED_MILESTONE']}**",
            f"- Next approved architecture step: **{values['NEXT_APPROVED_STEP']}**",
            "\nRecent commits (live Git):\n\n```text\n" + values["RECENT_COMMITS"] + "\n```",
            END,
        ]
    )


def generated_checkpoint(values: dict[str, str]) -> str:
    return "\n".join(
        [
            CHECKPOINT_BEGIN,
            f"- Canonical product version: `{values['PRODUCT_VERSION']}`",
            f"- Live branch: `{values['BRANCH']}`",
            f"- Live main HEAD: `{values['MAIN_HEAD']}`",
            f"- Working tree: `{values['WORKING_TREE']}`",
            f"- Latest accepted milestone: `{values['LATEST_ACCEPTED_MILESTONE']}`",
            f"- Next approved architecture step: `{values['NEXT_APPROVED_STEP']}`",
            CHECKPOINT_END,
        ]
    )


def render(source: Path, values: dict[str, str]) -> bytes:
    text = source.read_text(encoding="utf-8")
    text = text.replace("{{CURRENT_STATE}}", generated_current_state(values))
    text = text.replace("{{CHECKPOINT}}", generated_checkpoint(values))
    return text.encode("utf-8")


def reference_files() -> dict[Path, Path]:
    files = {}
    for folder in ("directives", "milestone-reports", "reference"):
        for source in (PROJECT_ROOT / folder).rglob("*"):
            if source.is_file():
                relative = source.relative_to(PROJECT_ROOT / folder)
                prefix = Path("reference") if folder == "reference" else Path("reference") / folder
                files[prefix / relative] = source
    # Canonical supporting evidence makes the Library export self-contained.
    for folder, source_root in (("adr", REPO_ROOT / "docs/adr"), ("spec", REPO_ROOT / "spec")):
        for source in source_root.rglob("*"):
            if source.is_file():
                files[Path("reference") / folder / source.relative_to(source_root)] = source
    files[Path("reference/knowledge_metadata.json")] = PROJECT_ROOT / "knowledge_metadata.json"
    return files


def generated_documentation_files() -> dict[Path, bytes]:
    """Return portable copies of generated, explicitly non-canonical artifacts."""
    names = [
        "MAIA_Technical_Overview_v1.9.docx",
        "MAIA_Technical_Overview_v1.9.pdf",
        "MAIA_Dokumentacja_Techniczna_v1.9.docx",
        "MAIA_Dokumentacja_Techniczna_v1.9.pdf",
    ]
    files = {
        Path("generated-documentation/README.md"): (
            b"# Generated technical documentation\\n\\n"
            b"These files are generated, non-canonical distribution artifacts. "
            b"Their authoritative inputs are `spec/*.yaml`, `docs/adr/`, and the "
            b"versioned project handbook. Regenerate with "
            b"`tools/generate_technical_documentation.py`; do not edit the DOCX or PDF as truth.\\n"
        )
    }
    for name in names:
        source = DOCUMENTATION_ROOT / name
        if source.is_file():
            files[Path("generated-documentation") / name] = source.read_bytes()
    return files


def expected_files() -> dict[Path, bytes]:
    values = facts()
    files = {Path(name): render(PROJECT_ROOT / name, values) for name in HANDBOOK_FILES}
    files.update({relative: source.read_bytes() for relative, source in reference_files().items()})
    files.update(generated_documentation_files())
    return files


def safe_target(destination: Path, relative: Path) -> Path:
    root = destination.absolute()
    if root.is_symlink() or root.resolve() != root:
        fail(f"mirror root must not traverse links: {root}")
    target = root / relative
    if not target.resolve().is_relative_to(root) or any(p.is_symlink() for p in [target, *target.parents]):
        fail(f"unsafe mirror target: {target}")
    return target


def sync_tree(destination: Path) -> None:
    # Never delete unknown files: a mirror may contain user-owned notes.
    for relative, content in expected_files().items():
        target = safe_target(destination, relative)
        target.parent.mkdir(parents=True, exist_ok=True)
        if not target.exists() or target.read_bytes() != content:
            target.write_bytes(content)


def sync_export() -> None:
    sync_tree(EXPORT_ROOT)
    safe_target(EXPORT_ZIP.parent, Path(EXPORT_ZIP.name))
    EXPORT_ZIP.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(EXPORT_ZIP, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for relative, content in sorted(expected_files().items()):
            info = zipfile.ZipInfo(relative.as_posix(), date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            archive.writestr(info, content)


def check_tree(destination: Path) -> list[str]:
    errors: list[str] = []
    desired = expected_files()
    if not destination.is_dir():
        return [f"missing mirror directory: {destination}"]
    for relative, content in desired.items():
        target = destination / relative
        if not target.is_file():
            errors.append(f"missing mirror file: {target}")
        elif target.read_bytes() != content:
            errors.append(f"mirror differs: {target}")
    return errors


def check_zip() -> list[str]:
    errors: list[str] = []
    if not EXPORT_ZIP.is_file():
        return [f"missing export zip: {EXPORT_ZIP}"]
    try:
        with zipfile.ZipFile(EXPORT_ZIP) as archive:
            names = sorted(archive.namelist())
            expected = sorted(path.as_posix() for path in expected_files())
            if names != expected:
                errors.append("export zip file list differs from export directory")
            for name in set(expected).intersection(names):
                if archive.read(name) != (EXPORT_ROOT / name).read_bytes():
                    errors.append(f"export zip content differs: {name}")
    except (OSError, zipfile.BadZipFile) as exc:
        errors.append(f"cannot inspect export zip: {exc}")
    return errors


def check_handbook(values: dict[str, str], *, require_generated_documentation: bool = True) -> list[str]:
    errors: list[str] = []
    for name in HANDBOOK_FILES:
        path = PROJECT_ROOT / name
        if not path.is_file():
            errors.append(f"missing handbook file: {path}")
    if errors:
        return errors
    for name, marker in (("00_START_HERE.md", "{{CHECKPOINT}}"), ("01_CURRENT_STATE.md", "{{CURRENT_STATE}}")):
        if (PROJECT_ROOT / name).read_text(encoding="utf-8").count(marker) != 1:
            errors.append(f"missing or duplicate live template marker in {name}")
    for name in REPORT_NAMES:
        if not (PROJECT_ROOT / "milestone-reports" / name).is_file():
            errors.append(f"missing historical report {name}")
    if require_generated_documentation:
        for name in (
            "MAIA_Technical_Overview_v1.9.docx",
            "MAIA_Technical_Overview_v1.9.pdf",
            "MAIA_Dokumentacja_Techniczna_v1.9.pdf",
            "MAIA_Dokumentacja_Techniczna_v1.9.docx",
        ):
            path = DOCUMENTATION_ROOT / name
            if not path.is_file() or path.stat().st_size == 0:
                errors.append(f"missing generated technical documentation {path}")
    return errors


def main() -> int:
    global SHARED_ROOT, EXPORT_ROOT, EXPORT_ZIP
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="materialize generated handbook facts")
    parser.add_argument("--check", action="store_true", help="validate checkout plus configured integration mirrors")
    parser.add_argument("--checkout-check", action="store_true", help="validate tracked checkout knowledge without mirrors")
    parser.add_argument("--sync-shared", action="store_true", help="sync shared mirror and Library export")
    parser.add_argument("--shared-root", type=Path, default=REPO_ROOT.parent / (REPO_ROOT.name + "-knowledge"))
    parser.add_argument("--export-root", type=Path, default=REPO_ROOT.parent / (REPO_ROOT.name + "-library"))
    args = parser.parse_args()
    SHARED_ROOT = args.shared_root.resolve()
    EXPORT_ROOT = args.export_root.resolve()
    EXPORT_ZIP = EXPORT_ROOT.parent / "MAIA_Project_Knowledge.zip"
    for destination in (SHARED_ROOT, EXPORT_ROOT):
        if destination.is_relative_to(REPO_ROOT) or REPO_ROOT.is_relative_to(destination):
            parser.error("external mirrors must be outside the repository")
    modes = sum((args.write, args.check, args.checkout_check))
    if modes != 1:
        parser.error("choose exactly one of --write, --check, or --checkout-check")
    if args.checkout_check and args.sync_shared:
        parser.error("--checkout-check cannot sync integration mirrors")

    values = facts()
    errors = check_handbook(values, require_generated_documentation=not args.checkout_check)
    if args.checkout_check:
        if errors:
            fail("; ".join(errors))
        print("PROJECT KNOWLEDGE CHECKOUT CHECK OK")
        return 0
    if args.write:
        if errors:
            fail("; ".join(errors))
        sync_tree(REPO_ROOT / ".local/project-knowledge")
        if args.sync_shared:
            sync_tree(SHARED_ROOT)
            sync_export()
        errors = check_handbook(values)
        if args.sync_shared:
            errors.extend(check_tree(SHARED_ROOT))
            errors.extend(check_tree(EXPORT_ROOT))
            errors.extend(check_zip())
        if errors:
            fail("; ".join(errors))
        print("PROJECT KNOWLEDGE WRITE OK")
        if args.sync_shared:
            print(f"SHARED MIRROR: {SHARED_ROOT}")
            print(f"LIBRARY EXPORT: {EXPORT_ROOT}")
        return 0

    if args.check:
        errors.extend(check_tree(REPO_ROOT / ".local/project-knowledge"))
        if args.sync_shared or SHARED_ROOT.exists() or EXPORT_ROOT.exists():
            errors.extend(check_tree(SHARED_ROOT))
            errors.extend(check_tree(EXPORT_ROOT))
            errors.extend(check_zip())
        if errors:
            fail("; ".join(errors))
        print("PROJECT KNOWLEDGE CHECK OK")
        return 0
    return 0


if __name__ == "__main__":
    sys.exit(main())
