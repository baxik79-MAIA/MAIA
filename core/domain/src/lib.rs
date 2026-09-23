//! MAIA canonical execution domain. No I/O, routing, persistence or authorization.
//!
//! Strong IDs cannot be interchanged:
//! ```compile_fail
//! use maia_domain::{ActionId, RunId};
//! let action: ActionId = "01900000-0000-7000-8000-000000000000".parse().unwrap();
//! let run: RunId = action;
//! ```
//! Actions intentionally have no generic paused state:
//! ```compile_fail
//! use maia_domain::ActionState;
//! let state = ActionState::Paused;
//! ```
#![forbid(unsafe_code)]

#[macro_use]
mod macros;
mod validation;
use validation::*;

// Generator formatting is independent of installed rustfmt versions.
#[rustfmt::skip]
#[path = "generated/contracts.rs"]
mod contracts;
pub use contracts::*;

/// Validation errors contain no input payloads or secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    InvalidUuidV7,
    InvalidActionType,
    InvalidTimestamp,
    EmptyValue,
    InvalidSha256Hex,
    InvalidCurrencyCode,
    InvalidInteger {
        primitive: &'static str,
    },
    InvalidStateTransition {
        machine: &'static str,
        from: &'static str,
        to: &'static str,
    },
    MissingConnectorForSourcePreconditions,
    InvalidOutcomeCertainty,
    VersionOverflow,
    AttemptOverflow,
    InvalidConnectorSelection,
    DuplicateSourcePrecondition,
    InvalidApprovalMetadata,
    InvalidFreshnessEvidence,
    InvalidAuditChainLink,
}
impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for DomainError {}

/// Informational set, ordered by canonical risk declaration order, not severity inference.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RiskSummary(Vec<RiskClass>);
impl RiskSummary {
    pub fn new(values: impl IntoIterator<Item = RiskClass>) -> Self {
        let mut values: Vec<_> = values.into_iter().collect();
        values.sort();
        values.dedup();
        Self(values)
    }
    pub fn as_slice(&self) -> &[RiskClass] {
        &self.0
    }
}

