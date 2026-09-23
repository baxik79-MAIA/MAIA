#!/usr/bin/env python3
"""MAIA autonomous session continuation driver.

Closes the gap where a Claude Code turn ends with a known next action and then
waits for a human to press Enter. The driver starts the next turn itself.

MECHANISM (preferred option 1: native, no wrapper hacks, no bypass)

  claude -p --resume <session-id> --output-format json
         --permission-mode acceptEdits
         --permission-prompts none
         --allowedTools <explicit scoped allowlist>

Verified locally on Claude Code 2.1.273:
  * `--resume <id>` carries full conversation context across turns and returns a
    stable session_id, so a turn can be started programmatically;
  * `--permission-prompts none` DENIES anything that would prompt rather than
    approving it. Combined with an explicit `--allowedTools`, that yields
    "pre-authorised operations only, everything else denied". This is an
    explicit grant, not a permission bypass: `bypassPermissions` and
    `--dangerously-skip-permissions` are never used, and a Write outside the
    allowlist was observed being denied and recorded in `permission_denials`.

CONTROL PROTOCOL

The driven turn must end its reply with a control block:

    <<<MAIA_CONTINUATION>>>
    {"status": "continue"|"stop",
     "work_item": "<id>",
     "reason": "<why>",
     "next_action": "<what the next turn should do>"}
    <<<END_MAIA_CONTINUATION>>>

A missing, malformed or ambiguous block stops the loop. The driver never infers
intent from prose: if the turn did not say clearly that it is safe to continue,
it is not safe to continue.

STOP CONDITIONS
  founder_decision_required | desk_escalation_required | authority_boundary |
  destructive_action | missing_access | milestone_complete  (declared by the turn)
  HOLD file | max iterations | budget | session identity mismatch |
  transport error | control block absent | backlog guard tripped  (driver-side)

Explicitly NOT stop conditions: a finished conversational turn, a report sent to
the Desk, or passing tests.

SAFETY
  * development-infrastructure only; not part of any Rust build graph, so it
    cannot enter a DEPLOYMENT_LOCKED artifact;
  * never uses bypassPermissions or --dangerously-skip-permissions;
  * grants no execution authority beyond the explicit allowlist below;
  * `git push` is deliberately absent from the allowlist;
  * refuses to run nested inside an existing Claude Code session unless forced;
  * guards the seven historical desk_outbox pending_*.md files and stops if any
    of them is dispatched;
  * logs every automatic continuation with work item, session id, timestamp and
    reason;
  * fails closed whenever session or thread identity is uncertain.

Usage:
  python tools/maia_continuation_driver.py --start --work-item M0.14 \\
      --goal "Implement M0.14 item 1: composition layer" --max-iterations 8
  python tools/maia_continuation_driver.py --resume --max-iterations 5
  python tools/maia_continuation_driver.py --hold        # engage kill switch
  python tools/maia_continuation_driver.py --release     # disengage
  python tools/maia_continuation_driver.py --status
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import os
import pathlib
import re
import stat
import subprocess
import sys
import uuid
import time

import night_shift_governance as governance
import desk_autoloop as desk

ROOT = pathlib.Path(__file__).resolve().parents[1]
STATE_DIR = ROOT / "reports" / "roundtable" / "continuation"
STATE_FILE = STATE_DIR / "state.json"
LOG_FILE = STATE_DIR / "continuation_log.jsonl"
HOLD_FILE = STATE_DIR / "HOLD"

DESK_OUTBOX = ROOT / ".local" / "desk_outbox"
CANONICAL_THREAD = os.environ.get("MAIA_DESK_THREAD_ID", "")

# Per-turn transport timeout.
#
# Measured, not guessed: M0.14 iteration 1 was a legitimate engineering turn that
# took 896,219 ms and completed correctly, leaving only about 4 seconds of
# headroom under the previous 900 s ceiling. Iteration 2 then halted with
# "transport: timeout" although its work had in fact completed, so the ceiling
# was misclassifying real work as failure. 1800 s is roughly twice the longest
# observed legitimate turn while staying bounded and fail-closed: a turn that
# genuinely hangs is still killed, it is simply no longer confused with one that
# is merely slow.
#
# This is the DRIVER transport timeout only. It has no bearing on MAIA product
# or provider timeouts, Ollama policy, or briefing policy.
DEFAULT_TURN_TIMEOUT_SECONDS = 1800

# Longest legitimate turn observed so far, kept so the headroom check in the
# driver test suite is anchored to evidence rather than to a round number.
LONGEST_OBSERVED_LEGITIMATE_TURN_MS = 896_219

BEGIN = "<<<MAIA_CONTINUATION>>>"
END = "<<<END_MAIA_CONTINUATION>>>"

DECLARED_STOPS = {
    "founder_decision_required",
    "desk_escalation_required",
    "authority_boundary",
    "destructive_action",
    "missing_access",
    "milestone_complete",
}

# Explicit, auditable grant. Everything absent from this list is DENIED by
# --permission-prompts none. `git push` is deliberately not here: the driver
# must never publish. Nor is unscoped Bash.
ALLOWED_TOOLS = [
    "Read",
    "Glob",
    "Grep",
    "Edit",
    "Write",
    "Bash(cargo *)",
    "Bash(python tools/*)",
    "Bash(git status*)",
    "Bash(git diff*)",
    "Bash(git log*)",
    "Bash(git add*)",
    "Bash(git commit*)",
    "Bash(git show*)",
]

RECONCILIATION_PREAMBLE = """
RECOVERY CONTEXT: the previous launch was interrupted or quota-limited.
A timeout means the driver stopped waiting. It does NOT mean the work failed, so
the stored next action may already be complete.

