#!/usr/bin/env python3
"""Deterministic tests for the continuation driver's decision logic.

The loop's safety rests on the control-block parser and the allowlist, both of
which are testable without invoking a provider. Run:

    python tools/tests/test_continuation_driver.py
"""

import pathlib
import sys
import os
os.environ["MAIA_DESK_THREAD_ID"] = "synthetic-desk-thread-abcdef"

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import maia_continuation_driver as mcd  # noqa: E402
from maia_continuation_driver import (  # noqa: E402
    ALLOWED_TOOLS,
    BEGIN,
    DECLARED_STOPS,
    END,
    extract_control,
)

FAILURES = []


def strip_prose(source: str) -> str:
    """Remove triple-quoted strings and # comments, leaving executable code."""
    out = []
    i = 0
    while i < len(source):
        if source.startswith('"""', i) or source.startswith("'''", i):
            quote = source[i:i + 3]
            end = source.find(quote, i + 3)
            i = len(source) if end == -1 else end + 3
            continue
        if source[i] == "#":
            end = source.find("\n", i)
            i = len(source) if end == -1 else end
            continue
        out.append(source[i])
        i += 1
    return "".join(out)


def check(name, condition, detail=""):
    if condition:
        print(f"  ok   {name}")
    else:
        print(f"  FAIL {name} {detail}")
        FAILURES.append(name)


def block(body: str) -> str:
    return f"some reply text\n{BEGIN}\n{body}\n{END}"


