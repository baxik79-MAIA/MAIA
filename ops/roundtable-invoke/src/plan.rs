//! The dry run: who would be asked, what it would cost, and what the user is told.
//!
//! Nothing here constructs a provider, calls one, or writes anything. The plan is
//! the router's `AssurancePlan`, obtained from `maia_assurance_router::route`
//! with the requested level as a policy floor; this module never decides a level
//! itself and never widens or narrows the panel the composition layer derives
//! from `(registry, level)`.

use crate::registry::{Adapter, LoadedRegistry, Locality};
use maia_assurance_router::{
    AssurancePlan, AssuranceReason, AssuranceRequest, OrchestrationPath, ReasoningBudget,
    RoutingOutcome, Signal, route,
};
use maia_domain::ReasoningAssuranceLevel;
use maia_roundtable::select_leader;
use maia_roundtable_composition::{CompositionError, leader_candidates, round_table_descriptors};
use std::fmt;

/// Longest prompt accepted, in bytes.
pub const MAX_PROMPT_BYTES: usize = 32 * 1024;
/// Longest subject accepted, in characters.
pub const MAX_SUBJECT_CHARS: usize = 200;
/// Output bound applied to every call. Fixed: not a flag, not configurable.
pub const MAX_OUTPUT_TOKENS: u32 = 2048;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// A4 needs a human acceptance flow that does not exist yet. Refused, never
    /// downgraded to A3.
    HumanAcceptanceUnavailable,
    /// The requested level is not a Round Table level.
    NotARoundTableLevel(ReasoningAssuranceLevel),
    BlankSubject,
    SubjectTooLong,
    BlankPrompt,
    PromptTooLarge,
    /// The router could not grant the level from this registry.
    Insufficient(Vec<AssuranceReason>),
    /// The router selected a path that is not a Round Table path.
    NotARoundTablePlan,
    Composition(CompositionError),
    /// No enabled participant can adjudicate at this level.
    NoLeader,
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HumanAcceptanceUnavailable => write!(
                f,
                "A4 is not available: it needs a human acceptance flow that does not exist yet. \
                 It is refused, not downgraded to A3."
            ),
            Self::NotARoundTableLevel(l) => {
                write!(f, "{l} is not a Round Table level; this tool runs A3 only")
            }
            Self::BlankSubject => write!(f, "subject is blank"),
            Self::SubjectTooLong => {
                write!(f, "subject exceeds {MAX_SUBJECT_CHARS} characters")
            }
            Self::BlankPrompt => write!(f, "prompt is blank"),
            Self::PromptTooLarge => write!(f, "prompt exceeds {MAX_PROMPT_BYTES} bytes"),
            Self::Insufficient(reasons) => write!(
                f,
                "the registry cannot satisfy the requested level (router reasons: {reasons:?}). \
                 A3 needs at least two enabled round_table_member participants and one \
                 adjudicator, each declaring A3. Nothing was substituted."
            ),
            Self::NotARoundTablePlan => write!(f, "the router did not select a Round Table path"),
            Self::Composition(e) => write!(f, "registry cannot be realized: {e:?}"),
            Self::NoLeader => write!(f, "no enabled participant can adjudicate at this level"),
        }
    }
}

impl std::error::Error for PlanError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedParticipant {
    pub id: String,
    pub provider: String,
    pub model_ref: String,
    pub adapter: Adapter,
}

impl PlannedParticipant {
    pub fn locality(&self) -> Locality {
        self.adapter.locality()
    }
}

/// Everything the dry run learned. Carries the router's plan so `run` realizes
/// exactly what `plan` showed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanReport {
    pub plan: AssurancePlan,
    pub subject: String,
    pub prompt_bytes: usize,
    /// First-round members, in the order the composition layer derives them.
    pub members: Vec<PlannedParticipant>,
    /// The leader `select_leader` picks; it adjudicates and also receives the
    /// prompt, whether or not it is a member.
    pub leader: PlannedParticipant,
}

impl PlanReport {
    /// Worst case: one call per member plus one adjudication by the leader.
    pub fn worst_case_calls(&self) -> usize {
        self.members.len() + 1
    }

    /// Every distinct participant that receives the prompt.
    pub fn recipients(&self) -> Vec<&PlannedParticipant> {
        let mut out: Vec<&PlannedParticipant> = self.members.iter().collect();
        if !out.iter().any(|p| p.id == self.leader.id) {
            out.push(&self.leader);
        }
        out
    }

