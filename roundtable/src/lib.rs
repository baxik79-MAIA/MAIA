//! Provider-neutral, reasoning-only Round Table vertical slice.
//!
//! This crate has no network, credential, persistence, connector or Action
//! execution dependency. Participant first-round invocations receive only the
//! original DecisionRequest; adjudication alone receives collected responses.
#![forbid(unsafe_code)]

use maia_domain::ReasoningAssuranceLevel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantId(String);
impl ParticipantId {
    pub fn new(value: impl Into<String>) -> Result<Self, RoundTableError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(RoundTableError::InvalidInput("participant_id"));
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelProvider(String);
impl ModelProvider {
    pub fn new(value: impl Into<String>) -> Result<Self, RoundTableError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(RoundTableError::InvalidInput("model_provider"));
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRef(String);
impl ModelRef {
    pub fn new(value: impl Into<String>) -> Result<Self, RoundTableError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(RoundTableError::InvalidInput("model_ref"));
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceReference {
    pub id: String,
    pub source_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageCostMetadata {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cost_known: bool,
    pub cost_minor: Option<u64>,
    pub currency: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantDescriptor {
    pub id: ParticipantId,
    pub provider: ModelProvider,
    pub model: ModelRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionRequest {
    pub id: String,
    pub subject: String,
    pub prompt: String,
    pub evidence: Vec<EvidenceReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantRequest {
    pub session_id: String,
    pub decision: DecisionRequest,
    pub max_output_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantResponse {
    pub participant: ParticipantDescriptor,
    pub response_text: String,
    pub evidence: Vec<EvidenceReference>,
    pub provider_request_id: Option<String>,
    pub usage: Option<UsageCostMetadata>,
    /// The model the provider actually used, when it reports one.
    ///
    /// Distinct from `participant.model`, which is what was *configured*. A
    /// provider may serve a request with a different model than requested, and
    /// an audit that records only the configured value would silently
    /// misattribute the reasoning. `None` means the adapter did not report it,
    /// which is recorded as unknown rather than assumed equal to the configured
    /// model.
    pub model_ref_used: Option<ModelRef>,
}

/// Milliseconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(pub u64);

/// Time source, injected so provenance is deterministic under test.
///
/// A port rather than a direct `SystemTime` call: timestamps are audit data, and
/// an audit trail that cannot be reproduced in a test is one nobody can verify.
pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}

/// Wall-clock implementation for production use.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        )
    }
}

/// Which part of the session a contribution belongs to.
///
/// Recorded per contribution so an adjudication can never be mistaken for an
/// independent first-round response when the record is read back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContributionRole {
    FirstRound,
    Adjudication,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disagreement {
    pub id: String,
    pub participants: Vec<ParticipantId>,
    pub summary: String,
    pub evidence: Vec<EvidenceReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundTableDecision {
    pub session_id: String,
    pub conclusion: String,
    pub evidence: Vec<EvidenceReference>,
    pub disagreement_ids: Vec<String>,
    pub execution_authority: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundTableSession {
    pub id: String,
    pub assurance: ReasoningAssuranceLevel,
    pub decision: DecisionRequest,
    pub responses: Vec<ParticipantResponse>,
    pub disagreements: Vec<Disagreement>,
    pub adjudication: RoundTableDecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParticipantFailureKind {
    Transient,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantFailure {
    pub kind: ParticipantFailureKind,
    pub provider_request_id: Option<String>,
}

/// Why a first-round contribution did not yield a usable response.
///
/// Deliberately coarse. The `Participant` port reports only transient versus
/// permanent, so it cannot distinguish a timeout from an exhausted quota, and
/// inventing that precision here would fabricate provenance the adapter never
/// supplied. Only genuinely distinguishable cases get a variant; the
/// transient/permanent distinction is preserved separately on the failure
/// itself.
///
/// These map one-to-one onto `AssuranceFailure` at the router boundary without
/// this crate depending on `core/assurance-router`: the router plans
/// assurance and calls the Round Table, so that edge would invert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContributionFailureReason {
    /// The participant produced no response: unreachable, timed out, refused or
    /// out of quota.
    ParticipantUnavailable,
    /// A response came back but violated the contract, e.g. it claimed an
    /// identity other than the participant that was invoked.
    InvalidResponse,
    /// No live participant could be resolved from its registration.
    ResolutionFailed,
}

/// The result of one attempted contribution.
///
/// The responded variant is much larger than the failed one. Boxing it would
/// even the variants out, but a panel holds a handful of participants, so the
/// saving is on the order of kilobytes while the cost is an allocation on the
/// common success path. Keeping the response inline is the better trade here.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContributionResult {
    Responded(ParticipantResponse),
    Failed {
        kind: ParticipantFailureKind,
        reason: ContributionFailureReason,
        provider_request_id: Option<String>,
    },
}

/// One participant's attempted first-round contribution, successful or not.
///
/// A failed attempt is still provenance (spec `orchestration.provenance.
/// failed_attempts_recorded`): it is retained so a partial provider outage can
/// be diagnosed afterwards, rather than vanishing from the session record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantOutcome {
    pub participant: ParticipantDescriptor,
    pub role: ContributionRole,
    /// Invocation order within the round, starting at 0.
    pub sequence: u32,
    pub started_at: Timestamp,
    pub finished_at: Timestamp,
    /// True when this contribution was produced without sight of any other
    /// participant's response.
    pub first_round_isolated: bool,
    pub result: ContributionResult,
}

impl ParticipantOutcome {
    pub fn responded(&self) -> bool {
        matches!(self.result, ContributionResult::Responded(_))
    }
    pub fn response(&self) -> Option<&ParticipantResponse> {
        match &self.result {
            ContributionResult::Responded(response) => Some(response),
            ContributionResult::Failed { .. } => None,
        }
    }
    pub fn failure_reason(&self) -> Option<ContributionFailureReason> {
        match &self.result {
            ContributionResult::Failed { reason, .. } => Some(*reason),
            ContributionResult::Responded(_) => None,
        }
    }
    pub fn provider_request_id(&self) -> Option<&str> {
        match &self.result {
            ContributionResult::Responded(r) => r.provider_request_id.as_deref(),
            ContributionResult::Failed {
                provider_request_id,
                ..
            } => provider_request_id.as_deref(),
        }
    }
    /// The model actually used, when the provider reported one.
    ///
    /// Deliberately not defaulted to the configured model: "we asked for X" and
    /// "X answered" are different claims, and an audit must be able to tell them
    /// apart.
    pub fn model_ref_used(&self) -> Option<&ModelRef> {
        self.response().and_then(|r| r.model_ref_used.as_ref())
    }
    /// Whether this contribution counts toward independent first-round quorum.
    pub fn counts_toward_quorum(&self) -> bool {
        self.role == ContributionRole::FirstRound && self.first_round_isolated && self.responded()
    }
}

/// Everything a round needs that is not the participants themselves.
///
/// Grouped rather than passed as six positional arguments, which had already
/// become unreadable and easy to transpose.
pub struct RoundConfig<'a> {
    pub session_id: &'a str,
    pub assurance: ReasoningAssuranceLevel,
    pub decision: &'a DecisionRequest,
    pub max_output_tokens: u32,
    pub clock: &'a dyn Clock,
}

impl<'a> RoundConfig<'a> {
    fn request(&self) -> ParticipantRequest {
        ParticipantRequest {
            session_id: self.session_id.to_owned(),
            decision: self.decision.clone(),
            max_output_tokens: self.max_output_tokens,
        }
    }
}

/// Invoke every participant's first round, isolating failures.
///
/// This never returns an error and never stops early: one unavailable provider
/// degrades its own contribution only, and the remaining participants are still
/// invoked (spec `orchestration.participant_outcomes`). Quorum is evaluated by
/// the caller *after* collection completes, never during it, so a decision is
/// never taken on a partially collected panel.
///
/// Every participant receives the same response-free request, so first-round
/// isolation holds regardless of which participants fail.
pub fn collect_first_round(
    config: &RoundConfig<'_>,
    participants: &[Box<dyn Participant>],
) -> Vec<ParticipantOutcome> {
    let request = config.request();
    participants
        .iter()
        .enumerate()
        .map(|(index, participant)| {
            let descriptor = participant.descriptor();
            let started_at = config.clock.now();
            // Each invocation gets the same response-free request; collected
            // responses stay unavailable until the round is complete.
            let result = classify(participant.invoke(request.clone()), &descriptor);
            ParticipantOutcome {
                participant: descriptor,
                role: ContributionRole::FirstRound,
                sequence: index as u32,
                started_at,
                finished_at: config.clock.now(),
                first_round_isolated: true,
                result,
            }
        })
        .collect()
}

/// Turn a participant's raw reply into a classified contribution result.
fn classify(
    reply: Result<ParticipantResponse, ParticipantFailure>,
    expected: &ParticipantDescriptor,
) -> ContributionResult {
    match reply {
        Ok(response) if response.participant != *expected => ContributionResult::Failed {
            kind: ParticipantFailureKind::Permanent,
            reason: ContributionFailureReason::InvalidResponse,
            provider_request_id: response.provider_request_id.clone(),
        },
        Ok(response) => ContributionResult::Responded(response),
        Err(failure) => ContributionResult::Failed {
            kind: failure.kind,
            reason: ContributionFailureReason::ParticipantUnavailable,
            provider_request_id: failure.provider_request_id,
        },
    }
}

pub trait Participant: Send + Sync {
    fn descriptor(&self) -> ParticipantDescriptor;
    fn invoke(
        &self,
        request: ParticipantRequest,
    ) -> Result<ParticipantResponse, ParticipantFailure>;
}

/// A participant considered for the leader role.
///
/// Carries the eligibility facts a registry holds, without this crate
/// depending on the crate the registry lives in. The caller converts a
/// registration into this; the Round Table never reads a registry directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaderCandidate {
    pub descriptor: ParticipantDescriptor,
    pub enabled: bool,
    /// Whether the registration declares an adjudicator role capability.
    pub can_adjudicate: bool,
    pub supported_levels: Vec<ReasoningAssuranceLevel>,
    /// Whether availability health currently reports the participant usable.
    pub healthy: bool,
}

impl LeaderCandidate {
    /// Eligibility, exactly as the canonical contract enumerates it.
    pub fn eligible_for(&self, level: ReasoningAssuranceLevel) -> bool {
        self.enabled
            && self.can_adjudicate
            && self.healthy
            && self.supported_levels.contains(&level)
    }
}

/// Why no leader could be selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaderSelectionFailure {
    /// Candidates were offered, but none satisfied every eligibility condition.
    NoEligibleCandidate,
    /// No candidates were offered at all.
    NoCandidates,
}

/// The selected leader, with the evidence behind the choice.
///
/// Recorded rather than merely returned: an audit needs to know how many
/// candidates were considered and how many were eligible, not just who won.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaderSelection {
    pub leader: ParticipantDescriptor,
    pub considered: usize,
    pub eligible: usize,
    pub assurance: ReasoningAssuranceLevel,
}

/// Select the Round Table leader deterministically from eligible candidates.
///
/// Determinism here means something stronger than "returns the same value for
/// the same input list": the leader does not depend on the *order* candidates
/// arrive in. Eligible candidates are ordered by participant id and the first is
/// chosen, so shuffling the input cannot change the outcome. Selecting whoever
/// happened to be listed first would make orchestration depend on registry
/// iteration order, which is not a decision anyone made.
///
/// Selection is by identity alone. It reads no provider or model, so swapping
/// one provider for another cannot change who leads, keeping leader selection
/// independent of provider resolution.
///
/// The leader's authority is confined to coordination and synthesis within the
/// session. It confers no execution, approval, policy or Core authority, and no
/// ownership of persistence beyond the session contract.
pub fn select_leader(
    assurance: ReasoningAssuranceLevel,
    candidates: &[LeaderCandidate],
) -> Result<LeaderSelection, LeaderSelectionFailure> {
    if candidates.is_empty() {
        return Err(LeaderSelectionFailure::NoCandidates);
    }
    let mut eligible: Vec<&LeaderCandidate> = candidates
        .iter()
        .filter(|c| c.eligible_for(assurance))
        .collect();
    if eligible.is_empty() {
        return Err(LeaderSelectionFailure::NoEligibleCandidate);
    }
    eligible.sort_by(|a, b| a.descriptor.id.as_str().cmp(b.descriptor.id.as_str()));
    Ok(LeaderSelection {
        leader: eligible[0].descriptor.clone(),
        considered: candidates.len(),
        eligible: eligible.len(),
        assurance,
    })
}

/// Why a registered participant could not be turned into a live one.
///
/// Classified rather than free-form: a resolver reports a category, never a
/// message, so a diagnostic string can never carry a path, endpoint or
/// credential into the session record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionFailure {
    /// Nothing is registered under this participant identity.
    UnknownParticipant,
    /// Registered, but the provider is not usable right now: capability absent,
    /// executable missing, or not authenticated.
    ProviderUnavailable,
    /// The resolver produced a participant whose identity is not the one that
    /// was asked for. Treated as a failure, never as a substitution.
    DescriptorMismatch,
}

/// Maps a registered participant identity to a live participant.
///
/// This is the seam that makes participants interchangeable. The registry
/// *describes* participants; the resolver *instantiates* them. Core depends on
/// this port alone and never on Claude Code, the Anthropic API, Codex, a
/// provider executable, a model name or any credential.
///
/// Implementations live with the adapters, outside Core.
pub trait ParticipantResolver: Send + Sync {
    fn resolve(
        &self,
        descriptor: &ParticipantDescriptor,
    ) -> Result<Box<dyn Participant>, ResolutionFailure>;
}

/// Resolve a participant and verify the resolver returned the one requested.
///
/// The identity check is not defensive paranoia about buggy resolvers: it is the
/// mechanism that makes "no silent fallback" enforceable. A resolver that
/// quietly substituted a reachable provider for an unreachable one would
/// otherwise be indistinguishable from success, and the session would carry
/// another provider's reasoning under the requested provider's name.
pub fn resolve_verified(
    resolver: &dyn ParticipantResolver,
    descriptor: &ParticipantDescriptor,
) -> Result<Box<dyn Participant>, ResolutionFailure> {
    let participant = resolver.resolve(descriptor)?;
    if participant.descriptor() != *descriptor {
        return Err(ResolutionFailure::DescriptorMismatch);
    }
    Ok(participant)
}

/// Invoke a first round over registered identities, resolving each in turn.
///
/// Resolution failure is isolated exactly like invocation failure: the
/// unresolvable participant becomes a failed outcome carrying
/// `ContributionFailureReason::ResolutionFailed`, and every other participant is
/// still resolved and invoked. Nothing is silently swapped for a provider that
/// happens to be available.
pub fn collect_first_round_resolved(
    config: &RoundConfig<'_>,
    descriptors: &[ParticipantDescriptor],
    resolver: &dyn ParticipantResolver,
) -> Vec<ParticipantOutcome> {
    let request = config.request();
    descriptors
        .iter()
        .enumerate()
        .map(|(index, descriptor)| {
            let started_at = config.clock.now();
            let result = match resolve_verified(resolver, descriptor) {
                Err(_) => ContributionResult::Failed {
                    // Resolution never succeeded, so nothing was attempted
                    // against the provider: permanent for this session.
                    kind: ParticipantFailureKind::Permanent,
                    reason: ContributionFailureReason::ResolutionFailed,
                    provider_request_id: None,
                },
                Ok(participant) => classify(participant.invoke(request.clone()), descriptor),
            };
            ParticipantOutcome {
                participant: descriptor.clone(),
                role: ContributionRole::FirstRound,
                sequence: index as u32,
                started_at,
                finished_at: config.clock.now(),
                first_round_isolated: true,
                result,
            }
        })
        .collect()
}

/// Run a first round over registered identities and evaluate quorum over it.
pub fn run_first_round_resolved(
    config: &RoundConfig<'_>,
    descriptors: &[ParticipantDescriptor],
    resolver: &dyn ParticipantResolver,
) -> FirstRound {
    let outcomes = collect_first_round_resolved(config, descriptors, resolver);
    let quorum = evaluate_quorum(config.assurance, &outcomes);
    FirstRound { outcomes, quorum }
}

/// Minimum valid independent first-round responses a Round Table assurance
/// level requires. `None` for levels that are not Round Table levels.
///
/// A4 is A3 plus recorded human reasoning acceptance, so it carries the same
/// first-round requirement.
pub const fn minimum_independent_responses(level: ReasoningAssuranceLevel) -> Option<usize> {
    match level {
        ReasoningAssuranceLevel::A3 | ReasoningAssuranceLevel::A4 => Some(2),
        _ => None,
    }
}

/// Why a panel could not satisfy the requested assurance level.
///
/// Quorum failure is reported at assurance granularity; the individual
/// contribution reasons stay in provenance rather than being collapsed into
/// this code. Both variants map onto `AssuranceFailure` at the router boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuorumReasonCode {
    /// Enough participants were attempted, but too few produced a valid
    /// independent response. This is the spec-pinned below-minimum code.
    ParticipantUnavailable,
    /// The panel could never have satisfied the level: fewer participants were
    /// attempted than the level requires.
    RequiredAssuranceUnachievable,
}

