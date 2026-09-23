//! M0.14 persistence + audit verification: proves that a session realized
//! through `composition::realize_plan` — not `orchestrate_session` called
//! directly — persists losslessly and that the returned
//! `AssuranceAuditRecord` genuinely corresponds to what gets persisted and
//! reloaded. `infra/roundtable-store`'s own end-to-end test already covers
//! persistence at the `orchestrate_session`/`SessionRecord` level; this file
//! covers the composition-specific gap: that `RealizedSession.outcome` (not
//! a second, redundant orchestration run) is what a caller persists, and
//! that `RealizedSession.audit` stays consistent with it after a reload.
//!
//! No live provider is involved anywhere in this file — every participant is
//! a deterministic fake, so this runs under the ordinary `cargo test
//! --workspace` invocation with no opt-in required.

use maia_assurance_router::{
    AssuranceFailure, AssurancePlan, AvailabilityHealth, OrchestrationPath,
    ParticipantRegistration, ParticipantRegistry, ParticipantRole,
};
use maia_domain::ReasoningAssuranceLevel;
use maia_roundtable::{
    Clock, DecisionRequest, EvidenceReference, Participant, ParticipantAdjudicator,
    ParticipantDescriptor, ParticipantFailure, ParticipantFailureKind, ParticipantRequest,
    ParticipantResolver, ParticipantResponse, ResolutionFailure, SessionRecord, SessionStore,
    Timestamp,
};
use maia_roundtable_composition::realize_plan;
use maia_roundtable_store::FileSessionStore;
use std::sync::atomic::{AtomicU64, Ordering};

struct TestClock(AtomicU64);
impl TestClock {
    fn new() -> Self {
        Self(AtomicU64::new(1_000))
    }
}
impl Clock for TestClock {
    fn now(&self) -> Timestamp {
        Timestamp(self.0.fetch_add(1, Ordering::SeqCst))
    }
}