    /// True if any recipient is not loopback-local. Only such a run needs the
    /// cost prompt; a loopback-only panel has no external cost.
    pub fn is_live(&self) -> bool {
        self.recipients()
            .iter()
            .any(|p| p.locality() == Locality::ThirdPartySubscription)
    }
}

/// Bounds on the request text, checked before anything else happens.
pub fn validate_request(subject: &str, prompt: &str) -> Result<(), PlanError> {
    if subject.trim().is_empty() {
        return Err(PlanError::BlankSubject);
    }
    if subject.chars().count() > MAX_SUBJECT_CHARS {
        return Err(PlanError::SubjectTooLong);
    }
    if prompt.trim().is_empty() {
        return Err(PlanError::BlankPrompt);
    }
    if prompt.len() > MAX_PROMPT_BYTES {
        return Err(PlanError::PromptTooLarge);
    }
    Ok(())
}

fn floor_request(level: ReasoningAssuranceLevel) -> AssuranceRequest {
    AssuranceRequest {
        impact: Signal::None,
        ambiguity: Signal::None,
        uncertainty: Signal::None,
        evidence_conflict: false,
        novelty: Signal::None,
        reversibility: Signal::None,
        policy_required_assurance: level,
        explicit_human_escalation: false,
        budget: ReasoningBudget {
            estimated_cost_minor: None,
            session_ceiling_minor: None,
        },
    }
}

/// Resolve who would be asked. Pure: no provider is constructed and nothing is
/// written or read.
pub fn build_plan(
    loaded: &LoadedRegistry,
    level: ReasoningAssuranceLevel,
    subject: &str,
    prompt: &str,
) -> Result<PlanReport, PlanError> {
    match level {
        ReasoningAssuranceLevel::A3 => {}
        ReasoningAssuranceLevel::A4 => return Err(PlanError::HumanAcceptanceUnavailable),
        other => return Err(PlanError::NotARoundTableLevel(other)),
    }
    validate_request(subject, prompt)?;

    let plan = match route(&floor_request(level), &loaded.registry) {
        RoutingOutcome::Selected(plan) => plan,
        RoutingOutcome::InsufficientAssurance(i) => return Err(PlanError::Insufficient(i.reasons)),
    };
    if plan.path != OrchestrationPath::A3RoundTable || plan.required != level {
        return Err(PlanError::NotARoundTablePlan);
    }

    let describe = |id: &str| -> Option<PlannedParticipant> {
        let reg = loaded.registry.participants.iter().find(|r| r.id == id)?;
        Some(PlannedParticipant {
            id: reg.id.clone(),
            provider: reg.provider.clone(),
            model_ref: reg.model_ref.clone(),
            adapter: loaded.adapter_of(id)?,
        })
    };

    let members = round_table_descriptors(&loaded.registry, plan.required)
        .map_err(PlanError::Composition)?
        .iter()
        .map(|d| describe(d.id.as_str()).ok_or(PlanError::NotARoundTablePlan))
        .collect::<Result<Vec<_>, _>>()?;
    let candidates = leader_candidates(&loaded.registry).map_err(PlanError::Composition)?;
    let leader = select_leader(plan.required, &candidates).map_err(|_| PlanError::NoLeader)?;
    let leader = describe(leader.leader.id.as_str()).ok_or(PlanError::NotARoundTablePlan)?;

    Ok(PlanReport {
        plan,
        subject: subject.to_owned(),
        prompt_bytes: prompt.len(),
        members,
        leader,
    })
}

fn where_it_goes(p: &PlannedParticipant) -> &'static str {
    match p.locality() {
        Locality::Loopback => "local loopback endpoint on this machine; no third party",
        Locality::ThirdPartySubscription => "third-party subscription service, not this machine",
    }
}

