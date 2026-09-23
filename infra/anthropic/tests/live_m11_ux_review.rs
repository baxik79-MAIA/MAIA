//! Opt-in independent M0.11 product/UX review; excludes Codex recommendations.
use maia_anthropic::AnthropicParticipant;
use maia_roundtable::{DecisionRequest, Participant, ParticipantRequest};

#[test]
#[ignore = "opt-in: requires Anthropic environment"]
fn independent_m11_alpha1_product_ux_review() {
    if std::env::var("M0_8_LIVE_ANTHROPIC").as_deref() != Ok("1") {
        return;
    }
    let participant = AnthropicParticipant::from_environment().expect("Anthropic configuration");
    let prompt = "You are an independent product and UX reviewer. Review this first local Windows Alpha 1 without seeing any engineer opinion. Intended flow: open a workspace, explicitly enter a local .txt/.md path, import a hash-bound immutable snapshot, ask a local Qwen evidence question, inspect citations, close/reopen and see history. Current UI: top bar has Open/refresh workspace and Refresh local status; left pane has workspace UUID, SQLite path and explicit evidence path/import; center has question box and Ask MAIA locally; right pane lists history and packet citations. It states local loopback only, no cloud fallback, no connector/action path, execution authority false. Synthetic evidence says advisory briefings are non-executable. Limitations: typed path only, 256 KiB files, UI-thread consultation, no chooser/search/sync. Provide concise structured independent review for A-P: first impression, hierarchy, discoverability, first run, workspace/evidence model, Ask MAIA, citations, history, offline trust, errors, feminine persona, accessibility, commercial feel, jargon, top blockers, beta improvements. For each significant issue give priority BLOCKER/HIGH/MEDIUM/LOW, observed problem, why it matters, concrete reversible improvement. Do not redesign architecture and do not suggest cloud/connectors/actions.";
    let response = participant
        .invoke(ParticipantRequest {
            session_id: "m11_alpha1_ux_review".into(),
            max_output_tokens: 900,
            decision: DecisionRequest {
                id: "m11_alpha1_ux_review".into(),
                subject: "M0.11 Alpha 1 local desktop UX".into(),
                evidence: vec![],
                prompt: prompt.into(),
            },
        })
        .expect("Messages response");
    let usage = response.usage.expect("usage");
    println!(
        "M11_UX_REVIEW request_id={} input_tokens={} output_tokens={} review={}",
        response
            .provider_request_id
            .unwrap_or_else(|| "missing".into()),
        usage.input_tokens.unwrap_or_default(),
        usage.output_tokens.unwrap_or_default(),
        response.response_text
    );
}
