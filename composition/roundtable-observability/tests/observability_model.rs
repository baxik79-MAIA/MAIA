//! M0.15.2 — the read-only observability model.
//!
//! Two properties matter more than field-by-field mapping, and both are things
//! a renderer could plausibly get wrong:
//!
//! 1. unknown survives as unknown, rather than being filled in from a value
//!    that happens to be nearby;
//! 2. "no Round Table" is distinguishable from "no sessions", because an empty
//!    list otherwise reads as success on a broken or absent installation.

use maia_domain::ReasoningAssuranceLevel;
use maia_roundtable::{
    ContributionFailureReason, ContributionRole, ContributionStatus, ContributionView,
    InsufficientAssurance, LeaderView, QuorumReasonCode, QuorumView, SessionFailureKind,
    SessionQuery, SessionQueryError, SessionSummary, SessionView,
};
use maia_roundtable_observability::{
    Availability, ContributionOutcome, ObservabilityState, Reported, UnavailableReason, load_panel,
    load_state, panel_from,
};

// ------------------------------------------------------------------ fixtures

fn contribution(
    id: &str,
    provider: &str,
    configured: &str,
    served: Option<&str>,
    status: ContributionStatus,
    duration: u64,
) -> ContributionView {
    ContributionView {
        participant_id: id.into(),
        provider: provider.into(),
        model_configured: configured.into(),
        model_ref_used: served.map(str::to_owned),
        role: ContributionRole::FirstRound,
        sequence: 0,
        first_round_isolated: true,
        status,
        duration_ms: duration,
        provider_request_id: Some(format!("req-{id}")),
    }
}

fn decided_view() -> SessionView {
    SessionView {
        summary: SessionSummary {
            id: "s-ok".into(),
            subject: "should we ship".into(),
            assurance: ReasoningAssuranceLevel::A3,
            decided: true,
            quorum: QuorumView {
                required_responses: 2,
                achieved_responses: 2,
                attempted_participants: 2,
                satisfied: true,
                reason_code: None,
            },
            participants_attempted: 2,
            participants_responded: 2,
            leader: Some(LeaderView {
                participant_id: "claude-code".into(),
                provider: "anthropic".into(),
                considered: 2,
                eligible: 1,
            }),
            failure: None,
        },
        prompt: "synthetic prompt".into(),
        contributions: vec![
            contribution(
                "claude-code",
                "anthropic",
                "configured-model",
                Some("served-model"),
                ContributionStatus::Responded,
                15_420,
            ),
            // The local provider reports no model.
            contribution(
                "local-qwen",
                "local-loopback",
                "qwen",
                None,
                ContributionStatus::Responded,
                9_729,
            ),
        ],
        disagreement_summaries: vec![],
        conclusion: Some("they broadly agree".into()),
        execution_authority: false,
    }
}

fn failed_view() -> SessionView {
    SessionView {
        summary: SessionSummary {
            id: "s-fail".into(),
            subject: "should we ship".into(),
            assurance: ReasoningAssuranceLevel::A3,
            decided: false,
            quorum: QuorumView {
                required_responses: 2,
                achieved_responses: 1,
                attempted_participants: 2,
                satisfied: false,
                reason_code: Some(QuorumReasonCode::ParticipantUnavailable),
            },
            participants_attempted: 2,
            participants_responded: 1,
            leader: None,
            failure: Some(SessionFailureKind::QuorumNotMet(InsufficientAssurance {
                assurance: ReasoningAssuranceLevel::A3,
                required_responses: 2,
                achieved_responses: 1,
                attempted_participants: 2,
                reason_code: QuorumReasonCode::ParticipantUnavailable,
            })),
        },
        prompt: "synthetic prompt".into(),
        contributions: vec![
            contribution(
                "claude-code",
                "anthropic",
                "configured-model",
                Some("served-model"),
                ContributionStatus::Responded,
                14_931,
            ),
            contribution(
                "local-qwen",
                "local-loopback",
                "qwen",
                None,
                ContributionStatus::Failed(ContributionFailureReason::ParticipantUnavailable),
                1_999,
            ),
        ],
        disagreement_summaries: vec![],
        conclusion: None,
        execution_authority: false,
    }
}

