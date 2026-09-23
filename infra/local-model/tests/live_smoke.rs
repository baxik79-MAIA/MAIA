use maia_briefing::{BriefingPacket, MAXIMUM_OUTPUT_TOKENS, PacketEvidence, consult_and_validate};
use maia_local_model::LoopbackLocalProvider;

#[test]
#[ignore = "requires the MAIA-owned local Ollama runtime on 127.0.0.1:11434"]
fn recovered_local_model_returns_a_validated_advisory_briefing() {
    let packet = BriefingPacket {
        id: "m010-live-smoke".into(),
        workspace_id: "01900000-0000-7000-8000-000000000000".into(),
        objective: "State the supplied fact in one advisory claim.".into(),
        instructions: "Use only supplied evidence and cite it.".into(),
        template_version: "briefing-v1".into(),
        response_contract: "claims JSON".into(),
        privacy_classification: "internal".into(),
        assurance: "A1".into(),
        routing_constraints: "local_loopback_only".into(),
        evidence: vec![PacketEvidence {
            citation_id: "e1".into(),
            artifact_id: "01900000-0000-7000-8000-000000000010".into(),
            content_hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            excerpt: "MAIA local briefing is advisory only.".into(),
        }],
    };
    let provider = LoopbackLocalProvider::new(
        "127.0.0.1:11434".parse().unwrap(),
        "qwen3:4b-instruct-2507-q4_K_M",
    )
    .unwrap();
    let result = consult_and_validate(&provider, &packet, MAXIMUM_OUTPUT_TOKENS).unwrap();
    assert!(!result.execution_authority);
    assert!(!result.claims.is_empty());
}
