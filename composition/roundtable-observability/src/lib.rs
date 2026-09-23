//! M0.15.2 — read-only observability model for Round Table sessions.
//!
//! The shape a surface renders, derived from the Round Table's public read
//! contract. Two properties drive the whole design.
//!
//! **Unknown is a fact, not a blank.** A surface that renders "the provider did
//! not report which model it served" the same way it renders "nobody has
//! looked" is misleading about what the system observed. `Reported<T>` forces
//! the distinction to survive into the view, so a renderer cannot quietly
//! collapse it.
//!
//! **Absence is a state, not an error.** MAIA must operate with the Round Table
//! entirely absent, so a viewer needs something honest to show when there is no
//! Round Table at all. `Availability` makes that a first-class value rather
//! than an empty list that looks like "no sessions yet".
//!
//! This crate depends on the read contract alone. It cannot reach orchestration
//! or storage, so a viewer built on it cannot invoke a provider, retry a
//! participant, or learn which backend the history came from.
#![forbid(unsafe_code)]

pub mod render;

use maia_domain::ReasoningAssuranceLevel;
use maia_roundtable::{
    ContributionFailureReason, ContributionRole, ContributionStatus, QuorumReasonCode,
    SessionFailureKind, SessionQuery, SessionQueryError, SessionSummary, SessionView,
};

/// Whether Round Table history can be shown at all.
///
/// `NotInstalled` and `Unavailable` are deliberately different. The first means
/// this build has no Round Table, which is a supported configuration and not a
/// fault. The second means there is one but its history could not be read,
/// which is a fault someone should act on. Collapsing them would tell an
/// operator that a broken deployment is working as intended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Available,
    NotInstalled,
    Unavailable(UnavailableReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnavailableReason {
    HistoryUnreachable,
    HistoryCorrupt,
}

impl Availability {
    /// One line a surface can show without composing its own wording.
    pub fn label(&self) -> &'static str {
        match self {
            Availability::Available => "Round Table available",
            Availability::NotInstalled => "Round Table is not installed in this build",
            Availability::Unavailable(UnavailableReason::HistoryUnreachable) => {
                "Round Table session history could not be reached"
            }
            Availability::Unavailable(UnavailableReason::HistoryCorrupt) => {
                "Round Table session history is unreadable"
            }
        }
    }

    /// True only when history is genuinely readable. A surface should gate
    /// rendering on this rather than on an empty list, which cannot tell
    /// "nothing recorded" apart from "nothing reachable".
    pub fn is_available(&self) -> bool {
        matches!(self, Availability::Available)
    }
}

/// A value the underlying system may or may not have supplied.
///
/// Distinct from `Option` at the type level on purpose. `Option::None` is used
/// throughout Rust for "absent", which a renderer tends to translate to an
/// empty cell. `NotReported` says something stronger and more useful: the
/// source was asked and genuinely does not supply this, so a blank would be an
/// understatement rather than a neutral default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reported<T> {
    Reported(T),
    NotReported,
}

impl<T> Reported<T> {
    pub fn from_option(value: Option<T>) -> Self {
        match value {
            Some(v) => Reported::Reported(v),
            None => Reported::NotReported,
        }
    }
    pub fn as_ref(&self) -> Option<&T> {
        match self {
            Reported::Reported(v) => Some(v),
            Reported::NotReported => None,
        }
    }
    pub fn is_reported(&self) -> bool {
        matches!(self, Reported::Reported(_))
    }
}

impl Reported<String> {
    /// Display text. The placeholder states why the value is missing rather
    /// than leaving a bare dash, because "not reported by the provider" and
    /// "we forgot to show it" look identical otherwise.
    pub fn display(&self) -> &str {
        match self {
            Reported::Reported(v) => v,
            Reported::NotReported => "not reported by provider",
        }
    }
}

/// What happened to one contribution, in rendering terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContributionOutcome {
    Responded,
    Failed(ContributionFailureReason),
}

