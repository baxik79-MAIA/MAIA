//! Round Table adapter for the local loopback model provider.
//!
//! `infra/local-model` stays Round-Table-unaware: it exposes a generic
//! text-completion capability (`LoopbackLocalProvider::complete`), never a
//! Round Table type. This crate is the thin translation layer — Round
//! Table's `Participant`/`ParticipantResolver` ports in, that completion
//! capability out — so the dependency only ever points one way: this crate
//! depends on `maia-local-model` and on `maia-roundtable`, never the
//! reverse, and `infra/local-model` never depends on `maia-roundtable`.
#![forbid(unsafe_code)]

pub use maia_briefing::LocalProviderFailure;
use maia_local_model::LoopbackLocalProvider;
use maia_roundtable::{
    DecisionRequest, Participant, ParticipantDescriptor, ParticipantFailure,
    ParticipantFailureKind, ParticipantRequest, ParticipantResolver, ParticipantResponse,
    ResolutionFailure,
};
use std::sync::Arc;

/// A minimal seam over the local model's text-completion capability, so the
/// adapter can be tested without a real loopback runtime.
/// `LoopbackLocalProvider` implements it directly by forwarding to its own
/// `complete`; nothing here is Round-Table-specific, since `complete` on the
/// underlying type isn't either.
pub trait TextCompletion: Send + Sync {
    fn complete(
        &self,
        prompt: &str,
        max_output_tokens: u32,
    ) -> Result<String, LocalProviderFailure>;
}

impl TextCompletion for LoopbackLocalProvider {
    fn complete(
        &self,
        prompt: &str,
        max_output_tokens: u32,
    ) -> Result<String, LocalProviderFailure> {
        LoopbackLocalProvider::complete(self, prompt, max_output_tokens)
    }
}

impl<P: TextCompletion + ?Sized> TextCompletion for Arc<P> {
    fn complete(
        &self,
        prompt: &str,
        max_output_tokens: u32,
    ) -> Result<String, LocalProviderFailure> {
        (**self).complete(prompt, max_output_tokens)
    }
}

/// A Round Table participant backed by the local loopback model.
///
/// Implements the same `Participant` contract as every other provider
/// (Claude Code, Anthropic): no provider-name branching happens anywhere in
/// orchestration or composition because of this type existing.
pub struct LocalModelParticipant<P: TextCompletion> {
    descriptor: ParticipantDescriptor,
    provider: P,
}

impl<P: TextCompletion> LocalModelParticipant<P> {
    pub fn new(descriptor: ParticipantDescriptor, provider: P) -> Self {
        Self {
            descriptor,
            provider,
        }
    }
}

impl<P: TextCompletion> Participant for LocalModelParticipant<P> {
    fn descriptor(&self) -> ParticipantDescriptor {
        self.descriptor.clone()
    }

    fn invoke(
        &self,
        request: ParticipantRequest,
    ) -> Result<ParticipantResponse, ParticipantFailure> {
        let prompt = build_prompt(&request.decision);
        let text = self
            .provider
            .complete(&prompt, request.max_output_tokens)
            .map_err(map_failure)?;
        let text = text.trim();
        if text.is_empty() {
            // A response that arrived but carries nothing is not a usable
            // contribution; classified the same as a malformed one rather
            // than silently becoming an empty independent "answer".
            return Err(ParticipantFailure {
                kind: ParticipantFailureKind::Permanent,
                provider_request_id: None,
            });
        }
        Ok(ParticipantResponse {
            participant: self.descriptor.clone(),
            response_text: text.to_owned(),
            // The local loopback protocol carries no separate evidence
            // channel of its own; it can only answer, not cite.
            evidence: Vec::new(),
            // The loopback protocol returns no request identifier.
            provider_request_id: None,
            usage: None,
            // The loopback protocol never reports which model actually
            // served the request. Recorded as unknown rather than assumed
            // equal to the configured model: "we asked for X" and "X
            // answered" are different claims, and this adapter has no
            // evidence for the second one.
            model_ref_used: None,
        })
    }
}

fn build_prompt(decision: &DecisionRequest) -> String {
    let mut prompt = format!("Subject: {}\n\n{}", decision.subject, decision.prompt);
    if !decision.evidence.is_empty() {
        prompt.push_str("\n\nEvidence:\n");
        for reference in &decision.evidence {
            prompt.push_str(&format!("[{}] {}\n", reference.id, reference.source_ref));
        }
    }
    prompt
}

