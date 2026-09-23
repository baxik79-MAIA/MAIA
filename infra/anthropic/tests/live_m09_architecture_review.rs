//! Explicitly opt-in independent M0.9 architecture review.
//!
//! The prompt deliberately contains no Codex proposal. Its output is limited to
//! review conclusions, evidence and risks; it does not request private reasoning.

use maia_anthropic::AnthropicParticipant;
use maia_roundtable::{DecisionRequest, Participant, ParticipantRequest};

#[test]
#[ignore = "opt-in: requires M0_8_LIVE_ANTHROPIC=1, ANTHROPIC_API_KEY and MAIA_ANTHROPIC_MODEL"]
fn independent_m09_assurance_router_review() {
    if std::env::var("M0_8_LIVE_ANTHROPIC").as_deref() != Ok("1") {
        return;
    }

    let participant = AnthropicParticipant::from_environment()
        .expect("local Anthropic environment configuration");
    let response = participant
        .invoke(ParticipantRequest {
            session_id: "m0_9_independent_architecture_review".into(),
            decision: DecisionRequest {
                id: "m0_9_assurance_router_review".into(),
                subject: "minimum selective reasoning-assurance architecture".into(),
                prompt: "MAIA requires A3 or A4 reasoning but a provider, verifier, budget or adjudication can fail. In at most 120 tokens, state explicit failure/degradation semantics that preserve provider independence, human authority and ApprovalGate. State whether MAIA may silently downgrade to A1. No hidden reasoning or headings.".into(),
                evidence: vec![],
            },
            max_output_tokens: 200,
        })
        .expect("Messages API review response");
    let usage = response
        .usage
        .as_ref()
        .expect("returned Messages usage captured");
    println!(
        "M09_REVIEW configured_model={} anthropic_request_id={} input_tokens={} output_tokens={} review={}",
        response.participant.model.as_str(),
        response.provider_request_id.as_deref().unwrap_or("missing"),
        usage.input_tokens.unwrap_or_default(),
        usage.output_tokens.unwrap_or_default(),
        response.response_text
    );
}