Before doing any implementation work, reconcile against durable evidence:
  - `git log --oneline -8` and `git show --stat HEAD` for what is committed;
  - the relevant tests, to confirm that work is sound;
  - reports/roundtable/ledger plus the desk outbox and inbox for what was sent.

Do NOT infer completion from a filename alone; use commit and test evidence.

If the stored next action is already done, say so in your reason, do NOT repeat
it, and continue with the genuinely next step. If it is only partly done, finish
it rather than restarting it.
""".strip()

CONTROL_INSTRUCTIONS = f"""
You are running under the MAIA autonomous continuation driver. Your turn will be
followed automatically by another turn; no human will press Enter.

Perform exactly ONE bounded increment: understand -> implement -> targeted test
-> commit -> queue independent review/report -> compact handoff -> return control.
Do not push. Repository artifacts, not conversation history, carry durable state.
Use targeted checks for ordinary increments. Full workspace/spec/clippy/fmt/D009
checks remain required at submilestone/promotion boundaries, safety-critical or
dependency-boundary changes. Record exact test commands/results in the report.
Use night_shift_governance.verification_policy on changed paths; it is a minimum.
Never omit a required gate merely to meet the turn threshold.
Keep review/Desk packets compact: commit range, file scope, tests, risks, questions
and evidence paths, not logs/history. Do not seed Codex with your verdict.
For continue, add a handoff object to the control block with:
 boundary: increment or submilestone; increment: current increment identifier;
 committed, review_queued, report_queued: true only with durable evidence;
 authority_pending, ambiguous_state: false only when resolved;
 recent_commits: up to 12 full 40-character hashes (include current HEAD from
 `git rev-parse HEAD`, never an abbreviated git log hash);
 unresolved_review_findings: up to 12 concise findings or repository references;
 architectural_constraints: up to 12 constraints/references required next time.
Each list entry <=512 characters; handoff <=8192 bytes. Include all unresolved
Founder/Desk decisions in durable artifacts and STOP at their authority boundary.
Never claim a safe boundary while a decision or partial implementation is unclear.

End your reply with one control block and nothing after it. For continuation,
use this complete shape, replacing placeholders with durable evidence. Every
continue block MUST include all handoff fields; never emit a legacy continue
block without handoff:

{BEGIN}
{{"status": "continue",
 "work_item": "<work item id>",
 "reason": "<one line: what you just did, or why you are stopping>",
 "next_action": "<one line: what the next turn should do>",
 "handoff": {{
   "boundary": "increment",
   "increment": "<current increment identifier>",
   "committed": true,
   "review_queued": true,
   "report_queued": true,
   "authority_pending": false,
   "ambiguous_state": false,
   "recent_commits": ["<full 40-character HEAD hash from git rev-parse HEAD>"],
   "unresolved_review_findings": [],
   "architectural_constraints": ["<required constraint or repository reference>"]
 }}}}
{END}

A stop block uses the same delimiters, status "stop", work_item, reason and
next_action (empty if stopping); non-Desk stops may omit handoff.
For desk_escalation_required, include the decision question in reason (<=2048
characters), proposed next_action (<=1024), and the SAME handoff structure above.
Use full 40-character commit hashes and truthful boundary flags; authority_pending
may be true. Add material_evidence (nonempty inline facts, <=2048 characters) to
handoff; local paths alone are insufficient. The entire handoff is <=8192 bytes.
The driver sends the correlated packet and consumes a structured Desk ruling;
do not send the same escalation yourself. Desk rulings do not grant product
execution authority or waive Founder decisions, permissions or session limits.

Use status "stop" only for a genuine stop condition:
  founder_decision_required, desk_escalation_required, authority_boundary,
  destructive_action, missing_access, milestone_complete
and put that exact token at the start of `reason`.