/// Query doubles, so the model can be exercised without any storage.
struct Ok2;
impl SessionQuery for Ok2 {
    fn list_sessions(&self) -> Result<Vec<SessionSummary>, SessionQueryError> {
        Ok(vec![decided_view().summary, failed_view().summary])
    }
    fn get_session(&self, id: &str) -> Result<Option<SessionView>, SessionQueryError> {
        Ok(match id {
            "s-ok" => Some(decided_view()),
            "s-fail" => Some(failed_view()),
            _ => None,
        })
    }
}

struct Failing(SessionQueryError);
impl SessionQuery for Failing {
    fn list_sessions(&self) -> Result<Vec<SessionSummary>, SessionQueryError> {
        Err(self.0.clone())
    }
    fn get_session(&self, _: &str) -> Result<Option<SessionView>, SessionQueryError> {
        Err(self.0.clone())
    }
}

struct Empty;
impl SessionQuery for Empty {
    fn list_sessions(&self) -> Result<Vec<SessionSummary>, SessionQueryError> {
        Ok(vec![])
    }
    fn get_session(&self, _: &str) -> Result<Option<SessionView>, SessionQueryError> {
        Ok(None)
    }
}

// ------------------------------------------------------------------- tests

#[test]
fn an_unreported_model_renders_as_unreported_not_as_the_configured_one() {
    let panel = panel_from(&decided_view());

    let claude = &panel.contributions[0];
    assert_eq!(
        claude.model_served,
        Reported::Reported("served-model".into())
    );
    assert_eq!(claude.model_served.display(), "served-model");

    let local = &panel.contributions[1];
    assert_eq!(local.model_configured, "qwen");
    assert_eq!(local.model_served, Reported::NotReported);
    assert!(!local.model_served.is_reported());
    // The placeholder says WHY it is missing. A bare dash would read as an
    // oversight in the surface rather than a fact about the provider.
    assert_eq!(local.model_served.display(), "not reported by provider");
    assert_ne!(
        local.model_served.display(),
        local.model_configured,
        "the configured model must never stand in for the served one"
    );
}

#[test]
fn a_failed_contribution_is_named_and_keeps_its_timing() {
    let panel = panel_from(&failed_view());

    assert_eq!(panel.contributions.len(), 2, "both attempts are rows");
    assert!(panel.contributions[0].outcome.succeeded());

    let failed = &panel.contributions[1];
    assert_eq!(
        failed.outcome,
        ContributionOutcome::Failed(ContributionFailureReason::ParticipantUnavailable)
    );
    assert_eq!(failed.outcome.label(), "unavailable");
    // A slow failure and an instant one are different operational facts.
    assert_eq!(failed.duration_ms, 1_999);

    assert_eq!(panel.failures().count(), 1);
}

#[test]
fn a_failed_session_reads_as_failed_closed_rather_than_merely_unfinished() {
    let panel = panel_from(&failed_view());
    assert!(!panel.entry.decided);
    assert_eq!(panel.entry.status(), "failed closed, 1/2 responded");
    assert!(panel.quorum.headline().contains("quorum not met"));
    assert!(
        panel
            .quorum
            .headline()
            .contains("a participant was unavailable")
    );
    assert!(panel.leader.is_none(), "no leader on a failed panel");
    assert!(panel.conclusion.is_none(), "no synthesis on a failed panel");
    assert!(!panel.execution_authority);
}

#[test]
fn a_decided_session_shows_the_basis_of_leadership_not_only_the_leader() {
    let panel = panel_from(&decided_view());
    assert_eq!(panel.entry.status(), "decided, 2/2 responded");
    let leader = panel.leader.as_ref().expect("leader present");
    assert_eq!(leader.participant_id, "claude-code");
    assert_eq!(leader.basis(), "1 of 2 candidates were eligible to lead");
    assert!(panel.quorum.headline().contains("quorum satisfied"));
    assert!(!panel.execution_authority);
}