/// An explicit refusal to proceed at the requested assurance level.
///
/// Returned instead of adjudicating a degraded panel. There is deliberately no
/// way to express "succeeded at a lower level": silent downgrade is forbidden,
/// so falling short is a failure the caller must handle, not a quieter success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsufficientAssurance {
    pub assurance: ReasoningAssuranceLevel,
    pub required_responses: usize,
    pub achieved_responses: usize,
    pub attempted_participants: usize,
    pub reason_code: QuorumReasonCode,
}

/// Whether the collected panel satisfied the requested assurance level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuorumResult {
    Satisfied {
        /// The valid, independent first-round responses, in invocation order.
        responses: Vec<ParticipantResponse>,
    },
    Insufficient(InsufficientAssurance),
}

/// A completed first round: every attempted contribution, plus the quorum verdict.
///
/// Outcomes are retained on **both** paths. A panel that failed quorum is
/// exactly the case where the provenance of the failed attempts matters most,
/// so it is never discarded just because no decision follows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirstRound {
    pub outcomes: Vec<ParticipantOutcome>,
    pub quorum: QuorumResult,
}

impl FirstRound {
    pub fn satisfied(&self) -> bool {
        matches!(self.quorum, QuorumResult::Satisfied { .. })
    }
    /// Contributions that failed, retained for diagnosis and audit.
    pub fn failed_outcomes(&self) -> impl Iterator<Item = &ParticipantOutcome> {
        self.outcomes.iter().filter(|o| !o.responded())
    }
}

/// Evaluate quorum over already-collected outcomes.
///
/// Evaluated strictly **after** collection completes (spec
/// `orchestration.quorum.evaluated_after_collection`), so a verdict is never
/// reached on a partially collected panel.
///
/// Only contributions that both responded *and* were first-round isolated count
/// toward independence: a response produced with sight of another participant's
/// answer is not independent evidence, whatever else it may be worth.
///
/// The adjudicator/leader is not considered here and cannot make up a shortfall
/// (D012). It is not part of the first round, so it is structurally incapable of
/// substituting for a missing independent contribution.
pub fn evaluate_quorum(
    assurance: ReasoningAssuranceLevel,
    outcomes: &[ParticipantOutcome],
) -> QuorumResult {
    let required = minimum_independent_responses(assurance).unwrap_or(usize::MAX);
    let responses: Vec<ParticipantResponse> = outcomes
        .iter()
        .filter(|o| o.counts_toward_quorum())
        .filter_map(|o| o.response().cloned())
        .collect();
    let attempted = outcomes.len();

    if responses.len() >= required {
        return QuorumResult::Satisfied { responses };
    }
    QuorumResult::Insufficient(InsufficientAssurance {
        assurance,
        required_responses: required,
        achieved_responses: responses.len(),
        attempted_participants: attempted,
        reason_code: if attempted < required {
            // The panel was never large enough; no amount of availability
            // would have satisfied the level.
            QuorumReasonCode::RequiredAssuranceUnachievable
        } else {
            QuorumReasonCode::ParticipantUnavailable
        },
    })
}