struct Fake {
    descriptor: ParticipantDescriptor,
    answer: &'static str,
}
impl Participant for Fake {
    fn descriptor(&self) -> ParticipantDescriptor {
        self.descriptor.clone()
    }
    fn invoke(&self, _: ParticipantRequest) -> Result<ParticipantResponse, ParticipantFailure> {
        Ok(ParticipantResponse {
            participant: self.descriptor.clone(),
            response_text: self.answer.into(),
            evidence: vec![],
            provider_request_id: Some(format!("req-{}", self.descriptor.id.as_str())),
            usage: None,
            model_ref_used: None,
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

/// Resolves every descriptor in `up` to a `Fake`, `down` to a `Down`, and
/// refuses anything else.
struct FixedResolver {
    up: Vec<(ParticipantDescriptor, &'static str)>,
    down: Vec<ParticipantDescriptor>,
}
impl ParticipantResolver for FixedResolver {
    fn resolve(
        &self,
        descriptor: &ParticipantDescriptor,
    ) -> Result<Box<dyn Participant>, ResolutionFailure> {
        if let Some((d, answer)) = self.up.iter().find(|(d, _)| d == descriptor) {
            return Ok(Box::new(Fake {
                descriptor: d.clone(),
                answer,
            }));
        }
        if let Some(d) = self.down.iter().find(|d| *d == descriptor) {
            return Ok(Box::new(Down {
                descriptor: d.clone(),
            }));
        }
        Err(ResolutionFailure::UnknownParticipant)
    }
}

fn registration(id: &str, roles: Vec<ParticipantRole>) -> ParticipantRegistration {
    ParticipantRegistration {
        id: id.into(),
        provider: format!("provider-{id}"),
        model_ref: format!("model-{id}"),
        role_capabilities: roles,
        enabled: true,
        assurance_levels: vec![ReasoningAssuranceLevel::A3],
        cost_metadata_capability: false,
        availability_health: AvailabilityHealth::Available,
    }
}

fn plan() -> AssurancePlan {
    AssurancePlan {
        required: ReasoningAssuranceLevel::A3,
        path: OrchestrationPath::A3RoundTable,
        reasons: vec![],
        execution_authority: false,
    }
}

fn decision(id: &str) -> DecisionRequest {
    DecisionRequest {
        id: id.into(),
        subject: "persistence test".into(),
        prompt: "synthetic non-sensitive prompt".into(),
        evidence: vec![EvidenceReference {
            id: "ev-1".into(),
            source_ref: "src-1".into(),
        }],
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "maia-composition-persist-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn a_realized_session_persists_losslessly_and_the_audit_record_matches_the_reload() {
    let registry = ParticipantRegistry {
        participants: vec![
            registration("one", vec![ParticipantRole::RoundTableMember]),
            registration("two", vec![ParticipantRole::RoundTableMember]),
            registration("leader", vec![ParticipantRole::Adjudicator]),
        ],
    };
    let descriptors = maia_roundtable_composition::round_table_descriptors(
        &registry,
        ReasoningAssuranceLevel::A3,
    )
    .unwrap();
    let leader_descriptor = maia_roundtable_composition::leader_candidates(&registry)
        .unwrap()
        .into_iter()
        .find(|c| c.descriptor.id.as_str() == "leader")
        .unwrap()
        .descriptor;

    let resolver = FixedResolver {
        up: vec![
            (descriptors[0].clone(), "ship it"),
            (descriptors[1].clone(), "do not ship"),
            (leader_descriptor.clone(), "they disagree on shipping"),
        ],
        down: vec![],
    };
    let leader = resolver.resolve(&leader_descriptor).unwrap();
    let adjudicator = ParticipantAdjudicator::new(leader, 64);

    let clock = TestClock::new();
    let plan = plan();
    let session_id = "session-persist-1";
    let decision = decision(session_id);

    let realized = realize_plan(
        session_id,
        &plan,
        &decision,
        &registry,
        &resolver,
        &adjudicator,
        64,
        &clock,
    )
    .unwrap();
    let session = realized.outcome.as_ref().expect("both fakes respond");

    let dir = temp_dir("decided");
    let record = SessionRecord::from_session(session);
    FileSessionStore::new(&dir).save(&record).unwrap();
    let reloaded = FileSessionStore::new(&dir)
        .load(session_id)
        .unwrap()
        .expect("the just-saved session reloads");
    assert_eq!(reloaded, record, "the round trip must be lossless");

    // The audit record is not a second, independent account of the session:
    // every fact it carries must correspond to what actually got persisted.
    let audit = &realized.audit;
    assert_eq!(
        audit.round_table_session_id.as_deref(),
        Some(reloaded.id.as_str())
    );
    assert_eq!(
        audit.selected_leader.as_deref(),
        reloaded.leader.as_ref().map(|l| l.leader.id.as_str())
    );
    assert_eq!(audit.quorum_satisfied, Some(true));
    assert!(reloaded.decided());
    assert_eq!(audit.attempted_participants.len(), reloaded.outcomes.len());
    assert_eq!(
        audit.contribution_outcomes.len(),
        reloaded.outcomes.len(),
        "every persisted contribution has a corresponding audit summary"
    );
    for outcome in &reloaded.outcomes {
        let summary = audit
            .contribution_outcomes
            .iter()
            .find(|c| c.participant_id == outcome.participant.id.as_str())
            .expect("every reloaded outcome has an audit summary");
        assert_eq!(summary.responded, outcome.responded());
    }
    assert_eq!(
        audit.adjudication.as_deref(),
        reloaded
            .adjudication
            .as_ref()
            .map(|a| a.conclusion.as_str())
    );
    assert!(!audit.execution_authority);
    assert!(!reloaded.adjudication.as_ref().unwrap().execution_authority);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_realization_still_persists_and_the_audit_record_matches_the_reload() {
    let registry = ParticipantRegistry {
        participants: vec![
            registration("one", vec![ParticipantRole::RoundTableMember]),
            registration("two", vec![ParticipantRole::RoundTableMember]),
            registration("leader", vec![ParticipantRole::Adjudicator]),
        ],
    };
    let descriptors = maia_roundtable_composition::round_table_descriptors(
        &registry,
        ReasoningAssuranceLevel::A3,
    )
    .unwrap();
    let leader_descriptor = maia_roundtable_composition::leader_candidates(&registry)
        .unwrap()
        .into_iter()
        .find(|c| c.descriptor.id.as_str() == "leader")
        .unwrap()
        .descriptor;

    // "two" is down: only one independent response, below the A3 minimum.
    let resolver = FixedResolver {
        up: vec![
            (descriptors[0].clone(), "only voice"),
            (leader_descriptor.clone(), "unused"),
        ],
        down: vec![descriptors[1].clone()],
    };
    let leader = resolver.resolve(&leader_descriptor).unwrap();
    let adjudicator = ParticipantAdjudicator::new(leader, 64);

    let clock = TestClock::new();
    let plan = plan();
    let session_id = "session-persist-failed-1";
    let decision = decision(session_id);

    let realized = realize_plan(
        session_id,
        &plan,
        &decision,
        &registry,
        &resolver,
        &adjudicator,
        64,
        &clock,
    )
    .unwrap();
    let failure = realized
        .outcome
        .as_ref()
        .expect_err("quorum cannot be met with only one independent response");

    let dir = temp_dir("failed");
    let record = SessionRecord::from_failure(session_id, plan.required, &decision, failure);
    FileSessionStore::new(&dir).save(&record).unwrap();
    let reloaded = FileSessionStore::new(&dir)
        .load(session_id)
        .unwrap()
        .expect("the just-saved failed session reloads");
    assert_eq!(reloaded, record, "the round trip must be lossless");
    assert!(!reloaded.decided());
    assert!(reloaded.failure.is_some());
    assert!(
        reloaded.adjudication.is_none(),
        "no synthesis on a failed panel"
    );

    let audit = &realized.audit;
    assert_eq!(
        audit.round_table_session_id.as_deref(),
        Some(reloaded.id.as_str())
    );
    assert_eq!(audit.quorum_satisfied, Some(false));
    assert_eq!(
        audit.failure_reason,
        Some(AssuranceFailure::ParticipantUnavailable)
    );
    assert!(audit.adjudication.is_none());
    assert!(!audit.execution_authority);
    assert_eq!(audit.attempted_participants.len(), reloaded.outcomes.len());

    let _ = std::fs::remove_dir_all(&dir);
}
