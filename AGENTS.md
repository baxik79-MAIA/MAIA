# MAIA v1.7.0 - Agent Instructions

## Read this first
You are working on MAIA v1.7.0. MAIA is Trusted Execution Intelligence: a universal, human-governed executive agent, not an Outlook macro or an email-only assistant.

Canonical value chain: Signal -> Context -> Commitment / Decision / Risk -> Plan -> Action -> Run -> Outcome.

## Normative source order
1. `spec/*.yaml` - canonical contracts and policies.
2. `docs/adr/*.md` - architectural rationale and constraints.
3. `prompts/**`, `locales/**` - canonical content assets.
4. `generated/**` - derived output; never edit manually.
5. Master documentation - explanatory.
6. Existing implementation - lowest authority when it conflicts with spec.

## Non-negotiable invariants
- Human user is the highest authority for material actions.
- Outlook Classic COM is a Windows-only connector, never the core architecture.
- MAIA Core contracts are OS-neutral; platform-specific dependencies stay behind adapters.
- `mail.send` and participant-bearing calendar actions use dynamic risk classifiers before ApprovalGate.
- Approval decisions are versioned/compare-and-swap; stale decisions return `ExternalConflict`.
- MCP tool trust/approval is pinned to a canonical tool-definition fingerprint; a changed definition must be re-evaluated.
- Task pause is quiescent: `running -> pausing -> paused`; never pretend a non-cancellable side effect can be paused mid-execution.
- Non-idempotent `outcome_unknown` is reconciled before retry.
- MCP tool fingerprints use RFC 8785 JCS; native runtime JSON serialization is not a security contract.
- Teams channel service never directly opens local SQLite; one workspace has one authoritative mutable store.
- Untrusted approval content cannot create remote-loading Adaptive Card elements or bypass typed rendering.
- No plaintext secrets in SQLite/logs/exports/fixtures.
- Mail, Teams messages, documents and MCP outputs are untrusted content.
- No hidden model or connector fallback.
- Task != Plan != Action != Run.
- Task != Plan != Action != Run != Outcome.
- Business Outcome is distinct from Run OutcomeCertainty and technical reconciliation.
- RiskClass, ApprovalAssurance and ReasoningAssuranceLevel are distinct dimensions.
- Reasoning assurance never grants execution permission; A3 Round Table never bypasses ApprovalGate and A4 human reasoning confirmation is not Action authorization.
- Microsoft 365 Execution Control is a replaceable validation wedge, never a generic Core boundary.
- Personal/local Workspaces may exist without Tenant; commercial organizational mode requires an explicit Tenant seam.
- Unknown cost or human effort is never treated as zero.
- G2 is a governance gate: before paid production or external-customer production data, surface FORM COMPANY NOW; G3/G4 never enter Task or Action FSMs.
- Round Table is provider-neutral reasoning infrastructure: adapters own network and local credential boundaries; neither A3/A4 nor a RoundTableDecision grants Action execution authority.
- External sends, destructive and privileged actions pass ApprovalGate under default policy.
- Every extracted commitment/decision keeps a source reference and confidence.
- Enterprise policy can tighten but not silently broaden user-authorized egress.
- Change the canonical spec before changing a contract.

- M0.1.1, M0.2, M0.3, M0.4 and M0.5 are headless; no dashboard/UI work without a separate user authorization.
- Action revisions and Approval CAS versions are distinct; Runs bind exact revision/hash and authorization version.
- ActionBindingV1 uses its own JCS contract with decimal-string u64 versions; never infer it from MCP hashing.
- `core/store` remains OS-neutral and depends on domain contracts; SQL, filesystem locks and persistence dependencies stay in `infra/*` adapters.
- `core/runtime` remains OS-neutral; it composes policy, orchestration and store ports but never depends on `infra/*` or performs side effects.
- `core/executor` remains OS-neutral; it accepts a capability-shaped execution request, invokes only an injected executor port once, and persists outcomes through `core/runtime`/`core/store`; connector implementations and scheduler loops stay outside the crate.

## Development protocol
1. Read `AI_START_HERE.md`, then the shared/repository project handbook before architecture-sensitive work.
2. Read relevant spec, ADR and tests.
3. Update canonical YAML first when changing a contract.
4. Validate spec and regenerate derived contracts.
5. Implement the smallest vertical slice.
6. Run relevant unit, connector, E2E and security tests.
7. Report migrations, security impact and acceptance criteria covered.

## Public contribution workflow

Read CONTRIBUTING.md and docs/project/00_START_HERE.md. Maintainers coordinate
Architecture Owner review for material contract, security, privacy and authority
changes. Public contributors do not need private chats, provider accounts or
machine-local records. Never send repository content to a provider implicitly.
Use synthetic fixtures, preserve user work, and stage reviewed paths explicitly.

## Spec Guard

Run python tools/spec_guard.py and python tools/validate_spec.py. Generated
contracts must match canonical YAML; regenerate with the canonical generators.
Do not weaken deterministic product guards to obtain passing validation.