/// Run the independent first round and evaluate quorum over it.
///
/// This is the M0.13 entry point: collection never aborts on a single provider
/// failure, and the quorum verdict is formed only once every participant has
/// been attempted.
pub fn run_first_round(
    config: &RoundConfig<'_>,
    participants: &[Box<dyn Participant>],
) -> FirstRound {
    let outcomes = collect_first_round(config, participants);
    let quorum = evaluate_quorum(config.assurance, &outcomes);
    FirstRound { outcomes, quorum }
}

/// Adjudicator backed by a live participant, typically the selected leader.
///
/// Provider-neutral: it holds a `Participant`, never a provider. The synthesis
/// prompt is assembled from the collected responses and disagreements, so the
/// leader sees the debate it is summarising — which is exactly what first-round
/// participants must not see, and why adjudication happens only after the
/// independent round is complete.
pub struct ParticipantAdjudicator {
    participant: Box<dyn Participant>,
    max_output_tokens: u32,
}

impl ParticipantAdjudicator {
    pub fn new(participant: Box<dyn Participant>, max_output_tokens: u32) -> Self {
        Self {
            participant,
            max_output_tokens,
        }
    }

    pub fn descriptor(&self) -> ParticipantDescriptor {
        self.participant.descriptor()
    }

    /// Build the synthesis prompt. Separate so its shape can be asserted
    /// directly rather than inferred from a provider's answer.
    pub fn synthesis_prompt(
        decision: &DecisionRequest,
        responses: &[ParticipantResponse],
        disagreements: &[Disagreement],
    ) -> String {
        let mut prompt = String::new();
        prompt.push_str(
            "You are synthesising a Round Table consultation. You have no execution \
             authority: do not propose that any action be taken on your behalf, and do \
             not claim the ability to perform one.\n\n",
        );
        prompt.push_str(&format!("Subject: {}\n", decision.subject));
        prompt.push_str(&format!("Question: {}\n\n", decision.prompt));
        prompt.push_str("Independent participant responses:\n");
        for (index, response) in responses.iter().enumerate() {
            prompt.push_str(&format!(
                "\n[{}] participant={} provider={}\n{}\n",
                index + 1,
                response.participant.id.as_str(),
                response.participant.provider.as_str(),
                response.response_text
            ));
        }
        if disagreements.is_empty() {
            prompt.push_str("\nNo disagreement was recorded.\n");
        } else {
            prompt.push_str("\nRecorded disagreements:\n");
            for disagreement in disagreements {
                prompt.push_str(&format!("- {}\n", disagreement.summary));
            }
        }
        prompt.push_str(
            "\nProduce a synthesis that states where the participants agree, where they \
             genuinely differ, and what you conclude. Where they differ, say so plainly \
             rather than averaging the positions into a false consensus.",
        );
        prompt
    }
}

impl Adjudicator for ParticipantAdjudicator {
    fn adjudicate(
        &self,
        decision: &DecisionRequest,
        responses: &[ParticipantResponse],
        disagreements: &[Disagreement],
    ) -> Result<String, RoundTableError> {
        // Adjudicating an empty or single-response panel would be a
        // degraded-panel decision, which the contract forbids. Quorum is the
        // caller's gate, but refuse here too rather than trusting it.
        if responses.len() < 2 {
            return Err(RoundTableError::AdjudicationFailed);
        }
        let synthesis = DecisionRequest {
            id: decision.id.clone(),
            subject: decision.subject.clone(),
            prompt: Self::synthesis_prompt(decision, responses, disagreements),
            evidence: decision.evidence.clone(),
        };
        let response = self
            .participant
            .invoke(ParticipantRequest {
                session_id: decision.id.clone(),
                decision: synthesis,
                max_output_tokens: self.max_output_tokens,
            })
            .map_err(|_| RoundTableError::AdjudicationFailed)?;
        if response.response_text.trim().is_empty() {
            return Err(RoundTableError::AdjudicationFailed);
        }
        Ok(response.response_text)
    }
}

pub trait Adjudicator: Send + Sync {
    fn adjudicate(
        &self,
        decision: &DecisionRequest,
        responses: &[ParticipantResponse],
        disagreements: &[Disagreement],
    ) -> Result<String, RoundTableError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoundTableError {
    InvalidInput(&'static str),
    AssuranceLevelNotRoundTable,
    TooFewParticipants,
    ParticipantFailure(ParticipantFailure),
    AdjudicationFailed,
}
impl std::fmt::Display for RoundTableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RoundTableError {}

pub fn run_round_table(
    session_id: impl Into<String>,
    assurance: ReasoningAssuranceLevel,
    decision: DecisionRequest,
    participants: &[Box<dyn Participant>],
    adjudicator: &dyn Adjudicator,
    max_output_tokens: u32,
) -> Result<RoundTableSession, RoundTableError> {
    if !matches!(
        assurance,
        ReasoningAssuranceLevel::A3 | ReasoningAssuranceLevel::A4
    ) {
        return Err(RoundTableError::AssuranceLevelNotRoundTable);
    }
    if participants.len() < 2 {
        return Err(RoundTableError::TooFewParticipants);
    }
    if decision.id.trim().is_empty()
        || decision.subject.trim().is_empty()
        || decision.prompt.trim().is_empty()
        || max_output_tokens == 0
    {
        return Err(RoundTableError::InvalidInput("decision_request"));
    }
    let session_id = session_id.into();
    if session_id.trim().is_empty() {
        return Err(RoundTableError::InvalidInput("session_id"));
    }
    // M0.8 compatibility shim: delegates to the M0.13 outcome collection and
    // then reproduces the original all-or-nothing contract, so existing callers
    // observe the same errors. Behaviour difference worth knowing: collection no
    // longer stops at the first failure, so every participant is invoked before
    // a failure is reported. The only consumers are this crate's tests and an
    // opt-in live test, and reporting every failure is more useful than
    // reporting the first. New callers should use `collect_first_round` and
    // evaluate quorum themselves.
    let outcomes = collect_first_round(
        &RoundConfig {
            session_id: &session_id,
            assurance,
            decision: &decision,
            max_output_tokens,
            clock: &SystemClock,
        },
        participants,
    );
    let mut responses = Vec::with_capacity(outcomes.len());
    for outcome in outcomes {
        match outcome.result {
            ContributionResult::Responded(response) => responses.push(response),
            ContributionResult::Failed {
                reason: ContributionFailureReason::InvalidResponse,
                ..
            } => {
                return Err(RoundTableError::InvalidInput(
                    "participant_response_identity",
                ));
            }
            ContributionResult::Failed {
                kind,
                provider_request_id,
                ..
            } => {
                return Err(RoundTableError::ParticipantFailure(ParticipantFailure {
                    kind,
                    provider_request_id,
                }));
            }
        }
    }
    let disagreements = collect_disagreements(&session_id, &responses);
    let conclusion = adjudicator
        .adjudicate(&decision, &responses, &disagreements)
        .map_err(|_| RoundTableError::AdjudicationFailed)?;
    let adjudication = RoundTableDecision {
        session_id: session_id.clone(),
        conclusion,
        evidence: decision.evidence.clone(),
        disagreement_ids: disagreements.iter().map(|d| d.id.clone()).collect(),
        execution_authority: false,
    };
    Ok(RoundTableSession {
        id: session_id,
        assurance,
        decision,
        responses,
        disagreements,
        adjudication,
    })
}

/// A completed Round Table session, including everything that went wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchestratedSession {
    pub id: String,
    pub assurance: ReasoningAssuranceLevel,
    pub decision: DecisionRequest,
    /// Every attempted contribution, successful or not.
    pub outcomes: Vec<ParticipantOutcome>,
    pub leader: LeaderSelection,
    pub disagreements: Vec<Disagreement>,
    pub adjudication: RoundTableDecision,
}

/// Why an orchestrated session did not reach a decision.
///
/// Each variant retains the outcomes collected so far: a session that failed is
/// exactly the one whose provenance someone will need to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrchestrationFailure {
    NotARoundTableLevel,
    NoLeader {
        failure: LeaderSelectionFailure,
        outcomes: Vec<ParticipantOutcome>,
    },
    QuorumNotMet {
        insufficient: InsufficientAssurance,
        outcomes: Vec<ParticipantOutcome>,
    },
    AdjudicationFailed {
        outcomes: Vec<ParticipantOutcome>,
    },
}

impl OrchestrationFailure {
    /// The contributions collected before the session failed.
    pub fn outcomes(&self) -> &[ParticipantOutcome] {
        match self {
            OrchestrationFailure::NotARoundTableLevel => &[],
            OrchestrationFailure::NoLeader { outcomes, .. }
            | OrchestrationFailure::QuorumNotMet { outcomes, .. }
            | OrchestrationFailure::AdjudicationFailed { outcomes } => outcomes,
        }
    }
}

