# MAIA v1.1.0 - Contract Correction Release

Key corrections after architecture review:

1. Dynamic risk classification replaces illegal hybrid `RiskClass` values in `ipc.yaml`.
2. Explicit `ConnectorHealth` transition map.
3. Explicit gate-to-`ApprovalState` mapping and policy semantics.
4. Compare-and-swap/versioned approval decisions across surfaces.
5. MCP tool-definition fingerprint pinning and rug-pull invalidation.
6. Credential-compromise incident runbook.
7. OS-neutral Core contract; Windows is only the Alpha reference/bootstrap platform.
8. Calendar-write limitation is explicit in `local_legacy`.
9. New privacy/compliance spec for third-party personal data and data-subject workflows.
10. Product expansion updated to **Multichannel Automation & Intelligent Assistance**.
11. Executable `tools/validate_spec.py` added; v1.0 Spec Guard was documentation-level rather than a complete executable validator.
12. Normative registries are generated from YAML (`generated/canonical_registry.md`) and should not be hand-maintained in narrative documents.
