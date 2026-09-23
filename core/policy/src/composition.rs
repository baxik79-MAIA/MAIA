use maia_domain::PolicyDecision;

/// Already-resolved decisions for the canonical tenant/workspace/user layers.
/// No lookup or scope resolution is performed here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyLayers {
    pub tenant: PolicyDecision,
    pub workspace: PolicyDecision,
    pub user: PolicyDecision,
}

impl PolicyLayers {
    pub const fn new(
        tenant: PolicyDecision,
        workspace: PolicyDecision,
        user: PolicyDecision,
    ) -> Self {
        Self {
            tenant,
            workspace,
            user,
        }
    }

    pub const fn decisions(self) -> [PolicyDecision; 3] {
        [self.tenant, self.workspace, self.user]
    }
}

/// Composes layers using the canonical maximum-applicable-restriction rule.
/// Deny is absorbing and lower layers cannot weaken a higher restriction.
pub const fn compose_policy_decisions(
    tenant: PolicyDecision,
    workspace: PolicyDecision,
    user: PolicyDecision,
) -> PolicyDecision {
    let mut result = tenant;
    let decisions = [workspace, user];
    let mut index = 0;
    while index < decisions.len() {
        if restriction_rank(decisions[index]) > restriction_rank(result) {
            result = decisions[index];
        }
        index += 1;
    }
    result
}

const fn restriction_rank(decision: PolicyDecision) -> u8 {
    match decision {
        PolicyDecision::Allow => 0,
        PolicyDecision::Confirm => 1,
        PolicyDecision::ElevatedConfirm => 2,
        PolicyDecision::Deny => 3,
    }
}

pub const fn compose_layers(layers: PolicyLayers) -> PolicyDecision {
    compose_policy_decisions(layers.tenant, layers.workspace, layers.user)
}
