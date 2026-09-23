use maia_domain::{Approval, ApprovalState, PolicyDecision, Sha256Hex, Timestamp, Version};
use maia_policy::{BindingValidityFacts, BindingValidityReport, check_binding_validity};
use maia_store::{
    ApprovalDecisionCommand, ApprovalRepository, AuditEventDraft, RevisionRepository,
};

use crate::error::{RuntimeError, RuntimeResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecideApprovalCommand {
    pub approval_id: maia_domain::ApprovalId,
    pub expected_version: Version,
    pub expected_state: ApprovalState,
    pub candidate: Approval,
    pub current_action_version: Version,
    pub current_action_hash: Sha256Hex,
    pub current_policy: PolicyDecision,
    pub now: Timestamp,
    pub approval_actor_authorized: bool,
    pub audit: AuditEventDraft,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecisionOutcome {
    Updated(Box<Approval>),
    Blocked(BindingValidityReport),
}

/// Revalidates persisted binding facts and legal domain transition, then
/// delegates the mutation to atomic T2 CAS.  No read-modify-write is split
/// across public store calls.
pub fn decide_approval<S>(
    store: &S,
    command: DecideApprovalCommand,
) -> RuntimeResult<ApprovalDecisionOutcome>
where
    S: ApprovalRepository + RevisionRepository,
{
    if command.candidate.id() != &command.approval_id {
        return Err(RuntimeError::Conflict);
    }
    let current = store
        .get_approval(&command.approval_id)
        .map_err(RuntimeError::from)?;
    let action = store
        .get_action_revision(current.action_id(), command.current_action_version)
        .map_err(RuntimeError::from)?;
    if current.action_hash() != &command.current_action_hash {
        return Err(RuntimeError::Conflict);
    }
    if command.candidate.action_id() != current.action_id()
        || command.candidate.action_version() != current.action_version()
        || command.candidate.action_hash() != current.action_hash()
        || command.candidate.task_id() != current.task_id()
        || command.candidate.policy_id() != current.policy_id()
        || command.candidate.policy_snapshot_hash() != current.policy_snapshot_hash()
        || command.candidate.policy_decision() != current.policy_decision()
        || command.candidate.required_assurance() != current.required_assurance()
        || command.candidate.origin_surface() != current.origin_surface()
    {
        return Err(RuntimeError::Conflict);
    }
    if !matches!(
        command.candidate.state(),
        ApprovalState::Approved | ApprovalState::Rejected
    ) {
        return Err(RuntimeError::InvalidInput(
            maia_domain::DomainError::InvalidApprovalMetadata,
        ));
    }
    command
        .candidate
        .validate()
        .map_err(RuntimeError::InvalidInput)?;
    current
        .state()
        .validate_transition(*command.candidate.state())
        .map_err(RuntimeError::InvalidInput)?;
    if command.candidate.version().get()
        != command
            .expected_version
            .get()
            .checked_add(1)
            .ok_or(RuntimeError::Conflict)?
    {
        return Err(RuntimeError::Conflict);
    }
    if current.version() != &command.expected_version || current.state() != &command.expected_state
    {
        return Err(RuntimeError::Conflict);
    }
    let report = check_binding_validity(BindingValidityFacts {
        action: &action,
        approval: &current,
        current_action_version: &command.current_action_version,
        current_action_hash: &command.current_action_hash,
        now: &command.now,
        current_policy: command.current_policy,
        approval_actor_authorized: command.approval_actor_authorized,
    });
    if !report.is_valid() {
        return Ok(ApprovalDecisionOutcome::Blocked(report));
    }
    let updated = store
        .decide_approval(&ApprovalDecisionCommand {
            approval_id: command.approval_id,
            expected_version: command.expected_version,
            expected_state: command.expected_state,
            candidate: command.candidate,
            audit: command.audit,
        })
        .map_err(RuntimeError::from)?;
    Ok(ApprovalDecisionOutcome::Updated(Box::new(updated)))
}
