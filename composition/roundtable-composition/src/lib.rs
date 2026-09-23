//! Composition layer: realizes an `AssurancePlan` against the Round Table
//! runtime and turns the result into an assurance audit record.
//!
//! This is the smallest layer outside MAIA Core that connects
//! `AssurancePlan` -> participant resolution -> a Round Table request ->
//! `orchestrate_session` -> result -> `AssuranceAuditRecord`. Dependency
//! direction is composition -> `maia-assurance-router` and composition ->
//! `maia-roundtable`, never the reverse, and MAIA Core never depends on this
//! crate or on the Round Table runtime it composes (ADR-0046, D009).
//!
//! The router's `AssurancePlan` is authoritative: this crate realizes it
//! exactly as planned and never reinterprets a required level into a weaker
//! panel. It also never branches on a provider name — every participant is
//! addressed only through the registry-declared role capabilities and the
//! provider-neutral `Participant`/`ParticipantResolver` ports; provider-
//! specific behaviour belongs in adapters outside this crate entirely.
#![forbid(unsafe_code)]

use maia_assurance_router::{
    AssuranceAuditRecord, AssuranceFailure, AssurancePlan, AvailabilityHealth,
    ContributionOutcomeSummary, OrchestrationPath, ParticipantRegistration, ParticipantRegistry,
    ParticipantRole, UsageCostRecord,
};
use maia_domain::ReasoningAssuranceLevel;
use maia_roundtable::{
    Adjudicator, Clock, ContributionFailureReason, DecisionRequest, LeaderCandidate, ModelProvider,
    ModelRef, OrchestratedSession, OrchestrationFailure, ParticipantDescriptor, ParticipantId,
    ParticipantOutcome, ParticipantResolver, RoundConfig, orchestrate_session,
};

/// Why the composition layer could not realize a plan at all, before any
/// Round Table session was attempted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositionError {
    /// The plan's path is not a Round Table path (A0-A2). Composition only
    /// realizes A3/A4 plans; the router's other paths are the caller's
    /// responsibility to realize through their own, non-Round-Table route.
    NotARoundTablePlan,
    /// A registration's identity, provider, or model reference could not be
    /// turned into a valid Round Table identity (e.g. empty after
    /// trimming). The registration is at fault, not the plan.
    InvalidRegistration { registration_id: String },
}

/// Turn registry registrations eligible for the Round Table member role into
/// the descriptors `orchestrate_session` needs for its first round.
///
/// Eligibility mirrors what the router itself already requires to grant the
/// level (`ParticipantRegistry::satisfies`): enabled, available, and
/// declaring both the level and the `RoundTableMember` role. Registration
/// identity (`ParticipantRegistration::id`) is carried through unchanged as
/// the descriptor's identity; it is never replaced by anything a resolver or
/// provider reports at runtime; that distinct, later identity
/// (`model_ref_used`) belongs to the response, not the descriptor.
pub fn round_table_descriptors(
    registry: &ParticipantRegistry,
    level: ReasoningAssuranceLevel,
) -> Result<Vec<ParticipantDescriptor>, CompositionError> {
    registry
        .participants
        .iter()
        .filter(|reg| {
            reg.enabled
                && reg.availability_health == AvailabilityHealth::Available
                && reg.assurance_levels.contains(&level)
                && reg
                    .role_capabilities
                    .contains(&ParticipantRole::RoundTableMember)
        })
        .map(descriptor_from_registration)
        .collect()
}

