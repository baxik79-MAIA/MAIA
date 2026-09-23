#!/usr/bin/env python3
"""File-backed Claude <-> Codex Round Table exchange.

Smallest transport seam that completes the cross-review automatically, with no
manual copy/paste, as soon as a callable Codex participant is available.

Layout (canonical outbox/inbox, mirroring reports/desk_outbox|desk_inbox):

    reports/roundtable/outbox/<id>.packet.md     review packet awaiting Codex
    reports/roundtable/outbox/<id>.receipt.json  dispatch receipt (sent or blocked)
    reports/roundtable/inbox/<id>.response.json  Codex assessment, once received

Fails closed: a dispatch that does not produce a parseable assessment is
recorded as blocked and never as a completed exchange. The tool never
fabricates a Codex response.

Usage:
    python tools/roundtable_codex_exchange.py --dispatch      # send pending packets
    python tools/roundtable_codex_exchange.py --status        # show exchange state
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import pathlib
import re
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTBOX = ROOT / "reports" / "roundtable" / "outbox"
INBOX = ROOT / "reports" / "roundtable" / "inbox"

# Codex runs read-only: a reviewer has no authority to mutate the repository.
CODEX_ARGS = ["exec", "--sandbox", "read-only", "--skip-git-repo-check", "-C", str(ROOT)]


def _utc() -> str:
    return _dt.datetime.now(_dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _receipt(packet_id: str, status: str, detail: str, **extra) -> dict:
    record = {
        "schema_version": "maia.roundtable_exchange_receipt.v1",
        "packet_id": packet_id,
        "participant": "codex",
        "status": status,
        "detail": detail,
        "timestamp_utc": _utc(),
    }
    record.update(extra)
    return record


def _extract_json(text: str):
    """Return the first parseable JSON object, or None. Never guesses."""
    start = text.find("{")
    while start != -1:
        depth, in_str, esc = 0, False, False
        for i in range(start, len(text)):
            c = text[i]
            if in_str:
                if esc:
                    esc = False
                elif c == "\\":
                    esc = True
                elif c == '"':
                    in_str = False
                continue
            if c == '"':
                in_str = True
            elif c == "{":
                depth += 1
            elif c == "}":
                depth -= 1
                if depth == 0:
                    try:
                        return json.loads(text[start : i + 1])
                    except json.JSONDecodeError:
                        break
        start = text.find("{", start + 1)
    return None


def dispatch() -> int:
    OUTBOX.mkdir(parents=True, exist_ok=True)
    INBOX.mkdir(parents=True, exist_ok=True)

    codex = shutil.which("codex")
    packets = sorted(OUTBOX.glob("*.packet.md"))
    if not packets:
        print("no pending packets")
        return 0

    blocked = 0
    for packet in packets:
        packet_id = packet.name[: -len(".packet.md")]
        receipt_path = OUTBOX / f"{packet_id}.receipt.json"
        response_path = INBOX / f"{packet_id}.response.json"

        if response_path.exists():
            print(f"{packet_id}: already answered")
            continue

        if not codex:
            record = _receipt(packet_id, "blocked", "codex executable not found on PATH")
            receipt_path.write_text(json.dumps(record, indent=2), encoding="utf-8")
            print(f"{packet_id}: BLOCKED - codex not found")
            blocked += 1
            continue

        prompt = packet.read_text(encoding="utf-8")
        proc = subprocess.run(
            [codex, *CODEX_ARGS],
            input=prompt,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        combined = (proc.stdout or "") + "\n" + (proc.stderr or "")

        # Distinguish a real transport/quota blocker from a substantive reply.
        quota = re.search(r"out of credits|insufficient quota|rate limit", combined, re.I)
        assessment = _extract_json(proc.stdout or "")

        if assessment is not None and "verdict" in assessment:
            response_path.write_text(json.dumps(assessment, indent=2), encoding="utf-8")
            record = _receipt(
                packet_id,
                "answered",
                "codex assessment received",
                verdict=assessment.get("verdict"),
                exit_code=proc.returncode,
            )
            receipt_path.write_text(json.dumps(record, indent=2), encoding="utf-8")
            print(f"{packet_id}: ANSWERED verdict={assessment.get('verdict')}")
            continue

        detail = (
            "codex workspace is out of credits"
            if quota
            else f"no parseable assessment (exit {proc.returncode})"
        )
        record = _receipt(
            packet_id,
            "blocked",
            detail,
            exit_code=proc.returncode,
            stderr_tail=(proc.stderr or "")[-600:],
        )
        receipt_path.write_text(json.dumps(record, indent=2), encoding="utf-8")
        print(f"{packet_id}: BLOCKED - {detail}")
        blocked += 1

    # Non-zero signals an incomplete exchange so a caller cannot mistake a
    # blocked dispatch for a completed cross-review.
    return 1 if blocked else 0


def status() -> int:
    OUTBOX.mkdir(parents=True, exist_ok=True)
    INBOX.mkdir(parents=True, exist_ok=True)
    packets = sorted(OUTBOX.glob("*.packet.md"))
    if not packets:
        print("no packets")
        return 0
    for packet in packets:
        packet_id = packet.name[: -len(".packet.md")]
        answered = (INBOX / f"{packet_id}.response.json").exists()
        receipt = OUTBOX / f"{packet_id}.receipt.json"
        detail = ""
        if receipt.exists():
            try:
                detail = json.loads(receipt.read_text(encoding="utf-8")).get("detail", "")
            except json.JSONDecodeError:
                detail = "unreadable receipt"
        print(f"{packet_id}: {'ANSWERED' if answered else 'PENDING'} {detail}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dispatch", action="store_true", help="send pending packets to Codex")
    parser.add_argument("--status", action="store_true", help="show exchange state")
    args = parser.parse_args()
    if args.dispatch:
        return dispatch()
    if args.status:
        return status()
    parser.print_help()
    return 0


if __name__ == "__main__":
    sys.exit(main())