/// The confirmation block, in the order the design fixes: what is sent and
/// where, how many calls, cost, authority, side effects, constraints.
///
/// `history` is `Some` for `run` and `None` for `plan`; the side-effects line
/// says what would be written, or that `plan` writes nothing. The full prompt is
/// saved with the session, which is stated plainly rather than softened.
pub fn render_plan(report: &PlanReport, history: Option<&str>) -> String {
    let mut out = String::new();
    out.push_str("MAIA Round Table invocation - plan\n");
    out.push_str("===================================\n");
    out.push_str(&format!(
        "level: {}   path: A3 Round Table   subject: {}\n\n",
        report.plan.required, report.subject
    ));

    out.push_str("1. What will be sent, and where\n");
    out.push_str(&format!(
        "   The full prompt ({} bytes) and any evidence references go to each of:\n",
        report.prompt_bytes
    ));
    for p in report.recipients() {
        let role = match (
            report.members.iter().any(|m| m.id == p.id),
            p.id == report.leader.id,
        ) {
            (true, true) => "member + leader",
            (true, false) => "member",
            (false, _) => "leader",
        };
        out.push_str(&format!(
            "   - {} ({}) provider={} model={}: {}\n",
            p.id,
            role,
            p.provider,
            p.model_ref,
            where_it_goes(p)
        ));
    }

    out.push_str(&format!(
        "\n2. How many calls\n   Worst case {}: {} first-round contribution(s) plus one adjudication \
         by the leader `{}`. Each call is bounded by the provider's per-call timeout and by \
         {} output tokens. No retries.\n",
        report.worst_case_calls(),
        report.members.len(),
        report.leader.id,
        MAX_OUTPUT_TOKENS
    ));

    out.push_str("\n3. Cost\n");
    if report.is_live() {
        out.push_str(
            "   cost_known: false. Subscription usage cannot be priced by MAIA: the cost is not \
             known and is not zero. No dollar figure is invented.\n",
        );
    } else {
        out.push_str(
            "   Every recipient is a local loopback endpoint: no external cost. cost_known: false \
             still applies: MAIA does not price local compute.\n",
        );
    }

    out.push_str(
        "\n4. Authority\n   This produces advice only. execution_authority: false. MAIA will take \
         no action on the result.\n",
    );

    out.push_str("\n5. Side effects\n");
    match history {
        Some(dir) => out.push_str(&format!(
            "   The session, including the full prompt, will be saved in full under `{dir}`. \
             The viewer prints stored prompts.\n"
        )),
        None => out.push_str(
            "   This is a dry run: nothing is constructed, called, or written, and no history \
             directory is created. `run` would save the session, including the full prompt, \
             under the --history directory you give it.\n",
        ),
    }

    out.push_str(
        "\n6. Constraints on the participants\n   Participants are run with read, edit, shell and \
         web tools denied. A participant cannot launch this tool.\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::load_registry;

    fn entry(id: &str, adapter: &str, roles: &str) -> String {
        format!(
            r#"{{"id":"{id}","adapter":"{adapter}","provider":"p-{id}","model_ref":"m-{id}",
                "roles":[{roles}],"assurance_levels":["A3"],"enabled":true}}"#
        )
    }

    const MEMBER: &str = r#""round_table_member""#;
    const BOTH: &str = r#""round_table_member","adjudicator""#;

    fn loaded(entries: &[String]) -> LoadedRegistry {
        load_registry(&format!(r#"{{"participants":[{}]}}"#, entries.join(","))).unwrap()
    }

    fn panel() -> LoadedRegistry {
        loaded(&[
            entry("b-local", "local-model", BOTH),
            entry("a-claude", "claude-code", BOTH),
        ])
    }

    #[test]
    fn plans_a3_from_the_router_and_names_a_deterministic_leader() {
        let r = build_plan(&panel(), ReasoningAssuranceLevel::A3, "s", "p").unwrap();
        assert_eq!(r.plan.path, OrchestrationPath::A3RoundTable);
        assert!(!r.plan.execution_authority);
        assert_eq!(r.members.len(), 2);
        assert_eq!(r.leader.id, "a-claude", "leader is the lowest eligible id");
        assert_eq!(r.worst_case_calls(), 3);
        assert!(r.is_live());
    }

    #[test]
    fn a4_is_refused_not_downgraded() {
        assert_eq!(
            build_plan(&panel(), ReasoningAssuranceLevel::A4, "s", "p"),
            Err(PlanError::HumanAcceptanceUnavailable)
        );
    }

    #[test]
    fn levels_below_a3_are_not_round_table_levels() {
        for l in [
            ReasoningAssuranceLevel::A0,
            ReasoningAssuranceLevel::A1,
            ReasoningAssuranceLevel::A2,
        ] {
            assert_eq!(
                build_plan(&panel(), l, "s", "p"),
                Err(PlanError::NotARoundTableLevel(l))
            );
        }
    }

    #[test]
    fn an_unconfigured_participant_is_unavailable_and_nothing_is_substituted() {
        // One member only: A3 needs two, so the router refuses.
        let one = loaded(&[entry("solo", "local-model", BOTH)]);
        assert!(matches!(
            build_plan(&one, ReasoningAssuranceLevel::A3, "s", "p"),
            Err(PlanError::Insufficient(_))
        ));
        // Two members, nobody can adjudicate.
        let none = loaded(&[
            entry("a", "local-model", MEMBER),
            entry("b", "claude-code", MEMBER),
        ]);
        assert!(matches!(
            build_plan(&none, ReasoningAssuranceLevel::A3, "s", "p"),
            Err(PlanError::Insufficient(_))
        ));
    }

    #[test]
    fn a_disabled_participant_is_not_asked() {
        let text = format!(
            r#"{{"participants":[{},{},{}]}}"#,
            entry("a", "local-model", BOTH),
            entry("b", "local-model", BOTH),
            entry("c", "claude-code", BOTH).replace(r#""enabled":true"#, r#""enabled":false"#),
        );
        let l = load_registry(&text).unwrap();
        let r = build_plan(&l, ReasoningAssuranceLevel::A3, "s", "p").unwrap();
        assert!(r.recipients().iter().all(|p| p.id != "c"));
        assert!(!r.is_live(), "the only live participant is disabled");
    }

    #[test]
    fn a_leader_outside_the_panel_is_still_a_recipient() {
        let l = loaded(&[
            entry("m1", "local-model", MEMBER),
            entry("m2", "local-model", MEMBER),
            entry("judge", "claude-code", r#""adjudicator""#),
        ]);
        let r = build_plan(&l, ReasoningAssuranceLevel::A3, "s", "p").unwrap();
        assert_eq!(r.members.len(), 2);
        assert_eq!(r.leader.id, "judge");
        assert_eq!(r.recipients().len(), 3);
        assert!(r.is_live(), "the leader receives the prompt");
        assert_eq!(r.worst_case_calls(), 3);
    }

    #[test]
    fn request_bounds_are_enforced() {
        let l = panel();
        let a3 = ReasoningAssuranceLevel::A3;
        assert_eq!(build_plan(&l, a3, "  ", "p"), Err(PlanError::BlankSubject));
        assert_eq!(build_plan(&l, a3, "s", " \n"), Err(PlanError::BlankPrompt));
        assert_eq!(
            build_plan(&l, a3, &"x".repeat(MAX_SUBJECT_CHARS + 1), "p"),
            Err(PlanError::SubjectTooLong)
        );
        assert_eq!(
            build_plan(&l, a3, "s", &"x".repeat(MAX_PROMPT_BYTES + 1)),
            Err(PlanError::PromptTooLarge)
        );
        assert!(build_plan(&l, a3, "s", &"x".repeat(MAX_PROMPT_BYTES)).is_ok());
    }

    #[test]
    fn rendered_plan_states_the_fixed_authority_and_cost_lines() {
        let r = build_plan(&panel(), ReasoningAssuranceLevel::A3, "quarterly risk", "p").unwrap();
        let text = render_plan(&r, None);
        assert!(text.contains("execution_authority: false"));
        assert!(text.contains("cost_known: false"));
        assert!(text.contains("not known and is not zero"));
        assert!(text.contains("third-party subscription service"));
        assert!(text.contains("local loopback endpoint"));
        assert!(text.contains("Worst case 3"));
        assert!(text.contains("dry run"));
        assert!(text.contains("no history directory is created"));
        // Ordering of the sections is part of the contract.
        let order: Vec<usize> = [
            "1. What will be sent",
            "2. How many calls",
            "3. Cost",
            "4. Authority",
            "5. Side effects",
            "6. Constraints",
        ]
        .iter()
        .map(|h| text.find(h).unwrap_or_else(|| panic!("missing {h}")))
        .collect();
        assert!(order.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn loopback_only_plan_has_no_external_cost_and_is_not_live() {
        let l = loaded(&[
            entry("a", "local-model", BOTH),
            entry("b", "local-model", BOTH),
        ]);
        let r = build_plan(&l, ReasoningAssuranceLevel::A3, "s", "p").unwrap();
        assert!(!r.is_live());
        let text = render_plan(&r, None);
        assert!(text.contains("no external cost"));
        assert!(!text.contains("third-party subscription service"));
    }

    #[test]
    fn a_run_plan_states_the_full_prompt_is_saved() {
        let r = build_plan(&panel(), ReasoningAssuranceLevel::A3, "s", "p").unwrap();
        let text = render_plan(&r, Some("C:/history"));
        assert!(text.contains("saved in full under `C:/history`"));
        assert!(!text.contains("dry run"));
    }
}
