//! M0.13 items 9 and 10: persistence round-trip and a full multi-provider session.
//!
//! Exercises the whole slice end to end with two interchangeable providers, a
//! third that is down, deterministic leader selection, quorum, synthesis and
//! persistence — then reloads the session from disk and checks nothing was lost.

use maia_domain::ReasoningAssuranceLevel;
use maia_roundtable::{
    Clock, ContributionFailureReason, DecisionRequest, LeaderCandidate, ModelProvider, ModelRef,
    OrchestrationFailure, Participant, ParticipantAdjudicator, ParticipantDescriptor,
    ParticipantFailure, ParticipantFailureKind, ParticipantId, ParticipantRequest,
    ParticipantResolver, ParticipantResponse, QuorumReasonCode, ResolutionFailure, RoundConfig,
    SessionRecord, SessionStore, SessionStoreError, Timestamp, orchestrate_session,
};
use maia_roundtable_store::FileSessionStore;

// ------------------------------------------------------------------- fixtures

struct StepClock(std::sync::atomic::AtomicU64);
impl Clock for StepClock {
    fn now(&self) -> Timestamp {
        Timestamp(self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst))
    }
}
fn clock() -> StepClock {
    StepClock(std::sync::atomic::AtomicU64::new(1_000))
}

fn descriptor(id: &str, provider: &str, model: &str) -> ParticipantDescriptor {
    ParticipantDescriptor {
        id: ParticipantId::new(id).unwrap(),
        provider: ModelProvider::new(provider).unwrap(),
        model: ModelRef::new(model).unwrap(),
    }
}

struct Speaker {
    descriptor: ParticipantDescriptor,
    answer: String,
    served_model: Option<String>,
}
impl Participant for Speaker {
    fn descriptor(&self) -> ParticipantDescriptor {
        self.descriptor.clone()
    }
    fn invoke(&self, _: ParticipantRequest) -> Result<ParticipantResponse, ParticipantFailure> {
        Ok(ParticipantResponse {
            participant: self.descriptor.clone(),
            response_text: self.answer.clone(),
            evidence: vec![],
            provider_request_id: Some(format!("req-{}", self.descriptor.id.as_str())),
            usage: None,
            model_ref_used: self
                .served_model
                .as_ref()
                .map(|m| ModelRef::new(m.clone()).unwrap()),
        })
    }
}

struct Down {
    descriptor: ParticipantDescriptor,
}
impl Participant for Down {
    fn descriptor(&self) -> ParticipantDescriptor {
        self.descriptor.clone()
    }
    fn invoke(&self, _: ParticipantRequest) -> Result<ParticipantResponse, ParticipantFailure> {
        Err(ParticipantFailure {
            kind: ParticipantFailureKind::Transient,
            provider_request_id: None,
        })
    }
}

/// Resolver built from an explicit table, as a real registry-backed one is.
struct Registry {
    speakers: Vec<(ParticipantDescriptor, String, Option<String>)>,
    down: Vec<ParticipantDescriptor>,
}
impl ParticipantResolver for Registry {
    fn resolve(
        &self,
        d: &ParticipantDescriptor,
    ) -> Result<Box<dyn Participant>, ResolutionFailure> {
        if self.down.iter().any(|x| x == d) {
            return Ok(Box::new(Down {
                descriptor: d.clone(),
            }));
        }
        match self.speakers.iter().find(|(x, _, _)| x == d) {
            Some((x, answer, served)) => Ok(Box::new(Speaker {
                descriptor: x.clone(),
                answer: answer.clone(),
                served_model: served.clone(),
            })),
            None => Err(ResolutionFailure::UnknownParticipant),
        }
    }
}

fn candidate(id: &str, provider: &str) -> LeaderCandidate {
    LeaderCandidate {
        descriptor: descriptor(id, provider, "leader-model"),
        enabled: true,
        can_adjudicate: true,
        supported_levels: vec![ReasoningAssuranceLevel::A3, ReasoningAssuranceLevel::A4],
        healthy: true,
    }
}

