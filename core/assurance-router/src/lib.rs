//! Provider-neutral reasoning-assurance planning. It performs no I/O, model
//! invocation, side effect, ApprovalGate decision, or execution authorization.
#![forbid(unsafe_code)]

use maia_domain::ReasoningAssuranceLevel;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Signal {
    None,
    Low,
    Medium,
    High,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvailabilityHealth {
    Available,
    Degraded,
    Unavailable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticipantRole {
    Primary,
    Verifier,
    RoundTableMember,
    Adjudicator,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantRegistration {
    pub id: String,
    pub provider: String,
    pub model_ref: String,
    pub role_capabilities: Vec<ParticipantRole>,
    pub enabled: bool,
    pub assurance_levels: Vec<ReasoningAssuranceLevel>,
    pub cost_metadata_capability: bool,
    pub availability_health: AvailabilityHealth,
}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParticipantRegistry {
    pub participants: Vec<ParticipantRegistration>,
}
impl ParticipantRegistry {
    pub fn available_for(&self, level: ReasoningAssuranceLevel, role: ParticipantRole) -> usize {
        self.participants
            .iter()
            .filter(|p| {
                p.enabled
                    && p.availability_health == AvailabilityHealth::Available
                    && p.assurance_levels.contains(&level)
                    && p.role_capabilities.contains(&role)
            })
            .count()
    }
    pub fn satisfies(&self, level: ReasoningAssuranceLevel) -> bool {
        match level {
            ReasoningAssuranceLevel::A0 => true,
            ReasoningAssuranceLevel::A1 => self.available_for(level, ParticipantRole::Primary) >= 1,
            ReasoningAssuranceLevel::A2 => {
                self.available_for(level, ParticipantRole::Primary) >= 1
                    && self.available_for(level, ParticipantRole::Verifier) >= 1
            }
            ReasoningAssuranceLevel::A3 | ReasoningAssuranceLevel::A4 => {
                self.available_for(level, ParticipantRole::RoundTableMember) >= 2
                    && self.available_for(level, ParticipantRole::Adjudicator) >= 1
            }
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReasoningBudget {
    pub estimated_cost_minor: Option<u64>,
    pub session_ceiling_minor: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssuranceRequest {
    pub impact: Signal,
    pub ambiguity: Signal,
    pub uncertainty: Signal,
    pub evidence_conflict: bool,
    pub novelty: Signal,
    pub reversibility: Signal,
    pub policy_required_assurance: ReasoningAssuranceLevel,
    pub explicit_human_escalation: bool,
    pub budget: ReasoningBudget,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssuranceReason {
    Impact,
    Ambiguity,
    Uncertainty,
    EvidenceConflict,
    Novelty,
    Reversibility,
    PolicyFloor,
    HumanEscalation,
    BudgetExceeded,
    CostUnknown,
    ParticipantUnavailable,
    RequiredAssuranceUnachievable,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssuranceFailure {
    ParticipantUnavailable,
    QuotaExhausted,
    Timeout,
    VerifierUnavailable,
    AdjudicationFailed,
    BudgetExceeded,
    CostUnknown,
    RequiredAssuranceUnachievable,
    /// A response was received but did not carry the identity of the
    /// participant that was invoked. Composition-layer equivalent of Round
    /// Table's `ContributionFailureReason::InvalidResponse`, kept as a
    /// distinct Core-owned variant so Core never depends on the Round Table
    /// implementation to describe this failure.
    InvalidResponse,
    /// A registered participant identity could not be turned into a live
    /// participant. Composition-layer equivalent of Round Table's
    /// `ContributionFailureReason::ResolutionFailed`.
    ResolutionFailed,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrchestrationPath {
    A0Deterministic,
    A1SingleParticipant,
    A2PrimaryAndVerifier,
    A3RoundTable,
    A4RoundTableAndHumanAcceptance,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssurancePlan {
    pub required: ReasoningAssuranceLevel,
    pub path: OrchestrationPath,
    pub reasons: Vec<AssuranceReason>,
    pub execution_authority: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsufficientAssurance {
    pub required: ReasoningAssuranceLevel,
    pub reasons: Vec<AssuranceReason>,
    pub execution_authority: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageCostRecord {
    pub participant_id: String,
    pub provider_request_id: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cost_minor: Option<u64>,
    pub cost_known: bool,
}
/// One participant's attempted contribution, summarized for the audit record
/// without pulling in the Round Table implementation's own outcome type.
///
/// Deliberately a Core-owned shape distinct from `maia_roundtable::
/// ParticipantOutcome`: the composition layer that realizes a plan against
/// the Round Table translates into this, so `maia-assurance-router` never
/// needs to depend on `maia-roundtable` to describe what was attempted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContributionOutcomeSummary {
    /// The registration identity that was attempted, not a runtime instance
    /// identity: this is what was asked for, whether or not it answered.
    pub participant_id: String,
    pub responded: bool,
    /// Present only when `responded` is false.
    pub failure: Option<AssuranceFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssuranceAuditRecord {
    pub request_id: String,
    /// Identities of participants whose contribution was used in
    /// `conclusions`/`model_refs`/`usage` below, i.e. those that actually
    /// responded. Distinct from `attempted_participants`, which also
    /// includes those that were asked but did not contribute.
    pub participant_ids: Vec<String>,
    pub model_refs: Vec<String>,
    pub conclusions: Vec<String>,
    pub evidence_refs: Vec<String>,
    pub disagreements: Vec<String>,
    /// The synthesis/adjudication result, when reasoning reached one. Also
    /// carries a Round Table session's synthesis text when the path was A3
    /// or A4: no separate synthesis field is kept, since this is the same
    /// concept under the same name at every assurance level.
    pub adjudication: Option<String>,
    pub required_assurance: ReasoningAssuranceLevel,
    pub achieved_assurance: Option<ReasoningAssuranceLevel>,
    pub usage: Vec<UsageCostRecord>,
    pub outcome: String,
    pub confidence: Option<String>,
    pub human_a4_acceptance: Option<String>,
    pub execution_authority: bool,
    /// The Round Table session identity this audit record is associated
    /// with, when the path was A3 or A4. Set as soon as a session is
    /// attempted, whether or not it reached a decision, so a failed session
    /// is still traceable to its record.
    pub round_table_session_id: Option<String>,
    /// The Round Table leader's participant identity, once leader selection
    /// succeeds. `None` when the path never reached leader selection.
    pub selected_leader: Option<String>,
    /// Every participant identity attempted in the first round, successful
    /// or not. A superset of `participant_ids`.
    pub attempted_participants: Vec<String>,
    /// Per-participant outcome for every attempted contribution.
    pub contribution_outcomes: Vec<ContributionOutcomeSummary>,
    /// Whether the first round satisfied quorum for `required_assurance`.
    /// `None` when quorum was never evaluated, e.g. leader selection failed
    /// first, or the path was not a Round Table level.
    pub quorum_satisfied: Option<bool>,
    /// Why the session did not reach a decision. `None` on success.
    pub failure_reason: Option<AssuranceFailure>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingOutcome {
    Selected(AssurancePlan),
    InsufficientAssurance(InsufficientAssurance),
}

pub fn insufficient_for_failure(
    required: ReasoningAssuranceLevel,
    failure: AssuranceFailure,
) -> InsufficientAssurance {
    let reason = match failure {
        AssuranceFailure::ParticipantUnavailable
        | AssuranceFailure::InvalidResponse
        | AssuranceFailure::ResolutionFailed => AssuranceReason::ParticipantUnavailable,
        AssuranceFailure::QuotaExhausted
        | AssuranceFailure::Timeout
        | AssuranceFailure::VerifierUnavailable
        | AssuranceFailure::AdjudicationFailed
        | AssuranceFailure::RequiredAssuranceUnachievable => {
            AssuranceReason::RequiredAssuranceUnachievable
        }
        AssuranceFailure::BudgetExceeded => AssuranceReason::BudgetExceeded,
        AssuranceFailure::CostUnknown => AssuranceReason::CostUnknown,
    };
    InsufficientAssurance {
        required,
        reasons: vec![reason, AssuranceReason::RequiredAssuranceUnachievable],
        execution_authority: false,
    }
}

fn rank(level: ReasoningAssuranceLevel) -> u8 {
    match level {
        ReasoningAssuranceLevel::A0 => 0,
        ReasoningAssuranceLevel::A1 => 1,
        ReasoningAssuranceLevel::A2 => 2,
        ReasoningAssuranceLevel::A3 => 3,
        ReasoningAssuranceLevel::A4 => 4,
    }
}
fn raise(a: ReasoningAssuranceLevel, b: ReasoningAssuranceLevel) -> ReasoningAssuranceLevel {
    if rank(a) >= rank(b) { a } else { b }
}
fn path(level: ReasoningAssuranceLevel) -> OrchestrationPath {
    match level {
        ReasoningAssuranceLevel::A0 => OrchestrationPath::A0Deterministic,
        ReasoningAssuranceLevel::A1 => OrchestrationPath::A1SingleParticipant,
        ReasoningAssuranceLevel::A2 => OrchestrationPath::A2PrimaryAndVerifier,
        ReasoningAssuranceLevel::A3 => OrchestrationPath::A3RoundTable,
        ReasoningAssuranceLevel::A4 => OrchestrationPath::A4RoundTableAndHumanAcceptance,
    }
}

pub fn route(request: &AssuranceRequest, registry: &ParticipantRegistry) -> RoutingOutcome {
    let mut level = ReasoningAssuranceLevel::A0;
    let mut reasons = Vec::new();
    for (signal, reason) in [
        (request.impact, AssuranceReason::Impact),
        (request.ambiguity, AssuranceReason::Ambiguity),
        (request.uncertainty, AssuranceReason::Uncertainty),
        (request.novelty, AssuranceReason::Novelty),
        (request.reversibility, AssuranceReason::Reversibility),
    ] {
        if signal != Signal::None {
            level = raise(level, ReasoningAssuranceLevel::A1);
            reasons.push(reason);
        }
        if signal == Signal::Medium {
            level = raise(level, ReasoningAssuranceLevel::A2);
        }
        if signal == Signal::High {
            level = raise(level, ReasoningAssuranceLevel::A3);
        }
    }
    if request.evidence_conflict {
        level = raise(level, ReasoningAssuranceLevel::A3);
        reasons.push(AssuranceReason::EvidenceConflict);
    }
    if request.explicit_human_escalation {
        level = raise(level, ReasoningAssuranceLevel::A3);
        reasons.push(AssuranceReason::HumanEscalation);
    }
    if rank(request.policy_required_assurance) > rank(level) {
        level = request.policy_required_assurance;
        reasons.push(AssuranceReason::PolicyFloor);
    }
    if request.budget.session_ceiling_minor.is_some()
        && request.budget.estimated_cost_minor.is_none()
    {
        reasons.push(AssuranceReason::CostUnknown);
        return RoutingOutcome::InsufficientAssurance(InsufficientAssurance {
            required: level,
            reasons,
            execution_authority: false,
        });
    }
    if matches!((request.budget.estimated_cost_minor, request.budget.session_ceiling_minor), (Some(cost), Some(limit)) if cost > limit)
    {
        reasons.push(AssuranceReason::BudgetExceeded);
        return RoutingOutcome::InsufficientAssurance(InsufficientAssurance {
            required: level,
            reasons,
            execution_authority: false,
        });
    }
    if !registry.satisfies(level) {
        reasons.extend([
            AssuranceReason::ParticipantUnavailable,
            AssuranceReason::RequiredAssuranceUnachievable,
        ]);
        return RoutingOutcome::InsufficientAssurance(InsufficientAssurance {
            required: level,
            reasons,
            execution_authority: false,
        });
    }
    RoutingOutcome::Selected(AssurancePlan {
        required: level,
        path: path(level),
        reasons,
        execution_authority: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(role: ParticipantRole, levels: Vec<ReasoningAssuranceLevel>) -> ParticipantRegistration {
        ParticipantRegistration {
            id: format!("{role:?}"),
            provider: "fake".into(),
            model_ref: "fake-model".into(),
            role_capabilities: vec![role],
            enabled: true,
            assurance_levels: levels,
            cost_metadata_capability: true,
            availability_health: AvailabilityHealth::Available,
        }
    }
    fn registry() -> ParticipantRegistry {
        ParticipantRegistry {
            participants: vec![
                p(
                    ParticipantRole::Primary,
                    vec![ReasoningAssuranceLevel::A1, ReasoningAssuranceLevel::A2],
                ),
                p(ParticipantRole::Verifier, vec![ReasoningAssuranceLevel::A2]),
                p(
                    ParticipantRole::RoundTableMember,
                    vec![ReasoningAssuranceLevel::A3, ReasoningAssuranceLevel::A4],
                ),
                p(
                    ParticipantRole::RoundTableMember,
                    vec![ReasoningAssuranceLevel::A3, ReasoningAssuranceLevel::A4],
                ),
                p(
                    ParticipantRole::Adjudicator,
                    vec![ReasoningAssuranceLevel::A3, ReasoningAssuranceLevel::A4],
                ),
            ],
        }
    }
    fn request() -> AssuranceRequest {
        AssuranceRequest {
            impact: Signal::None,
            ambiguity: Signal::None,
            uncertainty: Signal::None,
            evidence_conflict: false,
            novelty: Signal::None,
            reversibility: Signal::None,
            policy_required_assurance: ReasoningAssuranceLevel::A0,
            explicit_human_escalation: false,
            budget: ReasoningBudget {
                estimated_cost_minor: Some(1),
                session_ceiling_minor: Some(2),
            },
        }
    }
    #[test]
    fn paths_are_explicit_and_non_executable() {
        let mut r = request();
        r.impact = Signal::Medium;
        assert!(matches!(
            route(&r, &registry()),
            RoutingOutcome::Selected(AssurancePlan {
                path: OrchestrationPath::A2PrimaryAndVerifier,
                execution_authority: false,
                ..
            })
        ));
        r.evidence_conflict = true;
        assert!(matches!(
            route(&r, &registry()),
            RoutingOutcome::Selected(AssurancePlan {
                path: OrchestrationPath::A3RoundTable,
                ..
            })
        ));
        r.policy_required_assurance = ReasoningAssuranceLevel::A4;
        assert!(matches!(
            route(&r, &registry()),
            RoutingOutcome::Selected(AssurancePlan {
                path: OrchestrationPath::A4RoundTableAndHumanAcceptance,
                ..
            })
        ));
    }
    #[test]
    fn unknown_or_exceeded_budget_is_explicit_not_downgraded() {
        let mut r = request();
        r.policy_required_assurance = ReasoningAssuranceLevel::A3;
        r.budget.estimated_cost_minor = None;
        assert!(matches!(
            route(&r, &registry()),
            RoutingOutcome::InsufficientAssurance(_)
        ));
        r.budget.estimated_cost_minor = Some(3);
        assert!(matches!(
            route(&r, &registry()),
            RoutingOutcome::InsufficientAssurance(_)
        ));
    }
    #[test]
    fn unavailable_verifier_is_insufficient() {
        let mut r = request();
        r.policy_required_assurance = ReasoningAssuranceLevel::A2;
        let mut g = registry();
        g.participants
            .retain(|p| !p.role_capabilities.contains(&ParticipantRole::Verifier));
        assert!(matches!(
            route(&r, &g),
            RoutingOutcome::InsufficientAssurance(_)
        ));
    }
    #[test]
    fn every_runtime_failure_stays_non_executable_and_insufficient() {
        for failure in [
            AssuranceFailure::ParticipantUnavailable,
            AssuranceFailure::QuotaExhausted,
            AssuranceFailure::Timeout,
            AssuranceFailure::VerifierUnavailable,
            AssuranceFailure::AdjudicationFailed,
            AssuranceFailure::BudgetExceeded,
            AssuranceFailure::CostUnknown,
            AssuranceFailure::RequiredAssuranceUnachievable,
            AssuranceFailure::InvalidResponse,
            AssuranceFailure::ResolutionFailed,
        ] {
            let result = insufficient_for_failure(ReasoningAssuranceLevel::A3, failure);
            assert_eq!(result.required, ReasoningAssuranceLevel::A3);
            assert!(!result.execution_authority);
        }
    }
}
