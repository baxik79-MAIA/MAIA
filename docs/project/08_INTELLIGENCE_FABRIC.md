# Intelligence Fabric â€” architectural target

The Intelligence Fabric is the provider-neutral successor direction for the
current Model Router. This page describes the target architecture; it does not
claim that these capabilities are implemented in M0.5.

## Accepted architecture

The fabric may route among local providers, subscription-backed consultants and
metered consultants. A provider profile describes authentication mode,
local/external execution, privacy/egress eligibility, usage or cost class,
availability/health, reasoning and context support, tool support and whether
automatic routing is allowed. Providers are replaceable consultants; MAIA
retains canonical state and audit.

A future `ConsultationPacket` is the reduced, purpose-bound evidence package
sent to a consultant. It is subject to minimisation, redaction, policy and
approval. The packet is not a copy of MAIA's complete mailbox, repository or
memory.

## Implemented today

M0.2 provides pure policy and routing facts, and M0.5 provides a generic
one-shot executor port. No concrete model provider, model router or
Intelligence Fabric service is implemented.

## Deferred

Provider discovery, capability negotiation, health/usage accounting,
automatic routing, consultation packet schemas, credential handling and
provider adapters require their own canonical contracts and milestones. No
core domain module may import OpenAI, Anthropic, Ollama, llama.cpp, LM Studio
or another concrete provider while those decisions are pending.

## Product boundary

**ACCEPTED ARCHITECTURE.** Deterministic tools remain responsible for
deterministic work. Local, subscription-backed, metered and frontier models are
replaceable computational or consultation resources selected behind explicit
capability, privacy, policy and approval boundaries.

**PLANNED / DEFERRED.** A consultation packet, capability registry, local
provider detection, provider health/retry accounting and automatic routing are
not implemented merely because they are useful product goals. Each requires
canonical contract closure.