/// Turn every registration into a leader candidate, letting
/// `select_leader`'s own eligibility rules (enabled, adjudicator capability,
/// supports the level, healthy) decide who qualifies.
///
/// Every registration is offered, not just the ones already carrying the
/// `Adjudicator` role: `LeaderSelection::considered` is meant to reflect how
/// many identities were actually weighed, and pre-filtering here would
/// silently shrink that provenance.
pub fn leader_candidates(
    registry: &ParticipantRegistry,
) -> Result<Vec<LeaderCandidate>, CompositionError> {
    registry
        .participants
        .iter()
        .map(|reg| {
            let descriptor = descriptor_from_registration(reg)?;
            Ok(LeaderCandidate {
                descriptor,
                enabled: reg.enabled,
                can_adjudicate: reg
                    .role_capabilities
                    .contains(&ParticipantRole::Adjudicator),
                supported_levels: reg.assurance_levels.clone(),
                healthy: reg.availability_health == AvailabilityHealth::Available,
            })
        })
        .collect()
}

fn descriptor_from_registration(
    reg: &ParticipantRegistration,
) -> Result<ParticipantDescriptor, CompositionError> {
    let invalid = || CompositionError::InvalidRegistration {
        registration_id: reg.id.clone(),
    };
    Ok(ParticipantDescriptor {
        id: ParticipantId::new(reg.id.clone()).map_err(|_| invalid())?,
        provider: ModelProvider::new(reg.provider.clone()).map_err(|_| invalid())?,
        model: ModelRef::new(reg.model_ref.clone()).map_err(|_| invalid())?,
    })
}

/// A completed realization attempt: the underlying Round Table outcome,
/// alongside the assurance audit record derived from it.
///
/// Kept together rather than the audit record alone, because a caller that
/// persists Round Table history (`SessionStore`) needs the raw
/// `OrchestratedSession`/`OrchestrationFailure` to build a `SessionRecord`.
/// Discarding it after deriving the audit record would force a second,
/// redundant orchestration run — invoking live providers twice for one
/// logical session — just to recover it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealizedSession {
    pub outcome: Result<OrchestratedSession, OrchestrationFailure>,
    pub audit: AssuranceAuditRecord,
}

/// Realize an `AssurancePlan` against the Round Table: resolve participants
/// from `registry`, run `orchestrate_session`, and record the result as an
/// `AssuranceAuditRecord`.
///
/// Only A3/A4 plans are accepted (`CompositionError::NotARoundTablePlan`
/// otherwise) — the router's plan is authoritative, and this function
/// realizes exactly the level it names rather than substituting a lighter
/// path. `session_id` is recorded on the returned audit record even when the
/// session fails, so a failed attempt stays traceable.
///
/// The registry is a source of *possibility*, never of *authority*: it is
/// read only through `plan.required`, which is the one and only level this
/// function ever asks the registry to satisfy. `AssurancePlan` carries no
/// concrete participant selection today (only a required level), so nothing
/// here narrows or widens who is eligible beyond what `round_table_descriptors`
/// and `leader_candidates` already compute from `(registry, plan.required)`
/// alone — a pure function of those two inputs, with no other source of
/// participants and no path that adds one outside the registry. If
/// `AssurancePlan` ever grows an explicit participant selection, this
/// function must be revisited so that selection becomes the sole source of
/// the descriptor list, not an addition to it.
#[allow(clippy::too_many_arguments)]
pub fn realize_plan(
    session_id: &str,
    plan: &AssurancePlan,
    decision: &DecisionRequest,
    registry: &ParticipantRegistry,
    resolver: &dyn ParticipantResolver,
    adjudicator: &dyn Adjudicator,
    max_output_tokens: u32,
    clock: &dyn Clock,
) -> Result<RealizedSession, CompositionError> {
    if !matches!(
        plan.path,
        OrchestrationPath::A3RoundTable | OrchestrationPath::A4RoundTableAndHumanAcceptance
    ) {
        return Err(CompositionError::NotARoundTablePlan);
    }
    let descriptors = round_table_descriptors(registry, plan.required)?;
    let candidates = leader_candidates(registry)?;
    let config = RoundConfig {
        session_id,
        assurance: plan.required,
        decision,
        max_output_tokens,
        clock,
    };
    let outcome = orchestrate_session(&config, &descriptors, resolver, &candidates, adjudicator);
    let audit = audit_from_result(session_id, plan, decision, &outcome);
    Ok(RealizedSession { outcome, audit })
}

