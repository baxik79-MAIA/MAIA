"""Fake-only Desk transport and continuation integration regressions."""
import json
from pathlib import Path
import sys
import os
os.environ["MAIA_DESK_THREAD_ID"] = "synthetic-desk-thread-abcdef"
import tempfile
from types import SimpleNamespace
from test_night_shift_governance import control
import unittest
from unittest.mock import patch
from contextlib import ExitStack

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import desk_autoloop as desk
import maia_continuation_driver as drv
import roundtable_exchange as ex


class Loop(unittest.TestCase):
    def setUp(self):
        self.stack = ExitStack()
        self.addCleanup(self.stack.close)
        self.root = Path(self.stack.enter_context(tempfile.TemporaryDirectory()))
        self.stack.enter_context(patch.object(desk, "LEASE_FILE", self.root / "lease"))
        for name, value in dict(ROOT=self.root, STATE_DIR=self.root,
                                STATE_FILE=self.root / "state.json", LOG_FILE=self.root / "log",
                                HOLD_FILE=self.root / "HOLD").items():
            self.stack.enter_context(patch.object(drv, name, value))
        self.stack.enter_context(patch.object(ex, "LEDGER", self.root / "ledger"))
        self.stack.enter_context(patch.object(ex, "DESK_INBOX", self.root / "inbox"))
        ex.LEDGER.mkdir()
        ex.DESK_INBOX.mkdir()
        self.stack.enter_context(patch.object(drv, "backlog_snapshot", return_value=[]))
        self.breach = self.stack.enter_context(patch.object(drv, "protected_worktree_breach", return_value=[]))
        self.state = dict(work_item="test", session_id="session", iteration=1,
                          canonical_thread=ex.CANONICAL_THREAD, status="STOPPED",
                          protected_worktree={}, goal="bounded engineering")
        self.decision = "ACCEPT"
        self.mutate = lambda text: text
        self.transport = self.stack.enter_context(patch.object(ex, "send_desk", side_effect=self.send))

    def escalation(self):
        block = control()
        block.update(status="stop", reason="desk_escalation_required: tests prove linkage; approve continuation?",
                     next_action="proposal")
        block["handoff"].update(authority_pending=True,
            material_evidence="Offline tests verified receipt linkage and no duplicate dispatch.")
        return block

    def prepare(self):
        # Each subcase is a separate escalation fixture.
        if self.state.get("desk_exchange", {}).get("phase") not in (None, "applied"):
            self.state.pop("desk_exchange")
        desk.prepare(self.state, self.escalation(),
                     self.root / "outbox", drv._save_state)

    def send(self, message, milestone, kind, timeout, *, correlation_id, write_ledger=True):
        fixture = json.loads((Path(__file__).parent / "fixtures/deskbridge_success.json").read_text(encoding="utf-8"))
        receipt = message.parent / "sent_20000101_000000.receipt.json"
        fixture["receipt"]["source_pending_file"] = str(message)
        receipt.write_text(json.dumps(fixture["receipt"]), encoding="utf-8-sig")
        request_hash = desk.digest(message.read_bytes())
        sent = message.parent / ("sent_20000101_000000_" + message.name)
        message.replace(sent)
        reply = ex.DESK_INBOX / "reply.md"
        value = dict(nonce=correlation_id, decision=self.decision, reason="reviewed",
                     authorized_next_action="authorized bounded increment")
        reply.write_text(self.mutate(desk.BEGIN + json.dumps(value) + desk.END), encoding="utf-8-sig")
        fixture["inbox"]["related_outbox_receipt"] = str(receipt)
        reply.with_suffix(".json").write_text(json.dumps(fixture["inbox"]), encoding="utf-8-sig")
        if write_ledger:
            ex.write_record(dict(correlation_id=correlation_id, status="answered", transport_exit_code=0,
                session_id=ex.CANONICAL_THREAD, request=dict(sha256=request_hash, receipt=str(receipt)),
                response=dict(path=str(reply), sha256=desk.digest(reply.read_bytes()))))
        return 0

    def advance(self):
        return desk.advance(self.state, drv._save_state, lambda: drv.desk_guard(self.state))

    def test_accept_amend_and_restart_no_second_application(self):
        for decision in ("ACCEPT", "AMEND"):
            self.decision = decision
            self.prepare()
            self.assertTrue(self.advance())
            self.assertEqual(self.state["next_action"], "authorized bounded increment")
            self.state = drv._load_state()
            self.state["next_action"] = "later action"
            self.assertTrue(self.advance())
            self.assertEqual(self.state["next_action"], "later action")
        self.assertEqual(self.transport.call_count, 2)

    def test_reject_defer_no_resume(self):
        for decision in ("REJECT", "DEFER"):
            self.decision = decision
            self.prepare()
            self.assertFalse(self.advance())
            self.assertEqual(self.state["status"], "STOPPED")
            self.assertFalse(self.advance())

    def test_invalid_responses(self):
        for mutate in (lambda t: t.replace(self.state["desk_exchange"]["nonce"], "wrong"),
                       lambda t: "ACCEPT please continue", lambda t: t + t,
                       lambda t: t.replace('"ACCEPT"', '"MAYBE"'),
                       lambda t: t.replace('"reviewed"', json.dumps("x" * 2049)),
                       lambda t: t.replace('"reason":', '"reason":"duplicate","reason":')):
            self.prepare()
            self.mutate = mutate
            self.assertFalse(self.advance())
            self.assertEqual(self.state["desk_exchange"]["phase"], "blocked")
            self.assertFalse(self.advance())

    def test_restart_after_dispatch_never_resends(self):
        self.prepare()
        self.state["desk_exchange"]["phase"] = "dispatched"
        drv._save_state(self.state)
        self.state = drv._load_state()
        self.assertFalse(self.advance())
        self.transport.assert_not_called()

    def test_crash_after_dispatch_recovers_matching_ledger(self):
        self.prepare()
        item = self.state["desk_exchange"]
        self.send(Path(item["packet"]), "test", "desk_ruling", 420, correlation_id=item["nonce"])
        item["phase"] = "dispatched"
        drv._save_state(self.state)
        self.state = drv._load_state()
        self.assertTrue(self.advance())
        self.transport.assert_not_called()

    def test_restart_after_ruling_applies_once(self):
        self.prepare()
        item = self.state["desk_exchange"]
        item.update(phase="ruled", ruling=dict(decision="AMEND", authorized_next_action="amended"))
        drv._save_state(self.state)
        self.state = drv._load_state()
        self.assertTrue(self.advance())
        self.state = drv._load_state()
        with patch.object(drv, "_save_state") as save:
            self.assertTrue(self.advance())
            save.assert_not_called()
        self.transport.assert_not_called()

    def test_real_exchange_adapter_with_fake_uia(self):
        from types import SimpleNamespace
        self.prepare()
        item = self.state["desk_exchange"]
        bridge = self.root / "DeskBridge.ps1"
        bridge.write_text("fake")
        # Use the actual exchange function with just the UI subprocess faked.
        def ui(cmd, **kwargs):
            packet = Path(cmd[cmd.index("-PendingPath") + 1])
            self.send(packet, "test", "desk_ruling", 420, correlation_id=item["nonce"], write_ledger=False)
            receipt = packet.parent / "sent_20000101_000000.receipt.json"
            return SimpleNamespace(returncode=0, stdout=f"SENDONE PASS receipt={receipt}", stderr="")
        import importlib.util
        spec = importlib.util.spec_from_file_location("exchange_test", ex.__file__)
        adapter = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(adapter)
        with patch.object(adapter, "DESK_BRIDGE", bridge), patch.object(adapter, "LEDGER", ex.LEDGER), \
                patch.object(adapter, "DESK_INBOX", ex.DESK_INBOX), \
                patch.object(adapter.subprocess, "run", side_effect=ui) as run:
            self.assertEqual(adapter.send_desk(Path(item["packet"]), "test", "desk_ruling", 420,
                                              correlation_id=item["nonce"]), 0)
            Path(item["packet"]).write_bytes((Path(item["packet"]).parent / ("sent_20000101_000000_" + Path(item["packet"]).name)).read_bytes())
            with self.assertRaises(FileExistsError):
                adapter.send_desk(Path(item["packet"]), "test", "desk_ruling", 420,
                                  correlation_id=item["nonce"])
            run.assert_called_once()
            self.assertEqual(adapter.send_desk(Path(item["packet"]), "M0.12", "status_report", 420), 0)
            self.assertEqual(run.call_count, 2)
        self.assertEqual(json.loads((ex.LEDGER / (item["nonce"] + ".json")).read_text())["status"], "answered")

    def test_real_relative_paths_and_moved_request_integrity(self):
        self.prepare()
        item = self.state["desk_exchange"]
        message = Path(item["packet"])
        self.send(message, "test", "desk_ruling", 420, correlation_id=item["nonce"])
        self.assertFalse(message.exists())
        receipt = message.parent / "sent_20000101_000000.receipt.json"
        data = json.loads(receipt.read_text(encoding="utf-8-sig"))
        data["source_pending_file"] = str(message.relative_to(self.root))
        receipt.write_text(json.dumps(data), encoding="utf-8-sig")
        meta = ex.DESK_INBOX / "reply.json"
        data = json.loads(meta.read_text(encoding="utf-8-sig"))
        data["related_outbox_receipt"] = str(receipt.relative_to(self.root))
        meta.write_text(json.dumps(data), encoding="utf-8-sig")
        with patch.object(ex, "ROOT", self.root):
            self.assertEqual(ex.linked_desk_response(receipt, message, item["request_sha256"]),
                             ex.DESK_INBOX / "reply.md")
            sent = message.parent / ("sent_20000101_000000_" + message.name)
            sent.write_text("changed request")
            with self.assertRaisesRegex(ValueError, "sent request hash"):
                ex.linked_desk_response(receipt, message, item["request_sha256"])

    def test_rotation_saved_before_crash_reuses_pinned_session(self):
        self.prepare()
        self.state["session_turns"] = 24
        drv._save_state(self.state)
        with patch.object(drv.subprocess, "run", side_effect=lambda cmd, **kw:
                SimpleNamespace(returncode=0, stdout="a" * 40 if "rev-parse" in cmd else "")), \
                patch.object(drv, "run_turn", side_effect=RuntimeError("crash")):
            with self.assertRaisesRegex(RuntimeError, "crash"):
                drv.drive(None, "test", 1, None, 60, True)
        saved = drv._load_state()
        self.assertTrue(saved["needs_reconciliation"])
        self.assertEqual(saved["session_turns"], 0)
        pinned = saved["session_id"]
        def turn(session, prompt, model, timeout, fresh_id=None):
            self.assertEqual(session, pinned)
            self.assertIsNone(fresh_id)
            self.assertIn(drv.RECONCILIATION_PREAMBLE, prompt)
            b = dict(status="stop", reason="milestone_complete", work_item="test", next_action="")
            return dict(ok=True, envelope=dict(session_id=session, num_turns=1,
                result=drv.BEGIN + json.dumps(b) + drv.END))
        with patch.object(drv, "session_exists", return_value=True), \
                patch.object(drv, "run_turn", side_effect=turn) as run:
            drv.drive(None, "test", 1, None, 60, True)
            run.assert_called_once()
        self.transport.assert_called_once()
        self.assertEqual(drv._load_state()["session_id"], pinned)

    def test_threshold_guards_win_after_ruling(self):
        for kind in ("hold", "breach"):
            self.prepare()
            self.assertTrue(self.advance())
            self.state["session_turns"] = 24
            drv._save_state(self.state)
            if kind == "hold":
                drv.HOLD_FILE.write_text("stop")
            else:
                self.breach.return_value = ["protected"]
            with patch.object(drv, "run_turn") as turn:
                drv.drive(None, "test", 1, None, 60, True)
                turn.assert_not_called()
            self.assertEqual(drv._load_state()["session_id"], "session")
            drv.HOLD_FILE.unlink(missing_ok=True)
            self.breach.return_value = []

    def test_transport_failure_preserves_state(self):
        self.prepare()
        self.transport.side_effect = OSError("unavailable session")
        self.assertFalse(self.advance())
        self.assertTrue(Path(self.state["desk_exchange"]["packet"]).is_file())
        self.state = drv._load_state()
        self.assertFalse(self.advance())
        self.transport.assert_called_once()

    def test_guards_before_dispatch_and_resume(self):
        for phase in ("prepared", "ruled"):
            for guard in ("hold", "breach", "thread"):
                self.prepare()
                self.state["desk_exchange"].update(phase=phase, ruling=dict(
                    decision="ACCEPT", authorized_next_action="next"))
                if guard == "hold":
                    drv.HOLD_FILE.write_text("stop")
                elif guard == "breach":
                    self.breach.return_value = ["protected"]
                else:
                    self.state["canonical_thread"] = "wrong"
                self.assertFalse(self.advance())
                self.assertEqual(self.state["status"], "STOPPED")
                drv.HOLD_FILE.unlink(missing_ok=True)
                self.breach.return_value = []
                self.state["canonical_thread"] = ex.CANONICAL_THREAD
        self.transport.assert_not_called()

    def test_thread_and_linkage_and_hash_uncertainty(self):
        for damage in ("thread", "receipt", "hash", "duplicate"):
            self.prepare()
            item = self.state["desk_exchange"]
            self.send(Path(item["packet"]), "test", "desk_ruling", 420, correlation_id=item["nonce"])
            item["phase"] = "dispatched"
            meta = ex.DESK_INBOX / "reply.json"
            data = json.loads(meta.read_text(encoding="utf-8-sig"))
            if damage == "thread":
                data["thread_id"] = "wrong"
            elif damage == "receipt":
                data["related_outbox_receipt"] = "other"
            elif damage == "hash":
                (ex.DESK_INBOX / "reply.md").write_text("changed")
            else:
                (ex.DESK_INBOX / "duplicate.json").write_text(meta.read_text())
            meta.write_text(json.dumps(data))
            self.assertFalse(self.advance())
        self.transport.assert_not_called()

    def test_lease_excludes_second_worker(self):
        with desk.lease():
            with self.assertRaises(OSError):
                with desk.lease():
                    self.fail("second owner")
        with desk.lease():
            pass

    def test_guard_changed_during_transport_prevents_resume(self):
        for kind in ("hold", "breach"):
            self.prepare()
            def send(*args, **kwargs):
                result = self.send(*args, **kwargs)
                if kind == "hold":
                    drv.HOLD_FILE.write_text("stop")
                else:
                    self.breach.return_value = ["protected"]
                return result
            self.transport.side_effect = send
            self.assertFalse(self.advance())
            self.assertEqual(self.state["desk_exchange"]["phase"], "ruled")
            self.assertEqual(self.state["status"], "STOPPED")
            drv.HOLD_FILE.unlink(missing_ok=True)
            self.breach.return_value = []

    def test_threshold_rotates_accept_amend_and_reconciles_once(self):
        for decision in ("ACCEPT", "AMEND"):
            self.decision = decision
            self.state.update(session_id="old", session_turns=24)
            self.prepare()
            drv._save_state(self.state)
            def turn(session, prompt, model, timeout, fresh_id=None):
                self.assertIsNone(session)
                self.assertNotEqual(fresh_id, "old")
                self.assertIn(drv.RECONCILIATION_PREAMBLE, prompt)
                self.assertIn("Next action for this turn: authorized bounded increment", prompt)
                self.assertIn('"next_action":"authorized bounded increment"', prompt)
                block = dict(status="stop", work_item="test", reason="milestone_complete", next_action="")
                return dict(ok=True, envelope=dict(session_id=fresh_id, num_turns=1,
                    result=drv.BEGIN + json.dumps(block) + drv.END))
            with patch.object(drv.subprocess, "run", side_effect=lambda cmd, **kw:
                    SimpleNamespace(returncode=0, stdout="a" * 40 if "rev-parse" in cmd else "")), \
                    patch.object(drv, "run_turn", side_effect=turn) as run:
                self.assertEqual(drv.drive(None, "test", 2, None, 60, True), 0)
                self.assertEqual(drv.drive(None, "test", 2, None, 60, True), 0)
                run.assert_called_once()
            self.assertEqual(drv._load_state()["previous_session_id"], "old")
            self.state = drv._load_state()
        self.assertEqual(self.transport.call_count, 2)

    def test_threshold_uncertain_boundary_fails_closed(self):
        for field, value in (("committed", False), ("ambiguous_state", True),
                             ("recent_commits", ["b" * 40])):
            self.prepare()
            item = self.state["desk_exchange"]
            h = json.loads(item["handoff"])
            h[field] = value
            item["handoff"] = json.dumps(h)
            self.state["session_turns"] = 24
            drv._save_state(self.state)
            with patch.object(drv.subprocess, "run", side_effect=lambda cmd, **kw:
                    SimpleNamespace(returncode=0, stdout="a" * 40 if "rev-parse" in cmd else "")), \
                    patch.object(drv, "run_turn") as turn:
                drv.drive(None, "test", 2, None, 60, True)
                turn.assert_not_called()
            self.assertEqual(drv._load_state()["status"], "PAUSED_GOVERNANCE")
            self.state = drv._load_state()

    def test_packet_inline_evidence_and_fail_closed(self):
        self.prepare()
        text = Path(self.state["desk_exchange"]["packet"]).read_text()
        for value in ("a" * 40, "N1: tests needed", "DEVELOPMENT_EVOLUTION only",
                      "Offline tests verified receipt linkage", '"boundary":"increment"'):
            self.assertIn(value, text)
        self.assertLessEqual(len(self.state["desk_exchange"]["handoff"].encode()), 8192)
        for field, value in (("material_evidence", ""), ("recent_commits", ["abcdef"]),
                             ("architectural_constraints", []), ("unresolved_review_findings", None),
                             ("material_evidence", "x" * 2049),
                             ("material_evidence", r"C:\MAIA\reports\evidence.md")):
            b = self.escalation()
            b["handoff"][field] = value
            with self.assertRaises(ValueError):
                desk.prepare(self.state, b, self.root / "outbox", drv._save_state)
        for damage in ("missing", "budget"):
            b = self.escalation()
            if damage == "missing":
                del b["handoff"]
            else:
                b["handoff"]["architectural_constraints"] = ["x" * 512] * 12
                b["handoff"]["unresolved_review_findings"] = ["y" * 512] * 12
            with self.assertRaises(ValueError):
                desk.prepare(self.state, b, self.root / "outbox", drv._save_state)
        self.transport.assert_not_called()

    def test_oversize_escalation_and_unresolved_start_fail_closed(self):
        with self.assertRaises(ValueError):
            desk.prepare(self.state, dict(reason="x" * 2049, next_action="next"),
                         self.root / "outbox", drv._save_state)
        self.prepare()
        with patch.object(drv, "run_turn") as turn:
            self.assertEqual(drv.drive("new goal", "test", 2, None, 60, True), 1)
            turn.assert_not_called()
        self.transport.assert_not_called()

    def test_driver_escalation_dispatch_resume(self):
        for decision in ("ACCEPT", "AMEND", "REJECT", "DEFER"):
            self.decision = decision
            self.state.pop("desk_exchange", None)
            self.state.update(status="RUNNING", session_turns=0)
            drv._save_state(self.state)
            prompts = []
            def turn(session, prompt, model, timeout, fresh_id=None):
                prompts.append(prompt)
                reason = "desk_escalation_required: review" if len(prompts) == 1 else "milestone_complete"
                block = self.escalation()
                block["reason"] = reason
                return dict(ok=True, envelope=dict(session_id="session", num_turns=1,
                    result=drv.BEGIN + json.dumps(block) + drv.END))
            with patch.object(drv, "run_turn", side_effect=turn):
                self.assertEqual(drv.drive(None, "test", 2, None, 60, True), 0)
            self.assertEqual(len(prompts), 2 if decision in ("ACCEPT", "AMEND") else 1)
            if len(prompts) == 2:
                self.assertIn("Next action for this turn: authorized bounded increment", prompts[1])


if __name__ == "__main__":
    unittest.main()
