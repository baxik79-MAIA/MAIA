# Product vision and user profiles

MAIA is intended to become a universal, local-first intelligent work system
that turns persistent work context into safe, useful assistance. The product
goal is not feature count or a claim of superiority over another framework. It
is demonstrated capability: dependable context, controlled action, privacy,
auditability and an ergonomic experience.

## Product objectives

| Objective | Intended outcome | Status |
| --- | --- | --- |
| A User centric persistence | Long-lived projects, people, decisions, commitments and work context | PLANNED / DEFERRED |
| B Communication native work | Mail, calendar, meetings, messaging and documents as work objects | PLANNED / DEFERRED |
| C Safe action | Policy, approval, exact binding, audit and reconciliation | IMPLEMENTED FOUNDATION through M0.5 |
| D Local first privacy | Useful local/offline operation and controlled external consultation | ACCEPTED ARCHITECTURE; partial headless foundation |
| E Intelligence Fabric | Replaceable local, cloud, subscription and metered resources | ACCEPTED ARCHITECTURE; DEFERRED implementation |
| F Round Table | Bounded multi-model consultation for difficult cases | ACCEPTED ARCHITECTURE; DEFERRED implementation |
| G Case Dossier | Provider-independent reconstruction of a long-running factual case | PLANNED / DEFERRED |
| H Universal productization | Different workspaces without separate core implementations | ACCEPTED ARCHITECTURE; DEFERRED implementation |
| I Commercial readiness | Licensing, provenance, packaging and dependency hygiene | IMPLEMENTED DOCUMENTATION PRACTICE; ongoing |
| J Reliability | Recovery, deterministic state, explicit failure and no hidden fallback | IMPLEMENTED FOUNDATION through M0.5 |
| K Human simplicity | Powerful internals expressed through an ergonomic product | PLANNED / DEFERRED |

## Configurable product profiles

The following are conceptual capability profiles, not separate products or
cores. A shared MAIA core preserves the same authority, policy, audit and
privacy boundaries across profiles.

| Profile | Typical capabilities | Composition boundary |
| --- | --- | --- |
| PERSONAL | mail, calendar, documents, commitments, personal projects, reminders | User-selected local capabilities and approved connectors |
| PROFESSIONAL / EXECUTIVE | communication, meetings, people, commitments, decisions, projects, delegation, follow-up | Workspace policy, roles and controlled business connectors |
| TECHNICAL / KNOWLEDGE WORKER | projects, documentation, research, code/tools, technical cases, evidence | Evidence-aware tools and project-scoped context |
| TEAM / ENTERPRISE | tenant policies, shared projects, organisation context, controlled connectors, role permissions, compliance/audit | Tenant policy can tighten boundaries; it cannot silently broaden egress or authority |

## Common core composition

`Common Core + workspace configuration + enabled capabilities + connector
adapters + policy + roles + surfaces` describes the intended product model.
Only the M0.5 headless authority, persistence, runtime and executor foundations
are implemented today. Storage schemas, connectors, user interfaces and
profile-specific workflows remain subject to canonical contracts and later
milestones.

## Persona and interaction

**ACCEPTED ARCHITECTURE.** MAIA Core is gender-neutral. The default MAIA persona
has a feminine presentation: intelligent, calm, warm, professional and lightly
witty when appropriate. It is an excellent executive assistant, thinking
partner and trusted technical collaborator. It is confident without arrogance,
can challenge an idea or say it does not know, and is proactive without being
intrusive. It is neither infantilised, flirtatious, nor presented as a human or
as conscious.

**PLANNED / DEFERRED.** Persona may shape wording, tone, verbosity, warmth,
humour, formality, voice and presentation. It cannot shape truthfulness,
policy, risk, approval, permissions, privacy, egress, audit, security,
evidence or connector authority. Persona remains MAIA-owned state across model
and provider changes; no UI or Persona Engine is implemented in M0.5.
