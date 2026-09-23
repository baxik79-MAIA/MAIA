# MAIA 1.7.0 — M0.8 Round Table Vertical Slice

M0.8 adds a minimal provider-neutral, reasoning-only Round Table foundation and
an Anthropic Messages API adapter outside Core. First-round participant requests
are response-isolated; adjudication records disagreement and evidence and emits
a decision that never authorizes Action execution.

Anthropic model selection and the API credential remain local configuration.
The adapter has bounded timeout, safe transient-only retry, request-id and usage
capture, no hidden provider fallback, and an explicitly opt-in live smoke test.
