//! M0.15.1 — the Round Table read/query contract over persisted sessions.
//!
//! Architecture Desk, 2026-09-18: a product surface must depend on the Round
//! Table's public read contract, never on persistence files or the storage
//! format. These tests exercise the contract, and one of them asserts that the
//! contract cannot express authority even if a future viewer wanted it to.

use maia_domain::ReasoningAssuranceLevel;
use maia_roundtable::{
    ContributionFailureReason, ContributionResult, ContributionRole, ContributionStatus,
    DecisionRequest, InsufficientAssurance, LeaderSelection, ModelProvider, ModelRef,
    ParticipantDescriptor, ParticipantFailureKind, ParticipantId, ParticipantOutcome,
    ParticipantResponse, QuorumReasonCode, RoundTableDecision, SessionFailureKind, SessionQuery,
    SessionQueryError, SessionRecord, SessionStore, Timestamp,
};
use maia_roundtable_store::{FileSessionStore, StoredSessionQuery};

fn descriptor(id: &str, provider: &str, model: &str) -> ParticipantDescriptor {
    ParticipantDescriptor {
        id: ParticipantId::new(id).unwrap(),
        provider: ModelProvider::new(provider).unwrap(),
        model: ModelRef::new(model).unwrap(),
    }
}

fn responded(
    id: &str,
    provider: &str,
    configured: &str,
    served: Option<&str>,
    seq: u32,
    start: u64,
    end: u64,
) -> ParticipantOutcome {
    let d = descriptor(id, provider, configured);
    ParticipantOutcome {
        participant: d.clone(),
        role: ContributionRole::FirstRound,
        sequence: seq,
        started_at: Timestamp(start),
        finished_at: Timestamp(end),
        first_round_isolated: true,
        result: ContributionResult::Responded(ParticipantResponse {
            participant: d,
            response_text: format!("{id} says something"),
            evidence: vec![],
            provider_request_id: Some(format!("req-{id}")),
            usage: None,
            model_ref_used: served.map(|m| ModelRef::new(m).unwrap()),
        }),
    }
}

fn failed(id: &str, provider: &str, seq: u32, start: u64, end: u64) -> ParticipantOutcome {
    ParticipantOutcome {
        participant: descriptor(id, provider, "configured-model"),
        role: ContributionRole::FirstRound,
        sequence: seq,
        started_at: Timestamp(start),
        finished_at: Timestamp(end),
        first_round_isolated: true,
        result: ContributionResult::Failed {
            kind: ParticipantFailureKind::Transient,
            reason: ContributionFailureReason::ParticipantUnavailable,
            provider_request_id: None,
        },
    }
}

fn decision() -> DecisionRequest {
    DecisionRequest {
        id: "d1".into(),
        subject: "should we ship".into(),
        prompt: "synthetic non-sensitive prompt".into(),
        evidence: vec![],
    }
}

/// Mirrors the shape of the real M0.14 live success session.
fn decided_record() -> SessionRecord {
    let leader = descriptor("claude-code", "anthropic", "configured-model");
    SessionRecord {
        id: "session-decided".into(),
        assurance: ReasoningAssuranceLevel::A3,
        decision: decision(),
        outcomes: vec![
            responded(
                "claude-code",
                "anthropic",
                "configured-model",
                Some("served-model"),
                0,
                1_000,
                16_420,
            ),
            // The local provider reports no model: unknown must stay unknown.
            responded(
                "local-qwen",
                "local-loopback",
                "qwen",
                None,
                1,
                16_420,
                26_149,
            ),
        ],
        leader: Some(LeaderSelection {
            leader,
            considered: 2,
            eligible: 1,
            assurance: ReasoningAssuranceLevel::A3,
        }),
        disagreements: vec![],
        adjudication: Some(RoundTableDecision {
            session_id: "session-decided".into(),
            conclusion: "they broadly agree".into(),
            evidence: vec![],
            disagreement_ids: vec![],
            execution_authority: false,
        }),
        failure: None,
    }
}