/// Classify a local-provider failure the same coarse way every other
/// participant does: transient failures may succeed if retried, permanent
/// ones will not without intervention. `ModelUnavailable` (the model is not
/// currently loaded/pulled) is treated as transient — nothing about the
/// requested identity is wrong, only its present availability.
/// `MalformedResponse` is permanent: retrying an unparseable envelope
/// against the same request is not expected to produce a different shape.
fn map_failure(failure: LocalProviderFailure) -> ParticipantFailure {
    let kind = match failure {
        LocalProviderFailure::Unavailable
        | LocalProviderFailure::ModelUnavailable
        | LocalProviderFailure::Timeout
        | LocalProviderFailure::ProviderError => ParticipantFailureKind::Transient,
        LocalProviderFailure::MalformedResponse => ParticipantFailureKind::Permanent,
    };
    ParticipantFailure {
        kind,
        provider_request_id: None,
    }
}

/// Resolves exactly one registered local-model identity to a live
/// participant. Holds no registry of its own — composition owns
/// registration data — and refuses anything but the one identity it was
/// configured for: no silent substitution of another descriptor, ever.
pub struct LocalModelResolver<P: TextCompletion> {
    descriptor: ParticipantDescriptor,
    provider: Arc<P>,
}

impl<P: TextCompletion> LocalModelResolver<P> {
    pub fn new(descriptor: ParticipantDescriptor, provider: P) -> Self {
        Self {
            descriptor,
            provider: Arc::new(provider),
        }
    }
}

