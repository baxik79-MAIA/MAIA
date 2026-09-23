use maia_domain::{Action, ConnectorProfileId, ConnectorSelection, RiskClass, Sha256Hex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoutingBlocker {
    ConnectorSelectionInvalid,
    ConnectorMismatch,
    ConnectorBindingMismatch,
    PolicyRoutedRiskNotAllowed,
    PolicyRoutedFactsMissing,
    ToolFingerprintMissing,
    ToolFingerprintMismatch,
    NonMcpFingerprintPresent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingDecision {
    Allowed,
    Blocked(Vec<RoutingBlocker>),
}

pub struct RoutingFacts<'a> {
    pub action: &'a Action,
    pub requested_connector_id: Option<&'a ConnectorProfileId>,
    pub actual_connector_id: Option<&'a ConnectorProfileId>,
    pub current_connector_binding_hash: Option<&'a Sha256Hex>,
    pub current_tool_definition_fingerprint: Option<&'a Sha256Hex>,
    pub mcp_invocation: bool,
    pub policy_routed_compliant: bool,
    pub policy_routed_audited: bool,
}

pub fn check_routing(facts: RoutingFacts<'_>) -> RoutingDecision {
    let mut blockers = Vec::new();
    match *facts.action.connector_selection() {
        ConnectorSelection::None => {
            if facts.action.connector_profile_id().is_some()
                || facts.action.connector_binding_hash().is_some()
                || facts.requested_connector_id.is_some()
                || facts.actual_connector_id.is_some()
                || facts.current_connector_binding_hash.is_some()
            {
                blockers.push(RoutingBlocker::ConnectorSelectionInvalid);
            }
        }
        ConnectorSelection::Fixed => {
            let expected = facts.action.connector_profile_id();
            if expected.is_none() || facts.action.connector_binding_hash().is_none() {
                blockers.push(RoutingBlocker::ConnectorSelectionInvalid);
            }
            if facts.requested_connector_id != expected.as_ref()
                || facts.actual_connector_id != expected.as_ref()
            {
                blockers.push(RoutingBlocker::ConnectorMismatch);
            }
            if facts.current_connector_binding_hash
                != facts.action.connector_binding_hash().as_ref()
            {
                blockers.push(RoutingBlocker::ConnectorBindingMismatch);
            }
        }
        ConnectorSelection::PolicyRouted => {
            if !matches!(
                facts.action.risk_class(),
                RiskClass::Read | RiskClass::Analyze | RiskClass::Draft
            ) {
                blockers.push(RoutingBlocker::PolicyRoutedRiskNotAllowed);
            }
            if !facts.policy_routed_compliant
                || !facts.policy_routed_audited
                || facts.requested_connector_id.is_none()
                || facts.actual_connector_id.is_none()
                || facts.requested_connector_id != facts.actual_connector_id
            {
                blockers.push(RoutingBlocker::PolicyRoutedFactsMissing);
            }
        }
    }

    if facts.mcp_invocation {
        if facts.action.tool_definition_fingerprint().is_none()
            || facts.current_tool_definition_fingerprint.is_none()
        {
            blockers.push(RoutingBlocker::ToolFingerprintMissing);
        }
        if facts.current_tool_definition_fingerprint
            != facts.action.tool_definition_fingerprint().as_ref()
        {
            blockers.push(RoutingBlocker::ToolFingerprintMismatch);
        }
    } else if facts.action.tool_definition_fingerprint().is_some()
        || facts.current_tool_definition_fingerprint.is_some()
    {
        blockers.push(RoutingBlocker::NonMcpFingerprintPresent);
    }
    blockers.sort_unstable();
    blockers.dedup();
    if blockers.is_empty() {
        RoutingDecision::Allowed
    } else {
        RoutingDecision::Blocked(blockers)
    }
}