impl ContributionOutcome {
    pub fn label(&self) -> &'static str {
        match self {
            ContributionOutcome::Responded => "responded",
            ContributionOutcome::Failed(ContributionFailureReason::ParticipantUnavailable) => {
                "unavailable"
            }
            ContributionOutcome::Failed(ContributionFailureReason::InvalidResponse) => {
                "invalid response"
            }
            ContributionOutcome::Failed(ContributionFailureReason::ResolutionFailed) => {
                "could not be resolved"
            }
        }
    }
    pub fn succeeded(&self) -> bool {
        matches!(self, ContributionOutcome::Responded)
    }
}

/// One participant row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContributionRow {
    pub participant_id: String,
    pub provider: String,
    pub model_configured: String,
    /// The model actually served. `NotReported` when the provider does not say,
    /// never silently replaced by the configured model.
    pub model_served: Reported<String>,
    pub role: ContributionRole,
    pub sequence: u32,
    pub first_round_isolated: bool,
    pub outcome: ContributionOutcome,
    pub duration_ms: u64,
    pub provider_request_id: Reported<String>,
}

/// Quorum, phrased for a reader rather than for a state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuorumPanel {
    pub required: usize,
    pub achieved: usize,
    pub attempted: usize,
    pub satisfied: bool,
    pub reason_code: Option<QuorumReasonCode>,
}

impl QuorumPanel {
    pub fn headline(&self) -> String {
        if self.satisfied {
            format!(
                "quorum satisfied, {} of {} required",
                self.achieved, self.required
            )
        } else {
            format!(
                "quorum not met, {} of {} required{}",
                self.achieved,
                self.required,
                match self.reason_code {
                    Some(QuorumReasonCode::ParticipantUnavailable) =>
                        " — a participant was unavailable",
                    Some(QuorumReasonCode::RequiredAssuranceUnachievable) =>
                        " — the panel was never large enough",
                    None => "",
                }
            )
        }
    }
}

/// Why this participant leads, not merely that it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaderPanel {
    pub participant_id: String,
    pub provider: String,
    pub considered: usize,
    pub eligible: usize,
}

impl LeaderPanel {
    pub fn basis(&self) -> String {
        format!(
            "{} of {} candidates were eligible to lead",
            self.eligible, self.considered
        )
    }
}

/// A session as a list entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionListEntry {
    pub id: String,
    pub subject: String,
    pub assurance: ReasoningAssuranceLevel,
    pub decided: bool,
    pub responded: usize,
    pub attempted: usize,
    pub quorum_satisfied: bool,
    pub leader: Option<String>,
    pub failure: Option<SessionFailureKind>,
}

impl SessionListEntry {
    /// A short status a list can show. Failure is named, never rendered as an
    /// ordinary session that merely lacks a conclusion.
    pub fn status(&self) -> String {
        if self.decided {
            return format!("decided, {}/{} responded", self.responded, self.attempted);
        }
        match &self.failure {
            Some(SessionFailureKind::QuorumNotMet(i)) => format!(
                "failed closed, {}/{} responded",
                i.achieved_responses, i.required_responses
            ),
            Some(SessionFailureKind::NoLeader(_)) => "failed, no eligible leader".into(),
            Some(SessionFailureKind::AdjudicationFailed) => "failed, synthesis failed".into(),
            Some(SessionFailureKind::NotARoundTableLevel) => {
                "not a Round Table assurance level".into()
            }
            None => "incomplete".into(),
        }
    }
}

/// A whole session, ready to render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionPanel {
    pub entry: SessionListEntry,
    pub prompt: String,
    pub contributions: Vec<ContributionRow>,
    pub quorum: QuorumPanel,
    pub leader: Option<LeaderPanel>,
    pub disagreements: Vec<String>,
    /// Absent on a failed panel, because a failed panel is never adjudicated.
    pub conclusion: Option<String>,
    /// Surfaced so an operator can see the invariant held, rather than trusting
    /// that it did.
    pub execution_authority: bool,
}

impl SessionPanel {
    /// Contributions that did not produce an answer. Kept as a first-class
    /// accessor because a partial outage is the case an operator most needs to
    /// see, and it should not require filtering by hand at each call site.
    pub fn failures(&self) -> impl Iterator<Item = &ContributionRow> {
        self.contributions.iter().filter(|c| !c.outcome.succeeded())
    }
}