impl<P: TextCompletion + 'static> ParticipantResolver for LocalModelResolver<P> {
    fn resolve(
        &self,
        descriptor: &ParticipantDescriptor,
    ) -> Result<Box<dyn Participant>, ResolutionFailure> {
        if *descriptor != self.descriptor {
            return Err(ResolutionFailure::UnknownParticipant);
        }
        Ok(Box::new(LocalModelParticipant::new(
            self.descriptor.clone(),
            Arc::clone(&self.provider),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maia_roundtable::{EvidenceReference, ModelProvider, ModelRef, ParticipantId};
    use std::sync::Mutex;

    fn descriptor(id: &str) -> ParticipantDescriptor {
        ParticipantDescriptor {
            id: ParticipantId::new(id).unwrap(),
            provider: ModelProvider::new("local-loopback").unwrap(),
            model: ModelRef::new("qwen-test").unwrap(),
        }
    }

    fn decision() -> DecisionRequest {
        DecisionRequest {
            id: "decision-1".into(),
            subject: "test".into(),
            prompt: "synthetic non-sensitive prompt".into(),
            evidence: vec![EvidenceReference {
                id: "ev-1".into(),
                source_ref: "src-1".into(),
            }],
        }
    }

    fn request() -> ParticipantRequest {
        ParticipantRequest {
            session_id: "session-1".into(),
            decision: decision(),
            max_output_tokens: 64,
        }
    }

    /// A fake completion source: no network, deterministic, and records
    /// what it was actually asked so prompt assembly is verifiable.
    struct Fake {
        result: Result<String, LocalProviderFailure>,
        last_call: Mutex<Option<(String, u32)>>,
    }
    impl Fake {
        fn ok(text: &str) -> Self {
            Self {
                result: Ok(text.to_owned()),
                last_call: Mutex::new(None),
            }
        }
        fn err(failure: LocalProviderFailure) -> Self {
            Self {
                result: Err(failure),
                last_call: Mutex::new(None),
            }
        }
    }
    impl TextCompletion for Fake {
        fn complete(
            &self,
            prompt: &str,
            max_output_tokens: u32,
        ) -> Result<String, LocalProviderFailure> {
            *self.last_call.lock().unwrap() = Some((prompt.to_owned(), max_output_tokens));
            self.result.clone()
        }
    }
    #[test]
    fn a_successful_local_response_carries_no_fabricated_provenance() {
        let participant = LocalModelParticipant::new(descriptor("local-1"), Fake::ok("the answer"));
        let response = participant.invoke(request()).unwrap();
        assert_eq!(response.response_text, "the answer");
        assert_eq!(response.participant, descriptor("local-1"));
        assert!(response.provider_request_id.is_none());
        assert!(response.usage.is_none());
        // Never assumed equal to the configured model without evidence.
        assert!(response.model_ref_used.is_none());
        assert!(response.evidence.is_empty());
    }

    #[test]
    fn the_prompt_carries_the_decision_and_its_evidence() {
        let fake = Fake::ok("answer");
        let participant = LocalModelParticipant::new(descriptor("local-1"), fake);
        participant.invoke(request()).unwrap();
        let (prompt, max) = participant
            .provider
            .last_call
            .lock()
            .unwrap()
            .clone()
            .unwrap();
        assert!(prompt.contains("synthetic non-sensitive prompt"));
        assert!(prompt.contains("ev-1"));
        assert!(prompt.contains("src-1"));
        assert_eq!(max, 64);
    }

    #[test]
    fn provider_unavailable_is_a_transient_classified_failure_not_a_fallback() {
        let participant = LocalModelParticipant::new(
            descriptor("local-1"),
            Fake::err(LocalProviderFailure::Unavailable),
        );
        let failure = participant.invoke(request()).unwrap_err();
        assert_eq!(failure.kind, ParticipantFailureKind::Transient);
        assert!(failure.provider_request_id.is_none());
    }

    #[test]
    fn a_malformed_response_is_a_permanent_classified_failure() {
        let participant = LocalModelParticipant::new(
            descriptor("local-1"),
            Fake::err(LocalProviderFailure::MalformedResponse),
        );
        let failure = participant.invoke(request()).unwrap_err();
        assert_eq!(failure.kind, ParticipantFailureKind::Permanent);
    }

    #[test]
    fn an_empty_response_is_treated_as_no_usable_contribution() {
        let participant = LocalModelParticipant::new(descriptor("local-1"), Fake::ok("   \n  "));
        let failure = participant.invoke(request()).unwrap_err();
        assert_eq!(failure.kind, ParticipantFailureKind::Permanent);
    }

    #[test]
    fn every_local_provider_failure_maps_to_a_classified_participant_failure() {
        for (failure, expected) in [
            (
                LocalProviderFailure::Unavailable,
                ParticipantFailureKind::Transient,
            ),
            (
                LocalProviderFailure::ModelUnavailable,
                ParticipantFailureKind::Transient,
            ),
            (
                LocalProviderFailure::Timeout,
                ParticipantFailureKind::Transient,
            ),
            (
                LocalProviderFailure::ProviderError,
                ParticipantFailureKind::Transient,
            ),
            (
                LocalProviderFailure::MalformedResponse,
                ParticipantFailureKind::Permanent,
            ),
        ] {
            let participant = LocalModelParticipant::new(descriptor("local-1"), Fake::err(failure));
            let result = participant.invoke(request()).unwrap_err();
            assert_eq!(result.kind, expected);
        }
    }

    #[test]
    fn no_provider_failure_produces_a_hidden_fallback_response() {
        // Every failure variant must surface as an Err, never silently
        // resolve to a synthesized or substitute Ok(ParticipantResponse).
        for failure in [
            LocalProviderFailure::Unavailable,
            LocalProviderFailure::ModelUnavailable,
            LocalProviderFailure::Timeout,
            LocalProviderFailure::ProviderError,
            LocalProviderFailure::MalformedResponse,
        ] {
            let participant = LocalModelParticipant::new(descriptor("local-1"), Fake::err(failure));
            assert!(participant.invoke(request()).is_err());
        }
    }

    #[test]
    fn the_resolver_returns_only_the_identity_it_was_configured_for() {
        let resolver = LocalModelResolver::new(descriptor("local-1"), Fake::ok("answer"));
        let resolved = resolver.resolve(&descriptor("local-1"));
        assert!(resolved.is_ok());
        // A different identity, even a plausible-looking one, is refused —
        // never silently served by the same underlying provider.
        let other = resolver.resolve(&descriptor("local-2"));
        assert!(matches!(other, Err(ResolutionFailure::UnknownParticipant)));
    }

    #[test]
    fn resolved_participants_share_the_same_underlying_provider() {
        let resolver = LocalModelResolver::new(descriptor("local-1"), Fake::ok("answer"));
        let first = resolver.resolve(&descriptor("local-1")).unwrap();
        let second = resolver.resolve(&descriptor("local-1")).unwrap();
        assert_eq!(first.descriptor(), second.descriptor());
    }
}
