use maia_domain::{Action, Approval, ApprovalAssurance, ApprovalState, PolicyDecision};

use crate::assurance::assurance_satisfies;
use crate::binding::{BindingValidityFacts, BindingValidityReport, check_binding_validity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AuthorizationBlocker {
    BindingInvalid,
    CurrentPolicyDenied,
    StateNeverAuthorizes,
    ApprovalStateNotPermitted,
    InsufficientAssurance,
    ExecutionActorUnauthorized,
    FreshnessCheckFailed,
    RoutingCheckFailed,
    ToolDefinitionCheckFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AuthorizationBlockerSet(Vec<AuthorizationBlocker>);

impl AuthorizationBlockerSet {
    pub fn new(values: impl IntoIterator<Item = AuthorizationBlocker>) -> Self {
        let mut values: Vec<_> = values.into_iter().collect();
        values.sort_unstable();
        values.dedup();
        Self(values)
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn as_slice(&self) -> &[AuthorizationBlocker] {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorizationDecision {
    Allowed,
    Blocked(AuthorizationBlockerSet),
}

#[derive(Debug, Clone, Copy)]
pub struct CurrentAuthorizationFacts<'a> {
    pub action: &'a Action,
    pub approval: &'a Approval,
    pub binding: BindingValidityFacts<'a>,
    pub freshness_ok: bool,
    pub routing_ok: bool,
    pub tool_definition_ok: bool,
    /// Authorization of the actor that will perform the material operation.
    pub execution_actor_authorized: bool,
}

pub fn authorize_execution(facts: CurrentAuthorizationFacts<'_>) -> AuthorizationDecision {
    let report: BindingValidityReport = check_binding_validity(facts.binding);
    let mut blockers = Vec::new();
    if !report.is_valid() {
        blockers.push(AuthorizationBlocker::BindingInvalid);
    }

    let policy = facts.binding.current_policy;
    if policy == PolicyDecision::Deny {
        blockers.push(AuthorizationBlocker::CurrentPolicyDenied);
    }

    let state = *facts.approval.state();
    if matches!(
        state,
        ApprovalState::Pending
            | ApprovalState::Rejected
            | ApprovalState::Revoked
            | ApprovalState::Expired
    ) {
        blockers.push(AuthorizationBlocker::StateNeverAuthorizes);
    }

    let required_state_allowed = match policy {
        PolicyDecision::Allow => {
            matches!(state, ApprovalState::NotRequired | ApprovalState::Approved)
        }
        PolicyDecision::Confirm => state == ApprovalState::Approved,
        PolicyDecision::ElevatedConfirm => state == ApprovalState::Approved,
        PolicyDecision::Deny => false,
    };
    if !required_state_allowed {
        blockers.push(AuthorizationBlocker::ApprovalStateNotPermitted);
    }

    if !facts.execution_actor_authorized {
        blockers.push(AuthorizationBlocker::ExecutionActorUnauthorized);
    }

    let required = policy.required_assurance();
    if let Some(required) = required {
        let achieved = if state == ApprovalState::NotRequired {
            ApprovalAssurance::None
        } else {
            *facts.approval.achieved_assurance()
        };
        if !assurance_satisfies(achieved, required) {
            blockers.push(AuthorizationBlocker::InsufficientAssurance);
        }
    }

    if !facts.freshness_ok {
        blockers.push(AuthorizationBlocker::FreshnessCheckFailed);
    }
    if !facts.routing_ok {
        blockers.push(AuthorizationBlocker::RoutingCheckFailed);
    }
    if !facts.tool_definition_ok {
        blockers.push(AuthorizationBlocker::ToolDefinitionCheckFailed);
    }

    let blockers = AuthorizationBlockerSet::new(blockers);
    if blockers.is_empty() {
        AuthorizationDecision::Allowed
    } else {
        AuthorizationDecision::Blocked(blockers)
    }
}
