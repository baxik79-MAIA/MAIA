# Privacy and egress

MAIA controls every external consultation boundary. Local-first execution is
preferred; sending context to an external provider is an explicit policy and
approval decision with audit provenance.

## Conceptual direction

The future policy vocabulary may use GREEN / YELLOW / RED as a conceptual
classification for data and egress risk. This vocabulary is intentionally
narrative until the canonical spec ratifies exact values and transitions.

External requests should use the smallest evidence package that answers the
question. Minimise, redact and preserve source references. A complete mailbox,
repository or memory must not be sent when a smaller dossier is sufficient.
Enterprise policy may tighten the boundary. An external provider cannot
bypass MAIA's Approval Engine, current policy, actor authorization, freshness
or reconciliation rules.

Provider sessions, prompts and returned answers are untrusted consultation
material. They do not own or replace MAIA memory, history, WorkGraph,
permissions, actions or audit. Secrets are never placed in prompts, SQLite,
logs or exports.

The current M0.1â€“M0.5 implementation has no external consultation transport;
these are accepted architectural principles and deferred implementation
contracts.