fn entry_from(summary: &SessionSummary) -> SessionListEntry {
    SessionListEntry {
        id: summary.id.clone(),
        subject: summary.subject.clone(),
        assurance: summary.assurance,
        decided: summary.decided,
        responded: summary.participants_responded,
        attempted: summary.participants_attempted,
        quorum_satisfied: summary.quorum.satisfied,
        leader: summary.leader.as_ref().map(|l| l.participant_id.clone()),
        failure: summary.failure.clone(),
    }
}

/// Project a read-contract view into the rendering model.
pub fn panel_from(view: &SessionView) -> SessionPanel {
    SessionPanel {
        entry: entry_from(&view.summary),
        prompt: view.prompt.clone(),
        contributions: view
            .contributions
            .iter()
            .map(|c| ContributionRow {
                participant_id: c.participant_id.clone(),
                provider: c.provider.clone(),
                model_configured: c.model_configured.clone(),
                model_served: Reported::from_option(c.model_ref_used.clone()),
                role: c.role,
                sequence: c.sequence,
                first_round_isolated: c.first_round_isolated,
                outcome: match &c.status {
                    ContributionStatus::Responded => ContributionOutcome::Responded,
                    ContributionStatus::Failed(reason) => ContributionOutcome::Failed(*reason),
                },
                duration_ms: c.duration_ms,
                provider_request_id: Reported::from_option(c.provider_request_id.clone()),
            })
            .collect(),
        quorum: QuorumPanel {
            required: view.summary.quorum.required_responses,
            achieved: view.summary.quorum.achieved_responses,
            attempted: view.summary.quorum.attempted_participants,
            satisfied: view.summary.quorum.satisfied,
            reason_code: view.summary.quorum.reason_code,
        },
        leader: view.summary.leader.as_ref().map(|l| LeaderPanel {
            participant_id: l.participant_id.clone(),
            provider: l.provider.clone(),
            considered: l.considered,
            eligible: l.eligible,
        }),
        disagreements: view.disagreement_summaries.clone(),
        conclusion: view.conclusion.clone(),
        execution_authority: view.execution_authority,
    }
}

/// What a surface renders: either history, or an honest reason there is none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservabilityState {
    pub availability: Availability,
    pub sessions: Vec<SessionListEntry>,
}

impl ObservabilityState {
    /// The state to render when this build has no Round Table at all.
    ///
    /// A supported configuration, not a failure, and the caller states it
    /// explicitly rather than the model inferring absence from emptiness.
    pub fn not_installed() -> Self {
        Self {
            availability: Availability::NotInstalled,
            sessions: Vec::new(),
        }
    }
}

/// Read history through the contract, turning failures into rendered state.
///
/// A query error becomes an `Unavailable` state rather than propagating, so a
/// surface always has something truthful to show. It never becomes an empty
/// list, which would claim no sessions exist when the truth is that none could
/// be read.
pub fn load_state(query: &dyn SessionQuery) -> ObservabilityState {
    match query.list_sessions() {
        Ok(summaries) => ObservabilityState {
            availability: Availability::Available,
            sessions: summaries.iter().map(entry_from).collect(),
        },
        Err(SessionQueryError::Corrupt) => ObservabilityState {
            availability: Availability::Unavailable(UnavailableReason::HistoryCorrupt),
            sessions: Vec::new(),
        },
        Err(_) => ObservabilityState {
            availability: Availability::Unavailable(UnavailableReason::HistoryUnreachable),
            sessions: Vec::new(),
        },
    }
}

/// Read one session, if history is reachable and that session exists.
pub fn load_panel(
    query: &dyn SessionQuery,
    session_id: &str,
) -> Result<Option<SessionPanel>, Availability> {
    match query.get_session(session_id) {
        Ok(Some(view)) => Ok(Some(panel_from(&view))),
        Ok(None) => Ok(None),
        Err(SessionQueryError::Corrupt) => {
            Err(Availability::Unavailable(UnavailableReason::HistoryCorrupt))
        }
        Err(_) => Err(Availability::Unavailable(
            UnavailableReason::HistoryUnreachable,
        )),
    }
}