/// Run a complete Round Table session: leader selection, independent first
/// round, quorum, then synthesis.
///
/// Ordering is deliberate. The leader is chosen before the round runs, so the
/// choice cannot be influenced by who happened to answer well. Quorum is
/// evaluated after collection completes. Synthesis happens only once quorum
/// holds, so a degraded panel is never adjudicated into a decision that looks
/// like the requested assurance level was satisfied.
pub fn orchestrate_session(
    config: &RoundConfig<'_>,
    descriptors: &[ParticipantDescriptor],
    resolver: &dyn ParticipantResolver,
    candidates: &[LeaderCandidate],
    adjudicator: &dyn Adjudicator,
) -> Result<OrchestratedSession, OrchestrationFailure> {
    if minimum_independent_responses(config.assurance).is_none() {
        return Err(OrchestrationFailure::NotARoundTableLevel);
    }
    let leader = match select_leader(config.assurance, candidates) {
        Ok(leader) => leader,
        Err(failure) => {
            return Err(OrchestrationFailure::NoLeader {
                failure,
                outcomes: Vec::new(),
            });
        }
    };

    let round = run_first_round_resolved(config, descriptors, resolver);
    let responses = match round.quorum {
        QuorumResult::Satisfied { responses } => responses,
        QuorumResult::Insufficient(insufficient) => {
            return Err(OrchestrationFailure::QuorumNotMet {
                insufficient,
                outcomes: round.outcomes,
            });
        }
    };

    let disagreements = collect_disagreements(config.session_id, &responses);
    let conclusion = adjudicator
        .adjudicate(config.decision, &responses, &disagreements)
        .map_err(|_| OrchestrationFailure::AdjudicationFailed {
            outcomes: round.outcomes.clone(),
        })?;

    Ok(OrchestratedSession {
        id: config.session_id.to_owned(),
        assurance: config.assurance,
        decision: config.decision.clone(),
        outcomes: round.outcomes,
        leader,
        disagreements: disagreements.clone(),
        adjudication: RoundTableDecision {
            session_id: config.session_id.to_owned(),
            conclusion,
            evidence: config.decision.evidence.clone(),
            disagreement_ids: disagreements.iter().map(|d| d.id.clone()).collect(),
            // Invariant, on every path: reasoning never authorizes action.
            execution_authority: false,
        },
    })
}

/// Why a session ended without a decision, in a form that can be stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionFailureKind {
    NotARoundTableLevel,
    NoLeader(LeaderSelectionFailure),
    QuorumNotMet(InsufficientAssurance),
    AdjudicationFailed,
}

/// A session as it is persisted: decided or failed, always with its provenance.
///
/// One record type for both outcomes, deliberately. Storing only successful
/// sessions would lose exactly the history needed to diagnose a partial provider
/// outage, and a separate failure log would let the two drift apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub id: String,
    pub assurance: ReasoningAssuranceLevel,
    pub decision: DecisionRequest,
    /// Every attempted contribution, successful or not.
    pub outcomes: Vec<ParticipantOutcome>,
    pub leader: Option<LeaderSelection>,
    pub disagreements: Vec<Disagreement>,
    pub adjudication: Option<RoundTableDecision>,
    pub failure: Option<SessionFailureKind>,
}

impl SessionRecord {
    pub fn decided(&self) -> bool {
        self.adjudication.is_some() && self.failure.is_none()
    }

    /// Record a session that reached a decision.
    pub fn from_session(session: &OrchestratedSession) -> Self {
        Self {
            id: session.id.clone(),
            assurance: session.assurance,
            decision: session.decision.clone(),
            outcomes: session.outcomes.clone(),
            leader: Some(session.leader.clone()),
            disagreements: session.disagreements.clone(),
            adjudication: Some(session.adjudication.clone()),
            failure: None,
        }
    }

    /// Record a session that did not reach a decision. The contributions
    /// collected before it failed are kept.
    pub fn from_failure(
        session_id: &str,
        assurance: ReasoningAssuranceLevel,
        decision: &DecisionRequest,
        failure: &OrchestrationFailure,
    ) -> Self {
        let kind = match failure {
            OrchestrationFailure::NotARoundTableLevel => SessionFailureKind::NotARoundTableLevel,
            OrchestrationFailure::NoLeader { failure, .. } => {
                SessionFailureKind::NoLeader(*failure)
            }
            OrchestrationFailure::QuorumNotMet { insufficient, .. } => {
                SessionFailureKind::QuorumNotMet(insufficient.clone())
            }
            OrchestrationFailure::AdjudicationFailed { .. } => {
                SessionFailureKind::AdjudicationFailed
            }
        };
        Self {
            id: session_id.to_owned(),
            assurance,
            decision: decision.clone(),
            outcomes: failure.outcomes().to_vec(),
            leader: None,
            disagreements: Vec::new(),
            adjudication: None,
            failure: Some(kind),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStoreError {
    /// The record could not be written or read.
    Io,
    /// A stored record could not be understood, e.g. an unsupported schema
    /// version. Never silently repaired.
    Corrupt,
    /// The identifier is unusable as a record key.
    InvalidId,
}

// ---------------------------------------------------------------------------
// Observability: the Round Table's public read contract (M0.15.1)
//
// Architecture Desk, 2026-09-18: the Round Table owns Round Table session
// state. A consumer must not read persistence files, JSON or storage tables
// directly. It depends on this contract; storage adapters implement it. Public
// contract outside, private state inside.
//
// Deliberately separate from `SessionStore`. That port exists so the Round
// Table can write and reload its own history; this one exists so somebody else
// can look. Collapsing them would make every viewer a storage client, which is
// exactly the coupling the boundary forbids.
// ---------------------------------------------------------------------------

/// What happened to one participant, in terms a viewer can render.
///
/// A failure is a first-class value here, not an absence. "We asked and nothing
/// came back" and "we never asked" are different facts, and a surface that
/// renders them identically is lying about a provider outage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContributionStatus {
    Responded,
    Failed(ContributionFailureReason),
}

/// One participant's contribution, flattened for reading.
///
/// Every field is either a fact the session recorded or an explicit `None`.
/// Nothing is inferred to make a view look complete: `model_ref_used` is absent
/// when the provider never reported one, and a viewer showing the configured
/// model there would be asserting something nobody observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContributionView {
    pub participant_id: String,
    pub provider: String,
    /// The model that was configured for this participant.
    pub model_configured: String,
    /// The model the provider actually served, when it reported one.
    pub model_ref_used: Option<String>,
    pub role: ContributionRole,
    pub sequence: u32,
    pub first_round_isolated: bool,
    pub status: ContributionStatus,
    pub duration_ms: u64,
    pub provider_request_id: Option<String>,
}

/// Why a panel did or did not reach the assurance it was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuorumView {
    pub required_responses: usize,
    pub achieved_responses: usize,
    pub attempted_participants: usize,
    pub satisfied: bool,
    /// Present only when quorum was not satisfied.
    pub reason_code: Option<QuorumReasonCode>,
}

/// How the leader was chosen, not merely who won.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaderView {
    pub participant_id: String,
    pub provider: String,
    pub considered: usize,
    pub eligible: usize,
}

/// Enough of a session to list it without loading the whole thing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    pub id: String,
    pub subject: String,
    pub assurance: ReasoningAssuranceLevel,
    pub decided: bool,
    pub quorum: QuorumView,
    pub participants_attempted: usize,
    pub participants_responded: usize,
    pub leader: Option<LeaderView>,
    pub failure: Option<SessionFailureKind>,
}

/// A whole session, read-only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionView {
    pub summary: SessionSummary,
    pub prompt: String,
    /// Every attempted contribution, failures included and in order.
    pub contributions: Vec<ContributionView>,
    pub disagreement_summaries: Vec<String>,
    /// The synthesis, when one was produced. Absent on a failed panel, because
    /// a failed panel is never adjudicated.
    pub conclusion: Option<String>,
    /// Invariant surfaced deliberately rather than assumed: reasoning never
    /// authorizes action, and a viewer should be able to show that it did not.
    pub execution_authority: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionQueryError {
    /// History could not be reached at all.
    Unavailable,
    /// A stored session could not be understood. Never partially rendered.
    Corrupt,
    /// The identifier is unusable.
    InvalidId,
}

/// Read-only access to Round Table session history.
///
/// Strictly observational. There is intentionally no method here that invokes a
/// provider, retries a participant, mutates a session or changes routing: the
/// contract cannot express those, so a viewer built on it cannot acquire
/// authority by accident.
pub trait SessionQuery: Send + Sync {
    /// Session summaries, newest-known first where ordering is available.
    fn list_sessions(&self) -> Result<Vec<SessionSummary>, SessionQueryError>;
    /// One session in full, or `None` if no such session is recorded.
    fn get_session(&self, session_id: &str) -> Result<Option<SessionView>, SessionQueryError>;
}

impl SessionRecord {
    /// Project a stored record into the public read shape.
    ///
    /// Lives on the record rather than in an adapter so every storage backend
    /// yields identical views: the projection is Round Table semantics, not a
    /// storage detail, and two adapters disagreeing about what a session means
    /// would be a contract bug rather than an adapter bug.
    pub fn to_view(&self) -> SessionView {
        let contributions: Vec<ContributionView> = self
            .outcomes
            .iter()
            .map(|o| ContributionView {
                participant_id: o.participant.id.as_str().to_owned(),
                provider: o.participant.provider.as_str().to_owned(),
                model_configured: o.participant.model.as_str().to_owned(),
                model_ref_used: o.model_ref_used().map(|m| m.as_str().to_owned()),
                role: o.role,
                sequence: o.sequence,
                first_round_isolated: o.first_round_isolated,
                status: match o.failure_reason() {
                    Some(reason) => ContributionStatus::Failed(reason),
                    None => ContributionStatus::Responded,
                },
                duration_ms: o.finished_at.0.saturating_sub(o.started_at.0),
                provider_request_id: o.provider_request_id().map(str::to_owned),
            })
            .collect();

        let responded = contributions
            .iter()
            .filter(|c| c.status == ContributionStatus::Responded)
            .count();

        // Quorum is reported from the recorded failure when there is one, and
        // otherwise reconstructed from what the panel actually achieved.
        let quorum = match &self.failure {
            Some(SessionFailureKind::QuorumNotMet(i)) => QuorumView {
                required_responses: i.required_responses,
                achieved_responses: i.achieved_responses,
                attempted_participants: i.attempted_participants,
                satisfied: false,
                reason_code: Some(i.reason_code),
            },
            _ => {
                let required = minimum_independent_responses(self.assurance).unwrap_or(responded);
                let counted = self
                    .outcomes
                    .iter()
                    .filter(|o| o.counts_toward_quorum())
                    .count();
                QuorumView {
                    required_responses: required,
                    achieved_responses: counted,
                    attempted_participants: self.outcomes.len(),
                    satisfied: self.decided() && counted >= required,
                    reason_code: None,
                }
            }
        };

        SessionView {
            summary: SessionSummary {
                id: self.id.clone(),
                subject: self.decision.subject.clone(),
                assurance: self.assurance,
                decided: self.decided(),
                quorum,
                participants_attempted: contributions.len(),
                participants_responded: responded,
                leader: self.leader.as_ref().map(|l| LeaderView {
                    participant_id: l.leader.id.as_str().to_owned(),
                    provider: l.leader.provider.as_str().to_owned(),
                    considered: l.considered,
                    eligible: l.eligible,
                }),
                failure: self.failure.clone(),
            },
            prompt: self.decision.prompt.clone(),
            contributions,
            disagreement_summaries: self
                .disagreements
                .iter()
                .map(|d| d.summary.clone())
                .collect(),
            conclusion: self.adjudication.as_ref().map(|a| a.conclusion.clone()),
            execution_authority: self
                .adjudication
                .as_ref()
                .map(|a| a.execution_authority)
                .unwrap_or(false),
        }
    }
}

