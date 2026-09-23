#!/usr/bin/env python3
"""Unified Round Table exchange layer with a full audit ledger.

Reuses the existing MAIA transports rather than adding a parallel architecture:

  participant "desk"  -> Architecture Desk / ChatGPT, via the existing
                         locally supplied .local/DeskBridge.ps1 and the
                         canonical desk_outbox / desk_inbox directories.
  participant "codex" -> delegates to tools/roundtable_codex_exchange.py and
                         its reports/roundtable/outbox|inbox directories.

What this layer adds on top of the transports is the audit trail required by
the collaboration protocol: sender, recipient and role, timestamp, correlation
and session id, milestone, message type, status, and request/response linkage.

Fail-closed guarantees:
  * a dispatch that cannot be verified as delivered is recorded as failed;
  * a response is only ever recorded when the transport actually captured one;
  * no response is ever synthesized, inferred or defaulted;
  * a non-zero exit signals an incomplete exchange so a caller cannot mistake
    a blocked dispatch for a completed one.

Usage:
  python tools/roundtable_exchange.py --send --participant desk \\
      --message <path> --milestone M0.12 --type status_report
  python tools/roundtable_exchange.py --record --participant desk \\
      --receipt <path> --response <path> --milestone M0.12 --type status_report
  python tools/roundtable_exchange.py --ledger
"""

from __future__ import annotations

import argparse
import datetime as _dt
import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys
import uuid

ROOT = pathlib.Path(__file__).resolve().parents[1]
LEDGER = ROOT / "reports" / "roundtable" / "ledger"

DESK_BRIDGE = ROOT / ".local" / "DeskBridge.ps1"
DESK_OUTBOX = ROOT / ".local" / "desk_outbox"
DESK_INBOX = ROOT / ".local" / "desk_inbox"
CANONICAL_THREAD = os.environ.get("MAIA_DESK_THREAD_ID", "")

ROLES = {
    "desk": "orchestration_and_architecture_authority",
    "codex": "independent_review_participant",
}
SENDER = "claude-code"
SENDER_ROLE = "implementation_participant"


