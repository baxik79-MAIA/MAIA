"""Focused governance regressions; no provider calls or shared-worktree writes."""
import ast
import datetime as dt
import json
import os
import pathlib
import sys
os.environ["MAIA_DESK_THREAD_ID"] = "synthetic-desk-thread-abcdef"
import tempfile
import unittest
from contextlib import ExitStack
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
import maia_continuation_driver as drv
import maia_night_shift as ns
import night_shift_governance as gov


def control():
    return dict(status="continue", work_item="test", next_action="next increment", reason="done",
                handoff=dict(boundary="increment", increment="one", committed=True,
                             review_queued=True, report_queued=True, authority_pending=False,
                             ambiguous_state=False, recent_commits=["a" * 40],
                             unresolved_review_findings=["N1: tests needed"],
                             architectural_constraints=["DEVELOPMENT_EVOLUTION only"]))


class Governance(unittest.TestCase):
    def test_bounded_deterministic_handoff(self):
        b = control()
        encoded = gov.compact_handoff(b, True)
        self.assertEqual(encoded, gov.compact_handoff(dict(reversed(list(b.items()))), True))
        self.assertLessEqual(len(encoded.encode()), gov.MAX_HANDOFF_BYTES)
        self.assertNotIn("reason", json.loads(encoded))
        b["handoff"]["architectural_constraints"] = ["x" * 513]
        with self.assertRaises(ValueError):
            gov.compact_handoff(b, True)

    def test_unsafe_boundaries(self):
        for key, value in [("boundary", "mid_increment"), ("committed", False),
                           ("review_queued", False), ("report_queued", False),
                           ("authority_pending", True), ("ambiguous_state", True)]:
            b = control()
            b["handoff"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                gov.compact_handoff(b, True)
        with self.assertRaises(ValueError):
            gov.compact_handoff(control(), False)
        b = control()
        b["reason"] = "desk_escalation_required: pending ruling"
        with self.assertRaises(ValueError):
            gov.compact_handoff(b, True)

    def test_reset_and_backoff(self):
        now = dt.datetime(2026, 9, 19, tzinfo=dt.timezone.utc)
        for text in ["resets at 3pm", "429", "reset_at: 2026-09-18T00:00:00Z",
                     "reset_at: 2026-99-19T00:00:00Z",
                     "reset_at: 2026-09-20T00:00:00Z reset_at: 2026-09-21T00:00:00Z"]:
            self.assertIsNone(gov.resume_after(text, now))
        reset = gov.resume_after('"reset_at": "2026-09-19T00:00:01Z"', now)
        self.assertEqual(gov.retry_delay(reset, now), 300)
        self.assertIsNone(gov.retry_delay("2026-09-19T00:00:00", now))

    def test_verification(self):
        self.assertEqual(gov.verification_policy(["tools/small.py"]), "targeted")
        for paths, boundary, safety in [([], "promotion", False), ([], "submilestone", False),
                                        ([], "increment", True), (["core/a.rs"], "increment", False),
                                        (["x/Cargo.toml"], "increment", False),
                                        (["spec/a.yaml"], "increment", False)]:
            self.assertEqual(gov.verification_policy(paths, boundary, safety), "full")

    def test_observed_claude_resets(self):
        now = dt.datetime(2026, 9, 19, tzinfo=dt.timezone.utc)
        for message, expected in [
            ("You've hit your session limit ? resets 3:40am (Europe/Warsaw)",
             "2026-09-19T01:40:30+00:00"),
            ("You've hit your session limit ? resets 5:30pm (Europe/Warsaw)",
             "2026-09-19T15:30:30+00:00"),
        ]:
            with self.subTest(message=message):
                self.assertEqual(gov.resume_after(message, now), expected)
                tomorrow = dt.datetime.fromisoformat(expected) + dt.timedelta(days=1)
                self.assertEqual(gov.resume_after(message, now + dt.timedelta(hours=18)),
                                 tomorrow.isoformat())
        near_reset = now.replace(hour=1, minute=39)
        self.assertEqual(gov.retry_delay(gov.resume_after(
            "resets 3:40am (Europe/Warsaw)", near_reset), near_reset), 300)

    def test_clock_resets_fail_closed(self):
        now = dt.datetime(2026, 9, 19, tzinfo=dt.timezone.utc)
        invalid = ["resets 3:40am (Unknown/Timezone)", "resets 3:40am (+02:00)",
                   "resets 3:40am", "resets 3:40 (Europe/Warsaw)",
                   "resets 13:40pm (Europe/Warsaw)", "resets 0:40am (Europe/Warsaw)",
                   "resets 3:60am (Europe/Warsaw)", "resets 3:4am (Europe/Warsaw)",
                   "resets 3:40am (Europe/Warsaw", "resets 3:40am (../Europe/Warsaw)"]
        for message in invalid:
            with self.subTest(message=message):
                self.assertIsNone(gov.resume_after(message, now))
                self.assertIsNone(gov.resume_after(
                    'reset_at: 2026-09-19T01:40:00Z ' + message, now))
        self.assertIsNone(gov.resume_after(
            "resets 3:40am (Europe/Warsaw) resets 5:30pm (Europe/Warsaw)", now))
        self.assertIsNone(gov.resume_after(
            "resets 3:40am (Europe/Warsaw) reset_at: 2026-09-19T02:40:00Z", now))
        self.assertEqual(gov.resume_after(
            "resets 3:40am (Europe/Warsaw) reset_at: 2026-09-19T01:40:00Z", now),
            "2026-09-19T01:40:30+00:00")

    def test_clock_reset_dst(self):
        for month, day in [(3, 29), (10, 25)]:
            now = dt.datetime(2026, month, day, tzinfo=dt.timezone.utc)
            self.assertIsNone(gov.resume_after("resets 2:30am (Europe/Warsaw)", now))
        for now, expected in [
            (dt.datetime(2026, 3, 28, 20, tzinfo=dt.timezone.utc), "2026-03-29T01:40:30+00:00"),
            (dt.datetime(2026, 10, 24, 20, tzinfo=dt.timezone.utc), "2026-10-25T02:40:30+00:00"),
        ]:
            self.assertEqual(gov.resume_after("resets 3:40am (Europe/Warsaw)", now), expected)

    def test_continue_example_has_valid_handoff(self):
        example = drv.CONTROL_INSTRUCTIONS.split(drv.BEGIN, 1)[1].split(drv.END, 1)[0]
        block = json.loads(example)
        gov.compact_handoff(block, True)
        self.assertEqual(set(block["handoff"]), set(control()["handoff"]))

    def loop(self, callback, state_extra=None, hold=False, breach=False, iterations=2,
             session_probe=lambda _: True, reset_due=False):
        with tempfile.TemporaryDirectory() as tmp, ExitStack() as stack:
            root = pathlib.Path(tmp)
            for name, val in {"ROOT": root, "STATE_DIR": root, "STATE_FILE": root / "state.json",
                              "LOG_FILE": root / "log.jsonl", "HOLD_FILE": root / "HOLD"}.items():
                stack.enter_context(patch.object(drv, name, val))
            stack.enter_context(patch.object(drv, "backlog_snapshot", return_value=[]))
            stack.enter_context(patch.object(drv, "session_exists", side_effect=session_probe))
            if reset_due:
                stack.enter_context(patch.object(gov, "retry_delay", return_value=0))
            stack.enter_context(patch.object(drv, "protected_worktree_breach", return_value=["x"] if breach else []))
            stack.enter_context(patch.object(drv.subprocess, "run", side_effect=lambda cmd, **kw:
                SimpleNamespace(returncode=0, stdout="a" * 40 if "rev-parse" in cmd else "")))
            state = dict(work_item="test", session_id="original", iteration=1, goal="one",
                         canonical_thread=drv.CANONICAL_THREAD, protected_worktree={}, next_action="next")
            state.update(state_extra or {})
            drv._save_state(state)
            if hold:
                drv.HOLD_FILE.write_text("hold")
            turn = stack.enter_context(patch.object(drv, "run_turn", side_effect=callback))
            sleep = stack.enter_context(patch.object(drv.time, "sleep", side_effect=lambda _: drv.HOLD_FILE.write_text("hold")))
            result = drv.drive(None, "test", iterations, None, 60, True)
            self.last_log = drv.LOG_FILE.read_text() if drv.LOG_FILE.exists() else ""
            return result, drv._load_state(), turn.call_args_list, sleep.call_args_list

    @staticmethod
    def success(b=None, turns=2):
        def callback(session, prompt, model, timeout, fresh_id=None):
            block = b if b is not None else control()
            return dict(ok=True, envelope=dict(session_id=fresh_id or session,
                        num_turns=turns, result=drv.BEGIN + json.dumps(block) + drv.END))
        return callback

    def test_rotation_and_pinned_identity(self):
        _, state, calls, _ = self.loop(self.success())
        self.assertEqual(calls[0].args[0], "original")
        self.assertIsNone(calls[1].args[0])
        self.assertTrue(calls[1].kwargs["fresh_id"])
        self.assertIn("Compact durable handoff", calls[1].args[1])
        self.assertTrue(state["fresh_session"])

    def test_pending_decision_stops_rotation_and_resume(self):
        b = control()
        b["handoff"]["authority_pending"] = True
        _, state, calls, _ = self.loop(self.success(b))
        self.assertEqual(len(calls), 1)
        self.assertEqual(state["session_id"], "original")
        self.assertEqual(state["status"], "PAUSED_GOVERNANCE")
        _, _, calls, _ = self.loop(self.success(), state)
        self.assertEqual(len(calls), 0)

    def test_legacy_authority_cannot_continue(self):
        b = control()
        del b["handoff"]
        b["reason"] = "founder_decision_required: unresolved"
        _, state, calls, _ = self.loop(self.success(b))
        self.assertEqual(state["status"], "PAUSED_GOVERNANCE")
        self.assertEqual(len(calls), 1)

    def test_legacy_threshold(self):
        b = control()
        del b["handoff"]
        _, state, calls, _ = self.loop(self.success(b, turns=44))
        self.assertEqual(len(calls), 1)
        self.assertEqual(state["status"], "PAUSED_GOVERNANCE")

    def test_rate_limit_unknown_is_persistent_pause(self):
        _, state, calls, sleeps = self.loop(lambda *a, **kw: dict(ok=False, error="usage_limit"))
        self.assertEqual(state["status"], "PAUSED_RATE_LIMIT")
        self.assertTrue(state["needs_reconciliation"])
        self.assertEqual(state["next_action"], "next")
        self.assertEqual(len(calls), 1)
        self.assertEqual(len(sleeps), 0)
        _, _, calls, _ = self.loop(self.success(), state)
        self.assertEqual(len(calls), 0)

    def test_wait_checks_guards_and_due_resume(self):
        future = (dt.datetime.now(dt.timezone.utc) + dt.timedelta(hours=1)).isoformat()
        _, _, calls, sleeps = self.loop(self.success(), dict(status="PAUSED_RATE_LIMIT", resume_after=future))
        self.assertEqual(len(calls), 0)
        self.assertEqual(sleeps[0].args, (30,))
        past = (dt.datetime.now(dt.timezone.utc) - dt.timedelta(seconds=1)).isoformat()
        _, _, calls, _ = self.loop(self.success(), dict(status="PAUSED_RATE_LIMIT", resume_after=past), iterations=1)
        self.assertEqual(len(calls), 1)

    def test_guards_win(self):
        for options in [dict(hold=True), dict(breach=True),
                        dict(state_extra={"canonical_thread": "wrong"})]:
            _, _, calls, _ = self.loop(self.success(), **options)
            self.assertEqual(len(calls), 0)

    def test_cli_error_envelopes_and_fresh_flags(self):
        for code in (0, 1):
            with patch.object(drv.subprocess, "run", return_value=SimpleNamespace(returncode=code,
                              stdout=json.dumps(dict(is_error=True, subtype="error", result="429 rate limit")), stderr="")) as run:
                result = drv.run_turn(None, "prompt", None, 60, fresh_id="pinned")
                self.assertEqual(result["error"], "usage_limit")
                self.assertIn("--session-id", run.call_args.args[0])
                self.assertNotIn("--resume", run.call_args.args[0])

    def test_no_product_dependency(self):
        tree = ast.parse(pathlib.Path(gov.__file__).read_text())
        modules = {node.names[0].name for node in ast.walk(tree) if isinstance(node, ast.Import)}
        self.assertEqual(modules, {"datetime", "json", "re", "zoneinfo"})
        self.assertFalse(any(isinstance(node, ast.ImportFrom) for node in ast.walk(tree)))

    def test_mismatch_and_missing_control_persist_pause(self):
        for envelope in [dict(session_id="wrong", num_turns=1, result=""),
                         dict(session_id="original", num_turns=1, result="no control")]:
            _, state, calls, _ = self.loop(lambda *a, **kw: dict(ok=True, envelope=envelope))
            self.assertEqual(state["status"], "PAUSED_GOVERNANCE")
            self.assertEqual(len(calls), 1)

    def test_oversize_desk_packet_rejected_without_losing_decisions(self):
        with self.assertRaises(ValueError):
            ns.build_desk_packet(work_item="test", head_sha="a", state="ready",
                evidence=["x" * 16000], decisions=["pending decision"], codex_status="pending",
                dispositions=[], risks=[], recommendation="review", next_actions=[],
                requires_ruling=True, nonce="n")

    def test_compact_independent_packet(self):
        with patch.object(ns, "_git", return_value="x" * 9000), patch.object(ns, "commits_between", return_value=["abc"]):
            packet = ns.build_review_packet("one", "base", "tip", ["spec reference"], ["boundary correct?"])
        self.assertIn("diff truncated", packet)
        self.assertNotIn("CLAUDE DECISIONS", packet)
        self.assertNotIn("preferred verdict", packet)
        self.assertLess(len(packet.encode()), 24000)

    def test_full_hash_instruction(self):
        self.assertIn("full 40-character", drv.CONTROL_INSTRUCTIONS)
        self.assertIn("git rev-parse HEAD", drv.CONTROL_INSTRUCTIONS)

    def test_error_evidence_excludes_result_and_counters(self):
        for envelope in [dict(subtype="error", result="HTTP 429 rate limit"),
                         dict(is_error=True, result="example HTTP 429"),
                         dict(is_error=True, result="example 429", duration_ms=429),
                         dict(is_error=True, session_id="abc-429-def", result="ordinary failure")]:
            self.assertEqual(drv.classify_exit(1, json.dumps(envelope), ""), "exit 1")
        for text in ["example 429", "counter=1429", "abc-429-def"]:
            self.assertEqual(drv.classify_exit(1, text, ""), "exit 1")
        for envelope in [dict(is_error=True, status_code=429),
                         dict(is_error=True, error="HTTP 429"),
                         dict(is_error=True, errors=["API Error: 429"]),
                         dict(is_error=True, result="You've hit your weekly limit")]:
            self.assertEqual(drv.classify_exit(1, json.dumps(envelope), ""), "usage_limit")

    def test_zone_filesystem_errors_fail_closed(self):
        for error in [IsADirectoryError, PermissionError, OSError]:
            with patch.object(gov.zoneinfo, "ZoneInfo", side_effect=error):
                self.assertIsNone(gov.resume_after("resets 3:40am (Europe)",
                    dt.datetime(2026, 9, 19, tzinfo=dt.timezone.utc)))

    def test_work_item_failures_are_durable(self):
        for status in ("continue", "stop"):
            for item in (None, "wrong"):
                b = control()
                b["status"] = status
                if item is None:
                    del b["work_item"]
                else:
                    b["work_item"] = item
                _, state, calls, _ = self.loop(self.success(b))
                self.assertEqual(state["status"], "PAUSED_GOVERNANCE")
                self.assertIn("work_item", state["governance_reason"])
                self.assertIn(state["governance_reason"], self.last_log)
                self.assertIn("reported_next_action", self.last_log)
                self.assertEqual(state["next_action"], "next")
                self.assertEqual(len(calls), 1)

    def test_non_ascii_diff_byte_budget(self):
        with patch.object(ns, "_git", side_effect=["files changed", "界😀" * 4000]), \
                patch.object(ns, "commits_between", return_value=["abc"]):
            packet = ns.build_review_packet("test", "base", "tip", [], [])
        self.assertIn("diff truncated", packet)
        self.assertLess(len(packet.encode()), 24000)
        self.assertNotIn("\ufffd", packet)

    def test_transcript_probe(self):
        sid = "11111111-1111-4111-8111-111111111111"
        with tempfile.TemporaryDirectory() as tmp, patch.dict(os.environ, {"CLAUDE_CONFIG_DIR": tmp}):
            self.assertFalse(drv.session_exists(sid))
            project = pathlib.Path(tmp) / "projects" / "project"
            project.mkdir(parents=True)
            transcript = project / (sid + ".jsonl")
            record = dict(type="user", sessionId=sid, cwd=str(drv.ROOT), message={})
            transcript.write_text(json.dumps(record))
            self.assertTrue(drv.session_exists(sid))
            for text in ["", "{broken", json.dumps(dict(record, sessionId="wrong")),
                         json.dumps(dict(record, cwd=tmp))]:
                transcript.write_text(text)
                with self.assertRaises(ValueError):
                    drv.session_exists(sid)
            transcript.write_text(json.dumps(record))
            second = project.parent / "duplicate"
            second.mkdir()
            (second / transcript.name).write_text(json.dumps(record))
            with self.assertRaises(ValueError):
                drv.session_exists(sid)

    def test_observed_quota_end_to_end_and_safe_fresh_retry(self):
        real_turn, real_probe = drv.run_turn, drv.session_exists
        sid = "11111111-1111-4111-8111-111111111111"
        # Exercise raw CLI failures and JSON error envelopes, not a mocked
        # usage_limit classification. The only simulated boundary is the CLI.
        for message, persisted in [
                ("You've hit your session limit ?? resets 3:40am (Europe/Warsaw)", False),
                ("You've hit your session limit ?? resets 5:30pm (Europe/Warsaw)", False),
                ("HTTP 429 reset_at: 2099-09-19T00:00:00Z", False),
                ("HTTP 429 reset_at: 2099-09-19T00:00:00Z", True)]:
            for code in (0, 1):
                with self.subTest(message=message, code=code), tempfile.TemporaryDirectory() as tmp, \
                        patch.dict(os.environ, {"CLAUDE_CONFIG_DIR": tmp}):
                    commands, implementations = [], []
                    def cli(cmd, **kw):
                        commands.append(cmd)
                        if len(commands) == 1:
                            if persisted:
                                path = pathlib.Path(tmp) / "projects" / "project" / (sid + ".jsonl")
                                path.parent.mkdir(parents=True)
                                path.write_text(json.dumps(dict(type="user", sessionId=sid, cwd=str(drv.ROOT))))
                            return SimpleNamespace(returncode=code, stderr="", stdout=(message if code else
                                json.dumps(dict(is_error=True, subtype="error", session_id=sid,
                                                result=message, error=message if message.startswith("HTTP") else ""))))
                        paused = drv._load_state()
                        self.assertTrue(paused["needs_reconciliation"])
                        self.assertIsNotNone(paused["resume_after"])
                        self.assertIn("do NOT repeat", kw["input"])
                        self.assertEqual(paused["next_action"], "next")
                        implementations.append("one bounded increment")
                        b = dict(status="stop", work_item="test", reason="milestone_complete", next_action="")
                        return SimpleNamespace(returncode=0, stderr="", stdout=json.dumps(dict(
                            subtype="success", session_id=sid, result=drv.BEGIN + json.dumps(b) + drv.END)))
                    def turn(*a, **kw):
                        with patch.object(drv.subprocess, "run", side_effect=cli):
                            return real_turn(*a, **kw)
                    result, state, calls, _ = self.loop(turn,
                        dict(session_id=sid, fresh_session=True), session_probe=real_probe, reset_due=True)
                    self.assertEqual(result, 0)
                    self.assertEqual(state["status"], "STOPPED")
                    self.assertIn('"event": "paused"', self.last_log)
                    self.assertEqual(len(implementations), 1)
                    self.assertEqual(len(commands), 2)
                    for index, cmd in enumerate(commands):
                        resume = persisted and index == 1
                        self.assertIn("--resume" if resume else "--session-id", cmd)
                        self.assertNotIn("--session-id" if resume else "--resume", cmd)
                        self.assertIn(sid, cmd)

    def test_interrupted_first_launch_rechecks_creation_on_restart(self):
        # The state persisted before subprocess launch is sufficient even if
        # the process dies before receiving any CLI envelope.
        for exists in (False, True):
            _, _, calls, _ = self.loop(self.success(),
                dict(session_launch_pending=True, fresh_session=True, needs_reconciliation=True),
                session_probe=lambda _: exists, iterations=1)
            self.assertEqual(calls[0].args[0], "original" if exists else None)
            self.assertEqual(calls[0].kwargs["fresh_id"], None if exists else "original")
            self.assertIn("do NOT repeat", calls[0].args[1])

    def test_created_session_then_quota_resumes_without_reimplementation(self):
        real_turn, real_probe = drv.run_turn, drv.session_exists
        sid = "11111111-1111-4111-8111-111111111111"
        with tempfile.TemporaryDirectory() as tmp, patch.dict(os.environ, {"CLAUDE_CONFIG_DIR": tmp}):
            commands = []
            durable = pathlib.Path(tmp) / "implementation"
            def cli(cmd, **kw):
                commands.append(cmd)
                if len(commands) == 1:
                    durable.write_text("completed once")
                    path = pathlib.Path(tmp) / "projects" / "project" / (sid + ".jsonl")
                    path.parent.mkdir(parents=True)
                    path.write_text(json.dumps(dict(type="user", sessionId=sid, cwd=str(drv.ROOT))))
                    # Existing legacy continuation stays below its rotation threshold.
                    b = dict(status="continue", work_item="test", reason="done", next_action="verify")
                elif len(commands) == 2:
                    return SimpleNamespace(returncode=1, stdout="HTTP 429 reset_at: 2099-09-19T00:00:00Z", stderr="")
                else:
                    self.assertIn("do NOT repeat", kw["input"])
                    self.assertEqual(durable.read_text(), "completed once")
                    b = dict(status="stop", work_item="test", reason="milestone_complete", next_action="")
                return SimpleNamespace(returncode=0, stderr="", stdout=json.dumps(dict(
                    subtype="success", session_id=sid, num_turns=1, result=drv.BEGIN + json.dumps(b) + drv.END)))
            def turn(*a, **kw):
                with patch.object(drv.subprocess, "run", side_effect=cli):
                    return real_turn(*a, **kw)
            result, state, _, _ = self.loop(turn, dict(session_id=sid, fresh_session=True),
                                          session_probe=real_probe, reset_due=True)
            self.assertEqual(result, 0)
            self.assertEqual(state["status"], "STOPPED")
            self.assertIn("--session-id", commands[0])
            self.assertEqual(len(commands), 3)
            for cmd in commands[1:]:
                self.assertIn("--resume", cmd)
                self.assertNotIn("--session-id", cmd)
                self.assertIn(sid, cmd)

    def test_recovery_guards_and_ambiguity(self):
        recovery = dict(status="PAUSED_RATE_LIMIT", resume_after="2000-01-01T00:00:00Z",
                        session_launch_pending=True)
        for options in [dict(hold=True), dict(breach=True)]:
            probe = unittest.mock.Mock(side_effect=AssertionError("guard must win"))
            _, _, calls, _ = self.loop(self.success(), recovery, session_probe=probe, **options)
            self.assertFalse(calls)
            probe.assert_not_called()
        for error in (PermissionError, ValueError):
            _, state, calls, _ = self.loop(self.success(), recovery,
                session_probe=unittest.mock.Mock(side_effect=error))
            self.assertFalse(calls)
            self.assertEqual(state["status"], "PAUSED_GOVERNANCE")
            self.assertIn("session recovery", state["governance_reason"])

    def test_error_envelope_identity_mismatch_wins_over_quota(self):
        for code in (0, 1):
            with patch.object(drv.subprocess, "run", return_value=SimpleNamespace(returncode=code,
                    stdout=json.dumps(dict(is_error=True, session_id="wrong", result="HTTP 429")), stderr="")):
                outcome = drv.run_turn(None, "prompt", None, 60, fresh_id="pinned")
            self.assertEqual(outcome["error"], "session identity mismatch")
            _, state, calls, _ = self.loop(lambda *a, **kw: outcome)
            self.assertEqual(state["status"], "PAUSED_GOVERNANCE")
            self.assertEqual(len(calls), 1)


if __name__ == "__main__":
    unittest.main()
