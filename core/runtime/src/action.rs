use maia_domain::{Action, Approval, PolicyDecision, Sha256Hex};
use maia_policy::{PolicyLayers, compose_layers};
use maia_store::{
    ActionRevisionCommand, ApprovalSupersession, AuditEventDraft, RevisionRepository, StoreResult,
};

use crate::error::{RuntimeError, RuntimeResult};

/// Resolved facts for one material Action revision.  No provider or
/// canonicalizer is hidden here; callers provide the completed Action,
/// ActionBinding hash, policy layers and (when applicable) Approval binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviseActionCommand {
    pub action: Action,
    pub action_hash: Sha256Hex,
    pub policy_layers: PolicyLayers,
    pub approval: Option<Approval>,
    pub supersede_approval: Option<ApprovalSupersession>,
    pub audit: AuditEventDraft,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionRevisionOutcome {
    Created {
        action: Box<Action>,
        action_hash: Sha256Hex,
        approval: Option<Box<Approval>>,
        policy: PolicyDecision,
    },
    Blocked {
        action: Box<Action>,
        action_hash: Sha256Hex,
        policy: PolicyDecision,
    },
}

/// Runs policy composition and then enters the atomic T1 store operation.
/// A policy deny still records the immutable Action revision and audit event,
/// but deliberately creates no Approval record.
pub fn revise_action<S>(
    store: &S,
    command: ReviseActionCommand,
) -> RuntimeResult<ActionRevisionOutcome>
where
    S: RevisionRepository,
{
    command
        .action
        .validate()
        .map_err(RuntimeError::InvalidInput)?;
    let policy = compose_layers(command.policy_layers);
    validate_approval_shape(
        &command.action,
        &command.action_hash,
        policy,
        command.approval.as_ref(),
    )?;
    let store_command = ActionRevisionCommand {
        action: command.action.clone(),
        action_hash: command.action_hash.clone(),
        supersede_approval: command.supersede_approval,
        new_approval: command.approval.clone(),
        audit: command.audit,
    };
    insert_t1(store, &store_command)?;
    if policy == PolicyDecision::Deny {
        Ok(ActionRevisionOutcome::Blocked {
            action: Box::new(command.action),
            action_hash: command.action_hash,
            policy,
        })
    } else {
        Ok(ActionRevisionOutcome::Created {
            action: Box::new(command.action),
            action_hash: command.action_hash,
            approval: command.approval.map(Box::new),
            policy,
        })
    }
}

fn insert_t1<S: RevisionRepository>(
    store: &S,
    command: &ActionRevisionCommand,
) -> RuntimeResult<()> {
    let result: StoreResult<()> = store.insert_action_revision(command);
    result.map_err(RuntimeError::from)
}

fn validate_approval_shape(
    action: &Action,
    action_hash: &Sha256Hex,
    policy: PolicyDecision,
    approval: Option<&Approval>,
) -> RuntimeResult<()> {
    match (policy, approval) {
        (PolicyDecision::Deny, None) => Ok(()),
        (PolicyDecision::Deny, Some(_)) => Err(RuntimeError::InvalidInput(
            maia_domain::DomainError::InvalidApprovalMetadata,
        )),
        (PolicyDecision::Allow, Some(value))
            if value.state() == &maia_domain::ApprovalState::NotRequired
                && value.policy_decision() == &PolicyDecision::Allow =>
        {
            validate_approval_binding(action, action_hash, value)
        }
        (PolicyDecision::Confirm | PolicyDecision::ElevatedConfirm, Some(value))
            if value.state() == &maia_domain::ApprovalState::Pending
                && value.policy_decision() == &policy =>
        {
            validate_approval_binding(action, action_hash, value)
        }
        _ => Err(RuntimeError::InvalidInput(
            maia_domain::DomainError::InvalidApprovalMetadata,
        )),
    }
}

fn validate_approval_binding(
    action: &Action,
    action_hash: &Sha256Hex,
    approval: &Approval,
) -> RuntimeResult<()> {
    if approval.action_id() != action.id()
        || approval.action_version() != action.version()
        || approval.action_hash() != action_hash
    {
        return Err(RuntimeError::Conflict);
    }
    approval.validate().map_err(RuntimeError::InvalidInput)
}