Finishing a turn, sending a Desk report, or passing tests are NOT stop
conditions. If you know the next action, say continue and name it.
If the block is missing or malformed the driver stops, so emit it exactly.
""".strip()


def _utc() -> str:
    return _dt.datetime.now(_dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _load_state() -> dict:
    if not STATE_FILE.is_file():
        return {}
    try:
        return json.loads(STATE_FILE.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}


def _save_state(state: dict) -> None:
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    tmp = STATE_FILE.with_suffix(".tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        json.dump(state, handle, indent=2)
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(STATE_FILE)


def _log(record: dict) -> None:
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    record["timestamp_utc"] = _utc()
    with LOG_FILE.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(record) + "\n")


def backlog_snapshot() -> list[str]:
    """The historical pending_*.md files that must never be auto-dispatched."""
    if not DESK_OUTBOX.is_dir():
        return []
    return sorted(p.name for p in DESK_OUTBOX.glob("pending_*.md"))


def uncommitted_paths() -> list[str]:
    """Paths currently modified or untracked in the working tree."""
    try:
        # Explicit encoding: text=True would use the host locale codec, which
        # cannot decode non-ASCII paths or diff content and crashes the reader
        # thread, leaving stdout as None.
        out = subprocess.run(["git", "status", "--porcelain"], capture_output=True,
                             text=True, encoding="utf-8", errors="replace",
                             cwd=str(ROOT), timeout=60).stdout or ""
    except (OSError, subprocess.SubprocessError):
        return []
    return sorted(line[3:].strip() for line in out.splitlines() if line.strip())


def worktree_diff(path: str) -> str:
    """Unified diff of one path against HEAD, or '' if unavailable."""
    try:
        return subprocess.run(["git", "diff", "--", path], capture_output=True,
                              text=True, encoding="utf-8", errors="replace",
                              cwd=str(ROOT), timeout=60).stdout or ""
    except (OSError, subprocess.SubprocessError):
        return ""


def added_lines(diff_text: str) -> list[str]:
    """The '+' lines of a diff, which are the content being protected."""
    return [l for l in diff_text.splitlines()
            if l.startswith("+") and not l.startswith("+++")]


def protected_snapshot(paths: list[str]) -> dict:
    """Capture protected paths AND the specific content that must stay uncommitted."""
    return {p: added_lines(worktree_diff(p)) for p in paths}


def protected_worktree_breach(protected: dict | list) -> list[str]:
    """Pre-existing uncommitted work that has since been committed away.

    The driver may commit its own work, but it must never sweep up somebody
    else's in-progress changes. I made exactly that mistake by hand earlier in
    this project by staging a shared lockfile wholesale, and on 2026-09-17 a
    driven turn repeated it with the same file, so the failure is observed twice
    rather than theoretical.

    Checked at content level, not only path level. A path-level check asks "is
    this file still modified", which misses the case where a turn commits the
    protected hunk while leaving some other edit to the same file uncommitted:
    the path still looks dirty, but the protected content is gone. Shared
    lockfiles are exactly where that happens, because legitimate new work and
    somebody else's pending work land in one file.
    """
    # Legacy list form: fall back to the coarse check rather than silently
    # passing, so an older state file still fails closed.
    if isinstance(protected, list):
        still = set(uncommitted_paths())
        return [p for p in protected if p not in still]

    breached = []
    for path, protected_lines in protected.items():
        if not protected_lines:
            # Untracked at capture time: presence is the whole guarantee.
            if path not in set(uncommitted_paths()):
                breached.append(path)
            continue
        current = set(added_lines(worktree_diff(path)))
        if any(line not in current for line in protected_lines):
            breached.append(path)
    return breached


def extract_control(text: str) -> tuple[dict | None, str]:
    """Return the control block, or None with a reason. Never guesses."""
    if BEGIN not in text or END not in text:
        return None, "control block absent"
    start = text.rindex(BEGIN) + len(BEGIN)
    end = text.rindex(END)
    if end <= start:
        return None, "control block malformed"
    try:
        block = json.loads(text[start:end].strip())
    except json.JSONDecodeError:
        return None, "control block is not valid JSON"
    if not isinstance(block, dict):
        return None, "control block is not an object"
    status = block.get("status")
    if status not in {"continue", "stop"}:
        return None, f"control block status is not continue/stop: {status!r}"
    return block, ""


# Phrases that mean "the account ran out of allowance", not "the code is wrong".
USAGE_LIMIT_MARKERS = (
    "usage limit",
    "rate limit",
    "out of credits",
    "quota",
    "too many requests",
    "limit reached",
    "resets at",
    "hit your limit",
    "session limit",
    "weekly limit",
)


def error_evidence(stdout: str, stderr: str) -> tuple[str, str]:
    """Separate CLI error diagnostics from arbitrary result text and telemetry."""
    try:
        envelope = json.loads(stdout)
    except ValueError:
        return stdout + "\n" + stderr, stdout + "\n" + stderr
    if not isinstance(envelope, dict):
        return stderr, stderr
    diagnostic = "\n".join(json.dumps(envelope[k]) for k in
                           ("error", "errors", "status", "status_code") if k in envelope)
    # CLI quota messages can occupy result, but only on an explicit error.
    result = str(envelope.get("result", "")) if envelope.get("is_error") is True else ""
    return diagnostic + "\n" + result + "\n" + stderr, diagnostic + "\n" + stderr


def classify_exit(returncode: int, stdout: str, stderr: str) -> str:
    """Name the failure, so a spent allowance is not read as a broken driver.

    M0.14 iterations 3 and 4 both died with a bare "exit 1" and no output, and
    the cause turned out to be an exhausted claude.ai usage limit rather than
    anything wrong with the transport. Reported as a generic transport fault,
    that invites exactly the wrong response: re-running, re-diagnosing, or
    changing working code. Waiting is the only fix, so the driver should say so.
    """
    text, diagnostic = error_evidence(stdout, stderr)
    blob = text.lower()
    if (any(marker in blob for marker in USAGE_LIMIT_MARKERS)
            or re.search(r"\bhit your (?:[a-z]+\s+){0,3}limit\b", blob)
            or re.search(r'^\s*"?429"?\s*$', diagnostic, re.M)
            or re.search(r"\b(?:http(?:/\d(?:\.\d)?)?|status(?:_code)?|api error)\s*[:=]?\s*429\b", diagnostic, re.I)):
        return "usage_limit"
    return f"exit {returncode}"


def session_exists(session_id: str) -> bool:
    """Probe documented CLI JSONL storage; ambiguous evidence raises, never guesses.

    Only inspect the pinned ID, never emit transcript content. Searching project
    directories avoids depending on CLI path escaping or long-path hashing.
    """
    uuid.UUID(session_id)  # reject path traversal and malformed persisted IDs
    projects = pathlib.Path(os.environ.get("CLAUDE_CONFIG_DIR", pathlib.Path.home() / ".claude")) / "projects"
    try:
        directories = list(projects.iterdir())
    except FileNotFoundError:
        return False
    found = []
    for directory in directories:
        if not stat.S_ISDIR(directory.stat().st_mode):
            continue
        path = directory / (session_id + ".jsonl")
        try:
            handle = path.open(encoding="utf-8")
        except FileNotFoundError:
            continue
        messages = 0
        with handle:
            for line in handle:
                record = json.loads(line)
                if not isinstance(record, dict):
                    raise ValueError("invalid session transcript")
                if record.get("sessionId", session_id) != session_id:
                    raise ValueError("transcript session identity mismatch")
                if record.get("type") in ("user", "assistant"):
                    if (record.get("sessionId") != session_id
                            or not isinstance(record.get("cwd"), str)
                            or pathlib.Path(record.get("cwd", "")).resolve() != ROOT.resolve()):
                        raise ValueError("transcript worktree/session identity mismatch")
                    messages += 1
        if not messages:
            raise ValueError("session transcript has no resumable messages")
        found.append(path)
    if len(found) > 1:
        raise ValueError("duplicate session transcripts")
    return bool(found)


def run_turn(session_id: str | None, prompt: str, model: str | None,
             timeout: int, fresh_id: str | None = None) -> dict:
    """One driven Claude turn. Never raises on a provider failure."""
    cmd = ["claude", "-p", "--output-format", "json",
           "--permission-mode", "acceptEdits",
           "--permission-prompts", "none"]
    if fresh_id:
        cmd += ["--session-id", fresh_id]
    if session_id:
        cmd += ["--resume", session_id]
    if model:
        cmd += ["--model", model]
    # --allowedTools is variadic, so a trailing positional prompt is swallowed
    # as another tool name and the CLI then reports no input. Deliver the prompt
    # on stdin instead, which also sidesteps command-line length limits, exactly
    # as the M0.12 provider does.
    cmd += ["--allowedTools", *ALLOWED_TOOLS]

    try:
        proc = subprocess.run(cmd, input=prompt, capture_output=True, text=True,
                              encoding="utf-8", errors="replace",
                              timeout=timeout, cwd=str(ROOT))
    except subprocess.TimeoutExpired:
        return {"ok": False, "error": "timeout"}
    try:
        returned = json.loads(proc.stdout)
    except ValueError:
        returned = None
    if (isinstance(returned, dict) and "session_id" in returned
            and returned["session_id"] != (fresh_id or session_id)):
        return {"ok": False, "error": "session identity mismatch"}
    if proc.returncode != 0:
        # Capture BOTH streams. The first version kept only stderr, and when
        # iterations 3 and 4 of the M0.14 run died with exit 1 the log recorded
        # no detail at all, because the CLI had written nothing to stderr. A
        # halt whose cause cannot be read afterwards is barely better than no
        # halt, so keep whatever either stream said.
        out = (proc.stdout or "")[-800:]
        err = (proc.stderr or "")[-800:]
        return {"ok": False, "error": classify_exit(proc.returncode, proc.stdout or "", proc.stderr or ""),
                "stdout": out, "stderr": err,
                "resume_after": governance.resume_after(
                    error_evidence(proc.stdout or "", proc.stderr or "")[0],
                    _dt.datetime.now(_dt.timezone.utc))}
    try:
        envelope = json.loads(proc.stdout)
    except json.JSONDecodeError:
        return {"ok": False, "error": "non-JSON envelope",
                "stdout": (proc.stdout or "")[-600:]}
    if not isinstance(envelope, dict):
        return {"ok": False, "error": "non-object envelope"}
    if envelope.get("is_error") or envelope.get("subtype") != "success":
        blob = json.dumps(envelope)
        limited = classify_exit(1, blob, "") == "usage_limit"
        return {"ok": False, "error": "usage_limit" if limited else
                f"cli subtype {envelope.get('subtype')}",
                "resume_after": governance.resume_after(error_evidence(blob, "")[0], _dt.datetime.now(_dt.timezone.utc))}
    return {"ok": True, "envelope": envelope}


def drive(*args, **kwargs) -> int:
    try:
        with desk.lease():
            return _drive(*args, **kwargs)
    except OSError as exc:
        print(f"HALTED: lease or durable I/O unavailable: {exc}", file=sys.stderr)
        return 1


def desk_guard(state):
    if HOLD_FILE.exists():
        raise ValueError("kill switch")
    if not CANONICAL_THREAD or state.get("canonical_thread") != CANONICAL_THREAD:
        raise ValueError("canonical thread uncertainty")
    if protected_worktree_breach(state.get("protected_worktree", [])):
        raise ValueError("protected worktree breach")
    if set(state.get("backlog", [])) - set(backlog_snapshot()):
        raise ValueError("historical backlog breach")


def rotate_session(state, block):
    """One verified-boundary rotation path for continuation and Desk rulings."""
    desk_guard(state)
    handoff = governance.compact_handoff(block, not protected_worktree_breach(
        state.get("protected_worktree", [])))
    # Git errors or uncommitted increment work prohibit rotation.
    status = subprocess.run(["git", "status", "--porcelain"], cwd=str(ROOT),
                            capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=60)
    own_paths = {str(p.relative_to(ROOT)).replace("\\", "/")
                 for p in (STATE_FILE, LOG_FILE)}
    exchanges = state.get("desk_history", []) + ([state["desk_exchange"]]
                if state.get("desk_exchange") else [])
    own_paths.update("reports/roundtable/ledger/" + e["nonce"] + ".json"
                     for e in exchanges)
    dirty = [line[3:].strip() for line in status.stdout.splitlines()
             if line[3:].strip() not in own_paths]
    if status.returncode or any(p not in state.get("protected_worktree", []) for p in dirty):
        raise ValueError("increment worktree not committed")
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=str(ROOT),
                          capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=60)
    if head.returncode or head.stdout.strip() not in json.loads(handoff)["recent_commits"]:
        raise ValueError("handoff must include exact HEAD")
    state["handoff"] = handoff
    _log({"event": "session_rotation", "work_item": state["work_item"],
          "previous_session_id": state["session_id"], "iteration": state["iteration"],
          "reason": "verified increment boundary"})
    state["previous_session_id"] = state["session_id"]
    session_id = str(uuid.uuid4())
    state["session_id"] = session_id
    state["fresh_session"] = True
    state["session_turns"] = 0
    state.pop("governance_reason", None)


def _drive(goal: str | None, work_item: str, max_iterations: int,
          model: str | None, timeout: int, allow_nested: bool,
          turn_threshold: int = governance.DEFAULT_TURN_THRESHOLD) -> int:
    if os.environ.get("CLAUDECODE") and not allow_nested:
        print("FAIL: refusing to run nested inside an active Claude Code session.\n"
              "  The driver is the parent of the turns it drives. Run it from an\n"
              "  ordinary shell, or pass --allow-nested if you know why you want this.",
              file=sys.stderr)
        return 2

    state = _load_state()
    if goal is not None:
        if state.get("desk_exchange") and state["desk_exchange"]["phase"] != "applied":
            print("HALTED: unresolved Desk exchange; reconcile before starting fresh.")
            return 1
        history = state.get("desk_history", []) + ([state["desk_exchange"]]
                  if state.get("desk_exchange") else [])
        # Starting fresh: pin a session id so identity is verifiable later.
        state = {
            "work_item": work_item,
            "session_id": str(uuid.uuid4()),
            "goal": goal,
            "iteration": 0,
            "fresh_session": True,
            "started_utc": _utc(),
            "canonical_thread": CANONICAL_THREAD,
            "backlog": backlog_snapshot(),
            "protected_worktree": protected_snapshot(uncommitted_paths()),
            "desk_history": history,
        }
        _save_state(state)
    if not state.get("session_id"):
        print("FAIL: no session to resume; use --start with --goal.", file=sys.stderr)
        return 2

    if not CANONICAL_THREAD or state.get("canonical_thread") != CANONICAL_THREAD:
        print("HALTED: canonical thread identity mismatch", file=sys.stderr)
        return 1
    session_id = state["session_id"]
    protected_backlog = state.get("backlog", [])
    first = state["iteration"] == 0
    next_action = state.get("next_action") or state.get("goal", "")

    attempts = 0
    while attempts < max_iterations or state.get("desk_exchange", {}).get("phase") in (
            "prepared", "dispatched", "ruled"):
        # --- kill switch, checked before every turn -----------------------
        if HOLD_FILE.is_file():
            reason = HOLD_FILE.read_text(encoding="utf-8").strip() or "HOLD engaged"
            _log({"event": "halt", "work_item": state["work_item"],
                  "session_id": session_id, "iteration": state["iteration"],
                  "reason": f"kill switch: {reason}"})
            print(f"HALTED by kill switch: {reason}")
            return 0

        # --- historical backlog guard -------------------------------------
        current = backlog_snapshot()
        missing = [name for name in protected_backlog if name not in current]
        if missing:
            _log({"event": "halt", "work_item": state["work_item"],
                  "session_id": session_id, "iteration": state["iteration"],
                  "reason": f"backlog guard: historical pending files dispatched {missing}"})
            print(f"HALTED: historical pending files were dispatched: {missing}",
                  file=sys.stderr)
            return 1

        # --- unrelated working-tree guard ---------------------------------
        breach = protected_worktree_breach(state.get("protected_worktree", []))
        if breach:
            _log({"event": "halt", "work_item": state["work_item"],
                  "session_id": session_id, "iteration": state["iteration"],
                  "reason": f"worktree guard: pre-existing uncommitted work was committed {breach}"})
            print("HALTED: pre-existing uncommitted work was swept into a commit:\n  "
                  + "\n  ".join(breach)
                  + "\n  Inspect with `git show HEAD` before continuing.", file=sys.stderr)
            return 1

        if state.get("status") == "PAUSED_RATE_LIMIT":
            state["session_launch_pending"] = True
            delay = governance.retry_delay(state.get("resume_after"), _dt.datetime.now(_dt.timezone.utc))
            if delay is None:
                print("PAUSED_RATE_LIMIT: no trustworthy reset time; preserve state for reconciliation.")
                return 0
            if delay > 0:
                time.sleep(min(delay, 30))
                continue  # all guards run again, without consuming an iteration
        if state.get("desk_exchange", {}).get("phase") in ("prepared", "dispatched", "ruled"):
            if not desk.advance(state, _save_state, lambda: desk_guard(state)):
                print("STOPPED: Desk exchange requires reconciliation or declined continuation.")
                return 0
            next_action = state["next_action"]
        if (state.get("desk_exchange", {}).get("phase") == "applied"
                and state.get("status") == "RUNNING"
                and state.get("session_turns", 0) >= turn_threshold):
            try:
                item = state["desk_exchange"]
                if item["ruling"]["decision"] not in ("ACCEPT", "AMEND"):
                    raise ValueError("Desk continuation not authorized")
                handoff = json.loads(item["handoff"])
                if (handoff["work_item"] != state["work_item"]
                        or state["next_action"] != item["ruling"]["authorized_next_action"]):
                    raise ValueError("Desk boundary identity mismatch")
                # Only this verified Desk authority boundary is now resolved.
                handoff["authority_pending"] = False
                rotate_session(state, dict(status="continue", reason="Desk boundary resolved",
                    work_item=state["work_item"], next_action=state["next_action"], handoff=handoff))
                session_id = state["session_id"]
                state["needs_reconciliation"] = True
            except (ValueError, KeyError, TypeError, OSError, subprocess.SubprocessError) as exc:
                state["status"] = "PAUSED_GOVERNANCE"
                state["governance_reason"] = str(exc)
            _save_state(state)
        if state.get("status") in ("PAUSED_GOVERNANCE", "STOPPED"):
            print("PAUSED: inspect durable state and resolve the recorded boundary before resuming.")
            return 0
        if attempts >= max_iterations:
            break
        state["status"] = "RUNNING"
        state["iteration"] += 1
        iteration = state["iteration"]

        preamble = (RECONCILIATION_PREAMBLE + "\n\n"
                    if state.get("needs_reconciliation") else "")
        prompt = (f"{preamble}{CONTROL_INSTRUCTIONS}\n\n"
                  f"Work item: {state['work_item']}\n"
                  f"Goal: {state.get('goal','')}\n\n"
                  f"Next action for this turn: {next_action}")
        if state.get("handoff"):
            prompt += "\nCompact durable handoff (verify against repository):\n" + state["handoff"]
        prompt += f"\nObserved-turn threshold: {turn_threshold}; return after one increment."
        if first:
            first = False

        fresh = state.get("fresh_session", iteration == 1)
        if state.get("session_launch_pending"):
            try:
                fresh = not session_exists(session_id)
            except (OSError, ValueError) as exc:
                state["status"] = "PAUSED_GOVERNANCE"
                state["governance_reason"] = "session recovery evidence unavailable: " + type(exc).__name__
                _save_state(state)
                _log({"event": "halt", "session_id": session_id, "reason": state["governance_reason"]})
                return 1
        state["session_launch_pending"] = fresh
        state["fresh_session"] = fresh
        state["needs_reconciliation"] = True
        _save_state(state)  # persist uncertain creation before invoking the CLI
        try:
            desk_guard(state)
        except ValueError as exc:
            print(f"HALTED before turn: {exc}")
            return 1
        outcome = run_turn(None if fresh else session_id,
                           prompt, model, timeout, fresh_id=session_id if fresh else None)

        if not outcome["ok"]:
            if outcome["error"] == "session identity mismatch":
                state["status"] = "PAUSED_GOVERNANCE"
                state["governance_reason"] = outcome["error"]
            # A transport timeout is ambiguous: the child may well have finished
            # its work before the driver stopped waiting, which is exactly what
            # happened on 2026-09-18. Record that so the next resume reconciles
            # before acting rather than repeating a completed implementation step.
            if outcome["error"] in ("timeout", "usage_limit"):
                state["needs_reconciliation"] = True
            if outcome["error"] == "usage_limit":
                state["status"] = "PAUSED_RATE_LIMIT"
                state["resume_after"] = outcome.get("resume_after")
                _save_state(state)
                _log({"event": "paused", "reason": "usage_limit",
                      "resume_after": state["resume_after"], "session_id": session_id})
                print("PAUSED_RATE_LIMIT: continuation preserved; waiting for verified reset.")
                continue
            _log({"event": "halt", "work_item": state["work_item"],
                  "session_id": session_id, "iteration": iteration,
                  "reason": f"transport: {outcome['error']}",
                  "needs_reconciliation": state.get("needs_reconciliation", False),
                  "stderr_tail": outcome.get("stderr"),
                  "stdout_tail": outcome.get("stdout")})
            _save_state(state)
            print(f"HALTED: transport failure: {outcome['error']}", file=sys.stderr)
            for name in ("stderr", "stdout"):
                tail = outcome.get(name)
                if tail:
                    print(f"  {name}: {tail[-300:]}", file=sys.stderr)
            return 1

        attempts += 1
        envelope = outcome["envelope"]
        returned = envelope.get("session_id")
        if returned != session_id:
            # Fail closed: we cannot prove the turn belonged to our session.
            _log({"event": "halt", "work_item": state["work_item"],
                  "session_id": session_id, "iteration": iteration,
                  "reason": f"session identity mismatch: got {returned}"})
            state["status"] = "PAUSED_GOVERNANCE"
            state["governance_reason"] = "session identity mismatch"
            _save_state(state)
            print("HALTED: session identity mismatch", file=sys.stderr)
            return 1

        state["fresh_session"] = False
        state.pop("session_launch_pending", None)
        state.pop("resume_after", None)
        result_text = envelope.get("result") or ""
        block, why = extract_control(result_text)
        if block is None:
            _log({"event": "halt", "work_item": state["work_item"],
                  "session_id": session_id, "iteration": iteration,
                  "reason": f"control: {why}",
                  "tail": result_text[-400:]})
            state["status"] = "PAUSED_GOVERNANCE"
            state["governance_reason"] = why
            _save_state(state)
            print(f"HALTED: {why}. The driver does not infer intent from prose.",
                  file=sys.stderr)
            return 1

        if block.get("work_item") != state["work_item"]:
            state["status"] = "PAUSED_GOVERNANCE"
            state["governance_reason"] = "control work_item mismatch or omission"
            _log({"event": "halt", "work_item": state["work_item"],
                  "session_id": session_id, "iteration": iteration,
                  "reason": state["governance_reason"],
                  "reported_work_item": block.get("work_item"),
                  "reported_next_action": block.get("next_action")})
            _save_state(state)
            return 1
        denials = envelope.get("permission_denials") or []
        _log({"event": "continuation", "work_item": block.get("work_item")
              or state["work_item"], "session_id": session_id,
              "iteration": iteration, "status": block["status"],
              "reason": block.get("reason", ""),
              "next_action": block.get("next_action", ""),
              "duration_ms": envelope.get("duration_ms"),
              "num_turns": envelope.get("num_turns"),
              "permission_denials": len(denials),
              "model": list((envelope.get("modelUsage") or {}).keys())})

        print(f"[{iteration}] {block['status']}: {block.get('reason','')}")
        if denials:
            print(f"      {len(denials)} permission denial(s) — tools outside the allowlist")

        state["next_action"] = block.get("next_action", "")
        # The turn has reconciled and reported, so the ambiguity is resolved.
        state.pop("needs_reconciliation", None)
        observed = envelope.get("num_turns")
        if type(observed) is int and observed >= 0:
            state["session_turns"] = state.get("session_turns", 0) + observed
        else:
            state["session_turns"] = turn_threshold  # unknown cannot mean zero
        if block["status"] == "continue":
            try:
                rotate_session(state, block)
                session_id = state["session_id"]
            except (ValueError, OSError, subprocess.SubprocessError) as exc:
                state["governance_reason"] = str(exc)
                # Explicit unsafe handoff is a boundary, never a resume hint.
                if (block.get("handoff") is not None or state["session_turns"] >= turn_threshold
                        or str(exc) == "unresolved authority boundary"):
                    state["status"] = "PAUSED_GOVERNANCE"
        else:
            state["status"] = "STOPPED"
        _save_state(state)

        if block["status"] == "stop":
            reason = block.get("reason", "")
            token = reason.split(":")[0].split()[0] if reason else ""
            declared = token in DECLARED_STOPS
            if token == "desk_escalation_required":
                try:
                    desk.prepare(state, block, STATE_DIR / "desk_outbox", _save_state)
                except ValueError as exc:
                    state["governance_reason"] = str(exc)
                    _save_state(state)
                    return 1
                continue
            _log({"event": "stop", "work_item": state["work_item"],
                  "session_id": session_id, "iteration": iteration,
                  "reason": reason, "recognised_stop_condition": declared})
            print(f"STOPPED: {reason}"
                  + ("" if declared else "  (unrecognised stop token — review)"))
            return 0

        next_action = block.get("next_action") or state.get("goal", "")

    _log({"event": "halt", "work_item": state["work_item"],
          "session_id": session_id, "iteration": state["iteration"],
          "reason": f"iteration budget of {max_iterations} reached"})
    print(f"PAUSED: iteration budget of {max_iterations} reached; --resume to continue.")
    return 0


def show_status() -> int:
    state = _load_state()
    if not state:
        print("no continuation state")
        return 0
    print(json.dumps(state, indent=2))
    print(f"kill switch: {'ENGAGED' if HOLD_FILE.is_file() else 'clear'}")
    if LOG_FILE.is_file():
        lines = LOG_FILE.read_text(encoding="utf-8").strip().splitlines()
        print(f"log entries: {len(lines)}")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--start", action="store_true")
    ap.add_argument("--resume", action="store_true")
    ap.add_argument("--hold", action="store_true", help="engage the kill switch")
    ap.add_argument("--release", action="store_true", help="disengage the kill switch")
    ap.add_argument("--status", action="store_true")
    ap.add_argument("--work-item", default="unspecified")
    ap.add_argument("--goal", default=None)
    ap.add_argument("--max-iterations", type=int, default=5)
    ap.add_argument("--model", default=None)
    ap.add_argument("--turn-threshold", type=int, default=governance.DEFAULT_TURN_THRESHOLD)
    ap.add_argument("--timeout", type=int, default=DEFAULT_TURN_TIMEOUT_SECONDS,
                    help="per-turn transport timeout in seconds (bounded, fail-closed)")
    ap.add_argument("--allow-nested", action="store_true")
    ap.add_argument("--reason", default="engaged manually")
    args = ap.parse_args()

    if args.turn_threshold < 1:
        ap.error("--turn-threshold must be positive")
    if args.hold:
        STATE_DIR.mkdir(parents=True, exist_ok=True)
        HOLD_FILE.write_text(args.reason, encoding="utf-8")
        _log({"event": "hold", "reason": args.reason})
        print(f"kill switch ENGAGED: {args.reason}")
        return 0
    if args.release:
        if HOLD_FILE.is_file():
            HOLD_FILE.unlink()
        _log({"event": "release", "reason": args.reason})
        print("kill switch released")
        return 0
    if args.status:
        return show_status()
    if args.start:
        if not args.goal:
            print("FAIL: --start requires --goal", file=sys.stderr)
            return 2
        return drive(args.goal, args.work_item, args.max_iterations,
                     args.model, args.timeout, args.allow_nested, args.turn_threshold)
    if args.resume:
        return drive(None, args.work_item, args.max_iterations,
                     args.model, args.timeout, args.allow_nested, args.turn_threshold)
    ap.print_help()
    return 0


if __name__ == "__main__":
    sys.exit(main())
