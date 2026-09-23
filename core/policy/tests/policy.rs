use maia_domain::*;
use maia_policy::*;

const UUID: &str = "01900000-0000-7000-8000-000000000000";
const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn timestamp(value: &str) -> Timestamp {
    Timestamp::new(value).unwrap()
}
fn id() -> ActionId {
    UUID.parse().unwrap()
}
fn action() -> Action {
    Action::new(
        id(),
        UUID.parse().unwrap(),
        Ordinal::new(0).unwrap(),
        ActionType::new("mail.send").unwrap(),
        None,
        RiskClass::SendExternal,
        ActionState::Planned,
        OpaqueRef::new("input").unwrap(),
        vec![],
        None,
        Version::new(1).unwrap(),
        Version::new(1).unwrap(),
        Sha256Hex::new(HASH).unwrap(),
        CanonicalizerRef::new(
            NonEmptyString::new("test").unwrap(),
            Version::new(1).unwrap(),
        )
        .unwrap(),
        ConnectorSelection::None,
        None,
        None,
    )
    .unwrap()
}

fn approval(
    action: &Action,
    state: ApprovalState,
    policy: PolicyDecision,
    required: ApprovalAssurance,
    achieved: ApprovalAssurance,
    expires_at: Option<Timestamp>,
) -> Approval {
    let human = matches!(state, ApprovalState::Approved | ApprovalState::Rejected);
    Approval::new(
        UUID.parse().unwrap(),
        UUID.parse().unwrap(),
        action.id().clone(),
        PolicyId::new("policy").unwrap(),
        state,
        timestamp("2026-09-11T12:00:00.000Z"),
        human.then(|| timestamp("2026-09-11T12:01:00.000Z")),
        human.then(|| ActorRef::new("approval-actor").unwrap()),
        None,
        Sha256Hex::new(HASH).unwrap(),
        Version::new(1).unwrap(),
        expires_at,
        SurfaceId::new("desktop").unwrap(),
        human.then(|| SurfaceId::new("desktop").unwrap()),
        *action.version(),
        Sha256Hex::new(HASH).unwrap(),
        policy,
        required,
        achieved,
    )
    .unwrap()
}

fn binding<'a>(
    action: &'a Action,
    approval: &'a Approval,
    hash: &'a Sha256Hex,
    policy: PolicyDecision,
    actor: bool,
    now: &'a Timestamp,
) -> BindingValidityFacts<'a> {
    BindingValidityFacts {
        action,
        approval,
        current_action_version: action.version(),
        current_action_hash: hash,
        now,
        current_policy: policy,
        approval_actor_authorized: actor,
    }
}

#[test]
fn policy_composition_is_maximum_and_order_independent() {
    for a in PolicyDecision::ALL {
        for b in PolicyDecision::ALL {
            for c in PolicyDecision::ALL {
                let expected = compose_policy_decisions(*a, *b, *c);
                assert_eq!(expected, compose_policy_decisions(*c, *a, *b));
                assert_eq!(expected, compose_layers(PolicyLayers::new(*a, *b, *c)));
            }
        }
    }
    assert_eq!(
        compose_policy_decisions(
            PolicyDecision::Allow,
            PolicyDecision::Allow,
            PolicyDecision::Deny
        ),
        PolicyDecision::Deny
    );
}

#[test]
fn assurance_uses_canonical_order() {
    assert!(assurance_satisfies(
        ApprovalAssurance::None,
        ApprovalAssurance::None
    ));
    assert!(!assurance_satisfies(
        ApprovalAssurance::None,
        ApprovalAssurance::Confirm
    ));
    assert!(assurance_satisfies(
        ApprovalAssurance::ElevatedConfirm,
        ApprovalAssurance::Confirm
    ));
    assert!(!assurance_satisfies(
        ApprovalAssurance::Confirm,
        ApprovalAssurance::ElevatedConfirm
    ));
}

