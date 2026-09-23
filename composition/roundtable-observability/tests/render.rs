//! M0.15.4 — what an operator actually reads.
//!
//! These pin the rendered words for the distinctions that matter: absent versus
//! empty versus broken, unknown versus blank, failed versus merely unfinished,
//! and the execution-authority invariant being shown rather than assumed.

use maia_domain::ReasoningAssuranceLevel;
use maia_roundtable::{
    ContributionFailureReason, ContributionRole, ContributionStatus, ContributionView,
    InsufficientAssurance, LeaderView, QuorumReasonCode, QuorumView, SessionFailureKind,
    SessionQuery, SessionQueryError, SessionSummary, SessionView,
};
use maia_roundtable_observability::render::{
    render_missing, render_panel, render_state, render_unavailable,
};
use maia_roundtable_observability::{
    Availability, ObservabilityState, UnavailableReason, load_state, panel_from,
};

fn contribution(id: &str, served: Option<&str>, status: ContributionStatus) -> ContributionView {
    ContributionView {
        participant_id: id.into(),
        provider: "prov".into(),
        model_configured: "cfg-model".into(),
        model_ref_used: served.map(str::to_owned),
        role: ContributionRole::FirstRound,
        sequence: 0,
        first_round_isolated: true,
        status,
        duration_ms: 1234,
        provider_request_id: None,
    }
}

fn summary(decided: bool, failure: Option<SessionFailureKind>) -> SessionSummary {
    SessionSummary {
        id: "s1".into(),
        subject: "ship it?".into(),
        assurance: ReasoningAssuranceLevel::A3,
        decided,
        quorum: QuorumView {
            required_responses: 2,
            achieved_responses: if decided { 2 } else { 1 },
            attempted_participants: 2,
            satisfied: decided,
            reason_code: (!decided).then_some(QuorumReasonCode::ParticipantUnavailable),
        },
        participants_attempted: 2,
        participants_responded: if decided { 2 } else { 1 },
        leader: decided.then(|| LeaderView {
            participant_id: "alpha".into(),
            provider: "prov".into(),
            considered: 2,
            eligible: 1,
        }),
        failure,
    }
}

fn decided() -> SessionView {
    SessionView {
        summary: summary(true, None),
        prompt: "the prompt".into(),
        contributions: vec![
            contribution("alpha", Some("served-model"), ContributionStatus::Responded),
            contribution("beta", None, ContributionStatus::Responded),
        ],
        disagreement_summaries: vec![],
        conclusion: Some("they agree".into()),
        execution_authority: false,
    }
}

fn failed() -> SessionView {
    SessionView {
        summary: summary(
            false,
            Some(SessionFailureKind::QuorumNotMet(InsufficientAssurance {
                assurance: ReasoningAssuranceLevel::A3,
                required_responses: 2,
                achieved_responses: 1,
                attempted_participants: 2,
                reason_code: QuorumReasonCode::ParticipantUnavailable,
            })),
        ),
        prompt: "the prompt".into(),
        contributions: vec![
            contribution("alpha", Some("served-model"), ContributionStatus::Responded),
            contribution(
                "beta",
                None,
                ContributionStatus::Failed(ContributionFailureReason::ParticipantUnavailable),
            ),
        ],
        disagreement_summaries: vec![],
        conclusion: None,
        execution_authority: false,
    }
}

struct Listing(Vec<SessionSummary>);
impl SessionQuery for Listing {
    fn list_sessions(&self) -> Result<Vec<SessionSummary>, SessionQueryError> {
        Ok(self.0.clone())
    }
    fn get_session(&self, _: &str) -> Result<Option<SessionView>, SessionQueryError> {
        Ok(None)
    }
}