def main() -> int:
    print("control block parsing")

    ok, why = extract_control(
        block('{"status":"continue","work_item":"M0.14",'
              '"reason":"did the thing","next_action":"do the next thing"}')
    )
    check("a well-formed continue block parses", ok is not None and ok["status"] == "continue", why)
    check("next_action is carried", ok and ok["next_action"] == "do the next thing")

    ok, why = extract_control(
        block('{"status":"stop","work_item":"M0.14",'
              '"reason":"milestone_complete: all items done","next_action":""}')
    )
    check("a stop block parses", ok is not None and ok["status"] == "stop", why)
    token = ok["reason"].split(":")[0] if ok else ""
    check("a declared stop token is recognised", token in DECLARED_STOPS, token)

    # Everything below must fail closed. A driver that guesses is worse than one
    # that halts, because it continues on an intent nobody expressed.
    ok, why = extract_control("I finished the work. Next I will do the thing.")
    check("prose without a block does not continue", ok is None, why)

    ok, why = extract_control(block("{not json}"))
    check("malformed JSON does not continue", ok is None, why)

    ok, why = extract_control(block('{"work_item":"M0.14"}'))
    check("a block with no status does not continue", ok is None, why)

    ok, why = extract_control(block('{"status":"maybe"}'))
    check("an unknown status does not continue", ok is None, why)

    ok, why = extract_control(block('["not","an","object"]'))
    check("a non-object block does not continue", ok is None, why)

    ok, why = extract_control(f"{BEGIN} no terminator, only a beginning")
    check("an unterminated block does not continue", ok is None, why)

    # The last block wins, so a turn quoting an example earlier in its reply
    # cannot hijack the decision.
    text = (block('{"status":"stop","reason":"authority_boundary: example"}')
            + "\n" + block('{"status":"continue","reason":"real decision"}'))
    ok, why = extract_control(text)
    check("the final block is authoritative", ok and ok["reason"] == "real decision", why)

    print("\nallowlist policy")
    joined = " ".join(ALLOWED_TOOLS)
    check("git push is not grantable", "push" not in joined)
    check("unscoped Bash is not granted", "Bash" not in ALLOWED_TOOLS)
    check("no permission bypass appears in the allowlist",
          "bypass" not in joined.lower() and "dangerously" not in joined.lower())
    check("editing is granted, since the driver must do real work",
          "Edit" in ALLOWED_TOOLS and "Write" in ALLOWED_TOOLS)
    check("cargo is scoped", any(t.startswith("Bash(cargo") for t in ALLOWED_TOOLS))

    print("\ndriver source policy")
    source = (pathlib.Path(__file__).resolve().parents[1]
              / "maia_continuation_driver.py").read_text(encoding="utf-8")
    code = strip_prose(source)

    # These flags are named in the docstring precisely to say they are never
    # used, so the check must look at code rather than prose. Scanning the
    # whole file would fail on its own documentation, which is the same
    # self-referential trap the D010 test hit.
    for flag in ["bypassPermissions", "dangerously-skip-permissions"]:
        check(f"{flag} never appears in executable code", flag not in code)
    check("permission prompts are set to deny",
          '"--permission-prompts", "none"' in code)

    print("\nworktree guard, content level")
    # Regression for the 2026-09-17 halt: a driven turn committed the Founder's
    # pre-existing Cargo.lock delta together with its own legitimate one.
    #
    # The protected content is the Founder's two maia-desktop lines. A
    # legitimate new crate block in the SAME file must not trip the guard, and
    # the protected lines vanishing must.
    protected_lines = ['+ "image",', '+ "serde_json",']
    snapshot = {"Cargo.lock": protected_lines}

    def breach_with(current_added):
        real = mcd.worktree_diff
        mcd.worktree_diff = lambda _p: "\n".join(["+++ b/Cargo.lock"] + current_added)
        try:
            return mcd.protected_worktree_breach(snapshot)
        finally:
            mcd.worktree_diff = real

    check("protected hunk preserved alone is allowed",
          breach_with(protected_lines) == [])

    check("protected hunk preserved alongside a legitimate new hunk is allowed",
          breach_with(protected_lines + ['+[[package]]',
                                         '+name = "maia-roundtable-composition"']) == [])

    check("protected hunk committed away HALTS",
          breach_with(['+[[package]]', '+name = "maia-roundtable-composition"'])
          == ["Cargo.lock"])

    check("partially committed protected hunk HALTS",
          breach_with(['+ "image",', '+[[package]]']) == ["Cargo.lock"],
          "a path-level check would miss this: the file still looks modified")

    check("everything committed away HALTS", breach_with([]) == ["Cargo.lock"])

    # Untracked protected files. A file that was untracked when the run started
    # has no diff, so the content comparison has nothing to compare and presence
    # is the whole guarantee. This branch caught a real breach on 2026-09-18 (a
    # protected receipt swept into a commit) while having no direct test, so it
    # gets one: production evidence is not a substitute for a regression.
    untracked_snapshot = {"reports/roundtable/outbox/some.receipt.json": []}

    def untracked_breach(uncommitted_now):
        real = mcd.uncommitted_paths
        mcd.uncommitted_paths = lambda: uncommitted_now
        try:
            return mcd.protected_worktree_breach(untracked_snapshot)
        finally:
            mcd.uncommitted_paths = real

    check("untracked protected file still untracked is allowed",
          untracked_breach(["reports/roundtable/outbox/some.receipt.json",
                            "other/file.rs"]) == [])
    check("untracked protected file committed away HALTS",
          untracked_breach(["other/file.rs"])
          == ["reports/roundtable/outbox/some.receipt.json"])
    check("an empty working tree HALTS an untracked protected file",
          untracked_breach([]) == ["reports/roundtable/outbox/some.receipt.json"])

    # An older state file stores a plain list; it must still fail closed rather
    # than silently passing because the shape changed.
    legacy = mcd.protected_worktree_breach(["definitely/not/a/real/path.rs"])
    check("legacy list-form snapshot still fails closed",
          legacy == ["definitely/not/a/real/path.rs"])

    print("\ntransport timeout headroom")
    # Regression for the 2026-09-18 halt: M0.14 iteration 1 was a legitimate
    # engineering turn of 896,219 ms that completed correctly, leaving ~4s of
    # headroom under the old 900s ceiling. Iteration 2 was then misclassified as
    # a transport failure although its work had finished. The ceiling must
    # comfortably exceed the longest turn we have actually observed, while
    # staying bounded.
    longest_s = mcd.LONGEST_OBSERVED_LEGITIMATE_TURN_MS / 1000
    ceiling = mcd.DEFAULT_TURN_TIMEOUT_SECONDS
    check("default timeout exceeds the longest observed legitimate turn",
          ceiling > longest_s, f"{ceiling}s vs {longest_s:.0f}s")
    check("headroom is at least 1.5x that turn, not a few seconds",
          ceiling >= longest_s * 1.5, f"ratio {ceiling / longest_s:.2f}x")
    check("timeout stays bounded rather than becoming unlimited",
          isinstance(ceiling, int) and 0 < ceiling <= 7200, f"{ceiling}s")
    check("the old 900s ceiling would have failed this check", 900 < longest_s * 1.5)

    print("\npost-timeout reconciliation")
    # A timeout means the driver stopped waiting, not that the work failed, so
    # the stored next_action may already be done. The next turn must be told to
    # reconcile rather than blindly repeat it.
    check("a timeout flags the state for reconciliation",
          "needs_reconciliation" in mcd.drive.__code__.co_consts
          or 'needs_reconciliation' in strip_prose(
              pathlib.Path(mcd.__file__).read_text(encoding="utf-8")))
    preamble = mcd.RECONCILIATION_PREAMBLE
    check("the preamble tells the turn not to repeat completed work",
          "do NOT repeat" in preamble or "not repeat" in preamble.lower())
    check("the preamble demands commit/test evidence, not filenames",
          "filename" in preamble and "commit" in preamble)
    check("the preamble states a timeout is not a failure",
          "does NOT mean the work failed" in preamble)

    print("\nexit classification")
    # Regression for the 2026-09-18 repeated "transport: exit 1". Both halts
    # recorded no detail at all, because only stderr was kept and the CLI had
    # written nothing there. The cause was an exhausted claude.ai usage limit,
    # which needs waiting, not debugging — so it must be named, not reported as
    # a generic fault that invites re-running or changing working code.
    check("an exhausted allowance on stdout is named",
          mcd.classify_exit(1, "ERROR: Your claude.ai usage limit has reset at 3pm", "")
          == "usage_limit")
    check("an exhausted allowance on stderr is named",
          mcd.classify_exit(1, "", "Your workspace is out of credits") == "usage_limit")
    check("a rate limit is named", mcd.classify_exit(1, "429 rate limit", "") == "usage_limit")
    check("case does not hide it",
          mcd.classify_exit(1, "USAGE LIMIT REACHED", "") == "usage_limit")
    check("an ordinary failure keeps its exit code",
          mcd.classify_exit(1, "panicked at src/lib.rs", "") == "exit 1")
    check("an unexplained failure keeps its exit code",
          mcd.classify_exit(1, "", "") == "exit 1")

    driver_src = strip_prose(pathlib.Path(mcd.__file__).read_text(encoding="utf-8"))
    check("stdout is captured on a non-zero exit, not only stderr",
          '"stdout": out' in driver_src,
          "a halt whose cause cannot be read afterwards is barely a halt")

    print()
    if FAILURES:
        print(f"{len(FAILURES)} FAILED: {FAILURES}")
        return 1
    print("all continuation driver checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
