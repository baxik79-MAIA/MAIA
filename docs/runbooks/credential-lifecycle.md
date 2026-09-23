# Runbook: Credential lifecycle

1. User opens Settings > Credentials.
2. Select provider/profile type and requested scopes.
3. Secret is entered only into the dedicated secure form and submitted once to the privileged core.
4. Core stores secret material in OS/enterprise vault and returns only masked metadata.
5. Validate using the cheapest safe provider operation.
6. Rotation creates a new secret version, validates it, switches references atomically, then revokes the old secret when possible.
7. Deletion is blocked while an active Run requires the credential unless the Run is canceled or remapped.
8. Export never contains secret material or reusable vault identifiers.