def _utc() -> str:
    return _dt.datetime.now(_dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _digest(path: pathlib.Path) -> str | None:
    try:
        return hashlib.sha256(path.read_bytes()).hexdigest()
    except OSError:
        return None


def _correlation_id(milestone: str) -> str:
    safe = "".join(c for c in milestone if c.isalnum() or c in "._-")[:16]
    return f"rt-{safe}-{uuid.uuid4().hex}"


def write_record(record: dict) -> pathlib.Path:
    LEDGER.mkdir(parents=True, exist_ok=True)
    path = LEDGER / f"{record['correlation_id']}.json"
    tmp = path.with_suffix(".tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        json.dump(record, handle, indent=2)
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(path)
    return path


def base_record(participant: str, milestone: str, message_type: str,
                correlation_id: str) -> dict:
    return {
        "schema_version": "maia.roundtable_exchange.v1",
        "correlation_id": correlation_id,
        "sender": SENDER,
        "sender_role": SENDER_ROLE,
        "recipient": participant,
        "recipient_role": ROLES.get(participant, "unknown"),
        "milestone": milestone,
        "message_type": message_type,
        "created_utc": _utc(),
        "status": "created",
        "transport": None,
        "session_id": None,
        "request": None,
        "response": None,
        "blocker": None,
    }


def _bridge_path(value: str) -> pathlib.Path:
    if not isinstance(value, str) or not value.strip():
        raise ValueError("missing bridge path")
    path = pathlib.Path(value)
    return (path if path.is_absolute() else ROOT / path).resolve()


def linked_desk_response(receipt: pathlib.Path, message: pathlib.Path,
                         request_sha256: str | None = None) -> pathlib.Path | None:
    """Resolve only the response whose bridge sidecar names this exact receipt."""
    receipt = _bridge_path(str(receipt))
    message = _bridge_path(str(message))
    data = json.loads(receipt.read_text(encoding="utf-8-sig"))
    if (data.get("thread_id") != CANONICAL_THREAD or data.get("status") != "sent"
            or _bridge_path(data.get("source_pending_file")) != message):
        raise ValueError("receipt identity mismatch")
    # Write-ReceiptAndMovePending moves, rather than copies, the request.
    if request_sha256 is not None:
        if not re.fullmatch(r"sent_\d{8}_\d{6}\.receipt\.json", receipt.name):
            raise ValueError("invalid receipt filename")
        sent = receipt.parent / (receipt.name.removesuffix(".receipt.json") + "_" + message.name)
        if _digest(sent) != request_sha256:
            raise ValueError("sent request hash mismatch")
    matches = []
    for meta in DESK_INBOX.glob("*.json"):
        info = json.loads(meta.read_text(encoding="utf-8-sig"))
        if _bridge_path(info.get("related_outbox_receipt")) == receipt:
            if info.get("thread_id") != CANONICAL_THREAD or info.get("source") != "architecture_desk":
                raise ValueError("response thread/source mismatch")
            matches.append(meta.with_suffix(".md"))
    if len(matches) > 1:
        raise ValueError("ambiguous linked responses")
    return matches[0] if matches and matches[0].is_file() else None


def send_desk(message: pathlib.Path, milestone: str, message_type: str,
              response_timeout: int, *, correlation_id: str | None = None) -> int:
    """Dispatch one message to the Architecture Desk and capture the reply."""
    if not CANONICAL_THREAD:
        print("FAIL: configure MAIA_DESK_THREAD_ID explicitly", file=sys.stderr)
        return 2
    message = message.resolve()
    if not message.is_file():
        print(f"FAIL: message not found: {message}", file=sys.stderr)
        return 2
    if not DESK_BRIDGE.is_file():
        print(f"FAIL: DeskBridge not found: {DESK_BRIDGE}", file=sys.stderr)
        return 2

    correlation_id = correlation_id or _correlation_id(milestone)
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}", correlation_id):
        raise ValueError("invalid correlation id")
    LEDGER.mkdir(parents=True, exist_ok=True)
    record = base_record("desk", milestone, message_type, correlation_id)
    record["transport"] = "deskbridge_uia"
    record["session_id"] = CANONICAL_THREAD
    record["request"] = {
        "path": str(message),
        "sha256": _digest(message),
        "bytes": message.stat().st_size,
    }

    # Exclusive write-ahead record also protects direct callers from replay.
    with (LEDGER / f"{correlation_id}.json").open("x", encoding="utf-8") as handle:
        json.dump(record, handle)
        handle.flush()
        os.fsync(handle.fileno())

    proc = subprocess.run(
        ["pwsh", "-NoProfile", "-File", str(DESK_BRIDGE),
         "-Mode", "SendOne",
         "-PendingPath", str(message),
         "-ResponseTimeoutSeconds", str(response_timeout)],
        capture_output=True, text=True, encoding="utf-8", errors="replace",
        cwd=str(ROOT),
        timeout=response_timeout + 300,
    )
    combined = (proc.stdout or "") + "\n" + (proc.stderr or "")
    record["transport_exit_code"] = proc.returncode

    # Delivery is only claimed when the bridge itself reports SENDONE PASS,
    # which it writes only after observing the outgoing text in the thread.
    delivered = "SENDONE PASS" in combined
    receipt = None
    for line in combined.splitlines():
        if line.startswith("SENDONE PASS ") and "receipt=" in line:
            receipt = line.split("receipt=", 1)[1].strip()
    if receipt:
        record["request"]["receipt"] = receipt

    if not delivered:
        record["status"] = "dispatch_failed"
        record["blocker"] = combined.strip()[-800:] or "bridge produced no output"
        path = write_record(record)
        print(f"DISPATCH FAILED  correlation={correlation_id}\n  ledger={path}")
        return 1

    record["status"] = "delivered"

    # A response is recorded only if the bridge actually wrote one.
    try:
        reply = linked_desk_response(pathlib.Path(receipt), message,
                                     record["request"]["sha256"]) if receipt else None
    except (OSError, ValueError):
        reply = None
    if reply and proc.returncode == 0:
        record["response"] = {
            "path": str(reply),
            "sha256": _digest(reply),
            "bytes": reply.stat().st_size,
            "received_utc": _utc(),
        }
        record["status"] = "answered"
    else:
        record["status"] = "delivered_awaiting_response"
        record["blocker"] = "no response captured by the transport"

    path = write_record(record)
    print(f"{record['status'].upper()}  correlation={correlation_id}\n  ledger={path}")
    if record["response"]:
        print(f"  response={record['response']['path']}")
    return 0 if record["status"] == "answered" else 1


def record_existing(participant: str, milestone: str, message_type: str,
                    receipt: pathlib.Path | None, request: pathlib.Path | None,
                    response: pathlib.Path | None, status: str) -> int:
    """Backfill a ledger entry for an exchange performed by the transport directly."""
    correlation_id = _correlation_id(milestone)
    record = base_record(participant, milestone, message_type, correlation_id)
    record["transport"] = "deskbridge_uia" if participant == "desk" else "codex_cli"
    record["session_id"] = CANONICAL_THREAD if participant == "desk" else None
    record["backfilled"] = True

    if request and request.is_file():
        record["request"] = {
            "path": str(request), "sha256": _digest(request),
            "bytes": request.stat().st_size,
        }
    if receipt and receipt.is_file():
        record.setdefault("request", {})
        record["request"]["receipt"] = str(receipt)
        try:
            data = json.loads(receipt.read_text(encoding="utf-8"))
            record["request"]["subject"] = data.get("subject")
            record["request"]["sent_at"] = data.get("sent_at")
            record["request"]["repo_head"] = data.get("repo_head")
        except (OSError, json.JSONDecodeError):
            record["blocker"] = "receipt unreadable"
    if response and response.is_file():
        record["response"] = {
            "path": str(response), "sha256": _digest(response),
            "bytes": response.stat().st_size, "received_utc": _utc(),
        }
    record["status"] = status
    path = write_record(record)
    print(f"RECORDED  correlation={correlation_id}\n  ledger={path}")
    return 0


def show_ledger() -> int:
    if not LEDGER.is_dir():
        print("no ledger entries")
        return 0
    entries = sorted(LEDGER.glob("*.json"))
    if not entries:
        print("no ledger entries")
        return 0
    print(f"{'created (UTC)':21} {'milestone':10} {'to':6} {'type':16} {'status':28} correlation")
    for entry in entries:
        try:
            d = json.loads(entry.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            print(f"{'?':21} {'?':10} {'?':6} {'?':16} {'unreadable':28} {entry.stem}")
            continue
        print(f"{d.get('created_utc',''):21} {d.get('milestone',''):10} "
              f"{d.get('recipient',''):6} {d.get('message_type',''):16} "
              f"{d.get('status',''):28} {d.get('correlation_id','')}")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--send", action="store_true", help="dispatch a message")
    ap.add_argument("--record", action="store_true", help="backfill a ledger entry")
    ap.add_argument("--ledger", action="store_true", help="show the exchange ledger")
    ap.add_argument("--participant", choices=sorted(ROLES), default="desk")
    ap.add_argument("--message", type=pathlib.Path)
    ap.add_argument("--receipt", type=pathlib.Path)
    ap.add_argument("--request", type=pathlib.Path)
    ap.add_argument("--response", type=pathlib.Path)
    ap.add_argument("--milestone", default="unspecified")
    ap.add_argument("--type", dest="message_type", default="status_report")
    ap.add_argument("--status", default="answered")
    ap.add_argument("--response-timeout", type=int, default=420)
    args = ap.parse_args()

    if args.ledger:
        return show_ledger()
    if args.record:
        return record_existing(args.participant, args.milestone, args.message_type,
                               args.receipt, args.request, args.response, args.status)
    if args.send:
        if args.participant != "desk":
            print("FAIL: --send currently supports --participant desk; "
                  "use tools/roundtable_codex_exchange.py --dispatch for codex",
                  file=sys.stderr)
            return 2
        if not args.message:
            print("FAIL: --message is required", file=sys.stderr)
            return 2
        return send_desk(args.message, args.milestone, args.message_type,
                         args.response_timeout)
    ap.print_help()
    return 0


if __name__ == "__main__":
    sys.exit(main())