/// Mirrors the shape of the real M0.14 live failure session.
fn failed_record() -> SessionRecord {
    SessionRecord {
        id: "session-failed".into(),
        assurance: ReasoningAssuranceLevel::A3,
        decision: decision(),
        outcomes: vec![
            responded(
                "claude-code",
                "anthropic",
                "configured-model",
                Some("served-model"),
                0,
                1_000,
                15_931,
            ),
            failed("local-qwen", "local-loopback", 1, 15_931, 17_930),
        ],
        leader: None,
        disagreements: vec![],
        adjudication: None,
        failure: Some(SessionFailureKind::QuorumNotMet(InsufficientAssurance {
            assurance: ReasoningAssuranceLevel::A3,
            required_responses: 2,
            achieved_responses: 1,
            attempted_participants: 2,
            reason_code: QuorumReasonCode::ParticipantUnavailable,
        })),
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("maia-obs-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn seeded(name: &str, records: &[SessionRecord]) -> std::path::PathBuf {
    let dir = temp_dir(name);
    let store = FileSessionStore::new(&dir);
    for r in records {
        store.save(r).expect("seed session");
    }
    dir
}

// ---------------------------------------------------------------- the tests

#[test]
fn a_decided_session_renders_everything_a_viewer_needs() {
    let dir = seeded("decided", &[decided_record()]);
    let view = StoredSessionQuery::new(&dir)
        .get_session("session-decided")
        .expect("history reachable")
        .expect("session present");

    assert_eq!(view.summary.id, "session-decided");
    assert_eq!(view.summary.subject, "should we ship");
    assert_eq!(view.summary.assurance, ReasoningAssuranceLevel::A3);
    assert!(view.summary.decided);
    assert_eq!(view.summary.participants_attempted, 2);
    assert_eq!(view.summary.participants_responded, 2);

    let quorum = &view.summary.quorum;
    assert!(quorum.satisfied);
    assert_eq!(
        (quorum.required_responses, quorum.achieved_responses),
        (2, 2)
    );
    assert_eq!(quorum.reason_code, None);

    let leader = view.summary.leader.as_ref().expect("leader recorded");
    assert_eq!(leader.participant_id, "claude-code");
    // The basis of the choice, not just the winner.
    assert_eq!((leader.considered, leader.eligible), (2, 1));

    assert_eq!(view.conclusion.as_deref(), Some("they broadly agree"));
    assert!(!view.execution_authority);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unreported_model_stays_unknown_rather_than_being_filled_in() {
    let dir = seeded("unknown-model", &[decided_record()]);
    let view = StoredSessionQuery::new(&dir)
        .get_session("session-decided")
        .unwrap()
        .unwrap();

    let claude = &view.contributions[0];
    assert_eq!(claude.model_configured, "configured-model");
    assert_eq!(claude.model_ref_used.as_deref(), Some("served-model"));

    let local = &view.contributions[1];
    assert_eq!(local.model_configured, "qwen");
    assert_eq!(
        local.model_ref_used, None,
        "the local provider reported no model; showing the configured one would \
         assert something nobody observed"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_contribution_is_a_record_not_a_gap() {
    let dir = seeded("failed", &[failed_record()]);
    let view = StoredSessionQuery::new(&dir)
        .get_session("session-failed")
        .unwrap()
        .unwrap();

    assert!(!view.summary.decided);
    assert_eq!(view.summary.participants_attempted, 2);
    assert_eq!(view.summary.participants_responded, 1);

    // Both contributions are present; the failure is one of them, not a hole.
    assert_eq!(view.contributions.len(), 2);
    assert_eq!(view.contributions[0].status, ContributionStatus::Responded);
    assert_eq!(
        view.contributions[1].status,
        ContributionStatus::Failed(ContributionFailureReason::ParticipantUnavailable)
    );
    // Timing survives for the failed attempt too, so a slow failure is
    // distinguishable from an instant one.
    assert_eq!(view.contributions[1].duration_ms, 1_999);

    let quorum = &view.summary.quorum;
    assert!(!quorum.satisfied);
    assert_eq!(
        (quorum.required_responses, quorum.achieved_responses),
        (2, 1)
    );
    assert_eq!(
        quorum.reason_code,
        Some(QuorumReasonCode::ParticipantUnavailable)
    );

    assert!(view.summary.leader.is_none(), "no leader on a failed panel");
    assert!(view.conclusion.is_none(), "no synthesis on a failed panel");
    assert!(!view.execution_authority);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sessions_can_be_listed_with_both_outcomes_visible() {
    let dir = seeded("list", &[decided_record(), failed_record()]);
    let mut summaries = StoredSessionQuery::new(&dir).list_sessions().unwrap();
    summaries.sort_by(|a, b| a.id.cmp(&b.id));

    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].id, "session-decided");
    assert!(summaries[0].decided);
    assert_eq!(summaries[1].id, "session-failed");
    assert!(!summaries[1].decided);
    // A failed session is listed as a first-class entry, not omitted.
    assert!(summaries[1].failure.is_some());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_session_is_absent_not_an_error() {
    let dir = seeded("missing", &[]);
    assert_eq!(
        StoredSessionQuery::new(&dir)
            .get_session("never-written")
            .unwrap(),
        None
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_corrupt_record_is_refused_when_asked_for_but_does_not_blank_the_list() {
    let dir = seeded("corrupt", &[decided_record()]);
    std::fs::write(dir.join("broken.json"), "{not json").unwrap();
    let query = StoredSessionQuery::new(&dir);

    assert_eq!(
        query.get_session("broken").err(),
        Some(SessionQueryError::Corrupt),
        "a damaged record must be refused, never partially rendered"
    );
    // ...but the healthy session is still listed. A viewer that shows nothing
    // because one file is damaged is worse than one that shows the rest.
    let summaries = query.list_sessions().unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, "session-decided");

    let _ = std::fs::remove_dir_all(&dir);
}

/// SECONDARY regression guard only.
///
/// Architecture Desk, 2026-09-18: a source-text scan for forbidden verbs is
/// useful but must not be the primary proof of read-only authority, and the
/// Desk is right about why. This inspects how the contract is *written*, not
/// what a consumer can *do*: a method named `refresh` that invoked a provider
/// would sail straight past it.
///
/// The primary proofs are behavioural and live elsewhere:
///   * observing_a_real_store_leaves_every_byte_unchanged (below) — real
///     persistence is byte-identical after every observation path;
///   * observation_creates_no_store_directory_when_history_is_absent (below);
///   * rendering_every_state_performs_reads_and_nothing_else, and
///     a_failing_source_still_yields_only_read_attempts, in the observability
///     crate — a spy records that only the two read methods are ever called,
///     and that a failed read is not retried;
///   * observability_cannot_reach_orchestration_or_storage in the D009 suite —
///     dependency direction, walked transitively.
///
/// This test catches a careless rename. It does not establish the property.
#[test]
fn the_read_contract_names_no_mutating_operation() {
    let source = include_str!("../../../roundtable/src/lib.rs");
    let trait_start = source
        .find("pub trait SessionQuery")
        .expect("SessionQuery trait exists");
    let trait_body = &source[trait_start
        ..trait_start
            + source[trait_start..]
                .find('}')
                .expect("trait body is delimited")];

    for forbidden in [
        "invoke",
        "retry",
        "save",
        "delete",
        "mutate",
        "approve",
        "authorize",
        "execute",
        "run",
    ] {
        assert!(
            !trait_body.contains(forbidden),
            "SessionQuery must stay observational; `{forbidden}` appears in its definition"
        );
    }
    // And it really does offer the two read methods, so the check above is not
    // passing merely because the trait is empty.
    assert!(trait_body.contains("fn list_sessions"));
    assert!(trait_body.contains("fn get_session"));
}

// ---------------------------------------------------------------------------
// PRIMARY read-only proof against real persistence
// (Architecture Desk amendment, 2026-09-18)
//
// The source-text check above is a secondary regression guard, not the proof.
// This is the proof: run the observability layer over a real store directory
// and show that every byte on disk is unchanged afterwards. A contract that
// could mutate, retry or invoke would have to leave a trace here.
// ---------------------------------------------------------------------------

fn dir_fingerprint(dir: &std::path::Path) -> Vec<(String, u64, Vec<u8>)> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)
        .expect("store dir readable")
        .flatten()
    {
        let path = entry.path();
        let bytes = std::fs::read(&path).unwrap_or_default();
        out.push((
            path.file_name().unwrap().to_string_lossy().into_owned(),
            bytes.len() as u64,
            bytes,
        ));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn observing_a_real_store_leaves_every_byte_unchanged() {
    use maia_roundtable_observability::{load_panel, load_state};

    let dir = seeded("readonly-proof", &[decided_record(), failed_record()]);
    let before = dir_fingerprint(&dir);
    assert_eq!(before.len(), 2, "two sessions were seeded");

    let query = StoredSessionQuery::new(&dir);

    // Exercise every observation path there is, repeatedly, including the
    // error and missing paths.
    for _ in 0..3 {
        let state = load_state(&query);
        assert!(state.availability.is_available());
        assert_eq!(state.sessions.len(), 2);
        let _ = load_panel(&query, "session-decided").expect("reachable");
        let _ = load_panel(&query, "session-failed").expect("reachable");
        let _ = load_panel(&query, "does-not-exist").expect("reachable");
    }

    let after = dir_fingerprint(&dir);
    assert_eq!(
        before, after,
        "observation must not create, delete, rewrite or touch any stored byte"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn observation_creates_no_store_directory_when_history_is_absent() {
    use maia_roundtable_observability::load_state;

    // Reading a history that does not exist must not bring one into being. A
    // viewer that created an empty store would be writing through a read-only
    // contract, and would also make "never used" indistinguishable from
    // "used and empty" on the next look.
    let dir = temp_dir("readonly-absent");
    assert!(!dir.exists());

    let state = load_state(&StoredSessionQuery::new(&dir));
    assert!(state.sessions.is_empty());
    assert!(
        !dir.exists(),
        "observing absent history must not create the store directory"
    );
}
