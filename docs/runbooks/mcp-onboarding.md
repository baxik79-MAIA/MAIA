# Runbook: MCP server onboarding

1. Add server URL/transport and authentication metadata.
2. Validate TLS/endpoint policy and server identity.
3. Discover tools/resources and cache catalog with expiry.
4. Map every tool to MAIA risk class and required scopes.
5. Block tools whose schemas are ambiguous, privileged or incompatible with policy.
6. Test a read-only operation.
7. Enable write tools only through explicit policy and ApprovalGate.
8. Audit every external tool invocation and sanitized result metadata.
