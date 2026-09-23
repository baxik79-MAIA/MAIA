//! Provider-neutral local briefing semantics. No I/O, transport or execution.
#![forbid(unsafe_code)]
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacketEvidence {
    pub citation_id: String,
    pub artifact_id: String,
    pub content_hash: String,
    pub excerpt: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BriefingPacket {
    pub id: String,
    pub workspace_id: String,
    pub objective: String,
    pub instructions: String,
    pub template_version: String,
    pub response_contract: String,
    pub privacy_classification: String,
    pub assurance: String,
    pub routing_constraints: String,
    pub evidence: Vec<PacketEvidence>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimClass {
    SupportedByEvidence,
    Inference,
    UnknownNotSupported,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BriefingClaim {
    pub text: String,
    pub class: ClaimClass,
    pub citations: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawProviderResponse {
    pub claims: Vec<BriefingClaim>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedBriefingResult {
    pub packet_id: String,
    pub packet_hash: String,
    pub claims: Vec<BriefingClaim>,
    pub execution_authority: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BriefingError {
    EmptyPacket,
    PacketTooLarge,
    TooManyEvidence,
    InvalidCitation,
    DuplicateCitation,
    UnsupportedClaim,
    ExecutionAuthority,
    Provider(LocalProviderFailure),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalProviderFailure {
    Unavailable,
    ModelUnavailable,
    Timeout,
    MalformedResponse,
    ProviderError,
}
pub const MAXIMUM_EVIDENCE_COUNT: usize = 8;
pub const MAXIMUM_PACKET_BYTES: usize = 262_144;
pub const MAXIMUM_OUTPUT_TOKENS: u32 = 1_024;
pub trait LocalModelProvider: Send + Sync {
    fn requested_provider(&self) -> &str;
    fn requested_model(&self) -> &str;
    fn consult(
        &self,
        packet: &BriefingPacket,
        max_output_tokens: u32,
    ) -> Result<RawProviderResponse, LocalProviderFailure>;
}
pub fn packet_hash(packet: &BriefingPacket) -> Result<String, BriefingError> {
    if packet.id.is_empty()
        || packet.workspace_id.is_empty()
        || packet.objective.is_empty()
        || packet.evidence.is_empty()
    {
        return Err(BriefingError::EmptyPacket);
    }
    if packet.evidence.len() > MAXIMUM_EVIDENCE_COUNT {
        return Err(BriefingError::TooManyEvidence);
    }
    let mut evidence = packet.evidence.clone();
    evidence.sort_by(|left, right| {
        (
            &left.citation_id,
            &left.artifact_id,
            &left.content_hash,
            &left.excerpt,
        )
            .cmp(&(
                &right.citation_id,
                &right.artifact_id,
                &right.content_hash,
                &right.excerpt,
            ))
    });
    if evidence
        .windows(2)
        .any(|pair| pair[0].citation_id == pair[1].citation_id)
    {
        return Err(BriefingError::DuplicateCitation);
    }
    // The invocation id is provenance, not semantic packet identity.  This
    // ordered JSON tuple binds every canonical identity field without relying
    // on ambiguous delimiter concatenation.
    let text = serde_json::to_vec(&(
        "briefing_packet_v1",
        &packet.workspace_id,
        &packet.objective,
        &evidence,
        &packet.instructions,
        &packet.response_contract,
        &packet.privacy_classification,
        &packet.assurance,
        &packet.routing_constraints,
        &packet.template_version,
    ))
    .map_err(|_| BriefingError::PacketTooLarge)?;
    if text.len() > MAXIMUM_PACKET_BYTES {
        return Err(BriefingError::PacketTooLarge);
    };
    Ok(format!("{:x}", Sha256::digest(text)))
}
pub fn validate(
    packet: &BriefingPacket,
    raw: RawProviderResponse,
) -> Result<ValidatedBriefingResult, BriefingError> {
    let hash = packet_hash(packet)?;
    let allowed: std::collections::BTreeSet<_> = packet
        .evidence
        .iter()
        .map(|e| e.citation_id.as_str())
        .collect();
    for claim in &raw.claims {
        if claim.text.trim().is_empty() {
            return Err(BriefingError::UnsupportedClaim);
        }
        if matches!(claim.class, ClaimClass::SupportedByEvidence) && claim.citations.is_empty() {
            return Err(BriefingError::UnsupportedClaim);
        }
        if claim
            .citations
            .iter()
            .any(|c| !allowed.contains(c.as_str()))
        {
            return Err(BriefingError::InvalidCitation);
        }
        let citations: std::collections::BTreeSet<_> = claim.citations.iter().collect();
        if citations.len() != claim.citations.len() {
            return Err(BriefingError::DuplicateCitation);
        }
    }
    Ok(ValidatedBriefingResult {
        packet_id: packet.id.clone(),
        packet_hash: hash,
        claims: raw.claims,
        execution_authority: false,
    })
}
pub fn consult_and_validate(
    provider: &dyn LocalModelProvider,
    packet: &BriefingPacket,
    max_output_tokens: u32,
) -> Result<ValidatedBriefingResult, BriefingError> {
    if max_output_tokens == 0 || max_output_tokens > MAXIMUM_OUTPUT_TOKENS {
        return Err(BriefingError::PacketTooLarge);
    }
    let raw = provider
        .consult(packet, max_output_tokens)
        .map_err(BriefingError::Provider)?;
    validate(packet, raw)
}
pub fn result_hash(result: &ValidatedBriefingResult) -> String {
    let bytes = serde_json::to_vec(result).expect("briefing result serialization is infallible");
    format!("{:x}", Sha256::digest(bytes))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn p() -> BriefingPacket {
        BriefingPacket {
            id: "p".into(),
            workspace_id: "w".into(),
            objective: "brief".into(),
            instructions: "cite".into(),
            template_version: "1".into(),
            response_contract: "claims".into(),
            privacy_classification: "local".into(),
            assurance: "A1".into(),
            routing_constraints: "local".into(),
            evidence: vec![PacketEvidence {
                citation_id: "e1".into(),
                artifact_id: "a1".into(),
                content_hash: "h1".into(),
                excerpt: "fact".into(),
            }],
        }
    }
    #[test]
    fn cited_result_is_non_executable() {
        let r = validate(
            &p(),
            RawProviderResponse {
                claims: vec![BriefingClaim {
                    text: "fact".into(),
                    class: ClaimClass::SupportedByEvidence,
                    citations: vec!["e1".into()],
                }],
            },
        )
        .unwrap();
        assert!(!r.execution_authority)
    }
    #[test]
    fn rejects_invented_citation() {
        assert_eq!(
            validate(
                &p(),
                RawProviderResponse {
                    claims: vec![BriefingClaim {
                        text: "x".into(),
                        class: ClaimClass::SupportedByEvidence,
                        citations: vec!["bad".into()]
                    }]
                }
            ),
            Err(BriefingError::InvalidCitation)
        )
    }
    #[test]
    fn packet_identity_binds_all_semantic_contract_fields() {
        let original = p();
        let original_hash = packet_hash(&original).unwrap();
        let mut changed = original.clone();
        changed.template_version = "2".into();
        assert_ne!(original_hash, packet_hash(&changed).unwrap());
        changed = original.clone();
        changed.evidence[0].content_hash = "h2".into();
        assert_ne!(original_hash, packet_hash(&changed).unwrap());
        changed = original.clone();
        changed.id = "another-invocation".into();
        assert_eq!(original_hash, packet_hash(&changed).unwrap());
    }
    #[test]
    fn rejects_duplicate_packet_or_claim_citations() {
        let mut packet = p();
        packet.evidence.push(packet.evidence[0].clone());
        assert_eq!(packet_hash(&packet), Err(BriefingError::DuplicateCitation));
        assert_eq!(
            validate(
                &p(),
                RawProviderResponse {
                    claims: vec![BriefingClaim {
                        text: "x".into(),
                        class: ClaimClass::SupportedByEvidence,
                        citations: vec!["e1".into(), "e1".into()]
                    }]
                }
            ),
            Err(BriefingError::DuplicateCitation)
        );
    }
    struct FakeProvider(Result<RawProviderResponse, LocalProviderFailure>);
    impl LocalModelProvider for FakeProvider {
        fn requested_provider(&self) -> &str {
            "fake"
        }
        fn requested_model(&self) -> &str {
            "fake-model"
        }
        fn consult(
            &self,
            _: &BriefingPacket,
            _: u32,
        ) -> Result<RawProviderResponse, LocalProviderFailure> {
            self.0.clone()
        }
    }
    #[test]
    fn provider_failures_remain_unaccepted() {
        assert_eq!(
            consult_and_validate(
                &FakeProvider(Err(LocalProviderFailure::MalformedResponse)),
                &p(),
                128
            ),
            Err(BriefingError::Provider(
                LocalProviderFailure::MalformedResponse
            ))
        );
    }
    #[test]
    fn result_hash_binds_accepted_content() {
        let result = validate(&p(), RawProviderResponse { claims: vec![] }).unwrap();
        let original = result_hash(&result);
        let mut changed = result;
        changed.claims.push(BriefingClaim {
            text: "new".into(),
            class: ClaimClass::Inference,
            citations: vec![],
        });
        assert_ne!(original, result_hash(&changed));
    }
}
