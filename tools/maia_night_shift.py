#!/usr/bin/env python3
"""MAIA Night Shift — the v1.1 multi-model development loop.

Ties the three existing tools together rather than replacing them:

  maia_continuation_driver.py   Claude turns (the control plane)
  roundtable_codex_exchange.py  independent Codex review
  roundtable_exchange.py        Architecture Desk exchange

This module supplies what v1.1 adds on top: after a coherent increment, an
independent Codex review is dispatched or queued, a Desk packet is sent, and the
whole chain is correlated so a reader can follow

    work item -> commit range -> Codex packet -> Codex result
              -> Claude disposition -> Desk packet -> Desk response

None of these steps is a stop condition. Codex being unavailable, a Desk
response arriving or not arriving, and a review packet being queued are all
normal states that the loop continues through.

INDEPENDENT-FIRST
A review packet carries the specification, acceptance criteria, commit range,
diff and test evidence, and explicit questions. It deliberately carries none of
Claude's confidence, preferred verdict or expected findings: the point is
independent evidence, not manufactured consensus.
"""

from __future__ import annotations

import datetime as _dt
import json
import pathlib
import os
import re
import subprocess
import uuid

ROOT = pathlib.Path(__file__).resolve().parents[1]
STATE_DIR = ROOT / "reports" / "roundtable" / "continuation"
CORRELATION_DIR = ROOT / "reports" / "roundtable" / "correlation"
CODEX_OUTBOX = ROOT / "reports" / "roundtable" / "outbox"
DESK_OUTBOX = ROOT / ".local" / "desk_outbox"
STATUS_FILE = ROOT / "reports" / "roundtable" / "NIGHT_SHIFT_STATUS.md"

CANONICAL_THREAD = os.environ.get("MAIA_DESK_THREAD_ID", "")

# Do not burn cycles probing a known-exhausted allowance. Codex availability is
# rechecked at most this often.
CODEX_RETRY_INTERVAL_SECONDS = 1800

PENDING_CODEX_REVIEW = "PENDING_CODEX_REVIEW"

DISPOSITIONS = {"ACCEPT", "MODIFY", "REJECT", "DEFER"}


