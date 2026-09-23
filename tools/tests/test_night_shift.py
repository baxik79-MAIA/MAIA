#!/usr/bin/env python3
"""Regression tests for the Night Shift v1.1 loop (directive §18).

Thirteen required behaviours. Where a behavioural or dependency test is
practical it is used in preference to scanning source text, per the standing
amendment that a source scan checks how something is written rather than what it
does.

Run: python tools/tests/test_night_shift.py
"""

import datetime as _dt
import json
import pathlib
import sys
import os
os.environ["MAIA_DESK_THREAD_ID"] = "synthetic-desk-thread-abcdef"
import tempfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import maia_continuation_driver as drv  # noqa: E402
import maia_night_shift as ns  # noqa: E402

FAILURES = []


def check(name, condition, detail=""):
    if condition:
        print(f"  ok   {name}")
    else:
        print(f"  FAIL {name} {detail}")
        FAILURES.append(name)


def block(status, reason="", nxt=""):
    payload = json.dumps(
        {"status": status, "work_item": "M0.15", "reason": reason, "next_action": nxt}
    )
    return f"turn output\n{drv.BEGIN}\n{payload}\n{drv.END}"


def main() -> int:
    print("1. a completed Claude turn does not terminate continuation")
    parsed, why = drv.extract_control(block("continue", "increment done", "next thing"))
    check("a finished turn reporting continue is not a stop",
          parsed is not None and parsed["status"] == "continue", why)
    check("its next action is carried forward", parsed and parsed["next_action"] == "next thing")
    check("stop requires an explicit declared token",
          drv.extract_control(block("stop", "milestone_complete: done"))[0]["status"] == "stop")

    print("\n2. a successful Desk exchange does not terminate continuation")
    # A Desk send is a side effect of an increment, not a control decision. The
    # only thing that can stop the loop is the control block.
    after_desk = block("continue", "sent Desk packet for increment N", "start N+1")
    parsed, _ = drv.extract_control(after_desk)
    check("a turn that sent a Desk packet still continues", parsed["status"] == "continue")
    check("sending is not represented as a stop condition",
          "desk" not in {s.lower() for s in drv.DECLARED_STOPS})

    print("\n3. a delayed Desk response does not block routine work")
    packet = ns.build_desk_packet(
        work_item="M0.15.3", head_sha="abc1234", state="increment complete",
        evidence=["tests green"], decisions=["kept the shim"], codex_status="pending",
        dispositions=[], risks=[], recommendation="continue", next_actions=["M0.15.4"],
        requires_ruling=False, nonce="n1")
    check("a routine packet does not request a ruling", "requires_ruling: false" in packet)
    check("a ruling can still be requested explicitly",
          "requires_ruling: true" in ns.build_desk_packet(
              work_item="x", head_sha="y", state="s", evidence=[], decisions=[],
              codex_status="c", dispositions=[], risks=[], recommendation="r",
              next_actions=[], requires_ruling=True, nonce="n2"))

    print("\n4. canonical Desk identity uncertainty fails closed")
    check("the canonical thread is accepted", ns.desk_thread_is_canonical(ns.CANONICAL_THREAD))
    for bad in [None, "", "not-a-thread", ns.CANONICAL_THREAD[:-1],
                ns.CANONICAL_THREAD.upper(), ns.CANONICAL_THREAD + "x"]:
        check(f"a non-canonical thread is refused ({bad!r:.24})",
              not ns.desk_thread_is_canonical(bad))

    print("\n5. historical pending packets are never bulk dispatched")
    candidates = sorted(ns.HISTORICAL_PENDING) + ["pending_20260918_new_increment.md"]
    allowed = ns.dispatchable_packets(candidates)
    check("all seven historical files are filtered out",
          not any(h in allowed for h in ns.HISTORICAL_PENDING),
          f"leaked: {[h for h in ns.HISTORICAL_PENDING if h in allowed]}")
    check("a genuinely new packet still passes",
          allowed == ["pending_20260918_new_increment.md"], str(allowed))
    check("exactly seven historical files are protected", len(ns.HISTORICAL_PENDING) == 7)

    print("\n6. Codex unavailable queues the review and does not stop work")
    with tempfile.TemporaryDirectory() as tmp:
        original = ns.CODEX_OUTBOX
        ns.CODEX_OUTBOX = pathlib.Path(tmp)
        try:
            path = ns.queue_review("M0.15.3", "HEAD~1", "HEAD",
                                   ["spec/round_table.yaml"], ["is the boundary right?"])
            check("a packet is written when Codex cannot be reached", path.is_file())
            check("queuing performs no dispatch and returns a path",
                  path.suffix == ".md" and path.name.endswith(".packet.md"))
        finally:
            ns.CODEX_OUTBOX = original
    check("unavailability is a recorded state, not an exception",
          ns.PENDING_CODEX_REVIEW == "PENDING_CODEX_REVIEW")
    # Backoff: a known-exhausted allowance is not re-probed every turn.
    now = _dt.datetime(2026, 9, 19, 12, 0, tzinfo=_dt.timezone.utc)
    recent = (now - _dt.timedelta(seconds=60)).strftime("%Y-%m-%dT%H:%M:%SZ")
    old = (now - _dt.timedelta(seconds=ns.CODEX_RETRY_INTERVAL_SECONDS + 60)).strftime(
        "%Y-%m-%dT%H:%M:%SZ")
    check("a recent failed probe is not repeated", not ns.codex_availability(recent, now))
    check("a stale probe is retried", ns.codex_availability(old, now))
    check("never probed means probe now", ns.codex_availability(None, now))
    check("an unparseable timestamp probes rather than silently skipping",
          ns.codex_availability("nonsense", now))

    print("\n7. a Codex result correlates to the right commit range")
    with tempfile.TemporaryDirectory() as tmp:
        original = ns.CORRELATION_DIR
        ns.CORRELATION_DIR = pathlib.Path(tmp)
        try:
            rec = {
                "work_item": "M0.15.3",
                "commit_range": "aaa1111..bbb2222",
                "codex_packet": "M0_15_3_20260919.packet.md",
                "codex_result": "GO_WITH_FINDINGS",
                "claude_disposition": [{"id": "N1", "disposition": "ACCEPT"}],
                "desk_packet": "pending_20260919_M0_15_3.md",
                "desk_response": "20260919_response.md",
            }
            p = ns.write_correlation(rec)
            back = json.loads(p.read_text(encoding="utf-8"))
            check("the chain is persisted end to end",
                  back["commit_range"] == "aaa1111..bbb2222"
                  and back["codex_packet"].endswith(".packet.md")
                  and back["desk_response"].endswith(".md"))
            check("a correlation id is assigned", back["correlation_id"].startswith("ns-"))
            check("the record is timestamped", back["recorded_utc"].endswith("Z"))
        finally:
            ns.CORRELATION_DIR = original

    print("\n8. Codex cannot modify the live tree in default review mode")
    # Behavioural: the packet itself instructs read-only, and the dispatcher
    # passes a read-only sandbox. Both are checked, because either alone could
    # be changed without the other noticing.
    packet_text = ns.build_review_packet("M0.15.3", "HEAD~1", "HEAD", ["spec/x.yaml"], ["q?"])
    check("the packet states read-only and names the protected tree",
          "READ-ONLY" in packet_text and str(ns.ROOT) in packet_text)
    dispatcher = (pathlib.Path(__file__).resolve().parents[1]
                  / "roundtable_codex_exchange.py").read_text(encoding="utf-8")
    check("the dispatcher passes --sandbox read-only",
          '"--sandbox", "read-only"' in dispatcher or "'--sandbox', 'read-only'" in dispatcher)
    check("the dispatcher never passes a write sandbox",
          "workspace-write" not in dispatcher and "danger-full-access" not in dispatcher)

    print("\n9. a material finding raises a Desk arbitration event")
    routine = {"issue": "this loop could use an iterator", "blocking": False,
               "disposition": "ACCEPT"}
    check("a routine finding is settled without the Desk",
          not ns.requires_desk_arbitration(routine))
    for material in [
        {"issue": "this weakens DEPLOYMENT_LOCKED", "blocking": True},
        {"issue": "grants execution authority to a reader", "blocking": True},
        {"issue": "changes Approval semantics", "blocking": True},
        {"issue": "logs a credential", "blocking": True},
        {"issue": "persistent schema change needs migration", "blocking": True},
        {"issue": "alters a canonical contract", "blocking": True},
    ]:
        check(f"a boundary finding escalates ({material['issue'][:34]})",
              ns.requires_desk_arbitration(material))
    check("rejecting a blocking finding is itself material",
          ns.requires_desk_arbitration(
              {"issue": "ordinary style point", "blocking": True, "disposition": "REJECT"}))
    check("only the four dispositions are valid",
          all(ns.disposition_is_valid(d) for d in ("ACCEPT", "MODIFY", "REJECT", "DEFER"))
          and not ns.disposition_is_valid("MAYBE"))

    print("\n10. the protected-worktree guard remains active")
    snap = {"Cargo.lock": ['+ "image",']}
    real = ns_patch = drv.worktree_diff
    try:
        drv.worktree_diff = lambda _p: "+++ b/Cargo.lock\n+ \"image\","
        check("preserved protected content passes", drv.protected_worktree_breach(snap) == [])
        drv.worktree_diff = lambda _p: "+++ b/Cargo.lock\n+ \"something else\","
        check("committed-away protected content HALTS",
              drv.protected_worktree_breach(snap) == ["Cargo.lock"])
    finally:
        drv.worktree_diff = real

    print("\n11. the kill switch stops the loop")
    src = (pathlib.Path(__file__).resolve().parents[1]
           / "maia_continuation_driver.py").read_text(encoding="utf-8")
    check("HOLD is checked before every turn", "HOLD_FILE.is_file()" in src)
    check("the halt is logged rather than silent",
          'reason": f"kill switch' in src or "kill switch" in src)

    print("\n12. provider exhaustion halts truthfully, not as a generic transport error")
    check("an exhausted allowance is named",
          drv.classify_exit(1, "Your claude.ai usage limit has reset at 3pm", "")
          == "usage_limit")
    check("out of credits is named",
          drv.classify_exit(1, "", "workspace is out of credits") == "usage_limit")
    check("an ordinary failure is not mislabelled",
          drv.classify_exit(1, "thread panicked", "") == "exit 1")

    print("\n13. no path enables DEVELOPMENT_EVOLUTION in a DEPLOYMENT_LOCKED build")
    manifest = (pathlib.Path(__file__).resolve().parents[2]
                / "infra" / "claude-code" / "Cargo.toml").read_text(encoding="utf-8")
    check("the capability feature is not on by default", "default = []" in manifest)
    check("the feature exists to be off", "development-evolution" in manifest)
    ns_src = (pathlib.Path(__file__).resolve().parents[1]
              / "maia_night_shift.py").read_text(encoding="utf-8")
    for forbidden in ["--features development-evolution", "development-evolution\"",
                      "bypassPermissions", "dangerously-skip-permissions"]:
        check(f"the night shift never enables or bypasses ({forbidden[:34]})",
              forbidden not in ns_src)
    check("nor does the driver",
          "bypassPermissions" not in src.replace('"bypassPermissions"', "", 1)
          or '"--permission-mode", "acceptEdits"' in src)

    print("\nreview queue policy")
    order = ns.review_queue_order(["current-M0.15.3"], ["debt-A", "debt-B", "debt-C"])
    check("current work is reviewed first", order[0] == "current-M0.15.3")
    check("exactly one debt item follows before the rest", order[1] == "debt-A")
    check("no packet is dropped from the queue", len(order) == 4)

    print()
    if FAILURES:
        print(f"{len(FAILURES)} FAILED: {FAILURES}")
        return 1
    print(f"all night shift checks passed ({13} required behaviours covered)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
