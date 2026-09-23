# Runbook: Credential compromise / incident response

Use when an API key, OAuth token, connector credential, secret reference, or signing credential is suspected or confirmed compromised.

1. Identify the affected `CredentialProfile`, provider/tenant, scopes, dependent connectors/models and last-known-good time.
2. Immediately set the profile to suspended/blocked in MAIA so no new Run may select it.
3. Revoke/disable the credential at the authoritative provider or tenant control plane; do not rely only on local deletion.
4. Invalidate dependent cached sessions, subscriptions/webhooks and background jobs where the credential could still authorize work.
5. Rotate/reissue the credential with the minimum required scopes and store it through the native vault flow.
6. Audit `Run`, `Event` and `AuditRecord` entries from the last-known-good time through revocation; identify external side effects and unusual scope use.
7. Assess blast radius: data read, data written/sent, external recipients, MCP/tool calls, tenant resources and any downstream secret exposure.
8. Follow organization incident/privacy/security notification procedures where applicable; MAIA records the incident but does not decide legal notification obligations.
9. Re-enable the profile only after validation and policy review.
10. Record root cause, affected versions/scopes, rotation time, follow-up actions and tests that prevent recurrence.

**Fail closed:** if authoritative revocation cannot be confirmed, the profile remains blocked.
