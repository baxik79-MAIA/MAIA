"""Development-host Desk orchestration. State belongs to continuation/state.json.

Dispatch intent is irreversible: uncertain delivery requires reconciliation, never
a resend. Applying a ruling and publishing next_action is one atomic state save.
This module grants no product permissions and contains no UI automation.
"""
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import tempfile
import uuid
import subprocess

import roundtable_exchange as exchange
import night_shift_governance as governance

BEGIN = "<<<MAIA_DESK_RULING>>>"
END = "<<<END_MAIA_DESK_RULING>>>"
LEASE_FILE = Path(tempfile.gettempdir()) / "maia-continuation-desk-v1.lock"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON key")
        result[key] = value
    return result


def ruling(text, nonce):
    if len(text) > 65536 or text.count(BEGIN) != 1 or text.count(END) != 1:
        raise ValueError("exactly one bounded ruling block required")
    start, end = text.index(BEGIN) + len(BEGIN), text.index(END)
    if end <= start:
        raise ValueError("invalid ruling delimiters")
    value = json.loads(text[start:end], object_pairs_hook=unique_object)
    if not isinstance(value, dict) or set(value) != {
            "nonce", "decision", "reason", "authorized_next_action"}:
        raise ValueError("invalid ruling fields")
    for key, limit in (("nonce", 64), ("decision", 6), ("reason", 2048),
                       ("authorized_next_action", 1024)):
        if not isinstance(value[key], str) or len(value[key]) > limit:
            raise ValueError("invalid bounded string: " + key)
    if value["nonce"] != nonce or value["decision"] not in {"ACCEPT", "AMEND", "REJECT", "DEFER"}:
        raise ValueError("nonce or decision mismatch")
    if value["decision"] in {"ACCEPT", "AMEND"} and not value["authorized_next_action"].strip():
        raise ValueError("authorized next action required")
    return value


@contextmanager
def lease():
    """One host-wide OS lease; process death releases it, no stale PID stealing."""
    with LEASE_FILE.open("a+b") as handle:
        handle.seek(0)
        if os.name == "nt":
            import msvcrt
            # Locking beyond EOF is supported by Windows.
            msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
        else:
            import fcntl
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        try:
            yield
        finally:
            handle.seek(0)
            if os.name == "nt":
                msvcrt.locking(handle.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                fcntl.flock(handle, fcntl.LOCK_UN)


def prepare(state, block, directory, save):
    for key, limit in (("reason", 2048), ("next_action", 1024)):
        if not isinstance(block.get(key), str) or not block[key].strip() or len(block[key]) > limit:
            raise ValueError("invalid escalation " + key)
    if block.get("work_item") != state["work_item"]:
        raise ValueError("escalation work item mismatch")
    handoff = governance.compact_handoff(block, True, desk_escalation=True)
    previous = state.get("desk_exchange")
    if previous:
        if previous["phase"] != "applied":
            raise ValueError("unresolved Desk exchange")
        state.setdefault("desk_history", []).append(previous)
    nonce = uuid.uuid4().hex
    packet = (f"Subject: MAIA Desk ruling {nonce}\n"
              f"Canonical thread: {exchange.CANONICAL_THREAD}\n"
              "Development engineering only; no product execution authorization.\n"
              f"Work item: {state['work_item']}\n"
              f"Escalation: {block.get('reason', '')[:2048]}\n"
              f"Requested next action: {block.get('next_action', '')[:1024]}\n"
              f"Inline compact handoff/evidence:\n{handoff}\n"
              "Return exactly one block, with this exact nonce. Decisions: "
              "ACCEPT, AMEND, REJECT, DEFER. reason <=2048 characters; "
              "authorized_next_action <=1024 characters, nonempty for ACCEPT/AMEND.\n"
              f'{BEGIN}\n{{"nonce":"{nonce}","decision":"DEFER",'
              '"reason":"","authorized_next_action":""}\n' + END + "\n")
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / f"pending_{nonce}.md"
    with path.open("xb") as handle:
        handle.write(packet.encode())
        handle.flush()
        os.fsync(handle.fileno())
    state["desk_exchange"] = dict(nonce=nonce, phase="prepared", packet=str(path),
                                  handoff=handoff,
                                  request_sha256=digest(packet.encode()))
    state["status"] = "STOPPED"
    save(state)


def advance(state, save, guard):
    """Called under the driver's lease. Never redispatch an uncertain nonce."""
    item = state["desk_exchange"]
    if item["phase"] == "applied":
        return state.get("status") == "RUNNING"
    try:
        guard()
        if item["phase"] == "prepared":
            path = Path(item["packet"])
            if digest(path.read_bytes()) != item["request_sha256"]:
                raise ValueError("request hash mismatch")
            item["phase"] = "dispatched"
            save(state)  # write-ahead intent, BEFORE any UI/transport effect
            code = exchange.send_desk(path, state["work_item"], "desk_ruling", 420,
                                      correlation_id=item["nonce"])
            if code:
                raise ValueError("transport incomplete; reconciliation required")
        if item["phase"] == "dispatched":
            record = json.loads((exchange.LEDGER / (item["nonce"] + ".json")).read_text())
            if (record.get("status") != "answered" or record.get("session_id") != exchange.CANONICAL_THREAD
                    or record.get("correlation_id") != item["nonce"]
                    or record.get("request", {}).get("sha256") != item["request_sha256"]
                    or record.get("transport_exit_code") != 0):
                raise ValueError("unverified exchange evidence")
            evidence = exchange.linked_desk_response(Path(record["request"]["receipt"]),
                                                     Path(item["packet"]), item["request_sha256"])
            if evidence is None or str(evidence) != record["response"]["path"]:
                raise ValueError("response linkage mismatch")
            raw = evidence.read_bytes()
            if digest(raw) != record["response"]["sha256"]:
                raise ValueError("response hash mismatch")
            item["response_sha256"] = digest(raw)
            item["ruling"] = ruling(raw.decode("utf-8-sig"), item["nonce"])
            item["phase"] = "ruled"
            save(state)
        if item["phase"] == "ruled":
            guard()
            decision = item["ruling"]
            state["status"] = "STOPPED"
            if decision["decision"] in {"ACCEPT", "AMEND"}:
                state["next_action"] = decision["authorized_next_action"]
                state["needs_reconciliation"] = True
                state["status"] = "RUNNING"
            item["phase"] = "applied"
            save(state)  # consumption + continuation are one transaction
            return state["status"] == "RUNNING"
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as exc:
        item["error"] = str(exc)[:512]
        if item["phase"] == "dispatched":
            item["phase"] = "blocked"
        save(state)
    return False