fn audit_from_result(
    session_id: &str,
    plan: &AssurancePlan,
    decision: &DecisionRequest,
    result: &Result<OrchestratedSession, OrchestrationFailure>,
) -> AssuranceAuditRecord {
    match result {
        Ok(session) => {
            let responded: Vec<&ParticipantOutcome> =
                session.outcomes.iter().filter(|o| o.responded()).collect();
            AssuranceAuditRecord {
                request_id: decision.id.clone(),
                participant_ids: responded
                    .iter()
                    .map(|o| o.participant.id.as_str().to_owned())
                    .collect(),
                model_refs: responded
                    .iter()
                    .filter_map(|o| o.response())
                    .map(|r| {
                        r.model_ref_used
                            .as_ref()
                            .map(|m| m.as_str().to_owned())
                            .unwrap_or_else(|| r.participant.model.as_str().to_owned())
                    })
                    .collect(),
                conclusions: vec![session.adjudication.conclusion.clone()],
                evidence_refs: session
                    .adjudication
                    .evidence
                    .iter()
                    .map(|e| e.id.clone())
                    .collect(),
                disagreements: session.adjudication.disagreement_ids.clone(),
                adjudication: Some(session.adjudication.conclusion.clone()),
                required_assurance: plan.required,
                achieved_assurance: Some(session.assurance),
                usage: responded
                    .iter()
                    .filter_map(|o| o.response())
                    .map(usage_from_response)
                    .collect(),
                outcome: "decided".to_owned(),
                confidence: None,
                human_a4_acceptance: None,
                execution_authority: false,
                round_table_session_id: Some(session.id.clone()),
                selected_leader: Some(session.leader.leader.id.as_str().to_owned()),
                attempted_participants: session
                    .outcomes
                    .iter()
                    .map(|o| o.participant.id.as_str().to_owned())
                    .collect(),
                contribution_outcomes: contribution_summaries(&session.outcomes),
                quorum_satisfied: Some(true),
                failure_reason: None,
            }
        }
        Err(failure) => {
            let outcomes = failure.outcomes();
            let (selected_leader, quorum_satisfied, failure_reason) = match &failure {
                OrchestrationFailure::NotARoundTableLevel => {
                    (None, None, AssuranceFailure::RequiredAssuranceUnachievable)
                }
                OrchestrationFailure::NoLeader { .. } => {
                    (None, None, AssuranceFailure::ParticipantUnavailable)
                }
                OrchestrationFailure::QuorumNotMet { insufficient, .. } => (
                    None,
                    Some(false),
                    match insufficient.reason_code {
                        maia_roundtable::QuorumReasonCode::RequiredAssuranceUnachievable => {
                            AssuranceFailure::RequiredAssuranceUnachievable
                        }
                        maia_roundtable::QuorumReasonCode::ParticipantUnavailable => {
                            AssuranceFailure::ParticipantUnavailable
                        }
                    },
                ),
                OrchestrationFailure::AdjudicationFailed { .. } => {
                    (None, Some(true), AssuranceFailure::AdjudicationFailed)
                }
            };
            AssuranceAuditRecord {
                request_id: decision.id.clone(),
                participant_ids: outcomes
                    .iter()
                    .filter(|o| o.responded())
                    .map(|o| o.participant.id.as_str().to_owned())
                    .collect(),
                model_refs: Vec::new(),
                conclusions: Vec::new(),
                evidence_refs: Vec::new(),
                disagreements: Vec::new(),
                adjudication: None,
                required_assurance: plan.required,
                achieved_assurance: None,
                usage: Vec::new(),
                outcome: outcome_label(failure).to_owned(),
                confidence: None,
                human_a4_acceptance: None,
                execution_authority: false,
                round_table_session_id: Some(session_id.to_owned()),
                selected_leader,
                attempted_participants: outcomes
                    .iter()
                    .map(|o| o.participant.id.as_str().to_owned())
                    .collect(),
                contribution_outcomes: contribution_summaries(outcomes),
                quorum_satisfied,
                failure_reason: Some(failure_reason),
            }
        }
    }
}