#[test]
fn binding_collects_independent_failures_without_precedence() {
    let action = action();
    let expires = timestamp("2026-09-11T11:00:00.000Z");
    let approval = approval(
        &action,
        ApprovalState::Approved,
        PolicyDecision::Confirm,
        ApprovalAssurance::Confirm,
        ApprovalAssurance::Confirm,
        Some(expires),
    );
    let now = timestamp("2026-09-11T12:00:00.000Z");
    let other_hash = Sha256Hex::new(OTHER_HASH).unwrap();
    let report = check_binding_validity(binding(
        &action,
        &approval,
        &other_hash,
        PolicyDecision::Deny,
        false,
        &now,
    ));
    assert!(!report.is_valid());
    assert!(
        report
            .blockers()
            .as_slice()
            .contains(&BindingValidityBlocker::StaleAction)
    );
    assert!(
        report
            .blockers()
            .as_slice()
            .contains(&BindingValidityBlocker::Expired)
    );
    assert!(
        report
            .blockers()
            .as_slice()
            .contains(&BindingValidityBlocker::PolicyDenied)
    );
    assert!(
        report
            .blockers()
            .as_slice()
            .contains(&BindingValidityBlocker::ActorUnauthorized)
    );
}

#[test]
fn historical_approval_is_rechecked_against_current_policy() {
    let action = action();
    let approval = approval(
        &action,
        ApprovalState::Approved,
        PolicyDecision::Confirm,
        ApprovalAssurance::Confirm,
        ApprovalAssurance::Confirm,
        None,
    );
    let now = timestamp("2026-09-11T12:00:00.000Z");
    let hash = Sha256Hex::new(HASH).unwrap();
    let allowed = authorize_execution(CurrentAuthorizationFacts {
        action: &action,
        approval: &approval,
        binding: binding(&action, &approval, &hash, PolicyDecision::Allow, true, &now),
        freshness_ok: true,
        routing_ok: true,
        tool_definition_ok: true,
        execution_actor_authorized: true,
    });
    assert_eq!(allowed, AuthorizationDecision::Allowed);
    let denied = authorize_execution(CurrentAuthorizationFacts {
        action: &action,
        approval: &approval,
        binding: binding(&action, &approval, &hash, PolicyDecision::Deny, true, &now),
        freshness_ok: true,
        routing_ok: true,
        tool_definition_ok: true,
        execution_actor_authorized: true,
    });
    assert!(matches!(denied, AuthorizationDecision::Blocked(_)));
}

#[test]
fn authorization_matrix_requires_current_approval_and_assurance() {
    let action = action();
    let now = timestamp("2026-09-11T12:00:00.000Z");
    let hash = Sha256Hex::new(HASH).unwrap();
    let pending = approval(
        &action,
        ApprovalState::Pending,
        PolicyDecision::Confirm,
        ApprovalAssurance::Confirm,
        ApprovalAssurance::None,
        None,
    );
    let result = authorize_execution(CurrentAuthorizationFacts {
        action: &action,
        approval: &pending,
        binding: binding(
            &action,
            &pending,
            &hash,
            PolicyDecision::Confirm,
            true,
            &now,
        ),
        freshness_ok: true,
        routing_ok: true,
        tool_definition_ok: true,
        execution_actor_authorized: true,
    });
    assert!(matches!(result, AuthorizationDecision::Blocked(_)));
    let not_required = approval(
        &action,
        ApprovalState::NotRequired,
        PolicyDecision::Allow,
        ApprovalAssurance::None,
        ApprovalAssurance::None,
        None,
    );
    let result = authorize_execution(CurrentAuthorizationFacts {
        action: &action,
        approval: &not_required,
        binding: binding(
            &action,
            &not_required,
            &hash,
            PolicyDecision::Allow,
            true,
            &now,
        ),
        freshness_ok: true,
        routing_ok: true,
        tool_definition_ok: true,
        execution_actor_authorized: true,
    });
    assert_eq!(result, AuthorizationDecision::Allowed);
}

#[test]
fn recipient_boundary_is_fail_closed_and_preserves_operation_floor() {
    assert_eq!(
        classify_recipient_boundary(&[]),
        Err(RecipientClassificationError::EmptyInput)
    );
    assert_eq!(
        classify_recipient_boundary(&[RecipientBoundary::Internal, RecipientBoundary::Unknown]),
        Ok(RecipientBoundary::Unknown)
    );
    assert_eq!(
        classify_recipient_boundary(&[RecipientBoundary::Unknown, RecipientBoundary::External]),
        Ok(RecipientBoundary::External)
    );
    assert_eq!(
        risk_for_recipient_boundary(RecipientBoundary::Internal, RiskClass::WriteInternal),
        RiskClass::SendInternal
    );
    assert_eq!(
        risk_for_recipient_boundary(RecipientBoundary::Internal, RiskClass::Privileged),
        RiskClass::Privileged
    );
}