fn decision() -> DecisionRequest {
    DecisionRequest {
        id: "decision-1".into(),
        subject: "synthetic".into(),
        prompt: "synthetic non-sensitive prompt".into(),
        evidence: vec![],
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("maia-rt-store-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

// ------------------------------------------------------------------- the tests

/// D004 + D007 + D013: a real multi-provider session, persisted and reloaded.
#[test]
fn a_full_multi_provider_session_survives_a_restart_with_its_provenance() {
    let alpha = descriptor("alpha", "provider-a", "model-a");
    let bravo = descriptor("bravo", "provider-b", "configured-b");
    let charlie = descriptor("charlie", "provider-c", "model-c");

    let registry = Registry {
        speakers: vec![
            (alpha.clone(), "ship it".into(), None),
            // bravo is served by a different model than configured
            (
                bravo.clone(),
                "do not ship".into(),
                Some("actually-served-b".into()),
            ),
        ],
        // charlie is registered but its provider is down
        down: vec![charlie.clone()],
    };

    let clock = clock();
    let session = orchestrate_session(
        &RoundConfig {
            session_id: "session-e2e",
            assurance: ReasoningAssuranceLevel::A3,
            decision: &decision(),
            max_output_tokens: 64,
            clock: &clock,
        },
        &[alpha.clone(), bravo.clone(), charlie.clone()],
        &registry,
        &[
            candidate("zulu", "provider-z"),
            candidate("leader", "provider-l"),
        ],
        &ParticipantAdjudicator::new(
            Box::new(Speaker {
                descriptor: descriptor("leader", "provider-l", "leader-model"),
                answer: "they disagree on shipping".into(),
                served_model: None,
            }),
            64,
        ),
    )
    .expect("two healthy providers satisfy A3 even with a third down");

    // Three providers attempted, one down, quorum still met on the survivors.
    assert_eq!(session.outcomes.len(), 3);
    assert_eq!(
        session.outcomes[2].failure_reason(),
        Some(ContributionFailureReason::ParticipantUnavailable)
    );
    // Leader chosen by identity order, not list order: "leader" < "zulu".
    assert_eq!(session.leader.leader.id.as_str(), "leader");
    assert_eq!(session.leader.considered, 2);
    // Reasoning never authorizes action.
    assert!(!session.adjudication.execution_authority);
    // Genuine disagreement is recorded, not smoothed away.
    assert_eq!(session.disagreements.len(), 1);

    // Persist, then reload as a separate store instance: a process restart.
    let dir = temp_dir("e2e");
    let record = SessionRecord::from_session(&session);
    FileSessionStore::new(&dir).save(&record).unwrap();

    let reloaded = FileSessionStore::new(&dir)
        .load("session-e2e")
        .unwrap()
        .expect("the session must survive a restart");

    assert_eq!(reloaded, record, "the round trip must be lossless");
    assert!(reloaded.decided());
    assert_eq!(reloaded.outcomes.len(), 3);
    // The failed contribution is still there after the restart.
    assert_eq!(
        reloaded.outcomes[2].failure_reason(),
        Some(ContributionFailureReason::ParticipantUnavailable)
    );
    // And the model actually served is preserved, distinct from the configured one.
    assert_eq!(
        reloaded.outcomes[1].model_ref_used().map(|m| m.as_str()),
        Some("actually-served-b")
    );
    assert_eq!(
        reloaded.outcomes[1].participant.model.as_str(),
        "configured-b"
    );
    assert!(!reloaded.adjudication.unwrap().execution_authority);

    let _ = std::fs::remove_dir_all(&dir);
}

/// D013: a session that never reached a decision is persisted too.
#[test]
fn a_failed_session_is_persisted_with_the_contributions_it_did_collect() {
    let alpha = descriptor("alpha", "provider-a", "model-a");
    let bravo = descriptor("bravo", "provider-b", "model-b");
    let registry = Registry {
        speakers: vec![(alpha.clone(), "only voice".into(), None)],
        down: vec![bravo.clone()],
    };

    let clock = clock();
    let config = RoundConfig {
        session_id: "session-failed",
        assurance: ReasoningAssuranceLevel::A3,
        decision: &decision(),
        max_output_tokens: 64,
        clock: &clock,
    };
    let failure = orchestrate_session(
        &config,
        &[alpha, bravo],
        &registry,
        &[candidate("leader", "provider-l")],
        &ParticipantAdjudicator::new(
            Box::new(Speaker {
                descriptor: descriptor("leader", "provider-l", "leader-model"),
                answer: "unused".into(),
                served_model: None,
            }),
            64,
        ),
    )
    .unwrap_err();

    match &failure {
        OrchestrationFailure::QuorumNotMet { insufficient, .. } => assert_eq!(
            insufficient.reason_code,
            QuorumReasonCode::ParticipantUnavailable
        ),
        other => panic!("expected quorum refusal, got {other:?}"),
    }

    let dir = temp_dir("failed");
    let record = SessionRecord::from_failure(
        "session-failed",
        ReasoningAssuranceLevel::A3,
        &decision(),
        &failure,
    );
    FileSessionStore::new(&dir).save(&record).unwrap();

    let reloaded = FileSessionStore::new(&dir)
        .load("session-failed")
        .unwrap()
        .unwrap();
    assert!(!reloaded.decided());
    assert!(reloaded.adjudication.is_none());
    // The point of persisting a failure: the evidence of what went wrong.
    assert_eq!(reloaded.outcomes.len(), 2);
    assert_eq!(reloaded, record);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_session_is_absent_rather_than_an_error() {
    let dir = temp_dir("missing");
    let store = FileSessionStore::new(&dir);
    assert_eq!(store.load("never-written").unwrap(), None);
    assert_eq!(store.list().unwrap(), Vec::<String>::new());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_corrupt_record_is_refused_rather_than_partially_read() {
    let dir = temp_dir("corrupt");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("broken.json"), "{not json").unwrap();
    assert_eq!(
        FileSessionStore::new(&dir).load("broken").err(),
        Some(SessionStoreError::Corrupt)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stored_sessions_can_be_listed() {
    let dir = temp_dir("list");
    let store = FileSessionStore::new(&dir);
    for id in ["b-session", "a-session"] {
        store
            .save(&SessionRecord {
                id: id.into(),
                assurance: ReasoningAssuranceLevel::A3,
                decision: decision(),
                outcomes: vec![],
                leader: None,
                disagreements: vec![],
                adjudication: None,
                failure: None,
            })
            .unwrap();
    }
    assert_eq!(store.list().unwrap(), vec!["a-session", "b-session"]);
    let _ = std::fs::remove_dir_all(&dir);
}