fn outcome_label(failure: &OrchestrationFailure) -> &'static str {
    match failure {
        OrchestrationFailure::NotARoundTableLevel => "not_a_round_table_level",
        OrchestrationFailure::NoLeader { .. } => "no_leader",
        OrchestrationFailure::QuorumNotMet { .. } => "quorum_not_met",
        OrchestrationFailure::AdjudicationFailed { .. } => "adjudication_failed",
    }
}

fn contribution_summaries(outcomes: &[ParticipantOutcome]) -> Vec<ContributionOutcomeSummary> {
    outcomes
        .iter()
        .map(|o| ContributionOutcomeSummary {
            participant_id: o.participant.id.as_str().to_owned(),
            responded: o.responded(),
            failure: o.failure_reason().map(map_failure_reason),
        })
        .collect()
}

fn map_failure_reason(reason: ContributionFailureReason) -> AssuranceFailure {
    match reason {
        ContributionFailureReason::ParticipantUnavailable => {
            AssuranceFailure::ParticipantUnavailable
        }
        ContributionFailureReason::InvalidResponse => AssuranceFailure::InvalidResponse,
        ContributionFailureReason::ResolutionFailed => AssuranceFailure::ResolutionFailed,
    }
}

fn usage_from_response(response: &maia_roundtable::ParticipantResponse) -> UsageCostRecord {
    let usage = response.usage.as_ref();
    UsageCostRecord {
        participant_id: response.participant.id.as_str().to_owned(),
        provider_request_id: response.provider_request_id.clone(),
        input_tokens: usage.and_then(|u| u.input_tokens),
        output_tokens: usage.and_then(|u| u.output_tokens),
        cost_minor: usage.and_then(|u| u.cost_minor),
        cost_known: usage.map(|u| u.cost_known).unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maia_assurance_router::AssuranceReason;
    use maia_roundtable::{
        EvidenceReference, ParticipantFailure, ParticipantRequest, ParticipantResponse,
        ResolutionFailure, RoundTableError,
    };
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TestClock(AtomicU64);
    impl TestClock {
        fn new() -> Self {
            Self(AtomicU64::new(1_000))
        }
    }
    impl Clock for TestClock {
        fn now(&self) -> maia_roundtable::Timestamp {
            maia_roundtable::Timestamp(self.0.fetch_add(1, Ordering::SeqCst))
        }
    }

    /// A participant that answers with its own identity and a fixed reply,
    /// used to exercise composition without hardcoding a provider name.
    struct Fake {
        descriptor: ParticipantDescriptor,
        answer: &'static str,
    }
    impl maia_roundtable::Participant for Fake {
        fn descriptor(&self) -> ParticipantDescriptor {
            self.descriptor.clone()
        }
        fn invoke(&self, _: ParticipantRequest) -> Result<ParticipantResponse, ParticipantFailure> {
            Ok(ParticipantResponse {
                participant: self.descriptor.clone(),
                response_text: self.answer.into(),
                evidence: vec![EvidenceReference {
                    id: "ev-1".into(),
                    source_ref: "src-1".into(),
                }],
                provider_request_id: Some(format!("req-{}", self.descriptor.id.as_str())),
                usage: Some(maia_roundtable::UsageCostMetadata {
                    input_tokens: Some(10),
                    output_tokens: Some(20),
                    cost_known: true,
                    cost_minor: Some(30),
                    currency: Some("usd".into()),
                }),
                // Runtime instance identity can diverge from the configured
                // model: reported explicitly rather than assumed equal.
                model_ref_used: Some(ModelRef::new("served-model").unwrap()),
            })
        }
    }

    struct Resolver {
        known: Vec<ParticipantDescriptor>,
    }
    impl ParticipantResolver for Resolver {
        fn resolve(
            &self,
            descriptor: &ParticipantDescriptor,
        ) -> Result<Box<dyn maia_roundtable::Participant>, ResolutionFailure> {
            if !self.known.iter().any(|d| d == descriptor) {
                return Err(ResolutionFailure::UnknownParticipant);
            }
            Ok(Box::new(Fake {
                descriptor: descriptor.clone(),
                answer: "yes",
            }))
        }
    }

    struct Join;
    impl Adjudicator for Join {
        fn adjudicate(
            &self,
            _: &DecisionRequest,
            responses: &[ParticipantResponse],
            _: &[maia_roundtable::Disagreement],
        ) -> Result<String, RoundTableError> {
            Ok(responses
                .iter()
                .map(|r| r.response_text.clone())
                .collect::<Vec<_>>()
                .join(" | "))
        }
    }

    fn registration(
        id: &str,
        provider: &str,
        roles: Vec<ParticipantRole>,
    ) -> ParticipantRegistration {
        ParticipantRegistration {
            id: id.into(),
            provider: provider.into(),
            model_ref: "configured-model".into(),
            role_capabilities: roles,
            enabled: true,
            assurance_levels: vec![ReasoningAssuranceLevel::A3, ReasoningAssuranceLevel::A4],
            cost_metadata_capability: true,
            availability_health: AvailabilityHealth::Available,
        }
    }

    fn plan(path: OrchestrationPath, required: ReasoningAssuranceLevel) -> AssurancePlan {
        AssurancePlan {
            required,
            path,
            reasons: vec![AssuranceReason::EvidenceConflict],
            execution_authority: false,
        }
    }

    fn decision() -> DecisionRequest {
        DecisionRequest {
            id: "decision-1".into(),
            subject: "test".into(),
            prompt: "synthetic non-sensitive prompt".into(),
            evidence: vec![],
        }
    }

    fn registry_two_members_one_adjudicator(providers: (&str, &str, &str)) -> ParticipantRegistry {
        ParticipantRegistry {
            participants: vec![
                registration("one", providers.0, vec![ParticipantRole::RoundTableMember]),
                registration("two", providers.1, vec![ParticipantRole::RoundTableMember]),
                // Adjudicator only: this registration coordinates and
                // synthesizes but does not also occupy a first-round member
                // seat, keeping the fixture's member count exact.
                registration("leader", providers.2, vec![ParticipantRole::Adjudicator]),
            ],
        }
    }

    #[test]
    fn a_satisfied_panel_produces_a_linked_audit_record() {
        let registry =
            registry_two_members_one_adjudicator(("anthropic", "claude-code", "local-model"));
        let descriptors = round_table_descriptors(&registry, ReasoningAssuranceLevel::A3).unwrap();
        let candidates = leader_candidates(&registry).unwrap();
        let resolver = Resolver {
            known: descriptors.clone(),
        };
        let clock = TestClock::new();
        let decision = decision();
        let plan = plan(OrchestrationPath::A3RoundTable, ReasoningAssuranceLevel::A3);
        let record = realize_plan(
            "session-1",
            &plan,
            &decision,
            &registry,
            &resolver,
            &Join,
            64,
            &clock,
        )
        .unwrap();
        // The raw session is still available to a caller that needs it (e.g.
        // to persist a SessionRecord), not just the derived audit record.
        assert!(record.outcome.is_ok());
        let record = record.audit;

        assert_eq!(record.round_table_session_id.as_deref(), Some("session-1"));
        // Deterministic leader selection (lowest id among eligible
        // candidates): "leader" is the only Adjudicator-capable identity.
        assert_eq!(record.selected_leader.as_deref(), Some("leader"));
        assert_eq!(record.quorum_satisfied, Some(true));
        assert!(record.failure_reason.is_none());
        assert!(!record.execution_authority);
        assert_eq!(record.required_assurance, ReasoningAssuranceLevel::A3);
        assert_eq!(record.achieved_assurance, Some(ReasoningAssuranceLevel::A3));
        // Registration identity, not the reported runtime model, names the
        // participant; the served model is a distinct fact.
        let ids: Vec<&str> = record.participant_ids.iter().map(String::as_str).collect();
        assert!(ids.contains(&"one") && ids.contains(&"two"));
        assert!(record.model_refs.iter().all(|m| m == "served-model"));
        assert_eq!(candidates.len(), 3);
        assert_eq!(record.contribution_outcomes.len(), 2);
        assert!(record.contribution_outcomes.iter().all(|c| c.responded));
        assert_eq!(record.usage.len(), 2);
        assert_eq!(record.attempted_participants.len(), 2);
    }

    #[test]
    fn a_non_round_table_plan_is_rejected_before_any_session_is_attempted() {
        let registry = registry_two_members_one_adjudicator(("a", "b", "c"));
        let resolver = Resolver { known: vec![] };
        let clock = TestClock::new();
        let decision = decision();
        let plan = plan(
            OrchestrationPath::A2PrimaryAndVerifier,
            ReasoningAssuranceLevel::A2,
        );
        let result = realize_plan(
            "session-2",
            &plan,
            &decision,
            &registry,
            &resolver,
            &Join,
            64,
            &clock,
        );
        assert_eq!(result, Err(CompositionError::NotARoundTablePlan));
    }

    #[test]
    fn quorum_failure_still_links_the_session_id_and_records_attempted_participants() {
        // Only one Round Table member: the panel can never reach A3 quorum,
        // regardless of availability.
        let registry = ParticipantRegistry {
            participants: vec![
                registration("one", "provider-x", vec![ParticipantRole::RoundTableMember]),
                registration("leader", "provider-y", vec![ParticipantRole::Adjudicator]),
            ],
        };
        let descriptors = round_table_descriptors(&registry, ReasoningAssuranceLevel::A3).unwrap();
        let resolver = Resolver {
            known: descriptors.clone(),
        };
        let clock = TestClock::new();
        let decision = decision();
        let plan = plan(OrchestrationPath::A3RoundTable, ReasoningAssuranceLevel::A3);
        let record = realize_plan(
            "session-3",
            &plan,
            &decision,
            &registry,
            &resolver,
            &Join,
            64,
            &clock,
        )
        .unwrap();
        let record = record.audit;

        assert_eq!(record.round_table_session_id.as_deref(), Some("session-3"));
        assert_eq!(record.quorum_satisfied, Some(false));
        assert_eq!(
            record.failure_reason,
            Some(AssuranceFailure::RequiredAssuranceUnachievable)
        );
        assert!(record.selected_leader.is_none());
        assert!(!record.execution_authority);
        assert_eq!(record.outcome, "quorum_not_met");
        // Every attempted identity is retained even though no decision was
        // reached; only two members were ever attempted (descriptors is 2).
        assert_eq!(record.attempted_participants.len(), descriptors.len());
    }

    #[test]
    fn no_eligible_leader_is_recorded_without_a_leader_or_a_quorum_verdict() {
        // Two Round Table members, but nobody carries the Adjudicator role.
        let registry = ParticipantRegistry {
            participants: vec![
                registration("one", "provider-x", vec![ParticipantRole::RoundTableMember]),
                registration("two", "provider-y", vec![ParticipantRole::RoundTableMember]),
            ],
        };
        let descriptors = round_table_descriptors(&registry, ReasoningAssuranceLevel::A3).unwrap();
        let resolver = Resolver { known: descriptors };
        let clock = TestClock::new();
        let decision = decision();
        let plan = plan(OrchestrationPath::A3RoundTable, ReasoningAssuranceLevel::A3);
        let record = realize_plan(
            "session-4",
            &plan,
            &decision,
            &registry,
            &resolver,
            &Join,
            64,
            &clock,
        )
        .unwrap();
        let record = record.audit;

        assert!(record.selected_leader.is_none());
        // Leader selection failed before the first round ran, so quorum was
        // never evaluated: this must be None, not a false verdict.
        assert!(record.quorum_satisfied.is_none());
        assert_eq!(
            record.failure_reason,
            Some(AssuranceFailure::ParticipantUnavailable)
        );
        assert!(record.attempted_participants.is_empty());
    }

    #[test]
    fn leader_candidates_offer_every_registration_regardless_of_role() {
        let registry = registry_two_members_one_adjudicator(("a", "b", "c"));
        let candidates = leader_candidates(&registry).unwrap();
        assert_eq!(candidates.len(), 3);
        let leader = candidates
            .iter()
            .find(|c| c.descriptor.id.as_str() == "leader")
            .unwrap();
        assert!(leader.can_adjudicate);
        let non_leader = candidates
            .iter()
            .find(|c| c.descriptor.id.as_str() == "one")
            .unwrap();
        assert!(!non_leader.can_adjudicate);
    }

    #[test]
    fn an_invalid_registration_is_reported_by_identity_not_panic() {
        let registry = ParticipantRegistry {
            participants: vec![registration(
                "",
                "provider-x",
                vec![ParticipantRole::RoundTableMember],
            )],
        };
        let result = round_table_descriptors(&registry, ReasoningAssuranceLevel::A3);
        assert_eq!(
            result,
            Err(CompositionError::InvalidRegistration {
                registration_id: "".into()
            })
        );
    }

    #[test]
    fn composition_does_not_special_case_provider_names() {
        // Two runs differing only in provider strings must behave
        // identically: composition addresses participants purely through
        // role capabilities and identity, never a provider name.
        for providers in [("anthropic", "claude-code", "local-model"), ("x", "y", "z")] {
            let registry = registry_two_members_one_adjudicator(providers);
            let descriptors =
                round_table_descriptors(&registry, ReasoningAssuranceLevel::A3).unwrap();
            let candidates = leader_candidates(&registry).unwrap();
            let resolver = Resolver { known: descriptors };
            let clock = TestClock::new();
            let decision = decision();
            let plan = plan(OrchestrationPath::A3RoundTable, ReasoningAssuranceLevel::A3);
            let record = realize_plan(
                "session-5",
                &plan,
                &decision,
                &registry,
                &resolver,
                &Join,
                64,
                &clock,
            )
            .unwrap();
            let record = record.audit;
            assert_eq!(record.quorum_satisfied, Some(true));
            assert_eq!(candidates.len(), 3);
        }
    }

    // ---- Architecture Desk M0.14 review: composition realizes the plan, it
    // never broadens it. The registry supplies possibilities; AssurancePlan
    // carries the decision. ----

    #[test]
    fn a_registration_that_does_not_support_the_required_level_is_never_included() {
        let mut low = registration("low", "provider-x", vec![ParticipantRole::RoundTableMember]);
        low.assurance_levels = vec![ReasoningAssuranceLevel::A1, ReasoningAssuranceLevel::A2];
        let registry = ParticipantRegistry {
            participants: vec![
                registration("one", "provider-y", vec![ParticipantRole::RoundTableMember]),
                registration("two", "provider-z", vec![ParticipantRole::RoundTableMember]),
                low,
            ],
        };
        // "low" is enabled, available and a RoundTableMember, and would
        // widen an A3 panel from 2 to 3 if composition read anything beyond
        // the required level to decide eligibility. It must never appear.
        let descriptors = round_table_descriptors(&registry, ReasoningAssuranceLevel::A3).unwrap();
        let ids: Vec<&str> = descriptors.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids.len(), 2);
        assert!(!ids.contains(&"low"));
    }

    #[test]
    fn descriptor_and_leader_selection_are_pure_functions_of_registry_and_required_level() {
        let registry = registry_two_members_one_adjudicator(("a", "b", "c"));
        let first = round_table_descriptors(&registry, ReasoningAssuranceLevel::A3).unwrap();
        let second = round_table_descriptors(&registry, ReasoningAssuranceLevel::A3).unwrap();
        assert_eq!(first, second);
        let first_candidates = leader_candidates(&registry).unwrap();
        let second_candidates = leader_candidates(&registry).unwrap();
        assert_eq!(first_candidates, second_candidates);
    }

    #[test]
    fn realize_plan_never_substitutes_a_different_assurance_level_than_the_plan_requires() {
        let registry = registry_two_members_one_adjudicator(("a", "b", "c"));
        let descriptors = round_table_descriptors(&registry, ReasoningAssuranceLevel::A4).unwrap();
        let resolver = Resolver { known: descriptors };
        let clock = TestClock::new();
        let decision = decision();
        // The plan asks for A4, not A3: the realized session and the
        // resulting audit record must reflect exactly that, never a
        // composition-chosen substitute.
        let plan = plan(
            OrchestrationPath::A4RoundTableAndHumanAcceptance,
            ReasoningAssuranceLevel::A4,
        );
        let record = realize_plan(
            "session-6",
            &plan,
            &decision,
            &registry,
            &resolver,
            &Join,
            64,
            &clock,
        )
        .unwrap();
        let record = record.audit;
        assert_eq!(record.required_assurance, ReasoningAssuranceLevel::A4);
        assert_eq!(record.achieved_assurance, Some(ReasoningAssuranceLevel::A4));
    }

    #[test]
    fn adjudication_failure_is_audited_without_a_synthesis_result() {
        struct Refuse;
        impl Adjudicator for Refuse {
            fn adjudicate(
                &self,
                _: &DecisionRequest,
                _: &[ParticipantResponse],
                _: &[maia_roundtable::Disagreement],
            ) -> Result<String, RoundTableError> {
                Err(RoundTableError::AdjudicationFailed)
            }
        }
        let registry = registry_two_members_one_adjudicator(("a", "b", "c"));
        let descriptors = round_table_descriptors(&registry, ReasoningAssuranceLevel::A3).unwrap();
        let resolver = Resolver { known: descriptors };
        let clock = TestClock::new();
        let decision = decision();
        let plan = plan(OrchestrationPath::A3RoundTable, ReasoningAssuranceLevel::A3);
        let realized = realize_plan(
            "session-7",
            &plan,
            &decision,
            &registry,
            &resolver,
            &Refuse,
            64,
            &clock,
        )
        .unwrap();
        // The raw outcome is still available to a caller that needs it (e.g.
        // to persist a SessionRecord), not just the derived audit record.
        assert!(matches!(
            realized.outcome,
            Err(maia_roundtable::OrchestrationFailure::AdjudicationFailed { .. })
        ));
        let record = realized.audit;

        assert_eq!(record.round_table_session_id.as_deref(), Some("session-7"));
        // Quorum was satisfied; adjudication is what failed. Both facts must
        // be distinguishable in the record.
        assert_eq!(record.quorum_satisfied, Some(true));
        assert_eq!(
            record.failure_reason,
            Some(AssuranceFailure::AdjudicationFailed)
        );
        // No synthesis was produced, so none is recorded — a failure is
        // never dressed up as a successful session to ease audit population.
        assert!(record.adjudication.is_none());
        assert!(record.conclusions.is_empty());
        assert!(record.achieved_assurance.is_none());
        // The two members that did respond stay in provenance.
        assert_eq!(record.attempted_participants.len(), 2);
        assert!(
            record
                .contribution_outcomes
                .iter()
                .all(|c| c.responded && c.failure.is_none())
        );
        assert_eq!(record.outcome, "adjudication_failed");
    }
}
