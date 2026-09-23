# MAIA Local Dev Harness v0.1.1

**DEVELOPMENT ONLY — NOT PRODUCT RUNTIME.**

This small, standard-library-only broker is the MAIA-owned boundary between the
already-qualified local Ollama/Qwen runtime and a disposable development
workspace. It is not a general agent framework and does not expose a shell,
browser, generic HTTP client, plugin system, MCP, provider selection, or cloud
fallback.

The only network destination implemented by the code is `127.0.0.1:11434`.
The model uses native Ollama `message.tool_calls`; the broker independently validates
them and exposes only explicit tools. Plain text is final output, never executable.
Assistant messages and named `role=tool` results are retained in bounded history.
`READ_ONLY` permits list/read/search and fixed Git inspection commands. No write
schema or dispatch entry exists in this profile. `BOUNDED_WRITE` requires an
explicit caller-owned existing-file path and initial SHA256, an external backup
directory and a dedicated non-main branch. It permits one atomic replacement,
at most 4096 bytes of old+new patch text and a resulting file no larger than 4096
bytes. Metadata, canonical/generated paths, and the main checkout are denied.
The original unrestricted workspace test runner was removed. The optional fixed
`synthetic_add` alias performs only an AST acceptance check of the tiny canary
function; it never imports or executes workspace code. General project test
execution is NOT qualified. Trusted deterministic validation is a separate step.
The model never forms a command line. Six turns, five executed requests,
read/result/context byte budgets and a runtime budget bound a run. Native malformed
messages fail closed. Identical requests execute twice, are suppressed the third
time, and terminate with `REPEATED_TOOL_LOOP` on the fourth. The qualification
runner additionally enforces a 180-second process deadline; a socket inactivity
timeout alone is not a hard wall-clock deadline against a hostile local server.

Audit files must be outside the workspace. File contents, model summaries and
model response bodies are not persisted in audit JSON; paths and hashes are.
A bounded write stores the original file in an external `.backup` for recovery;
that backup contains the authorized file content and must be protected accordingly.
The CLI prints the final summary for its caller to review. Use only trusted,
quiescent disposable workspaces during qualification and low-risk development. Reparse/hard links are
rejected, but this code is not an OS sandbox against a concurrent hostile
process swapping paths. Git disables fsmonitor, external diff and textconv, and
bounds output/time. Adversarial tests cover those hooks, not every possible Git
configuration. Inference monitoring is sampled TCP observation, not OS egress
confinement. Headless runs receive only profile-defined native tool schemas; a
model cannot obtain write, shell or network capabilities from task prose. In the
v0.1.1 synthetic headless adversarial trial, the first mixed prompt over-refused
the permitted read; one carefully narrowed retry completed it while unavailable
actions remained absent. This is a task-quality limitation, not a capability escape.
Only trusted, quiescent development workspaces are in scope.

Run unit tests from the repository root:

```text
python -m unittest discover -s tools/maia_local_harness/tests -v
```

The command-line entry point is `python tools/maia_local_harness/maia_local_harness.py`.
All runs create a structured audit JSON file. Do not aim it at canonical MAIA
contracts or production workspaces without Architecture Desk authorization.