#[test]
fn no_round_table_is_distinguishable_from_no_sessions() {
    // The whole point: both render an empty list, and they mean opposite things.
    let absent = ObservabilityState::not_installed();
    let empty = load_state(&Empty);

    assert_eq!(absent.sessions.len(), empty.sessions.len());
    assert_ne!(
        absent.availability, empty.availability,
        "an absent Round Table must not look like one with no history yet"
    );

    assert_eq!(absent.availability, Availability::NotInstalled);
    assert!(!absent.availability.is_available());
    assert_eq!(
        absent.availability.label(),
        "Round Table is not installed in this build"
    );

    assert_eq!(empty.availability, Availability::Available);
    assert!(empty.availability.is_available());
}

#[test]
fn unreachable_history_is_not_reported_as_an_empty_history() {
    for (err, expected) in [
        (
            SessionQueryError::Unavailable,
            UnavailableReason::HistoryUnreachable,
        ),
        (
            SessionQueryError::Corrupt,
            UnavailableReason::HistoryCorrupt,
        ),
        (
            SessionQueryError::InvalidId,
            UnavailableReason::HistoryUnreachable,
        ),
    ] {
        let state = load_state(&Failing(err.clone()));
        assert_eq!(state.availability, Availability::Unavailable(expected));
        assert!(!state.availability.is_available());
        assert!(state.sessions.is_empty());
        // Crucially, this is NOT the same state as a healthy empty history.
        assert_ne!(state.availability, Availability::Available);
    }
}

#[test]
fn a_broken_installation_is_distinguishable_from_an_absent_one() {
    // An operator should act on one and not the other.
    let absent = ObservabilityState::not_installed().availability;
    let broken = load_state(&Failing(SessionQueryError::Unavailable)).availability;
    assert_ne!(absent, broken);
    assert!(absent.label().contains("not installed"));
    assert!(broken.label().contains("could not be reached"));
}

#[test]
fn listing_shows_successes_and_failures_side_by_side() {
    let state = load_state(&Ok2);
    assert!(state.availability.is_available());
    assert_eq!(state.sessions.len(), 2);
    assert!(state.sessions[0].decided);
    assert!(!state.sessions[1].decided);
    // A failed session is a listed entry with a named status, not an omission.
    assert!(state.sessions[1].failure.is_some());
    assert_eq!(state.sessions[1].status(), "failed closed, 1/2 responded");
}

#[test]
fn loading_one_session_distinguishes_missing_from_unreachable() {
    assert!(
        load_panel(&Ok2, "s-ok")
            .expect("history reachable")
            .is_some(),
        "an existing session loads"
    );
    assert_eq!(
        load_panel(&Ok2, "nope").expect("history reachable"),
        None,
        "a missing session is absent, not an error"
    );
    assert_eq!(
        load_panel(&Failing(SessionQueryError::Corrupt), "s-ok").unwrap_err(),
        Availability::Unavailable(UnavailableReason::HistoryCorrupt),
        "unreadable history is a state, not a missing session"
    );
}

#[test]
fn the_model_cannot_reach_orchestration_or_storage() {
    // Enforced by the dependency graph rather than by reviewer vigilance: if
    // this crate could reach composition or the store, a viewer built on it
    // could invoke providers or bind itself to a storage format.
    let manifest = include_str!("../Cargo.toml");
    for forbidden in [
        "maia-roundtable-composition",
        "maia-roundtable-store",
        "maia-claude-code",
        "maia-anthropic",
        "maia-local-model",
    ] {
        let declared = manifest
            .lines()
            .any(|l| l.trim_start().starts_with(forbidden));
        assert!(
            !declared,
            "observability must not depend on `{forbidden}`; it reads a contract, \
             it does not orchestrate or store"
        );
    }
    assert!(
        manifest.contains("maia-roundtable ="),
        "it must depend on the read contract, or the check above is vacuous"
    );
}