#[test]
fn absent_empty_and_broken_read_differently_and_only_broken_asks_for_action() {
    let absent = render_state(&ObservabilityState::not_installed());
    let empty = render_state(&load_state(&Listing(vec![])));
    let broken = render_state(&ObservabilityState {
        availability: Availability::Unavailable(UnavailableReason::HistoryUnreachable),
        sessions: vec![],
    });

    assert_ne!(absent, empty);
    assert_ne!(empty, broken);
    assert_ne!(absent, broken);

    assert!(absent.contains("not installed"));
    assert!(absent.contains("not a fault"));
    assert!(!absent.contains("No sessions"), "absent is not empty");
    assert!(!absent.contains("Action needed"));

    assert!(empty.contains("No sessions have been recorded yet"));
    assert!(!empty.contains("Action needed"));
    assert!(!empty.contains("not installed"));

    assert!(broken.contains("Action needed"));
    assert!(
        !broken.contains("No sessions have been recorded"),
        "a broken history must never be described as empty"
    );
}

#[test]
fn unreachable_and_corrupt_history_are_worded_differently() {
    let a = render_unavailable(&Availability::Unavailable(
        UnavailableReason::HistoryUnreachable,
    ));
    let b = render_unavailable(&Availability::Unavailable(
        UnavailableReason::HistoryCorrupt,
    ));
    assert_ne!(a, b);
    assert!(a.contains("cannot be read"));
    assert!(b.contains("none can be read"));
}

#[test]
fn a_list_names_failure_rather_than_showing_it_as_a_quiet_session() {
    let state = load_state(&Listing(vec![
        decided().summary,
        SessionSummary {
            id: "s2".into(),
            ..failed().summary
        },
    ]));
    let text = render_state(&state);
    assert!(text.contains("2 session(s)"));
    assert!(text.contains("decided, 2/2 responded"));
    assert!(text.contains("failed closed, 1/2 responded"));
    assert!(text.contains("leader: alpha"));
    assert!(text.contains("leader: none"));
}

#[test]
fn a_decided_session_shows_conclusion_and_the_basis_of_leadership() {
    let text = render_panel(&panel_from(&decided()));
    assert!(text.contains("Conclusion: they agree"));
    assert!(text.contains("1 of 2 candidates were eligible to lead"));
    assert!(text.contains("quorum satisfied, 2 of 2 required"));
    assert!(text.contains("Disagreements: none recorded"));
}

#[test]
fn an_unreported_model_is_never_replaced_by_the_configured_one() {
    let text = render_panel(&panel_from(&decided()));
    assert!(text.contains("model served: served-model"));
    // beta reported nothing: the line must say so, and must not borrow cfg-model.
    let beta = text
        .lines()
        .find(|l| l.contains("model served: not reported by provider"))
        .expect("an unreported model is stated as unreported");
    assert!(beta.contains("model configured: cfg-model"));
    assert!(!beta.contains("model served: cfg-model"));
    assert!(beta.contains("request id: not reported by provider"));
}

#[test]
fn a_failed_session_names_the_failure_and_offers_no_conclusion() {
    let text = render_panel(&panel_from(&failed()));
    assert!(text.contains("failed closed"));
    assert!(text.contains("quorum not met"));
    assert!(text.contains("unavailable"));
    assert!(text.contains("1 contribution(s) did not respond"));
    assert!(text.contains("did not reach a decision"));
    assert!(!text.contains("Conclusion: they"));
    assert!(text.contains("Leader: none"));
}

#[test]
fn execution_authority_is_stated_and_a_violation_cannot_be_missed() {
    let ok = render_panel(&panel_from(&decided()));
    assert!(ok.contains("Execution authority: none"));
    assert!(!ok.contains("INVARIANT VIOLATION"));

    let mut bad = decided();
    bad.execution_authority = true;
    let text = render_panel(&panel_from(&bad));
    assert!(text.contains("INVARIANT VIOLATION"));
    assert!(!text.contains("Execution authority: none"));
}

#[test]
fn a_missing_session_is_stated_as_missing() {
    let text = render_missing("nope");
    assert!(text.contains("No session with id `nope` was found."));
}

#[test]
fn rendering_is_deterministic() {
    let panel = panel_from(&failed());
    assert_eq!(render_panel(&panel), render_panel(&panel));
}