impl Action {
    pub fn validate(&self) -> Result<(), DomainError> {
        if !self.source_preconditions.is_empty() && self.connector_profile_id.is_none() {
            return Err(DomainError::MissingConnectorForSourcePreconditions);
        }
        match self.connector_selection {
            ConnectorSelection::None
                if self.connector_profile_id.is_some() || self.connector_binding_hash.is_some() =>
            {
                return Err(DomainError::InvalidConnectorSelection);
            }
            ConnectorSelection::Fixed
                if self.connector_profile_id.is_none() || self.connector_binding_hash.is_none() =>
            {
                return Err(DomainError::InvalidConnectorSelection);
            }
            ConnectorSelection::PolicyRouted if !self.risk_class.permits_policy_routing() => {
                return Err(DomainError::InvalidConnectorSelection);
            }
            _ => {}
        }
        let mut sources: Vec<_> = self
            .source_preconditions
            .iter()
            .map(|s| {
                (
                    s.external_id.as_str(),
                    s.source_version_token.as_str(),
                    s.normalized_payload_hash.as_str(),
                )
            })
            .collect();
        sources.sort_unstable();
        if sources.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DomainError::DuplicateSourcePrecondition);
        }
        Ok(())
    }
}
impl Version {
    /// Checked structural successor; does not perform aggregate CAS or persist history.
    pub fn checked_next(self) -> Result<Self, DomainError> {
        Self::new(
            self.get()
                .checked_add(1)
                .ok_or(DomainError::VersionOverflow)?,
        )
    }
}
impl Attempt {
    pub fn checked_next(self) -> Result<Self, DomainError> {
        Self::new(
            self.get()
                .checked_add(1)
                .ok_or(DomainError::AttemptOverflow)?,
        )
    }
}
impl Approval {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.policy_decision.required_assurance() != Some(self.required_assurance)
            || !self.policy_decision.permits_record_state(self.state)
        {
            return Err(DomainError::InvalidApprovalMetadata);
        }
        if matches!(
            self.state,
            ApprovalState::NotRequired | ApprovalState::Pending
        ) && (self.decided_by.is_some()
            || self.decided_at.is_some()
            || self.decided_surface.is_some()
            || self.achieved_assurance != ApprovalAssurance::None)
        {
            return Err(DomainError::InvalidApprovalMetadata);
        }
        if self.state == ApprovalState::Pending && self.decision_note.is_some() {
            return Err(DomainError::InvalidApprovalMetadata);
        }
        if matches!(
            self.state,
            ApprovalState::Approved | ApprovalState::Rejected
        ) && (self.decided_by.is_none()
            || self.decided_at.is_none()
            || self.decided_surface.is_none()
            || self.achieved_assurance < ApprovalAssurance::Confirm
            || (self.state == ApprovalState::Approved
                && self.achieved_assurance < self.required_assurance))
        {
            return Err(DomainError::InvalidApprovalMetadata);
        }
        Ok(())
    }
    /// Structural lifecycle update only. Human decisions require complete metadata,
    /// current policy and atomic store CAS outside this crate. Historical fields survive.
    pub fn transition_to(&mut self, next: ApprovalState) -> Result<(), DomainError> {
        self.state.validate_transition(next)?;
        let mut candidate = self.clone();
        candidate.state = next;
        candidate.version = self.version.checked_next()?;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
}
impl FreshnessEvidence {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.result == FreshnessResult::Mismatched
            && (self.observed_source_version_token.is_none()
                || self.observed_payload_hash.is_none()
                || (self.observed_source_version_token.as_ref()
                    == Some(&self.precondition.source_version_token)
                    && self.observed_payload_hash.as_ref()
                        == Some(&self.precondition.normalized_payload_hash)))
        {
            return Err(DomainError::InvalidFreshnessEvidence);
        }
        if self.result == FreshnessResult::Matched
            && (self.observed_source_version_token.as_ref()
                != Some(&self.precondition.source_version_token)
                || self.observed_payload_hash.as_ref()
                    != Some(&self.precondition.normalized_payload_hash))
        {
            return Err(DomainError::InvalidFreshnessEvidence);
        }
        Ok(())
    }
}
impl Run {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.outcome_certainty != self.state.outcome_certainty() {
            return Err(DomainError::InvalidOutcomeCertainty);
        }
        Ok(())
    }
    /// Validates the FSM edge and updates certainty atomically. The caller must
    /// establish reconciliation evidence before leaving reconciliation.
    pub fn transition_to(&mut self, next: RunState) -> Result<(), DomainError> {
        self.state.validate_transition(next)?;
        let mut candidate = self.clone();
        candidate.state = next;
        candidate.outcome_certainty = next.outcome_certainty();
        candidate.version = self.version.checked_next()?;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
}
impl AuditRecord {
    pub fn validate(&self) -> Result<(), DomainError> {
        const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";
        let is_genesis = self.sequence.get() == 1;
        let has_genesis_link = self.prev_hash.as_str() == GENESIS;
        if is_genesis != has_genesis_link {
            return Err(DomainError::InvalidAuditChainLink);
        }
        Ok(())
    }
}
macro_rules! unconstrained {
    ($($name:ident),+) => {$(impl $name {
        pub fn validate(&self) -> Result<(), DomainError> { Ok(()) }
    })+};
}
unconstrained!(
    AgentTask,
    ExecutionPlan,
    CostEstimate,
    SourcePrecondition,
    CanonicalizerRef,
    ReconciliationEvidence,
    EvidenceImportSource,
    EvidenceImportRequest,
    Artifact,
    EvidenceCitation,
    Outcome,
    UsageRecord
);
impl AgentTask {
    /// Checks structural legality and applies the mutable record CAS successor.
    /// The authoritative store must still match the expected version/state atomically.
    pub fn transition_to(&mut self, next: AgentTaskState) -> Result<(), DomainError> {
        self.state.validate_transition(next)?;
        let mut candidate = self.clone();
        candidate.state = next;
        candidate.version = self.version.checked_next()?;
        *self = candidate;
        Ok(())
    }
}
impl Action {
    /// Checks structural legality only; Action.version is an immutable revision.
    pub fn transition_to(&mut self, next: ActionState) -> Result<(), DomainError> {
        self.state.validate_transition(next)?;
        self.state = next;
        Ok(())
    }
}