def _utc() -> str:
    return _dt.datetime.now(_dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _git(*args: str) -> str:
    try:
        return (
            subprocess.run(
                ["git", *args],
                capture_output=True,
                text=True,
                encoding="utf-8",
                errors="replace",
                cwd=str(ROOT),
                timeout=120,
            ).stdout
            or ""
        )
    except (OSError, subprocess.SubprocessError):
        return ""


def head() -> str:
    return _git("rev-parse", "--short", "HEAD").strip()


def commits_between(base: str, tip: str) -> list[str]:
    out = _git("log", "--oneline", f"{base}..{tip}")
    return [line for line in out.splitlines() if line.strip()]


# --------------------------------------------------------------- Codex review


def codex_availability(last_checked_utc: str | None, now: _dt.datetime | None = None) -> bool:
    """Whether it is worth probing Codex again yet.

    Not whether Codex works — whether enough time has passed to ask. A known
    exhausted allowance does not recover in seconds, and re-probing it every
    turn wastes the loop's time without changing the answer.
    """
    if not last_checked_utc:
        return True
    now = now or _dt.datetime.now(_dt.timezone.utc)
    try:
        last = _dt.datetime.strptime(last_checked_utc, "%Y-%m-%dT%H:%M:%SZ").replace(
            tzinfo=_dt.timezone.utc
        )
    except ValueError:
        return True
    return (now - last).total_seconds() >= CODEX_RETRY_INTERVAL_SECONDS


def build_review_packet(work_item: str, base: str, tip: str, spec_refs: list[str],
                        questions: list[str]) -> str:
    """An independent review packet.

    Carries evidence and questions only. Claude's confidence, preferred verdict
    and expected findings are deliberately absent: priming the reviewer would
    produce agreement rather than an independent second opinion, which is the
    entire reason for asking.
    """
    diff_stat = _git("diff", "--stat", f"{base}..{tip}")
    diff = _git("diff", f"{base}..{tip}")
    if len(diff.encode("utf-8")) > 8_000:
        diff = diff.encode("utf-8")[:8_000].decode("utf-8", errors="ignore") + "\n... diff truncated; review the named files in the tree ...\n"

    lines = [
        f"# Independent review — {work_item}",
        "",
        "packet_schema: maia.codex_review_packet.v1",
        f"generated_utc: {_utc()}",
        f"commit_range: {base}..{tip}",
        f"review_mode: READ-ONLY. Do not edit the source worktree at {ROOT}.",
        "            Use an isolated worktree only if an experiment is genuinely required.",
        "",
        "## Governing specification and constraints",
        *[f"- {ref}" for ref in spec_refs],
        "",
        "## Commits under review",
        "```",
        *commits_between(base, tip),
        "```",
        "",
        "## Changed files",
        "```",
        diff_stat.rstrip(),
        "```",
        "",
        "## Diff",
        "```diff",
        diff.rstrip(),
        "```",
        "",
        "## Review questions",
        *[f"{i}. {q}" for i, q in enumerate(questions, 1)],
        "",
        "Reply with a single JSON object and nothing else:",
        '{"verdict":"GO"|"GO_WITH_FINDINGS"|"NO_GO",',
        ' "blocking_findings":[{"id":"B1","file":"...","issue":"...",'
        '"why_blocking":"...","suggested_fix":"..."}],',
        ' "non_blocking_findings":[{"id":"N1","file":"...","issue":"...",'
        '"suggested_fix":"..."}],',
        ' "confidence":"low"|"medium"|"high"}',
        "",
    ]
    packet = "\n".join(lines)
    if len(packet.encode("utf-8")) > 24_000:
        raise ValueError("review packet too large; use evidence paths and narrower commit scope")
    return packet


def queue_review(work_item: str, base: str, tip: str, spec_refs: list[str],
                 questions: list[str]) -> pathlib.Path:
    """Write a review packet into the Codex outbox. Never dispatches."""
    CODEX_OUTBOX.mkdir(parents=True, exist_ok=True)
    stamp = _dt.datetime.now(_dt.timezone.utc).strftime("%Y%m%d_%H%M%S")
    safe = re.sub(r"[^A-Za-z0-9._-]", "_", work_item)
    path = CODEX_OUTBOX / f"{safe}_{stamp}.packet.md"
    path.write_text(build_review_packet(work_item, base, tip, spec_refs, questions),
                    encoding="utf-8", newline="\n")
    return path


def pending_reviews() -> list[str]:
    if not CODEX_OUTBOX.is_dir():
        return []
    answered = set()
    inbox = ROOT / "reports" / "roundtable" / "inbox"
    if inbox.is_dir():
        answered = {p.name.replace(".response.json", "") for p in inbox.glob("*.response.json")}
    return sorted(
        p.name.replace(".packet.md", "")
        for p in CODEX_OUTBOX.glob("*.packet.md")
        if p.name.replace(".packet.md", "") not in answered
    )


def review_queue_order(current: list[str], historical: list[str]) -> list[str]:
    """Current increment first, then ONE historical debt item, then the rest.

    Draining the backlog first would starve current work of timely review, which
    is the review that can still change what gets built. Dispatching everything
    at once would also bury a reviewer, so debt is drained progressively.
    """
    order = list(current)
    if historical:
        order.append(historical[0])
        order.extend(historical[1:])
    return order


# ------------------------------------------------------------ correlation


def write_correlation(record: dict) -> pathlib.Path:
    """Persist one link in the work-item chain."""
    CORRELATION_DIR.mkdir(parents=True, exist_ok=True)
    record.setdefault("schema_version", "maia.night_shift_correlation.v1")
    record.setdefault("recorded_utc", _utc())
    cid = record.setdefault("correlation_id", f"ns-{uuid.uuid4().hex}")
    path = CORRELATION_DIR / f"{cid}.json"
    path.write_text(json.dumps(record, indent=2), encoding="utf-8")
    return path


def disposition_is_valid(disposition: str) -> bool:
    return disposition in DISPOSITIONS


def requires_desk_arbitration(finding: dict) -> bool:
    """Whether a review finding must go to the Desk rather than be settled here.

    Routine implementation findings are Claude's to resolve with evidence.
    Anything touching a hard boundary is not, regardless of how confident either
    side is: those are the Desk's by definition, and confidence is not evidence.
    """
    if finding.get("disposition") == "REJECT" and finding.get("blocking"):
        # Claude rejecting a blocking finding is a material disagreement.
        return True
    text = " ".join(
        str(finding.get(k, "")) for k in ("issue", "why_blocking", "area")
    ).lower()
    boundaries = (
        "deployment_locked",
        "execution authority",
        "execution_authority",
        "approval",
        "security",
        "privacy",
        "credential",
        "secret",
        "constitutional kernel",
        "canonical contract",
        "cross-capability",
        "persistent schema",
        "migration",
        "irreversible",
    )
    return any(b in text for b in boundaries)


# ---------------------------------------------------------------- Desk packet


def desk_thread_is_canonical(thread_id: str | None) -> bool:
    """Fail closed on any uncertainty about which thread this is.

    Posting engineering state into the wrong conversation is not recoverable by
    apologising afterwards, so anything other than an exact match is refused.
    """
    return bool(CANONICAL_THREAD) and thread_id == CANONICAL_THREAD


def build_desk_packet(*, work_item: str, head_sha: str, state: str, evidence: list[str],
                      decisions: list[str], codex_status: str,
                      dispositions: list[str], risks: list[str],
                      recommendation: str, next_actions: list[str],
                      requires_ruling: bool, nonce: str) -> str:
    """The v1.1 packet shape. A packet with no engineering state adds nothing."""
    def section(title: str, items: list[str]) -> list[str]:
        rows = [f"- {i}" for i in items] if items else ["- (none)"]
        return [f"## {title}", *rows, ""]

    packet = "\n".join([
        f"Subject: {work_item} — {state[:80]}; nonce {nonce}",
        "",
        "From: Claude Code (implementation participant)",
        "To: Architecture Desk (architecture authority)",
        f"Repo HEAD: {head_sha}",
        f"requires_ruling: {'true' if requires_ruling else 'false'}",
        "",
        "## CURRENT STATE",
        state,
        "",
        *section("EVIDENCE", evidence),
        *section("CLAUDE DECISIONS", decisions),
        "## CODEX",
        codex_status,
        "",
        *section("DISPOSITION", dispositions),
        *section("RISKS / DEBT", risks),
        "## RECOMMENDATION",
        recommendation,
        "",
        *section("NEXT", next_actions),
        f"Please include nonce {nonce} in your reply.",
        "",
    ])
    if len(packet.encode("utf-8")) > 16_000:
        raise ValueError("Desk packet too large; compact evidence without dropping decisions")
    return packet


def stage_desk_packet(body: str, work_item: str) -> pathlib.Path:
    """Write an outbound Desk packet. Never dispatches, never touches history."""
    DESK_OUTBOX.mkdir(parents=True, exist_ok=True)
    stamp = _dt.datetime.now(_dt.timezone.utc).strftime("%Y%m%d_%H%M%S")
    safe = re.sub(r"[^A-Za-z0-9._-]", "_", work_item)
    path = DESK_OUTBOX / f"pending_{stamp}_{safe}.md"
    path.write_text(body, encoding="utf-8", newline="\n")
    return path


# The seven historical files predate the night shift and are the Founder's.
HISTORICAL_PENDING = {
    "pending_20260911_220143.md",
    "pending_20260912_015230_recovery_blocker.md",
    "pending_20260912_020200_slow_network.md",
    "pending_20260912_105327.md",
    "pending_20260913_115847.md",
    "pending_20260913_163011.md",
    "pending_20260915_125500.md",
}


def dispatchable_packets(candidates: list[str]) -> list[str]:
    """Filter out the historical backlog.

    They must never be bulk dispatched. This is a filter rather than a check so
    that a caller which forgets to exclude them still cannot send them.
    """
    return [c for c in candidates if c not in HISTORICAL_PENDING]


# ------------------------------------------------------------- status report


def write_status(*, work_item: str, submilestone: str, head_sha: str,
                 completed: list[str], decisions: list[str], codex_state: str,
                 desk_state: str, evidence: list[str], risks: list[str],
                 next_action: str, founder_attention: bool) -> pathlib.Path:
    STATUS_FILE.parent.mkdir(parents=True, exist_ok=True)
    local = _dt.datetime.now().astimezone().strftime("%Y-%m-%d %H:%M %Z")
    body = "\n".join([
        "# MAIA Night Shift — status",
        "",
        "Informational only. Never canonical architecture authority.",
        "",
        f"- **Updated (UTC):** {_utc()}",
        f"- **Local:** {local}",
        f"- **Work item:** {work_item}",
        f"- **Submilestone:** {submilestone}",
        f"- **HEAD:** {head_sha}",
        f"- **Founder attention required:** {'YES' if founder_attention else 'no'}",
        "",
        "## Completed increments",
        *([f"- {c}" for c in completed] or ["- (none yet)"]),
        "",
        "## Claude decisions",
        *([f"- {d}" for d in decisions] or ["- (none)"]),
        "",
        "## Codex review",
        codex_state,
        "",
        "## Architecture Desk",
        desk_state,
        "",
        "## Evidence",
        *([f"- {e}" for e in evidence] or ["- (none)"]),
        "",
        "## Risks and debt",
        *([f"- {r}" for r in risks] or ["- (none)"]),
        "",
        "## Next action",
        next_action,
        "",
    ])
    STATUS_FILE.write_text(body, encoding="utf-8", newline="\n")
    return STATUS_FILE
