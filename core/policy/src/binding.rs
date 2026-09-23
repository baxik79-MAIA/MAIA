use maia_domain::{Action, Approval, ApprovalState, PolicyDecision, Sha256Hex, Timestamp, Version};

use crate::assurance::assurance_satisfies;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BindingValidityBlocker {
    StaleAction,
    Expired,
    PolicyDenied,
    ActorUnauthorized,
    InsufficientAssurance,
    InvalidApprovalRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BindingValidityBlockerSet(Vec<BindingValidityBlocker>);

impl BindingValidityBlockerSet {
    pub fn new(values: impl IntoIterator<Item = BindingValidityBlocker>) -> Self {
        let mut values: Vec<_> = values.into_iter().collect();
        values.sort_unstable();
        values.dedup();
        Self(values)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn as_slice(&self) -> &[BindingValidityBlocker] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BindingValidityFacts<'a> {
    pub action: &'a Action,
    pub approval: &'a Approval,
    pub current_action_version: &'a Version,
    pub current_action_hash: &'a Sha256Hex,
    pub now: &'a Timestamp,
    pub current_policy: PolicyDecision,
    /// Authorization of the actor recorded on the Approval decision.
    pub approval_actor_authorized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingValidityReport {
    blockers: BindingValidityBlockerSet,
}

impl BindingValidityReport {
    pub fn blockers(&self) -> &BindingValidityBlockerSet {
        &self.blockers
    }
    pub fn is_valid(&self) -> bool {
        self.blockers.is_empty()
    }
}

pub fn check_binding_validity(facts: BindingValidityFacts<'_>) -> BindingValidityReport {
    let mut blockers = Vec::new();
    let approval = facts.approval;
    let record_valid = approval.validate().is_ok();

    if approval.action_id() != facts.action.id()
        || facts.action.version() != facts.current_action_version
        || approval.action_version() != facts.current_action_version
        || approval.action_hash() != facts.current_action_hash
    {
        blockers.push(BindingValidityBlocker::StaleAction);
    }

    if let Some(expires_at) = approval.expires_at() {
        if facts.now >= expires_at {
            blockers.push(BindingValidityBlocker::Expired);
        }
    }

    if facts.current_policy == PolicyDecision::Deny {
        blockers.push(BindingValidityBlocker::PolicyDenied);
    }

    if !facts.approval_actor_authorized {
        blockers.push(BindingValidityBlocker::ActorUnauthorized);
    }

    if approval.state() == &ApprovalState::Approved
        && !assurance_satisfies(
            *approval.achieved_assurance(),
            *approval.required_assurance(),
        )
    {
        blockers.push(BindingValidityBlocker::InsufficientAssurance);
    }

    // A malformed record is a distinct fail-closed runtime fact. The canonical
    // binding values remain the five named failure reasons plus valid.
    if !record_valid {
        blockers.push(BindingValidityBlocker::InvalidApprovalRecord);
    }

    BindingValidityReport {
        blockers: BindingValidityBlockerSet::new(blockers),
    }
}
