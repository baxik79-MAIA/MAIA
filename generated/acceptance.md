### alpha_local_legacy

**A001**

- **Wymaganie:** Application runs on a supported Windows machine without external commercial AI API keys.

**A002**

- **Wymaganie:** Outlook Classic connector can read/search selected mailbox scope and create drafts through TrustedBridge without exposing credentials to MAIA core.

**A003**

- **Wymaganie:** Local model endpoint can analyze messages and generate drafts; all sends require approval.

**A004**

- **Wymaganie:** Task/Plan/Action/Run provenance survives restart and is auditable.

**A005**

- **Wymaganie:** Commitment extraction produces source-linked candidate commitments and never silently marks them confirmed.

**A006**

- **Wymaganie:** No plaintext secret exists in SQLite, logs, exports, bundles or fixtures.

**A007**

- **Wymaganie:** Spec validator and generated-contract drift check pass in CI.

**A008**

- **Wymaganie:** PL and EN UI strings are externalized.

**A009**

- **Wymaganie:** All spec/*.yaml parse successfully and Spec Guard rejects risk-enum mismatch, missing dynamic-risk resolver references and incomplete FSM transitions.

**A010**

- **Wymaganie:** Windows/COM dependencies are confined to the Outlook Classic adapter/surface boundary; MAIA Core has no direct Outlook COM/Win32 business dependency.

**A011**

- **Wymaganie:** Task pause is quiescent: no new Run starts after pause request, non-cancellable in-flight side effects drain/reconcile, and Task enters paused only when no Run remains in flight. This includes superseded revisions and created-to-starting transitions.

**A012**

- **Wymaganie:** Outlook Classic sidecar uses a dedicated STA COM execution context, handles rejected/busy COM calls with bounded retry/message-filter behavior, and reports ConnectorHealth=degraded rather than hanging.

### mvp_universal

**M001**

- **Wymaganie:** All Alpha criteria pass.

**M002**

- **Wymaganie:** Connector contract supports Outlook Classic and Microsoft Graph without core business logic changes.

**M003**

- **Wymaganie:** Teams agent surface can receive a request, show an approval card and return task result using the same task identity as desktop.

**M004**

- **Wymaganie:** Microsoft Graph mail sync uses delta/change notification strategy with recovery path.

**M005**

- **Wymaganie:** User can add, validate, rotate and delete multiple model/API credential profiles without code changes.

**M006**

- **Wymaganie:** WorkGraph links at least people, projects, threads, meetings, decisions and commitments with source provenance.

**M007**

- **Wymaganie:** External send, destructive and privileged actions cannot auto-execute under default policy.

**M008**

- **Wymaganie:** MCP client can connect to an approved server and MCP server can expose read-only scoped MAIA tools.

**M009**

- **Wymaganie:** Approval decisions are compare-and-swap/versioned: first terminal decision wins and stale or concurrent decisions return ExternalConflict.

**M010**

- **Wymaganie:** MCP tool review/approval is pinned to a canonical tool-definition fingerprint; material definition change invalidates the previous trust decision.

**M011**

- **Wymaganie:** Core/spec/connector-contract test suites pass on a non-Windows CI runner; platform-specific connector tests remain scoped to their platform.

**M012**

- **Wymaganie:** Teams production topology uses a managed public HTTPS channel endpoint; a local-authoritative Core is reached only through an authenticated outbound relay and the cloud adapter never opens local SQLite directly.

**M013**

- **Wymaganie:** MCP tool fingerprints use RFC 8785 JCS + SHA-256 and pass the same canonicalization golden vector across every supported runtime implementation.

**M014**

- **Wymaganie:** Payload mutation invalidates every old approval binding for the superseding payload/version, revokes pending/approved approvals, preserves not_required/rejected/expired/revoked historical states, and recomputes the action hash for a new action/approval version that is risk-resolved and re-gated before execution; not_required remains a valid result under the canonical gate mapping.

**M015**

- **Wymaganie:** Approval surfaces render untrusted content through a typed/sanitized presentation policy that blocks remote/data-URI resource exfiltration and untrusted Adaptive Card media/navigation elements.

**M016**

- **Wymaganie:** Material side effects derived from mutable external objects perform a connector source-version freshness check; version mismatch invalidates stale plan/approval.

**M017**

- **Wymaganie:** Ambiguous non-idempotent side-effect outcomes enter reconciliation and are never blindly retried.

**M018**

- **Wymaganie:** ActionBindingV1 has deterministic RFC8785 JCS/SHA-256 with exact u64 decimal strings, complete canonical input, connector identity/scope and visible MCP fingerprint binding.

**M019**

- **Wymaganie:** Immutable Action/Plan revisions and Run authorization record versions remain reconstructible; results affect only the exact executed revision.

**M020**

- **Wymaganie:** Current policy/assurance, expiry and versioned CAS constrain execution; not_required is an audit binding without a human decision; deny never authorizes.

**M021**

- **Wymaganie:** Every retry creates a new RunId in the exact Action revision attempt scope; unresolved effects block equivalent execution across IDs and revisions; authoritative non-execution evidence is required.

**M022**

- **Wymaganie:** Freshness evidence is per Run; unverifiable material freshness fails closed; pause includes superseded in-flight revisions and prevents created Runs from starting.

**M023**

- **Wymaganie:** Trusted recipient boundaries classify unknown as external, reject empty mail recipients and preserve explicit destructive/privileged operation floors.

### m0_2_1_contract_closure

**P001**

- **Wymaganie:** AgentTask and Run carry mutable record CAS versions distinct from immutable plan/action revisions and attempts.

**P002**

- **Wymaganie:** Authoritative persistence identities, parent integrity and no-runtime-hard-delete history rules are canonical.

**P003**

- **Wymaganie:** T1-T5 serialized atomic transaction boundaries are canonical and approval creation is not coupled to later Run creation.

**P004**

- **Wymaganie:** Workspace-scoped append-only AuditRecord uses monotonic sequence, RFC8785-JCS SHA-256 hash chaining and privacy-minimized facts.

**P005**

- **Wymaganie:** Local authority, migration ledger, backup/recovery, injected clock and fail-closed persistence error semantics are canonical without implementing SQLite.

### m0_7_execution_intelligence_contract

**E001**

- **Wymaganie:** Signal, Context, Commitment, Decision, Risk, Plan, Action, Run and Outcome remain distinct canonical concepts.

**E002**

- **Wymaganie:** Business Outcome is separate from Run OutcomeCertainty and can be candidate, observed, verified, rejected, disputed or superseded.

**E003**

- **Wymaganie:** Verified Outcome retains provenance, evidence and an explicit verification method; model confidence alone never verifies an Outcome.

**E004**

- **Wymaganie:** ReasoningAssuranceLevel A0-A4 is independent from RiskClass and never grants execution permission.

**E005**

- **Wymaganie:** A3 Round Table records independent reasoning, disagreement, evidence comparison, adjudication and measured usage without bypassing ApprovalGate.

**E006**

- **Wymaganie:** A4 human reasoning confirmation does not replace any independently required ApprovalGate authorization.

**E007**

- **Wymaganie:** TTFV and TTFA are instrumentable from authorized usable context without treating generic model output, diagnostics or no-op plumbing as qualifying value or action.

**E008**

- **Wymaganie:** Product advantage claims have a named benchmark or instrumented measurement for successful Outcome rate, human effort, cost per successful Outcome, TTFV, TTFA and repeatability.

**E009**

- **Wymaganie:** Unknown cost and unknown human effort remain unknown and are never silently represented as zero.

**E010**

- **Wymaganie:** Personal and local Workspaces may exist without Tenant; commercial organizational mode requires the Tenant seam and fails closed when it is absent.

**E011**

- **Wymaganie:** G2 alerts FORM COMPANY NOW before paid production, recurring B2B commercial contract or external-customer production-data processing; G3 and G4 remain outside Task and Action FSMs.

**E012**

- **Wymaganie:** Microsoft 365 Execution Control remains a replaceable validation hypothesis and does not redefine generic Core execution contracts.

### m0_8_round_table_vertical_slice

**R001**

- **Wymaganie:** Round Table receives a DecisionRequest, collects response-isolated ParticipantResponses, records disagreement and evidence, adjudicates and returns a reasoning-only RoundTableDecision.

**R002**

- **Wymaganie:** A3 and A4 preserve independent ApprovalGate and Action authorization requirements; RoundTableDecision never authorizes an Action.

**R003**

- **Wymaganie:** Provider-neutral Participant and ModelProvider contracts permit a configured Anthropic adapter without making Claude, an API key or an organization identifier part of Core contracts.

**R004**

- **Wymaganie:** Anthropic Messages adapter uses local credential boundary, bounded timeout, safe transient-only retry, request-id and returned usage capture, with no hidden provider fallback.

**R005**

- **Wymaganie:** Fake participants exercise the vertical slice without network or credentials; the live Anthropic smoke test is explicitly opt-in and skipped by default.

### m0_9_assurance_routing

**Q001**

- **Wymaganie:** A provider-neutral router selects A0-A4 from explicit assurance signals and policy floor, records reason codes, and never grants execution or ApprovalGate authority.

**Q002**

- **Wymaganie:** A0 is deterministic, A1 uses one enabled participant, A2 requires a meaningfully independent verifier, A3 uses the response-isolated Round Table, and A4 additionally requires recorded human reasoning acceptance.

**Q003**

- **Wymaganie:** The registry records participant identity, provider, model reference, role capability, enabled state, assurance levels, cost-metadata capability and availability health without hardcoding providers or models.

**Q004**

- **Wymaganie:** Estimated cost is checked before reasoning, measured cost is retained afterward when returned, unknown cost is never zero, and a session ceiling fails explicitly without hidden fallback.

**Q005**

- **Wymaganie:** Provider, quota, timeout, verifier, adjudication and required-assurance failures return explicit InsufficientAssurance with a reason code; the router never silently downgrades.

**Q006**

- **Wymaganie:** The audit record contains request, identities and model references, conclusions, evidence, disagreement/adjudication, assurance, usage/cost, provider request IDs when returned, outcome/confidence and A4 human acceptance.

### m0_12_claude_code_subscription_provider

**C001**

- **Wymaganie:** A ClaudeCodeProvider discovers the locally installed Claude Code executable, records its version, and fails closed with an explicit error when the executable is absent or not authenticated.

**C002**

- **Wymaganie:** The provider invokes Claude Code non-interactively in print mode with an explicit working directory and argument-safe process invocation; no shell command is constructed from packet content.

**C003**

- **Wymaganie:** The child environment is unconditionally scrubbed of ANTHROPIC_API_KEY and equivalent credential or endpoint overrides, so the invocation stays on subscription-backed authentication and never depends on an API key.

**C004**

- **Wymaganie:** Tool authority is restricted by an explicit deny list; bypassPermissions is never used and an empty allowlist is never treated as deny-all.

**C005**

- **Wymaganie:** A versioned ConsultationPacket produces a schema-validated ConsultationResult whose consultation_id matches the request; timeout, cancellation, non-zero exit, malformed JSON, empty response and schema violation all fail closed without fabricating a successful response.

**C006**

- **Wymaganie:** Every invocation persists a local audit record containing the raw response, execution metadata and correlation ID; audit write failure fails the invocation and no scrubbed credential value is ever written.

**C007**

- **Wymaganie:** A recursion guard environment marker prevents a Claude Code invocation from spawning a nested Claude Code invocation.

**C008**

- **Wymaganie:** The provider is absent by construction from a DEPLOYMENT_LOCKED build; subprocess code is not compiled without the development-evolution feature and MAIA Core never depends on the provider.

### m0_13_round_table_live_orchestration

**D001**

- **Wymaganie:** A failing participant degrades only its own contribution and never aborts collection; surviving participants still produce outcomes, though the session may still terminate as InsufficientAssurance when quorum is no longer satisfied.

**D002**

- **Wymaganie:** Falling below the A3 minimum of two valid independent first-round responses returns InsufficientAssurance with an explicit reason code; no silent downgrade and no single-participant Round Table.

**D003**

- **Wymaganie:** Leader selection is deterministic, registry-driven, recorded, and hardcodes no provider or model.

**D004**

- **Wymaganie:** Two interchangeable providers run the same session through the same provider-neutral port with no MAIA Core change.

**D005**

- **Wymaganie:** Every contribution carries provenance including the model reference actually used.

**D006**

- **Wymaganie:** First-round response isolation is preserved and provable under partial participant failure.

**D007**

- **Wymaganie:** Round Table session history persists locally and survives process restart.

**D008**

- **Wymaganie:** A reasoning-only participant never obtains execution authority and RoundTableDecision.execution_authority remains false on every path.

**D009**

- **Wymaganie:** MAIA Core and shipped applications build and operate with the Round Table implementation absent from their dependency graph.

**D010**

- **Wymaganie:** No participant, provider or model identifier is hardcoded in core/*.

**D011**

- **Wymaganie:** Failed participant attempts remain visible in session provenance with classified failure metadata.

**D012**

- **Wymaganie:** Adjudicator or leader participation never substitutes for a missing first-round quorum.

**D013**

- **Wymaganie:** Session persistence and restart preserve both successful and failed contribution provenance.

**D014**

- **Wymaganie:** Provider resolution failure is isolated and explicitly classified and never silently resolves to another provider.

### beta_enterprise

**B001**

- **Wymaganie:** Tenant-managed policies can override user policies toward stricter behavior.

**B002**

- **Wymaganie:** Teams proactive briefing and scheduled jobs work with the same approval and privacy gates.

**B003**

- **Wymaganie:** Relationship intelligence and commitment tracking pass source/provenance golden tests.

**B004**

- **Wymaganie:** Connector and model fallbacks are visible before material side effects and persisted afterward.

**B005**

- **Wymaganie:** Threat-model regression, dependency audit, secret scan and prompt-injection tests pass.

**B006**

- **Wymaganie:** Credential-compromise runbook is tested for at least one API-key profile and one OAuth/tenant connector profile.

**B007**

- **Wymaganie:** Third-party personal-data governance controls support subject-linked locate/export/rectify/restrict/erase-or-anonymize workflows according to configured retention/legal policy.

**B008**

- **Wymaganie:** WorkGraph inferred edges are visibly marked as suggestions, source-linked, and cannot authorize side effects until confirmed or explicitly promoted by policy.

**B009**

- **Wymaganie:** Third-party erasure workflow supports policy-driven deletion or pseudonymized random tombstones without deterministic hashes of PII and removes/suppresses derived indexes and memory.

**B010**

- **Wymaganie:** Approval-fatigue controls use explicit scoped/expiring grants or approved templates; RelationshipProfile familiarity alone never grants execution authority.

### v1_0

**V001**

- **Wymaganie:** All Beta criteria pass.

**V002**

- **Wymaganie:** Classic-to-new-Outlook migration path is tested: disabling COM connector does not orphan MAIA history or WorkGraph.

**V003**

- **Wymaganie:** Accessibility audit meets WCAG AA for applicable desktop/web controls.

**V004**

- **Wymaganie:** No critical/high known vulnerability without documented risk acceptance.

**V005**

- **Wymaganie:** Signed update verification and rollback/recovery runbook pass.
