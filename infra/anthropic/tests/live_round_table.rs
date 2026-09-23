//! Explicitly opt-in A3 acceptance test.
//!
//! The caller supplies the already-fixed independent Codex conclusion via
//! `MAIA_A3_CODEX_CONCLUSION` before Claude is invoked. This keeps the test
//! harness from representing a fixture answer as a live Codex participant.

use maia_anthropic::AnthropicParticipant;
use maia_domain::ReasoningAssuranceLevel;
use maia_roundtable::{
    Adjudicator, DecisionRequest, ModelProvider, ModelRef, Participant, ParticipantDescriptor,
    ParticipantFailure, ParticipantId, ParticipantRequest, ParticipantResponse, RoundTableError,
    run_round_table,
};

struct CodexIndependentParticipant {
    conclusion: String,
}

impl Participant for CodexIndependentParticipant {
    fn descriptor(&self) -> ParticipantDescriptor {
        ParticipantDescriptor {
            id: ParticipantId::new("codex-local-review").expect("fixed participant id"),
            provider: ModelProvider::new("codex").expect("fixed provider"),
            model: ModelRef::new("local-independent-review").expect("fixed model reference"),
        }
    }

    fn invoke(
        &self,
        request: ParticipantRequest,
    ) -> Result<ParticipantResponse, ParticipantFailure> {
        // ParticipantRequest has no response field; this is only the common question.
        assert_eq!(request.decision.id, "m0_8_a3_context_isolation");
        Ok(ParticipantResponse {
            participant: self.descriptor(),
            response_text: self.conclusion.clone(),
            evidence: vec![],
            provider_request_id: None,
            usage: None,
            model_ref_used: None,
        })
    }
}

struct EvidenceBoundAdjudicator;

impl Adjudicator for EvidenceBoundAdjudicator {
    fn adjudicate(
        &self,
        _: &DecisionRequest,
        responses: &[ParticipantResponse],
        disagreements: &[maia_roundtable::Disagreement],
    ) -> Result<String, RoundTableError> {
        if responses.len() != 2 {
            return Err(RoundTableError::AdjudicationFailed);
        }
        let codex = responses
            .iter()
            .find(|response| response.participant.provider.as_str() == "codex")
            .ok_or(RoundTableError::AdjudicationFailed)?;
        let claude = responses
            .iter()
            .find(|response| response.participant.provider.as_str() == "anthropic")
            .ok_or(RoundTableError::AdjudicationFailed)?;
        Ok(format!(
            "Codex conclusion: {}\nClaude conclusion: {}\nExplicit response-content disagreement records: {}. This adjudication is reasoning-only and grants no execution authority.",
            codex.response_text,
            claude.response_text,
            disagreements.len()
        ))
    }
}

#[test]
#[ignore = "opt-in: requires M0_8_LIVE_ANTHROPIC=1, ANTHROPIC_API_KEY and MAIA_ANTHROPIC_MODEL"]
fn live_a3_round_table_preserves_independence_and_non_execution() {
    if std::env::var("M0_8_LIVE_ANTHROPIC").as_deref() != Ok("1") {
        return;
    }

    let codex_conclusion = std::env::var("MAIA_A3_CODEX_CONCLUSION")
        .expect("fresh Codex conclusion fixed before the Claude invocation");
    assert!(!codex_conclusion.trim().is_empty());

    let claude = AnthropicParticipant::from_environment()
        .expect("local Anthropic environment configuration");
    let session = run_round_table(
        "m0_8_a3_context_isolation_session",
        ReasoningAssuranceLevel::A3,
        DecisionRequest {
            id: "m0_8_a3_context_isolation".into(),
            subject: "Round Table first-round independence".into(),
            prompt: "Assess independently: should MAIA enforce first-round context isolation between Round Table participants in addition to provider-level isolation? Constraints: A3 is reasoning-only; a participant must not receive another participant response before adjudication; ApprovalGate remains independent. Reply in at most 120 tokens and exactly three lines: DECISION: <answer>; EVIDENCE: <reason>; RISK: <risk>. Do not provide hidden reasoning or headings.".into(),
            evidence: vec![],
        },
        &[Box::new(CodexIndependentParticipant { conclusion: codex_conclusion }), Box::new(claude)],
        &EvidenceBoundAdjudicator,
        160,
    )
    .expect("independent A3 round table result");

    assert_eq!(session.responses.len(), 2);
    assert!(!session.disagreements.is_empty());
    assert!(!session.adjudication.execution_authority);
    let claude_response = session
        .responses
        .iter()
        .find(|response| response.participant.provider.as_str() == "anthropic")
        .expect("Anthropic response retained separately");
    assert!(claude_response.provider_request_id.is_some());
    let usage = claude_response
        .usage
        .as_ref()
        .expect("returned Messages usage captured");
    assert!(usage.input_tokens.is_some());
    assert!(usage.output_tokens.is_some());
    assert!(!usage.cost_known);
    println!(
        "A3_RESULT anthropic_request_id={} input_tokens={} output_tokens={} disagreement_count={} execution_authority={} adjudication={}",
        claude_response
            .provider_request_id
            .as_deref()
            .unwrap_or("missing"),
        usage.input_tokens.unwrap_or_default(),
        usage.output_tokens.unwrap_or_default(),
        session.disagreements.len(),
        session.adjudication.execution_authority,
        session.adjudication.conclusion
    );
}
