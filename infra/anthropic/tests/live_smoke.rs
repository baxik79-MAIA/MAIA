use maia_anthropic::AnthropicParticipant;
use maia_roundtable::{DecisionRequest, Participant, ParticipantRequest};

#[test]
#[ignore = "opt-in: requires M0_8_LIVE_ANTHROPIC=1, ANTHROPIC_API_KEY and MAIA_ANTHROPIC_MODEL"]
fn live_messages_smoke_is_explicitly_opt_in() {
    if std::env::var("M0_8_LIVE_ANTHROPIC").as_deref() != Ok("1") {
        return;
    }
    let participant = AnthropicParticipant::from_environment()
        .expect("local Anthropic environment configuration");
    let response = participant
        .invoke(ParticipantRequest {
            session_id: "m0_8_live_smoke".into(),
            decision: DecisionRequest {
                id: "m0_8_live_smoke".into(),
                subject: "connectivity verification".into(),
                prompt: "Reply with exactly: MAIA M0.8 smoke passed".into(),
                evidence: vec![],
            },
            max_output_tokens: 32,
        })
        .expect("Messages API response");
    assert!(!response.response_text.trim().is_empty());
    let usage = response
        .usage
        .as_ref()
        .expect("returned Messages usage captured");
    println!(
        "SMOKE_RESULT configured_model={} anthropic_request_id={} input_tokens={} output_tokens={}",
        response.participant.model.as_str(),
        response.provider_request_id.as_deref().unwrap_or("missing"),
        usage.input_tokens.unwrap_or_default(),
        usage.output_tokens.unwrap_or_default()
    );
}