// ---------------------------------------------------------------------------
// PRIMARY read-only proof (Architecture Desk amendment, 2026-09-18)
//
// The Desk ruled that scanning the trait's source text for forbidden verbs is a
// useful secondary regression guard but must not be the primary proof. It is
// weak for a real reason: it inspects how the contract is *written*, not what a
// consumer can *do*. A method named `refresh` that invoked a provider would
// sail past it.
//
// These tests are behavioural instead. They observe what the observability
// layer actually does to its source while rendering every state, which is the
// property that matters.
// ---------------------------------------------------------------------------

use std::sync::Mutex;

/// Records every interaction the observability layer performs.
#[derive(Default)]
struct RecordingQuery {
    calls: Mutex<Vec<String>>,
}

impl SessionQuery for RecordingQuery {
    fn list_sessions(&self) -> Result<Vec<SessionSummary>, SessionQueryError> {
        self.calls.lock().unwrap().push("list_sessions".into());
        Ok(vec![decided_view().summary, failed_view().summary])
    }
    fn get_session(&self, id: &str) -> Result<Option<SessionView>, SessionQueryError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("get_session({id})"));
        Ok(match id {
            "s-ok" => Some(decided_view()),
            "s-fail" => Some(failed_view()),
            _ => None,
        })
    }
}

#[test]
fn rendering_every_state_performs_reads_and_nothing_else() {
    let spy = RecordingQuery::default();

    // Exercise the whole surface: listing, a decided session, a failed one, and
    // a missing one. If observation could invoke, retry or mutate anything, it
    // would have to do so through this contract, and the spy would see it.
    let state = load_state(&spy);
    assert!(state.availability.is_available());
    let _ = load_panel(&spy, "s-ok").expect("reachable");
    let _ = load_panel(&spy, "s-fail").expect("reachable");
    let _ = load_panel(&spy, "absent").expect("reachable");

    let calls = spy.calls.lock().unwrap().clone();
    assert_eq!(
        calls,
        vec![
            "list_sessions".to_string(),
            "get_session(s-ok)".to_string(),
            "get_session(s-fail)".to_string(),
            "get_session(absent)".to_string(),
        ],
        "observation must perform reads only, in the order asked for"
    );

    // Named explicitly so a future method that invokes, retries or mutates
    // would break this assertion rather than quietly joining the list.
    for call in &calls {
        let verb = call.split('(').next().unwrap();
        assert!(
            verb == "list_sessions" || verb == "get_session",
            "unexpected interaction `{verb}`: observation is read-only"
        );
    }
}

#[test]
fn projection_is_pure_and_repeatable() {
    // A projection that mutated hidden state, cached across calls, or depended
    // on call order would make a viewer's output depend on history rather than
    // on the session. Rendering the same view twice must be identical.
    let view = decided_view();
    assert_eq!(panel_from(&view), panel_from(&view));

    // And the source view is unchanged by being projected.
    let before = decided_view();
    let _ = panel_from(&before);
    assert_eq!(
        before,
        decided_view(),
        "projection must not mutate its input"
    );
}

#[test]
fn a_failing_source_still_yields_only_read_attempts() {
    // Even on the error path, the layer must not try anything other than
    // reading: no retry loop, no fallback query, no repair attempt.
    struct CountingFailure {
        attempts: Mutex<usize>,
    }
    impl SessionQuery for CountingFailure {
        fn list_sessions(&self) -> Result<Vec<SessionSummary>, SessionQueryError> {
            *self.attempts.lock().unwrap() += 1;
            Err(SessionQueryError::Unavailable)
        }
        fn get_session(&self, _: &str) -> Result<Option<SessionView>, SessionQueryError> {
            *self.attempts.lock().unwrap() += 1;
            Err(SessionQueryError::Unavailable)
        }
    }

    let source = CountingFailure {
        attempts: Mutex::new(0),
    };
    let state = load_state(&source);
    assert_eq!(
        state.availability,
        Availability::Unavailable(UnavailableReason::HistoryUnreachable)
    );
    assert_eq!(
        *source.attempts.lock().unwrap(),
        1,
        "a failed read must not be retried; retrying is an action, not an observation"
    );

    let _ = load_panel(&source, "anything");
    assert_eq!(
        *source.attempts.lock().unwrap(),
        2,
        "one attempt per request, no retry"
    );
}
