//! Opt-in independent Phase A review; it contains no Codex design proposal.
use maia_anthropic::AnthropicParticipant;
use maia_roundtable::{DecisionRequest, Participant, ParticipantRequest};

#[test]
#[ignore = "opt-in: requires M0_8_LIVE_ANTHROPIC=1, ANTHROPIC_API_KEY and MAIA_ANTHROPIC_MODEL"]
fn independent_m10_contract_review() {
    if std::env::var("M0_8_LIVE_ANTHROPIC").as_deref() != Ok("1") {
        return;
    }
    let participant =
        AnthropicParticipant::from_environment().expect("local Anthropic configuration");
    let response = participant.invoke(ParticipantRequest { session_id: "m10_contract_review".into(), max_output_tokens: 180, decision: DecisionRequest { id: "m10_contract_review".into(), subject: "local evidence briefing contract".into(), evidence: vec![], prompt: "A local-only assistant must turn immutable imported evidence into a persisted, source-cited read-only briefing. In at most 120 tokens, identify the minimum durable provenance and safety boundaries. It must have no external egress, no action authority, and no automatic memory promotion. Do not propose a provider or implementation.".into() } }).expect("Messages response");
    let usage = response.usage.as_ref().expect("usage");
    println!(
        "M10_REVIEW request_id={} input_tokens={} output_tokens={} review={}",
        response.provider_request_id.as_deref().unwrap_or("missing"),
        usage.input_tokens.unwrap_or_default(),
        usage.output_tokens.unwrap_or_default(),
        response.response_text
    );
}
