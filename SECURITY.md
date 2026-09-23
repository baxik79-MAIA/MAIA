# Security policy

MAIA is experimental. There is no supported stable production release or guaranteed response SLA. Do not use real customer data to reproduce an issue.

## Reporting a vulnerability

Do not post credentials, private data or exploitable details in public issues or pull requests. GitHub private vulnerability reporting / Security Advisories is the canonical private reporting channel: https://github.com/baxik79-MAIA/MAIA/security/advisories/new. Use **Report a vulnerability** on the repository Security tab. Enablement is a post-push repository setting and has not been verified during local snapshot preparation. If unavailable, withhold vulnerability details until the channel is enabled; do not publish them in an issue.

Include affected commit/version, a minimal synthetic reproduction, expected versus actual behavior, impact and proposed mitigation if known. Keep disclosure private while maintainers assess the report. If a credential was exposed, its owner must revoke/rotate it; deleting a file or adding an ignore rule does not remove Git history.

## Security boundaries

See [the security model](docs/SECURITY_MODEL.md). Preserve ApprovalGate, immutable execution bindings, unknown-outcome reconciliation, untrusted-content handling, egress controls and deployment locks. Model output and multi-model review never constitute action authorization.
