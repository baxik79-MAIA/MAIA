# Runbook: Ambiguous or risky action

1. Stop before the side effect.
2. Show exact recipients/target, content summary, attachments, connector and risk class.
3. Explain why approval is required and any external-domain or destructive effect.
4. Hash the approved payload.
5. Execute only if the payload hash still matches.
6. If payload materially changes, invalidate approval and request again.
7. Persist the decision and actual execution result in the audit trail.
