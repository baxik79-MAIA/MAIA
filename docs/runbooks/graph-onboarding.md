# Runbook: Microsoft Graph onboarding

1. Register/identify the approved Entra application pattern for the tenant.
2. Request the least delegated/application permissions needed for the enabled capabilities.
3. Complete admin consent only where policy requires it.
4. Validate identity, tenant, mailbox and scopes.
5. Start bounded initial synchronization; persist delta links per folder/resource.
6. If webhooks are available, create subscriptions and lifecycle notification handling.
7. Record scopes, tenant, consent mode and expiry/renewal state in connector metadata.
8. Do not enable send/delete capabilities unless explicitly granted and policy-approved.
