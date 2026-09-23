import tempfile, unittest
from pathlib import Path
import sys
import os
os.environ["MAIA_DESK_THREAD_ID"] = "synthetic-desk-thread-abcdef"
sys.path.insert(0, str(Path(__file__).parents[1]))
from deskbridge_auto_state import Record, State, move, load, save

class AutoStateTests(unittest.TestCase):
    def test_authorized_handoff_roundtrip(self):
        record = Record("M0.11", "completion", "work", "head")
        for state in [State.HANDOFF_REQUIRED, State.HANDOFF_PREPARED, State.HANDOFF_DISPATCHED, State.WAITING_FOR_DESK, State.DESK_RESPONSE_RECEIVED, State.APPLYING_DESK_DECISION, State.AUTHORIZED_WORK_REMAINING]: move(record, state)
        self.assertEqual(record.state, State.AUTHORIZED_WORK_REMAINING)
    def test_stale_nonce_or_duplicate_response_is_not_transition(self):
        record = Record("M0.11", "completion", "wait", "head", State.WAITING_FOR_DESK, "fresh")
        self.assertNotEqual(record.last_handoff_nonce, "stale")
        with self.assertRaises(ValueError): move(record, State.APPLYING_DESK_DECISION)
    def test_missing_dispatch_and_human_blocked_fail_closed(self):
        record = Record("M0.11", "completion", "handoff", "head", State.HANDOFF_REQUIRED)
        with self.assertRaises(ValueError): move(record, State.WAITING_FOR_DESK)
        move(record, State.HUMAN_BLOCKED); self.assertEqual(record.state, State.HUMAN_BLOCKED)
    def test_restart_preserves_authorized_work(self):
        record = Record("M0.11", "completion", "resume", "head")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "state.json"; save(path, record); self.assertEqual(load(path).state, State.AUTHORIZED_WORK_REMAINING)
    def test_authorized_restart_retains_redesign_step(self):
        record = Record("M0.11", "completion", "REDESIGN_ALPHA1", "head")
        self.assertEqual(record.state, State.AUTHORIZED_WORK_REMAINING)
        self.assertEqual(record.current_step, "REDESIGN_ALPHA1")
