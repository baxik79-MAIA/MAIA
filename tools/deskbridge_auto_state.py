#!/usr/bin/env python3
"""Small development-only durable state for authorized DeskBridge work loops.

This is not MAIA runtime state and never reads credentials.  It records only
the recovery facts needed to continue authorized engineering across sessions.
"""
from __future__ import annotations
import argparse, json, subprocess
from dataclasses import asdict, dataclass
from enum import StrEnum
from pathlib import Path

DEFAULT = Path(__file__).resolve().parents[1] / ".local" / "deskbridge_auto_state.json"

class State(StrEnum):
    AUTHORIZED_WORK_REMAINING = "AUTHORIZED_WORK_REMAINING"
    HANDOFF_REQUIRED = "HANDOFF_REQUIRED"
    HANDOFF_PREPARED = "HANDOFF_PREPARED"
    HANDOFF_DISPATCHED = "HANDOFF_DISPATCHED"
    WAITING_FOR_DESK = "WAITING_FOR_DESK"
    DESK_RESPONSE_RECEIVED = "DESK_RESPONSE_RECEIVED"
    APPLYING_DESK_DECISION = "APPLYING_DESK_DECISION"
    HUMAN_BLOCKED = "HUMAN_BLOCKED"
    MILESTONE_COMPLETE = "MILESTONE_COMPLETE"

ALLOWED = {
    State.AUTHORIZED_WORK_REMAINING: {State.HANDOFF_REQUIRED, State.HUMAN_BLOCKED, State.MILESTONE_COMPLETE},
    State.HANDOFF_REQUIRED: {State.HANDOFF_PREPARED, State.HUMAN_BLOCKED},
    State.HANDOFF_PREPARED: {State.HANDOFF_DISPATCHED, State.HUMAN_BLOCKED},
    State.HANDOFF_DISPATCHED: {State.WAITING_FOR_DESK, State.HUMAN_BLOCKED},
    State.WAITING_FOR_DESK: {State.DESK_RESPONSE_RECEIVED, State.HUMAN_BLOCKED},
    State.DESK_RESPONSE_RECEIVED: {State.APPLYING_DESK_DECISION, State.HUMAN_BLOCKED},
    State.APPLYING_DESK_DECISION: {State.AUTHORIZED_WORK_REMAINING, State.MILESTONE_COMPLETE, State.HUMAN_BLOCKED},
    State.HUMAN_BLOCKED: {State.AUTHORIZED_WORK_REMAINING, State.HANDOFF_REQUIRED},
    State.MILESTONE_COMPLETE: set(),
}

@dataclass
class Record:
    current_milestone: str
    authorized_boundary: str
    current_step: str
    last_repository_head: str
    state: str = State.AUTHORIZED_WORK_REMAINING
    last_handoff_nonce: str | None = None
    dispatch_status: str = "not_dispatched"
    acknowledgement_nonce: str | None = None
    acknowledgement_status: str = "not_received"
    pending_report_path: str | None = None
    stop_reason: str | None = None

def head() -> str:
    return subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
def load(path: Path) -> Record:
    return Record(**json.loads(path.read_text(encoding="utf-8")))
def save(path: Path, record: Record) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(asdict(record), indent=2) + "\n", encoding="utf-8")
def move(record: Record, destination: State) -> None:
    current = State(record.state)
    if destination not in ALLOWED[current]: raise ValueError(f"invalid transition {current} -> {destination}")
    record.state = destination
def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--path", type=Path, default=DEFAULT)
    parser.add_argument("--init", action="store_true")
    parser.add_argument("--milestone", default="M0.11")
    parser.add_argument("--boundary", default="M0.11 completion")
    parser.add_argument("--step", default="first incomplete authorized step")
    parser.add_argument("--set-step")
    parser.add_argument("--resume", action="store_true", help="fail-closed recovery instruction for the active state")
    parser.add_argument("--transition", choices=[state.value for state in State])
    parser.add_argument("--stop-reason")
    args = parser.parse_args()
    record = Record(args.milestone, args.boundary, args.step, head()) if args.init else load(args.path)
    if args.transition: move(record, State(args.transition))
    if args.set_step is not None: record.current_step = args.set_step
    if args.stop_reason is not None: record.stop_reason = args.stop_reason
    if args.resume:
        if record.last_repository_head != head():
            raise SystemExit("AUTO SUPERVISOR: stale repository HEAD; reconcile state before resuming")
        state = State(record.state)
        actions = {
            State.AUTHORIZED_WORK_REMAINING: f"RESUME {record.current_step}",
            State.HANDOFF_REQUIRED: "DISPATCH prepared correlated handoff",
            State.WAITING_FOR_DESK: "POLL correlated Desk response",
            State.DESK_RESPONSE_RECEIVED: "APPLY Desk decision",
            State.HUMAN_BLOCKED: f"HUMAN ACTION REQUIRED: {record.stop_reason or 'unspecified'}",
            State.MILESTONE_COMPLETE: "PERFORM required handoff transition",
        }
        print(f"AUTO SUPERVISOR: {actions.get(state, 'CONTINUE current transition')}")
    record.last_repository_head = head()
    save(args.path, record); print(json.dumps(asdict(record), indent=2)); return 0
if __name__ == "__main__": raise SystemExit(main())