/// Persistence port for Round Table session history.
///
/// The port lives here so the Round Table owns its own state; implementations
/// live outside Core. Notably they cannot live in the shared SQLite adapter:
/// both shipped applications depend on it, so placing Round Table persistence
/// there would pull the Round Table implementation into their dependency graphs
/// and break the ADR-0046 boundary.
pub trait SessionStore: Send + Sync {
    fn save(&self, record: &SessionRecord) -> Result<(), SessionStoreError>;
    fn load(&self, session_id: &str) -> Result<Option<SessionRecord>, SessionStoreError>;
    fn list(&self) -> Result<Vec<String>, SessionStoreError>;
}

fn collect_disagreements(session_id: &str, responses: &[ParticipantResponse]) -> Vec<Disagreement> {
    if responses
        .windows(2)
        .all(|pair| pair[0].response_text == pair[1].response_text)
    {
        return Vec::new();
    }
    vec![Disagreement {
        id: format!("{session_id}:response-content"),
        participants: responses.iter().map(|r| r.participant.id.clone()).collect(),
        summary: "Independent participant responses differ.".into(),
        evidence: responses.iter().flat_map(|r| r.evidence.clone()).collect(),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        descriptor: ParticipantDescriptor,
        answer: &'static str,
    }
    impl Participant for Fake {
        fn descriptor(&self) -> ParticipantDescriptor {
            self.descriptor.clone()
        }
        fn invoke(
            &self,
            request: ParticipantRequest,
        ) -> Result<ParticipantResponse, ParticipantFailure> {
            assert!(request.decision.prompt.contains("synthetic"));
            Ok(ParticipantResponse {
                participant: self.descriptor(),
                response_text: self.answer.into(),
                evidence: vec![],
                provider_request_id: None,
                usage: None,
                model_ref_used: None,
            })
        }
    }
    struct Join;
    impl Adjudicator for Join {
        fn adjudicate(
            &self,
            _: &DecisionRequest,
            responses: &[ParticipantResponse],
            _: &[Disagreement],
        ) -> Result<String, RoundTableError> {
            Ok(responses
                .iter()
                .map(|r| r.response_text.clone())
                .collect::<Vec<_>>()
                .join(" | "))
        }
    }
    fn participant(id: &str, answer: &'static str) -> Box<dyn Participant> {
        Box::new(Fake {
            descriptor: ParticipantDescriptor {
                id: ParticipantId::new(id).unwrap(),
                provider: ModelProvider::new("fake").unwrap(),
                model: ModelRef::new("fake-test-model").unwrap(),
            },
            answer,
        })
    }
    fn request() -> DecisionRequest {
        DecisionRequest {
            id: "decision-1".into(),
            subject: "test".into(),
            prompt: "synthetic non-sensitive prompt".into(),
            evidence: vec![],
        }
    }

    /// Monotonic fake clock: every read advances by one millisecond, so
    /// start/finish ordering is observable without depending on wall time.
    struct TestClock(std::sync::atomic::AtomicU64);
    impl TestClock {
        fn new() -> Self {
            Self(std::sync::atomic::AtomicU64::new(1_000))
        }
    }
    impl Clock for TestClock {
        fn now(&self) -> Timestamp {
            Timestamp(self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst))
        }
    }

    fn config<'a>(
        session_id: &'a str,
        assurance: ReasoningAssuranceLevel,
        decision: &'a DecisionRequest,
        clock: &'a dyn Clock,
    ) -> RoundConfig<'a> {
        RoundConfig {
            session_id,
            assurance,
            decision,
            max_output_tokens: 64,
            clock,
        }
    }
    #[test]
    fn independent_responses_are_captured_before_adjudication_and_never_authorize_execution() {
        let session = run_round_table(
            "session-1",
            ReasoningAssuranceLevel::A3,
            request(),
            &[participant("one", "yes"), participant("two", "no")],
            &Join,
            64,
        )
        .unwrap();
        assert_eq!(session.responses.len(), 2);
        assert_eq!(session.disagreements.len(), 1);
        assert!(!session.adjudication.execution_authority);
    }
    /// A participant that always fails, for isolation tests.
    struct Failing {
        descriptor: ParticipantDescriptor,
        kind: ParticipantFailureKind,
    }
    impl Participant for Failing {
        fn descriptor(&self) -> ParticipantDescriptor {
            self.descriptor.clone()
        }
        fn invoke(&self, _: ParticipantRequest) -> Result<ParticipantResponse, ParticipantFailure> {
            Err(ParticipantFailure {
                kind: self.kind.clone(),
                provider_request_id: Some("req_failed".into()),
            })
        }
    }
    fn failing(id: &str, kind: ParticipantFailureKind) -> Box<dyn Participant> {
        Box::new(Failing {
            descriptor: ParticipantDescriptor {
                id: ParticipantId::new(id).unwrap(),
                provider: ModelProvider::new("fake").unwrap(),
                model: ModelRef::new("fake-test-model").unwrap(),
            },
            kind,
        })
    }

    /// A participant that answers claiming to be somebody else.
    struct Impostor {
        descriptor: ParticipantDescriptor,
        claimed: ParticipantDescriptor,
    }
    impl Participant for Impostor {
        fn descriptor(&self) -> ParticipantDescriptor {
            self.descriptor.clone()
        }
        fn invoke(&self, _: ParticipantRequest) -> Result<ParticipantResponse, ParticipantFailure> {
            Ok(ParticipantResponse {
                participant: self.claimed.clone(),
                response_text: "answer".into(),
                evidence: vec![],
                provider_request_id: None,
                usage: None,
                model_ref_used: None,
            })
        }
    }

    // ---- M0.13 D001/D006/D011: failure isolation during collection ----

    #[test]
    fn d001_one_failing_participant_does_not_abort_collection() {
        let outcomes = collect_first_round(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[
                participant("one", "yes"),
                failing("two", ParticipantFailureKind::Transient),
                participant("three", "no"),
            ],
        );
        // Every participant is represented, including the one that failed.
        assert_eq!(outcomes.len(), 3);
        assert!(outcomes[0].responded());
        assert!(!outcomes[1].responded());
        assert!(outcomes[2].responded());
        // The failure did not stop the participant after it from being invoked.
        assert_eq!(outcomes[2].response().unwrap().response_text, "no");
    }

    #[test]
    fn d011_failed_contribution_is_retained_as_provenance() {
        let outcomes = collect_first_round(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[failing("one", ParticipantFailureKind::Permanent)],
        );
        let outcome = &outcomes[0];
        assert_eq!(outcome.participant.id.as_str(), "one");
        assert_eq!(outcome.sequence, 0);
        assert_eq!(
            outcome.failure_reason(),
            Some(ContributionFailureReason::ParticipantUnavailable)
        );
        match &outcome.result {
            ContributionResult::Failed {
                kind,
                provider_request_id,
                ..
            } => {
                // transient/permanent survives even though the reason is coarse
                assert_eq!(*kind, ParticipantFailureKind::Permanent);
                assert_eq!(provider_request_id.as_deref(), Some("req_failed"));
            }
            other => panic!("expected a failed contribution, got {other:?}"),
        }
    }

    #[test]
    fn d006_first_round_isolation_holds_under_partial_failure() {
        let outcomes = collect_first_round(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[
                failing("one", ParticipantFailureKind::Transient),
                participant("two", "yes"),
                failing("three", ParticipantFailureKind::Permanent),
            ],
        );
        assert!(
            outcomes.iter().all(|o| o.first_round_isolated),
            "isolation must hold for every contribution regardless of failures"
        );
        assert_eq!(
            outcomes.iter().map(|o| o.sequence).collect::<Vec<_>>(),
            vec![0, 1, 2],
            "sequence must reflect invocation order across failures"
        );
    }

    #[test]
    fn a_participant_claiming_another_identity_fails_only_its_own_contribution() {
        let real = ParticipantDescriptor {
            id: ParticipantId::new("two").unwrap(),
            provider: ModelProvider::new("fake").unwrap(),
            model: ModelRef::new("fake-test-model").unwrap(),
        };
        let claimed = ParticipantDescriptor {
            id: ParticipantId::new("one").unwrap(),
            provider: ModelProvider::new("fake").unwrap(),
            model: ModelRef::new("fake-test-model").unwrap(),
        };
        let outcomes = collect_first_round(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[
                Box::new(Impostor {
                    descriptor: real,
                    claimed,
                }),
                participant("three", "yes"),
            ],
        );
        assert_eq!(
            outcomes[0].failure_reason(),
            Some(ContributionFailureReason::InvalidResponse)
        );
        assert!(
            outcomes[1].responded(),
            "an impostor must not prevent honest participants from contributing"
        );
    }

    #[test]
    fn all_participants_failing_yields_outcomes_not_an_error() {
        let outcomes = collect_first_round(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[
                failing("one", ParticipantFailureKind::Transient),
                failing("two", ParticipantFailureKind::Permanent),
            ],
        );
        // Collection reports what happened; it is the caller's job to decide
        // that this panel cannot satisfy quorum.
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes.iter().all(|o| !o.responded()));
    }

    // ---- M0.13 D002/D012: quorum evaluated after collection ----

    #[test]
    fn d002_quorum_is_satisfied_by_two_independent_responses() {
        let round = run_first_round(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[
                participant("one", "yes"),
                failing("two", ParticipantFailureKind::Transient),
                participant("three", "no"),
            ],
        );
        // One provider was down; the session still proceeds on the survivors.
        match &round.quorum {
            QuorumResult::Satisfied { responses } => {
                assert_eq!(responses.len(), 2);
                assert_eq!(responses[0].response_text, "yes");
                assert_eq!(responses[1].response_text, "no");
            }
            other => panic!("expected quorum, got {other:?}"),
        }
        // The failed attempt is still on the record.
        assert_eq!(round.failed_outcomes().count(), 1);
        assert_eq!(round.outcomes.len(), 3);
    }

    #[test]
    fn d002_below_minimum_is_insufficient_assurance_not_a_downgrade() {
        let round = run_first_round(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[
                participant("one", "yes"),
                failing("two", ParticipantFailureKind::Transient),
                failing("three", ParticipantFailureKind::Permanent),
            ],
        );
        match &round.quorum {
            QuorumResult::Insufficient(insufficient) => {
                assert_eq!(insufficient.required_responses, 2);
                assert_eq!(insufficient.achieved_responses, 1);
                assert_eq!(insufficient.attempted_participants, 3);
                assert_eq!(
                    insufficient.reason_code,
                    QuorumReasonCode::ParticipantUnavailable
                );
                assert_eq!(insufficient.assurance, ReasoningAssuranceLevel::A3);
            }
            other => panic!("a single surviving participant is not a Round Table: {other:?}"),
        }
        assert!(!round.satisfied());
    }

    #[test]
    fn d013_provenance_survives_a_failed_quorum() {
        let round = run_first_round(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[
                failing("one", ParticipantFailureKind::Transient),
                failing("two", ParticipantFailureKind::Permanent),
            ],
        );
        assert!(!round.satisfied());
        // The case where provenance matters most must not discard it.
        assert_eq!(round.outcomes.len(), 2);
        assert_eq!(round.failed_outcomes().count(), 2);
        assert!(
            round
                .outcomes
                .iter()
                .all(|o| o.failure_reason()
                    == Some(ContributionFailureReason::ParticipantUnavailable))
        );
    }

    #[test]
    fn a_panel_too_small_for_the_level_is_unachievable_not_merely_unavailable() {
        let round = run_first_round(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[participant("one", "yes")],
        );
        match &round.quorum {
            QuorumResult::Insufficient(insufficient) => assert_eq!(
                insufficient.reason_code,
                QuorumReasonCode::RequiredAssuranceUnachievable,
                "one participant could never satisfy A3, regardless of availability"
            ),
            other => panic!("expected insufficient, got {other:?}"),
        }
    }

    #[test]
    fn d012_a_leader_contribution_cannot_top_up_a_short_quorum() {
        // The failure mode this guards: a short panel, and someone later decides
        // the adjudicator can reason too, so it contributes an answer to make up
        // the shortfall. Model that attempt directly and prove it is rejected.
        let mut outcomes = collect_first_round(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[
                participant("one", "yes"),
                failing("two", ParticipantFailureKind::Permanent),
            ],
        );
        assert!(matches!(
            evaluate_quorum(ReasoningAssuranceLevel::A3, &outcomes),
            QuorumResult::Insufficient(_)
        ));

        // The leader answers after the first round, having seen what was already
        // collected, and is appended to the panel.
        let leader = ParticipantDescriptor {
            id: ParticipantId::new("leader").unwrap(),
            provider: ModelProvider::new("fake").unwrap(),
            model: ModelRef::new("fake-test-model").unwrap(),
        };
        outcomes.push(ParticipantOutcome {
            participant: leader.clone(),
            role: ContributionRole::FirstRound,
            sequence: 2,
            started_at: Timestamp(0),
            finished_at: Timestamp(1),
            // Not isolated: it was produced with sight of the first round.
            first_round_isolated: false,
            result: ContributionResult::Responded(ParticipantResponse {
                participant: leader,
                response_text: "yes".into(),
                evidence: vec![],
                provider_request_id: None,
                usage: None,
                model_ref_used: None,
            }),
        });

        match evaluate_quorum(ReasoningAssuranceLevel::A3, &outcomes) {
            QuorumResult::Insufficient(insufficient) => {
                assert_eq!(
                    insufficient.achieved_responses, 1,
                    "the leader's post-hoc answer must not be counted as independent"
                );
                assert_eq!(
                    insufficient.reason_code,
                    QuorumReasonCode::ParticipantUnavailable
                );
            }
            other => panic!("a leader must not be able to manufacture quorum: {other:?}"),
        }
    }

    #[test]
    fn a_non_isolated_contribution_does_not_count_toward_independence() {
        let mut outcomes = collect_first_round(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[participant("one", "yes"), participant("two", "no")],
        );
        assert!(matches!(
            evaluate_quorum(ReasoningAssuranceLevel::A3, &outcomes),
            QuorumResult::Satisfied { .. }
        ));
        // Mark one as having seen another response: it is no longer independent.
        outcomes[1].first_round_isolated = false;
        match evaluate_quorum(ReasoningAssuranceLevel::A3, &outcomes) {
            QuorumResult::Insufficient(insufficient) => {
                assert_eq!(insufficient.achieved_responses, 1)
            }
            other => panic!("a non-isolated response is not independent evidence: {other:?}"),
        }
    }

    #[test]
    fn non_round_table_levels_never_satisfy_quorum() {
        assert_eq!(
            minimum_independent_responses(ReasoningAssuranceLevel::A3),
            Some(2)
        );
        assert_eq!(
            minimum_independent_responses(ReasoningAssuranceLevel::A4),
            Some(2)
        );
        assert_eq!(
            minimum_independent_responses(ReasoningAssuranceLevel::A2),
            None
        );

        let outcomes = collect_first_round(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[participant("one", "yes"), participant("two", "no")],
        );
        // A2 is not a Round Table level; it must not be satisfiable here.
        assert!(matches!(
            evaluate_quorum(ReasoningAssuranceLevel::A2, &outcomes),
            QuorumResult::Insufficient(_)
        ));
    }

    // ---- M0.13 D005: contribution provenance ----

    #[test]
    fn d005_every_contribution_carries_full_provenance() {
        let clock = TestClock::new();
        let outcomes = collect_first_round(
            &config("session-1", ReasoningAssuranceLevel::A3, &request(), &clock),
            &[
                participant("one", "yes"),
                failing("two", ParticipantFailureKind::Permanent),
            ],
        );

        for (index, outcome) in outcomes.iter().enumerate() {
            assert_eq!(outcome.role, ContributionRole::FirstRound);
            assert_eq!(outcome.sequence, index as u32);
            assert!(outcome.first_round_isolated);
            assert!(
                outcome.started_at < outcome.finished_at,
                "a contribution must record when it began and ended"
            );
            // Identity provenance is present for successes and failures alike.
            assert!(!outcome.participant.id.as_str().is_empty());
            assert!(!outcome.participant.provider.as_str().is_empty());
        }
        // The failed attempt still carries its classification and request id.
        assert_eq!(
            outcomes[1].failure_reason(),
            Some(ContributionFailureReason::ParticipantUnavailable)
        );
        assert_eq!(outcomes[1].provider_request_id(), Some("req_failed"));
    }

    #[test]
    fn d005_model_actually_used_is_recorded_as_unknown_rather_than_assumed() {
        let outcomes = collect_first_round(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[participant("one", "yes")],
        );
        // The fake reports no model, so "used" is unknown. It must NOT be
        // silently backfilled from the configured model: "we asked for X" and
        // "X answered" are different claims.
        assert_eq!(outcomes[0].model_ref_used(), None);
        assert_eq!(outcomes[0].participant.model.as_str(), "fake-test-model");
    }

    #[test]
    fn d005_a_provider_serving_a_different_model_is_visible_in_provenance() {
        struct Swapped {
            descriptor: ParticipantDescriptor,
        }
        impl Participant for Swapped {
            fn descriptor(&self) -> ParticipantDescriptor {
                self.descriptor.clone()
            }
            fn invoke(
                &self,
                _: ParticipantRequest,
            ) -> Result<ParticipantResponse, ParticipantFailure> {
                Ok(ParticipantResponse {
                    participant: self.descriptor.clone(),
                    response_text: "answer".into(),
                    evidence: vec![],
                    provider_request_id: None,
                    usage: None,
                    // Served by a different model than the one configured.
                    model_ref_used: Some(ModelRef::new("actually-used-model").unwrap()),
                })
            }
        }
        let outcomes = collect_first_round(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[Box::new(Swapped {
                descriptor: descriptor("one", "provider-a", "configured-model"),
            })],
        );
        assert_eq!(
            outcomes[0].model_ref_used().map(|m| m.as_str()),
            Some("actually-used-model"),
            "the substitution must be visible, not hidden behind the configured model"
        );
        assert_eq!(outcomes[0].participant.model.as_str(), "configured-model");
    }

    #[test]
    fn an_adjudication_contribution_never_counts_toward_quorum() {
        let mut outcomes = collect_first_round(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[participant("one", "yes"), participant("two", "no")],
        );
        assert!(matches!(
            evaluate_quorum(ReasoningAssuranceLevel::A3, &outcomes),
            QuorumResult::Satisfied { .. }
        ));
        // Relabelling a contribution as adjudication removes it from quorum,
        // even though it responded and was isolated.
        outcomes[1].role = ContributionRole::Adjudication;
        assert!(!outcomes[1].counts_toward_quorum());
        assert!(matches!(
            evaluate_quorum(ReasoningAssuranceLevel::A3, &outcomes),
            QuorumResult::Insufficient(_)
        ));
    }

    // ---- M0.13 D003: deterministic leader selection ----

    fn candidate(id: &str, provider: &str) -> LeaderCandidate {
        LeaderCandidate {
            descriptor: ParticipantDescriptor {
                id: ParticipantId::new(id).unwrap(),
                provider: ModelProvider::new(provider).unwrap(),
                model: ModelRef::new("fake-test-model").unwrap(),
            },
            enabled: true,
            can_adjudicate: true,
            supported_levels: vec![ReasoningAssuranceLevel::A3, ReasoningAssuranceLevel::A4],
            healthy: true,
        }
    }

    #[test]
    fn d003_leader_selection_does_not_depend_on_candidate_order() {
        let a = candidate("alpha", "provider-a");
        let b = candidate("bravo", "provider-b");
        let c = candidate("charlie", "provider-c");

        let forward = select_leader(
            ReasoningAssuranceLevel::A3,
            &[a.clone(), b.clone(), c.clone()],
        )
        .unwrap();
        let reversed = select_leader(ReasoningAssuranceLevel::A3, &[c, b, a]).unwrap();

        assert_eq!(
            forward.leader, reversed.leader,
            "shuffling the registry must not change who leads"
        );
        assert_eq!(forward.leader.id.as_str(), "alpha");
        assert_eq!(forward.considered, 3);
        assert_eq!(forward.eligible, 3);
    }

    #[test]
    fn d003_every_eligibility_condition_excludes_independently() {
        let level = ReasoningAssuranceLevel::A3;
        let mut disabled = candidate("one", "provider-a");
        disabled.enabled = false;
        let mut not_adjudicator = candidate("two", "provider-a");
        not_adjudicator.can_adjudicate = false;
        let mut unhealthy = candidate("three", "provider-a");
        unhealthy.healthy = false;
        let mut wrong_level = candidate("four", "provider-a");
        wrong_level.supported_levels = vec![ReasoningAssuranceLevel::A4];

        for excluded in [&disabled, &not_adjudicator, &unhealthy, &wrong_level] {
            assert!(!excluded.eligible_for(level));
            assert_eq!(
                select_leader(level, std::slice::from_ref(excluded)).err(),
                Some(LeaderSelectionFailure::NoEligibleCandidate)
            );
        }

        // The eligible one is chosen even though it sorts last.
        let eligible = candidate("zulu", "provider-b");
        let selection = select_leader(
            level,
            &[disabled, not_adjudicator, unhealthy, wrong_level, eligible],
        )
        .unwrap();
        assert_eq!(selection.leader.id.as_str(), "zulu");
        assert_eq!(selection.considered, 5);
        assert_eq!(selection.eligible, 1);
    }

    #[test]
    fn d003_leader_selection_ignores_provider_and_model() {
        // Same identities, different providers: the leader must not move.
        let level = ReasoningAssuranceLevel::A3;
        let first = select_leader(
            level,
            &[
                candidate("alpha", "provider-a"),
                candidate("bravo", "provider-b"),
            ],
        )
        .unwrap();
        let swapped = select_leader(
            level,
            &[
                candidate("alpha", "provider-z"),
                candidate("bravo", "provider-y"),
            ],
        )
        .unwrap();
        assert_eq!(
            first.leader.id, swapped.leader.id,
            "changing providers must not change orchestration semantics"
        );
    }

    #[test]
    fn no_candidates_is_distinguished_from_none_eligible() {
        let level = ReasoningAssuranceLevel::A3;
        assert_eq!(
            select_leader(level, &[]).err(),
            Some(LeaderSelectionFailure::NoCandidates)
        );
        let mut ineligible = candidate("one", "provider-a");
        ineligible.enabled = false;
        assert_eq!(
            select_leader(level, &[ineligible]).err(),
            Some(LeaderSelectionFailure::NoEligibleCandidate)
        );
    }

    // ---- M0.13 D004/D010/D014: participant resolution ----

    fn descriptor(id: &str, provider: &str, model: &str) -> ParticipantDescriptor {
        ParticipantDescriptor {
            id: ParticipantId::new(id).unwrap(),
            provider: ModelProvider::new(provider).unwrap(),
            model: ModelRef::new(model).unwrap(),
        }
    }

    /// Resolver backed by an explicit table, as a real one would be.
    struct TableResolver {
        answers: Vec<(ParticipantDescriptor, &'static str)>,
        unavailable: Vec<ParticipantDescriptor>,
    }
    impl ParticipantResolver for TableResolver {
        fn resolve(
            &self,
            descriptor: &ParticipantDescriptor,
        ) -> Result<Box<dyn Participant>, ResolutionFailure> {
            if self.unavailable.contains(descriptor) {
                return Err(ResolutionFailure::ProviderUnavailable);
            }
            match self.answers.iter().find(|(d, _)| d == descriptor) {
                Some((d, answer)) => Ok(Box::new(Fake {
                    descriptor: d.clone(),
                    answer,
                })),
                None => Err(ResolutionFailure::UnknownParticipant),
            }
        }
    }

    /// A resolver that quietly hands back a different provider than requested.
    struct SubstitutingResolver {
        substitute: ParticipantDescriptor,
    }
    impl ParticipantResolver for SubstitutingResolver {
        fn resolve(
            &self,
            _requested: &ParticipantDescriptor,
        ) -> Result<Box<dyn Participant>, ResolutionFailure> {
            Ok(Box::new(Fake {
                descriptor: self.substitute.clone(),
                answer: "substituted",
            }))
        }
    }

    #[test]
    fn d004_two_providers_run_the_same_session_through_the_same_port() {
        let a = descriptor("one", "provider-a", "model-a");
        let b = descriptor("two", "provider-b", "model-b");
        let resolver = TableResolver {
            answers: vec![(a.clone(), "yes"), (b.clone(), "no")],
            unavailable: vec![],
        };
        let round = run_first_round_resolved(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[a, b],
            &resolver,
        );
        match &round.quorum {
            QuorumResult::Satisfied { responses } => {
                assert_eq!(responses.len(), 2);
                // Two genuinely different providers, one port, no Core change.
                assert_eq!(responses[0].participant.provider.as_str(), "provider-a");
                assert_eq!(responses[1].participant.provider.as_str(), "provider-b");
            }
            other => panic!("expected quorum, got {other:?}"),
        }
    }

    #[test]
    fn d014_resolution_failure_is_isolated_and_classified() {
        let a = descriptor("one", "provider-a", "model-a");
        let missing = descriptor("gone", "provider-x", "model-x");
        let b = descriptor("two", "provider-b", "model-b");
        let resolver = TableResolver {
            answers: vec![(a.clone(), "yes"), (b.clone(), "no")],
            unavailable: vec![],
        };
        let round = run_first_round_resolved(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[a, missing.clone(), b],
            &resolver,
        );
        // The unresolvable participant failed on its own, and the participant
        // after it was still resolved and invoked.
        assert_eq!(round.outcomes.len(), 3);
        assert_eq!(
            round.outcomes[1].failure_reason(),
            Some(ContributionFailureReason::ResolutionFailed)
        );
        assert_eq!(round.outcomes[1].participant, missing);
        assert!(round.outcomes[2].responded());
        assert!(round.satisfied(), "survivors still satisfy quorum");
    }

    #[test]
    fn d014_a_resolver_may_not_silently_substitute_another_provider() {
        let requested = descriptor("one", "provider-a", "model-a");
        let other = descriptor("two", "provider-b", "model-b");
        let resolver = SubstitutingResolver {
            substitute: other.clone(),
        };

        // Direct resolution is rejected rather than accepted as success.
        assert_eq!(
            resolve_verified(&resolver, &requested).err(),
            Some(ResolutionFailure::DescriptorMismatch)
        );

        // And through collection, the substitution becomes a classified failure
        // rather than another provider's reasoning wearing the requested name.
        let round = run_first_round_resolved(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            std::slice::from_ref(&requested),
            &resolver,
        );
        assert_eq!(
            round.outcomes[0].failure_reason(),
            Some(ContributionFailureReason::ResolutionFailed)
        );
        assert_eq!(round.outcomes[0].participant, requested);
        assert!(
            round
                .outcomes
                .iter()
                .all(|o| o.response().map(|r| &r.response_text) != Some(&"substituted".to_string())),
            "a substituted response must never enter the session"
        );
    }

    #[test]
    fn an_unavailable_provider_is_distinguished_from_an_unknown_one() {
        let known = descriptor("one", "provider-a", "model-a");
        let resolver = TableResolver {
            answers: vec![(known.clone(), "yes")],
            unavailable: vec![known.clone()],
        };
        assert_eq!(
            resolve_verified(&resolver, &known).err(),
            Some(ResolutionFailure::ProviderUnavailable)
        );
        assert_eq!(
            resolve_verified(&resolver, &descriptor("nope", "p", "m")).err(),
            Some(ResolutionFailure::UnknownParticipant)
        );
    }

    #[test]
    fn total_resolution_failure_is_insufficient_assurance_not_a_panic() {
        let resolver = TableResolver {
            answers: vec![],
            unavailable: vec![],
        };
        let round = run_first_round_resolved(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[descriptor("one", "p", "m"), descriptor("two", "p", "m")],
            &resolver,
        );
        assert!(!round.satisfied());
        assert_eq!(round.failed_outcomes().count(), 2);
    }

    /// Strip Rust comments, tracking string literals so a `//` inside a string
    /// is not mistaken for a comment.
    fn strip_comments(source: &str) -> String {
        let mut out = String::with_capacity(source.len());
        let mut chars = source.chars().peekable();
        let (mut in_string, mut in_line, mut in_block, mut escaped) = (false, false, false, false);
        while let Some(c) = chars.next() {
            if in_line {
                if c == '\n' {
                    in_line = false;
                    out.push(c);
                }
                continue;
            }
            if in_block {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    in_block = false;
                }
                continue;
            }
            if in_string {
                out.push(c);
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    in_string = false;
                }
                continue;
            }
            match c {
                '"' => {
                    in_string = true;
                    out.push(c);
                }
                '/' if chars.peek() == Some(&'/') => {
                    chars.next();
                    in_line = true;
                }
                '/' if chars.peek() == Some(&'*') => {
                    chars.next();
                    in_block = true;
                }
                _ => out.push(c),
            }
        }
        out
    }

    /// D010: no provider, model or participant identifier may be hardcoded in
    /// this crate. Provider neutrality is the whole point of the port.
    ///
    /// Comments are excluded deliberately. The invariant is about what the code
    /// *does*, not what prose explains: a doc comment naming providers as
    /// examples of what Core must never depend on is clarifying, whereas
    /// `ModelRef::new("some-model")` in code would be the actual violation.
    ///
    /// The test module is excluded too, and not for convenience: this very test
    /// names providers in its needle list, and fixtures legitimately name fake
    /// ones. Scanning itself would make the check fail unconditionally, which is
    /// as useless as passing unconditionally.
    #[test]
    fn d010_no_provider_or_model_is_hardcoded_in_core() {
        let source = include_str!("lib.rs");
        let production = source
            .split_once("#[cfg(test)]")
            .map(|(before, _)| before)
            .expect("the test module marker must exist for this scan to be scoped");
        let code = strip_comments(production).to_ascii_lowercase();
        for needle in [
            "anthropic",
            "claude",
            "codex",
            "openai",
            "gpt-",
            "sonnet",
            "haiku",
            "opus",
        ] {
            assert!(
                !code.contains(needle),
                "`{needle}` appears in Round Table port code; the Round Table must \
                 name no provider or model, only the neutral port"
            );
        }
    }

    #[test]
    fn comment_stripper_does_not_blind_the_hardcoding_check() {
        // Comments are removed...
        assert!(!strip_comments("// claude\nlet x = 1;").contains("claude"));
        assert!(!strip_comments("/* claude */ let x = 1;").contains("claude"));
        // ...but code, including string literals, is not.
        assert!(strip_comments("let m = \"claude-x\";").contains("claude-x"));
        // A `//` inside a string must not swallow the rest of the line.
        assert!(strip_comments("let u = \"a//b\"; let m = \"claude\";").contains("claude"));
    }

    // ---- M0.13 D008: adjudication and full session orchestration ----

    fn leader_adjudicator(answer: &'static str) -> ParticipantAdjudicator {
        ParticipantAdjudicator::new(participant("leader", answer), 64)
    }

    #[test]
    fn synthesis_prompt_shows_the_leader_the_debate_and_forbids_execution() {
        let responses = vec![
            ParticipantResponse {
                participant: descriptor("one", "provider-a", "m"),
                response_text: "position one".into(),
                evidence: vec![],
                provider_request_id: None,
                usage: None,
                model_ref_used: None,
            },
            ParticipantResponse {
                participant: descriptor("two", "provider-b", "m"),
                response_text: "position two".into(),
                evidence: vec![],
                provider_request_id: None,
                usage: None,
                model_ref_used: None,
            },
        ];
        let disagreements = vec![Disagreement {
            id: "d1".into(),
            participants: vec![],
            summary: "they differ on scope".into(),
            evidence: vec![],
        }];
        let prompt =
            ParticipantAdjudicator::synthesis_prompt(&request(), &responses, &disagreements);

        assert!(prompt.contains("position one"));
        assert!(prompt.contains("position two"));
        assert!(prompt.contains("they differ on scope"));
        assert!(prompt.contains("no execution authority"));
        assert!(
            prompt.contains("false consensus"),
            "the leader must be told not to average away real disagreement"
        );
    }

    #[test]
    fn adjudication_refuses_a_degraded_panel_even_if_quorum_was_bypassed() {
        let adjudicator = leader_adjudicator("synthesis");
        let single = vec![ParticipantResponse {
            participant: descriptor("one", "provider-a", "m"),
            response_text: "only voice".into(),
            evidence: vec![],
            provider_request_id: None,
            usage: None,
            model_ref_used: None,
        }];
        assert_eq!(
            adjudicator
                .adjudicate(&request(), &single, &[])
                .unwrap_err(),
            RoundTableError::AdjudicationFailed,
            "the adjudicator must not synthesise a one-voice panel even if called directly"
        );
        assert_eq!(
            adjudicator.adjudicate(&request(), &[], &[]).unwrap_err(),
            RoundTableError::AdjudicationFailed
        );
    }

    #[test]
    fn d008_a_full_session_never_grants_execution_authority() {
        let a = descriptor("one", "provider-a", "model-a");
        let b = descriptor("two", "provider-b", "model-b");
        let resolver = TableResolver {
            answers: vec![(a.clone(), "yes"), (b.clone(), "no")],
            unavailable: vec![],
        };
        let session = orchestrate_session(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[a, b],
            &resolver,
            &[candidate("leader", "provider-c")],
            &leader_adjudicator("synthesis"),
        )
        .unwrap();

        assert!(!session.adjudication.execution_authority);
        assert_eq!(session.leader.leader.id.as_str(), "leader");
        assert_eq!(session.outcomes.len(), 2);
        assert_eq!(session.adjudication.conclusion, "synthesis");
        // The panel disagreed, and that is recorded rather than smoothed over.
        assert_eq!(session.disagreements.len(), 1);
        assert_eq!(session.adjudication.disagreement_ids.len(), 1);
    }

    #[test]
    fn a_session_that_loses_quorum_is_refused_but_keeps_its_provenance() {
        let a = descriptor("one", "provider-a", "model-a");
        let gone = descriptor("two", "provider-b", "model-b");
        let resolver = TableResolver {
            answers: vec![(a.clone(), "yes")],
            unavailable: vec![gone.clone()],
        };
        let failure = orchestrate_session(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[a, gone],
            &resolver,
            &[candidate("leader", "provider-c")],
            &leader_adjudicator("synthesis"),
        )
        .unwrap_err();

        match &failure {
            OrchestrationFailure::QuorumNotMet { insufficient, .. } => {
                assert_eq!(insufficient.achieved_responses, 1);
                assert_eq!(
                    insufficient.reason_code,
                    QuorumReasonCode::ParticipantUnavailable
                );
            }
            other => panic!("expected quorum refusal, got {other:?}"),
        }
        // No synthesis was produced, but the evidence of what happened survives.
        assert_eq!(failure.outcomes().len(), 2);
        assert_eq!(
            failure.outcomes()[1].failure_reason(),
            Some(ContributionFailureReason::ResolutionFailed)
        );
    }

    #[test]
    fn a_session_without_an_eligible_leader_never_invokes_participants() {
        let a = descriptor("one", "provider-a", "model-a");
        let b = descriptor("two", "provider-b", "model-b");
        let resolver = TableResolver {
            answers: vec![(a.clone(), "yes"), (b.clone(), "no")],
            unavailable: vec![],
        };
        let mut ineligible = candidate("leader", "provider-c");
        ineligible.healthy = false;

        let failure = orchestrate_session(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A3,
                &request(),
                &TestClock::new(),
            ),
            &[a, b],
            &resolver,
            &[ineligible],
            &leader_adjudicator("synthesis"),
        )
        .unwrap_err();

        match &failure {
            OrchestrationFailure::NoLeader { failure, .. } => {
                assert_eq!(*failure, LeaderSelectionFailure::NoEligibleCandidate)
            }
            other => panic!("expected leader failure, got {other:?}"),
        }
        // Leader selection precedes the round, so nothing was spent on providers.
        assert!(
            failure.outcomes().is_empty(),
            "participants must not be invoked when no leader can be selected"
        );
    }

    #[test]
    fn a_non_round_table_level_is_refused_before_anything_is_invoked() {
        let resolver = TableResolver {
            answers: vec![],
            unavailable: vec![],
        };
        let failure = orchestrate_session(
            &config(
                "session-1",
                ReasoningAssuranceLevel::A2,
                &request(),
                &TestClock::new(),
            ),
            &[],
            &resolver,
            &[candidate("leader", "provider-c")],
            &leader_adjudicator("synthesis"),
        )
        .unwrap_err();
        assert_eq!(failure, OrchestrationFailure::NotARoundTableLevel);
    }

    // ---- M0.8 compatibility shim ----

    #[test]
    fn compatibility_shim_preserves_m0_8_participant_failure_contract() {
        let error = run_round_table(
            "session-1",
            ReasoningAssuranceLevel::A3,
            request(),
            &[
                participant("one", "yes"),
                failing("two", ParticipantFailureKind::Transient),
            ],
            &Join,
            64,
        )
        .unwrap_err();
        assert_eq!(
            error,
            RoundTableError::ParticipantFailure(ParticipantFailure {
                kind: ParticipantFailureKind::Transient,
                provider_request_id: Some("req_failed".into()),
            })
        );
    }

    /// Counts invocations, so a test can see whether it was reached at all.
    struct Counting {
        inner: Box<dyn Participant>,
        calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }
    impl Participant for Counting {
        fn descriptor(&self) -> ParticipantDescriptor {
            self.inner.descriptor()
        }
        fn invoke(
            &self,
            request: ParticipantRequest,
        ) -> Result<ParticipantResponse, ParticipantFailure> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.inner.invoke(request)
        }
    }

    #[test]
    fn compatibility_shim_reaches_every_participant_before_reporting_the_first_failure() {
        // spec/round_table.yaml: single_participant_failure_aborts_collection is
        // false, and the shim delegates to that implementation. The M0.8 error is
        // preserved, but a failure early in the panel must not stop later
        // participants being invoked and recorded.
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let error = run_round_table(
            "session-1",
            ReasoningAssuranceLevel::A3,
            request(),
            &[
                failing("one", ParticipantFailureKind::Transient),
                Box::new(Counting {
                    inner: participant("two", "yes"),
                    calls: calls.clone(),
                }),
            ],
            &Join,
            64,
        )
        .unwrap_err();
        assert!(matches!(error, RoundTableError::ParticipantFailure(_)));
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "a participant after a failed one must still be invoked"
        );
    }

    #[test]
    fn fewer_than_two_participants_or_non_roundtable_assurance_is_rejected() {
        assert_eq!(
            run_round_table(
                "session-1",
                ReasoningAssuranceLevel::A3,
                request(),
                &[participant("one", "yes")],
                &Join,
                64
            )
            .unwrap_err(),
            RoundTableError::TooFewParticipants
        );
        assert_eq!(
            run_round_table(
                "session-1",
                ReasoningAssuranceLevel::A2,
                request(),
                &[participant("one", "yes"), participant("two", "yes")],
                &Join,
                64
            )
            .unwrap_err(),
            RoundTableError::AssuranceLevelNotRoundTable
        );
    }
}
