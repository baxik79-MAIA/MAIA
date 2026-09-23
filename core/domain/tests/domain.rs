use maia_domain::*;
use std::any::TypeId;

const UUID: &str = "01900000-0000-7000-8000-000000000000";
fn timestamp() -> Timestamp {
    "2026-09-11T12:34:56.789Z".parse().unwrap()
}
fn source() -> SourcePrecondition {
    SourcePrecondition::new(
        OpaqueRef::new("external").unwrap(),
        OpaqueRef::new("token").unwrap(),
        Sha256Hex::new("a".repeat(64)).unwrap(),
    )
    .unwrap()
}
fn canonicalizer() -> CanonicalizerRef {
    CanonicalizerRef::new(
        NonEmptyString::new("test.input").unwrap(),
        Version::new(1).unwrap(),
    )
    .unwrap()
}
fn action(
    connector: Option<ConnectorProfileId>,
    sources: Vec<SourcePrecondition>,
) -> Result<Action, DomainError> {
    let selection = if connector.is_some() {
        ConnectorSelection::Fixed
    } else {
        ConnectorSelection::None
    };
    let connector_hash = connector
        .as_ref()
        .map(|_| Sha256Hex::new("b".repeat(64)).unwrap());
    Action::new(
        UUID.parse().unwrap(),
        UUID.parse().unwrap(),
        Ordinal::new(0).unwrap(),
        ActionType::new("mail.send").unwrap(),
        connector,
        RiskClass::SendExternal,
        ActionState::Planned,
        OpaqueRef::new("input").unwrap(),
        sources,
        None,
        Version::new(1).unwrap(),
        Version::new(1).unwrap(),
        Sha256Hex::new("c".repeat(64)).unwrap(),
        canonicalizer(),
        selection,
        connector_hash,
        None,
    )
}
fn run(state: RunState, certainty: OutcomeCertainty) -> Result<Run, DomainError> {
    Run::new(
        UUID.parse().unwrap(),
        Version::new(1).unwrap(),
        UUID.parse().unwrap(),
        Attempt::new(1).unwrap(),
        None,
        None,
        None,
        None,
        state,
        certainty,
        None,
        None,
        None,
        Version::new(1).unwrap(),
        Sha256Hex::new("a".repeat(64)).unwrap(),
        UUID.parse().unwrap(),
        Version::new(1).unwrap(),
    )
}

macro_rules! fsm_test {
    ($test:ident, $ty:ident) => {
        #[test]
        fn $test() {
            for &(from, to) in $ty::TRANSITIONS {
                assert!(from.is_valid_transition(to));
                assert_eq!(from.validate_transition(to), Ok(()));
            }
            for &from in $ty::ALL {
                for &to in $ty::ALL {
                    assert_eq!(
                        from.validate_transition(to).is_ok(),
                        $ty::TRANSITIONS.contains(&(from, to))
                    );
                }
                assert!(!from.is_valid_transition(from));
            }
        }
    };
}
fsm_test!(task_edges, AgentTaskState);
fsm_test!(action_edges, ActionState);
fsm_test!(run_edges, RunState);
fsm_test!(approval_edges, ApprovalState);

#[test]
fn historical_approvals_are_preserved() {
    for state in [
        ApprovalState::NotRequired,
        ApprovalState::Rejected,
        ApprovalState::Expired,
        ApprovalState::Revoked,
    ] {
        assert!(state.validate_transition(ApprovalState::Revoked).is_err());
    }
    assert!(ApprovalState::Pending.is_valid_transition(ApprovalState::Revoked));
    assert!(ApprovalState::Approved.is_valid_transition(ApprovalState::Revoked));
}
#[test]
fn illegal_shortcuts_are_rejected() {
    assert!(
        AgentTaskState::Running
            .validate_transition(AgentTaskState::Paused)
            .is_err()
    );
    assert!(
        ActionState::Planned
            .validate_transition(ActionState::Running)
            .is_err()
    );
    assert!(!ActionState::ALL.iter().any(|s| s.as_str() == "paused"));
    assert!(
        RunState::OutcomeUnknown
            .validate_transition(RunState::Starting)
            .is_err()
    );
    assert!(RunState::OutcomeUnknown.is_valid_transition(RunState::Reconciling));
}
#[test]
fn certainty_and_atomic_transition() {
    for &state in RunState::ALL {
        for &certainty in OutcomeCertainty::ALL {
            assert_eq!(
                run(state, certainty).is_ok(),
                certainty == state.outcome_certainty()
            );
        }
    }
    let mut value = run(RunState::Running, OutcomeCertainty::NotApplicable).unwrap();
    let before = value.clone();
    assert!(value.transition_to(RunState::Starting).is_err());
    assert_eq!(value, before);
    value.transition_to(RunState::OutcomeUnknown).unwrap();
    assert_eq!(*value.outcome_certainty(), OutcomeCertainty::Unknown);
    value.transition_to(RunState::Reconciling).unwrap();
    assert_eq!(*value.outcome_certainty(), OutcomeCertainty::Unknown);
    value.transition_to(RunState::Completed).unwrap();
    assert_eq!(*value.outcome_certainty(), OutcomeCertainty::Known);
}
#[test]
fn uuid_v7_canonical_forms() {
    for variant in ['8', '9', 'a', 'b'] {
        let mut value = UUID.to_owned();
        value.replace_range(19..20, &variant.to_string());
        assert_eq!(WorkspaceId::new(value.clone()).unwrap().to_string(), value);
    }
    for value in [
        "",
        "01900000000070008000000000000000",
        "01900000-0000-4000-8000-000000000000",
        "01900000-0000-7000-c000-000000000000",
        "01900000-0000-7000-8000-00000000000A",
        "01900000_0000-7000-8000-000000000000",
        "01900000-0000-7000-8000-00000000000é",
    ] {
        assert_eq!(WorkspaceId::new(value), Err(DomainError::InvalidUuidV7));
    }
}
#[test]
fn strong_ids_have_distinct_types() {
    let ids = [
        TypeId::of::<WorkspaceId>(),
        TypeId::of::<AgentTaskId>(),
        TypeId::of::<ExecutionPlanId>(),
        TypeId::of::<ActionId>(),
        TypeId::of::<RunId>(),
        TypeId::of::<ApprovalId>(),
        TypeId::of::<ConnectorProfileId>(),
        TypeId::of::<ModelProfileId>(),
    ];
    for (i, id) in ids.iter().enumerate() {
        assert!(!ids[i + 1..].contains(id));
    }
}
#[test]
fn action_type_vectors() {
    for value in ["mail.send", "a.b", "m365.read_v2", "a.b.c", "a_.b0"] {
        assert_eq!(ActionType::new(value).unwrap().as_str(), value);
    }
    for value in [
        "",
        "mail",
        ".mail",
        "mail.",
        "mail..send",
        "Mail.send",
        "mail.Send",
        "1mail.send",
        "mail.1send",
        "mail.send-now",
        " mail.send",
        "mail.send\n",
        "máil.send",
    ] {
        assert!(ActionType::new(value).is_err(), "{value}");
    }
}
#[test]
fn timestamp_calendar_vectors() {
    for value in [
        "2000-02-29T23:59:59.999Z",
        "2024-02-29T00:00:00.000Z",
        "1900-02-28T12:00:00.001Z",
        "0000-01-01T00:00:00.000Z",
        "9999-12-31T23:59:59.999Z",
    ] {
        assert_eq!(Timestamp::new(value).unwrap().to_string(), value);
    }
    for value in [
        "1900-02-29T00:00:00.000Z",
        "2023-02-29T00:00:00.000Z",
        "2024-04-31T00:00:00.000Z",
        "2024-00-01T00:00:00.000Z",
        "2024-13-01T00:00:00.000Z",
        "2024-01-00T00:00:00.000Z",
        "2024-01-01T24:00:00.000Z",
        "2024-01-01T23:60:00.000Z",
        "2024-01-01T23:59:61.000Z",
        "2024-01-01T00:00:00Z",
        "2024-01-01T00:00:00.00Z",
        "2024-01-01T00:00:00.0000Z",
        "2024-01-01T00:00:00.000+00:00",
        "2024-01-01t00:00:00.000Z",
        "2024-01-01T00:00:00.000z",
        "é024-01-01T00:00:00.000Z",
    ] {
        assert!(Timestamp::new(value).is_err(), "{value}");
    }
}
#[test]
fn hash_vectors() {
    assert!(Sha256Hex::new("0123456789abcdef".repeat(4)).is_ok());
    for value in [
        "a".repeat(63),
        "a".repeat(65),
        "A".repeat(64),
        "g".repeat(64),
        "é".repeat(32),
    ] {
        assert!(Sha256Hex::new(value).is_err());
    }
}
#[test]
fn integer_boundaries() {
    assert!(Version::new(0).is_err());
    assert_eq!(Version::new(1).unwrap().get(), 1);
    assert_eq!(Version::new(u64::MAX).unwrap().get(), u64::MAX);
    assert_eq!(Ordinal::new(0).unwrap().get(), 0);
    assert_eq!(Ordinal::new(u32::MAX).unwrap().get(), u32::MAX);
    assert!(Attempt::new(0).is_err());
    assert_eq!(Attempt::new(1).unwrap().get(), 1);
    assert_eq!(Attempt::new(u32::MAX).unwrap().get(), u32::MAX);
    assert_eq!(AmountMicros::new(u64::MAX).unwrap().get(), u64::MAX);
}
#[test]
fn cost_zero_and_currency_shape() {
    let cost = CostEstimate::new(
        AmountMicros::new(0).unwrap(),
        CurrencyCode::new("PLN").unwrap(),
    )
    .unwrap();
    assert_eq!(cost.amount_micros().get(), 0);
    assert!(CurrencyCode::new("XYZ").is_ok());
    for value in ["", "US", "USDD", "usd", "UsD", "12A", "ÜSD"] {
        assert!(CurrencyCode::new(value).is_err());
    }
}
#[test]
fn opaque_strings_are_preserved() {
    let value = "  opaque:Żółć\n ";
    assert_eq!(PolicyId::new(value).unwrap().as_str(), value);
    assert_eq!(ActorRef::new(value).unwrap().as_str(), value);
    assert_eq!(OpaqueRef::new(value).unwrap().as_str(), value);
    assert_eq!(SurfaceId::new(value).unwrap().as_str(), value);
    assert!(NonEmptyString::new(" ").is_ok());
    assert!(PolicyId::new("").is_err());
    assert!(ActorRef::new("").is_err());
    assert!(OpaqueRef::new("").is_err());
    assert!(SurfaceId::new("").is_err());
    assert!(NonEmptyString::new("").is_err());
}
#[test]
fn risk_summary_orders_and_deduplicates() {
    let values = RiskClass::ALL
        .iter()
        .rev()
        .copied()
        .chain(RiskClass::ALL.iter().copied());
    assert_eq!(RiskSummary::new(values).as_slice(), RiskClass::ALL);
    assert!(RiskSummary::default().as_slice().is_empty());
}
#[test]
fn action_connector_constraint_and_transition() {
    assert!(action(None, vec![]).is_ok());
    assert_eq!(
        action(None, vec![source()]),
        Err(DomainError::MissingConnectorForSourcePreconditions)
    );
    let mut value = action(Some(UUID.parse().unwrap()), vec![source()]).unwrap();
    let before = value.clone();
    assert!(value.transition_to(ActionState::Completed).is_err());
    assert_eq!(before, value);
    value.transition_to(ActionState::Gated).unwrap();
    assert_eq!(*value.state(), ActionState::Gated);
}
#[test]
fn optional_execution_fields() {
    let task = AgentTask::new(
        UUID.parse().unwrap(),
        Version::new(1).unwrap(),
        UUID.parse().unwrap(),
        NonEmptyString::new("request").unwrap(),
        SurfaceId::new("custom").unwrap(),
        None,
        AgentTaskState::Draft,
        PrivacyClass::LocalOnly,
        ActorRef::new("user").unwrap(),
        timestamp(),
    )
    .unwrap();
    assert!(task.origin_ref().is_none());
    let plan = ExecutionPlan::new(
        UUID.parse().unwrap(),
        UUID.parse().unwrap(),
        Version::new(1).unwrap(),
        RiskSummary::default(),
        None,
        GateType::Allow,
        timestamp(),
    )
    .unwrap();
    assert!(plan.estimated_cost().is_none());
    let zero = CostEstimate::new(
        AmountMicros::new(0).unwrap(),
        CurrencyCode::new("USD").unwrap(),
    )
    .unwrap();
    let known = ExecutionPlan::new(
        plan.id().clone(),
        plan.task_id().clone(),
        *plan.version(),
        plan.risk_summary().clone(),
        Some(zero),
        *plan.approval_requirement(),
        plan.created_at().clone(),
    )
    .unwrap();
    assert_ne!(plan, known);
    let value = action(None, vec![]).unwrap();
    assert!(value.connector_profile_id().is_none() && value.result_ref().is_none());
    for mask in 0..16 {
        let value = Run::new(
            UUID.parse().unwrap(),
            Version::new(1).unwrap(),
            UUID.parse().unwrap(),
            Attempt::new(1).unwrap(),
            (mask & 1 != 0).then(|| UUID.parse().unwrap()),
            (mask & 2 != 0).then(|| UUID.parse().unwrap()),
            (mask & 4 != 0).then(|| UUID.parse().unwrap()),
            (mask & 8 != 0).then(|| UUID.parse().unwrap()),
            RunState::Created,
            OutcomeCertainty::NotApplicable,
            None,
            None,
            None,
            Version::new(1).unwrap(),
            Sha256Hex::new("a".repeat(64)).unwrap(),
            UUID.parse().unwrap(),
            Version::new(1).unwrap(),
        )
        .unwrap();
        assert_eq!(value.requested_connector_id().is_some(), mask & 1 != 0);
        assert_eq!(value.actual_connector_id().is_some(), mask & 2 != 0);
        assert_eq!(value.requested_model_id().is_some(), mask & 4 != 0);
        assert_eq!(value.actual_model_id().is_some(), mask & 8 != 0);
        assert!(
            value.reconciliation_ref().is_none()
                && value.started_at().is_none()
                && value.ended_at().is_none()
        );
    }
    let approval = Approval::new(
        UUID.parse().unwrap(),
        UUID.parse().unwrap(),
        UUID.parse().unwrap(),
        PolicyId::new("policy").unwrap(),
        ApprovalState::Pending,
        timestamp(),
        None,
        None,
        None,
        Sha256Hex::new("0".repeat(64)).unwrap(),
        Version::new(1).unwrap(),
        None,
        SurfaceId::new("desktop").unwrap(),
        None,
        Version::new(1).unwrap(),
        Sha256Hex::new("d".repeat(64)).unwrap(),
        PolicyDecision::Confirm,
        ApprovalAssurance::Confirm,
        ApprovalAssurance::None,
    )
    .unwrap();
    assert!(approval.decision_note().is_none());
    assert!(
        approval.decided_at().is_none()
            && approval.decided_by().is_none()
            && approval.expires_at().is_none()
            && approval.decided_surface().is_none()
    );
}

fn approval_record(
    state: ApprovalState,
    version: u64,
    human: bool,
) -> Result<Approval, DomainError> {
    Approval::new(
        UUID.parse().unwrap(),
        UUID.parse().unwrap(),
        UUID.parse().unwrap(),
        PolicyId::new("policy").unwrap(),
        state,
        timestamp(),
        human.then(timestamp),
        human.then(|| ActorRef::new("actor").unwrap()),
        None,
        Sha256Hex::new("a".repeat(64)).unwrap(),
        Version::new(version).unwrap(),
        None,
        SurfaceId::new("desktop").unwrap(),
        human.then(|| SurfaceId::new("teams").unwrap()),
        Version::new(3).unwrap(),
        Sha256Hex::new("b".repeat(64)).unwrap(),
        if state == ApprovalState::NotRequired {
            PolicyDecision::Allow
        } else {
            PolicyDecision::Confirm
        },
        if state == ApprovalState::NotRequired {
            ApprovalAssurance::None
        } else {
            ApprovalAssurance::Confirm
        },
        if human {
            ApprovalAssurance::Confirm
        } else {
            ApprovalAssurance::None
        },
    )
}

#[test]
fn revision_and_record_overflow_preserve_state() {
    assert_eq!(
        Version::new(u64::MAX).unwrap().checked_next(),
        Err(DomainError::VersionOverflow)
    );
    assert_eq!(
        Attempt::new(u32::MAX).unwrap().checked_next(),
        Err(DomainError::AttemptOverflow)
    );
    assert_eq!(
        Version::new(9_007_199_254_740_992)
            .unwrap()
            .checked_next()
            .unwrap()
            .get(),
        9_007_199_254_740_993
    );
    let mut approval = approval_record(ApprovalState::Pending, u64::MAX, false).unwrap();
    let before = approval.clone();
    assert_eq!(
        approval.transition_to(ApprovalState::Expired),
        Err(DomainError::VersionOverflow)
    );
    assert_eq!(approval, before);
}

#[test]
fn approval_audit_metadata_and_revocation_versions() {
    assert!(approval_record(ApprovalState::NotRequired, 1, false).is_ok());
    assert_eq!(
        approval_record(ApprovalState::NotRequired, 1, true),
        Err(DomainError::InvalidApprovalMetadata)
    );
    assert!(approval_record(ApprovalState::Approved, 1, false).is_err());
    let mut approved = approval_record(ApprovalState::Approved, 5, true).unwrap();
    let before = approved.clone();
    assert!(approved.transition_to(ApprovalState::Expired).is_err());
    assert_eq!(approved, before);
    approved.transition_to(ApprovalState::Revoked).unwrap();
    assert_eq!(approved.version().get(), 6);
    assert_eq!(approved.action_version().get(), 3);
    assert_eq!(approved.action_hash(), before.action_hash());
    assert_eq!(
        approved.policy_snapshot_hash(),
        before.policy_snapshot_hash()
    );
    assert_eq!(approved.decided_by(), before.decided_by());
    let mut pending = approval_record(ApprovalState::Pending, 1, false).unwrap();
    pending.transition_to(ApprovalState::Expired).unwrap();
    assert_eq!(pending.version().get(), 2);
}

#[test]
fn connector_selection_and_duplicate_sources() {
    assert_eq!(
        action(Some(UUID.parse().unwrap()), vec![source(), source()]),
        Err(DomainError::DuplicateSourcePrecondition)
    );
    for selection in ConnectorSelection::ALL {
        for &risk in RiskClass::ALL {
            for has_id in [false, true] {
                for has_hash in [false, true] {
                    let value = Action::new(
                        UUID.parse().unwrap(),
                        UUID.parse().unwrap(),
                        Ordinal::new(0).unwrap(),
                        ActionType::new("test.operation").unwrap(),
                        has_id.then(|| UUID.parse().unwrap()),
                        risk,
                        ActionState::Planned,
                        OpaqueRef::new("input").unwrap(),
                        vec![],
                        None,
                        Version::new(1).unwrap(),
                        Version::new(1).unwrap(),
                        Sha256Hex::new("a".repeat(64)).unwrap(),
                        canonicalizer(),
                        *selection,
                        has_hash.then(|| Sha256Hex::new("b".repeat(64)).unwrap()),
                        None,
                    );
                    let expected = match selection {
                        ConnectorSelection::None => !has_id && !has_hash,
                        ConnectorSelection::Fixed => has_id && has_hash,
                        ConnectorSelection::PolicyRouted => matches!(
                            risk,
                            RiskClass::Read | RiskClass::Analyze | RiskClass::Draft
                        ),
                    };
                    assert_eq!(value.is_ok(), expected);
                }
            }
        }
    }
}

#[test]
fn terminal_retryable_run_and_exact_execution_binding() {
    let mut value = run(RunState::RetryableError, OutcomeCertainty::Known).unwrap();
    for &next in RunState::ALL {
        assert!(value.transition_to(next).is_err());
    }
    assert_eq!(value.action_version().get(), 1);
    assert_eq!(value.approval_version().get(), 1);
    assert_eq!(value.action_hash().as_str(), "a".repeat(64));
    assert_eq!(value.approval_id().as_str(), UUID);
    for &state in RunState::ALL {
        assert_eq!(
            state.is_in_flight(),
            matches!(
                state,
                RunState::Starting
                    | RunState::Running
                    | RunState::OutcomeUnknown
                    | RunState::Reconciling
            )
        );
    }
}

#[test]
fn matched_freshness_requires_matching_observations() {
    for token in [None, Some("token"), Some("changed")] {
        for hash in [None, Some("a"), Some("b")] {
            let value = FreshnessEvidence::new(
                UUID.parse().unwrap(),
                UUID.parse().unwrap(),
                Version::new(1).unwrap(),
                Sha256Hex::new("c".repeat(64)).unwrap(),
                FreshnessEnforcement::CompareBeforeExecute,
                source(),
                token.map(|t| OpaqueRef::new(t).unwrap()),
                hash.map(|h| Sha256Hex::new(h.repeat(64)).unwrap()),
                FreshnessResult::Matched,
            );
            assert_eq!(value.is_ok(), token == Some("token") && hash == Some("a"));
        }
    }
}

#[test]
fn canonical_restriction_and_assurance_order() {
    assert!(PolicyDecision::Allow < PolicyDecision::Confirm);
    assert!(PolicyDecision::Confirm < PolicyDecision::ElevatedConfirm);
    assert!(PolicyDecision::ElevatedConfirm < PolicyDecision::Deny);
    assert!(ApprovalAssurance::None < ApprovalAssurance::Confirm);
    assert!(ApprovalAssurance::Confirm < ApprovalAssurance::ElevatedConfirm);
}

#[test]
fn approval_snapshot_assurance_and_deny_are_structural_constraints() {
    for &decision in PolicyDecision::ALL {
        for &required in ApprovalAssurance::ALL {
            for &achieved in ApprovalAssurance::ALL {
                let result = Approval::new(
                    UUID.parse().unwrap(),
                    UUID.parse().unwrap(),
                    UUID.parse().unwrap(),
                    PolicyId::new("policy").unwrap(),
                    ApprovalState::Approved,
                    timestamp(),
                    Some(timestamp()),
                    Some(ActorRef::new("actor").unwrap()),
                    None,
                    Sha256Hex::new("a".repeat(64)).unwrap(),
                    Version::new(1).unwrap(),
                    None,
                    SurfaceId::new("desktop").unwrap(),
                    Some(SurfaceId::new("teams").unwrap()),
                    Version::new(1).unwrap(),
                    Sha256Hex::new("b".repeat(64)).unwrap(),
                    decision,
                    required,
                    achieved,
                );
                let expected_required = match decision {
                    PolicyDecision::Allow => Some(ApprovalAssurance::None),
                    PolicyDecision::Confirm => Some(ApprovalAssurance::Confirm),
                    PolicyDecision::ElevatedConfirm => Some(ApprovalAssurance::ElevatedConfirm),
                    PolicyDecision::Deny => None,
                };
                assert_eq!(
                    result.is_ok(),
                    matches!(
                        decision,
                        PolicyDecision::Confirm | PolicyDecision::ElevatedConfirm
                    ) && expected_required == Some(required)
                        && achieved >= required
                        && achieved >= ApprovalAssurance::Confirm
                );
            }
        }
    }
}

#[test]
fn task_and_run_record_cas_versions_increment_and_fail_closed_on_overflow() {
    let mut task = AgentTask::new(
        UUID.parse().unwrap(),
        Version::new(1).unwrap(),
        UUID.parse().unwrap(),
        NonEmptyString::new("request").unwrap(),
        SurfaceId::new("desktop").unwrap(),
        None,
        AgentTaskState::Draft,
        PrivacyClass::LocalOnly,
        ActorRef::new("actor").unwrap(),
        timestamp(),
    )
    .unwrap();
    task.transition_to(AgentTaskState::Preflight).unwrap();
    assert_eq!(task.version().get(), 2);

    let mut max_task = AgentTask::new(
        UUID.parse().unwrap(),
        Version::new(u64::MAX).unwrap(),
        UUID.parse().unwrap(),
        NonEmptyString::new("request").unwrap(),
        SurfaceId::new("desktop").unwrap(),
        None,
        AgentTaskState::Draft,
        PrivacyClass::LocalOnly,
        ActorRef::new("actor").unwrap(),
        timestamp(),
    )
    .unwrap();
    let before_task = max_task.clone();
    assert_eq!(
        max_task.transition_to(AgentTaskState::Preflight),
        Err(DomainError::VersionOverflow)
    );
    assert_eq!(max_task, before_task);

    let mut run = run(RunState::Created, OutcomeCertainty::NotApplicable).unwrap();
    run.transition_to(RunState::Starting).unwrap();
    assert_eq!(run.version().get(), 2);
    let mut max_run = Run::new(
        UUID.parse().unwrap(),
        Version::new(u64::MAX).unwrap(),
        UUID.parse().unwrap(),
        Attempt::new(1).unwrap(),
        None,
        None,
        None,
        None,
        RunState::Created,
        OutcomeCertainty::NotApplicable,
        None,
        None,
        None,
        Version::new(1).unwrap(),
        Sha256Hex::new("a".repeat(64)).unwrap(),
        UUID.parse().unwrap(),
        Version::new(1).unwrap(),
    )
    .unwrap();
    let before_run = max_run.clone();
    assert_eq!(
        max_run.transition_to(RunState::Starting),
        Err(DomainError::VersionOverflow)
    );
    assert_eq!(max_run, before_run);
}

#[test]
fn audit_chain_genesis_link_is_structurally_fail_closed() {
    let zero = Sha256Hex::new("0".repeat(64)).unwrap();
    let hash = Sha256Hex::new("a".repeat(64)).unwrap();
    let base = || {
        AuditRecord::new(
            UUID.parse().unwrap(),
            UUID.parse().unwrap(),
            AuditSequence::new(1).unwrap(),
            timestamp(),
            None,
            AuditEventType::new("run.created").unwrap(),
            AuditSubjectType::new("run").unwrap(),
            None,
            None,
            None,
            None,
            zero.clone(),
            hash.clone(),
        )
    };
    assert!(base().is_ok());
    assert!(
        AuditRecord::new(
            UUID.parse().unwrap(),
            UUID.parse().unwrap(),
            AuditSequence::new(1).unwrap(),
            timestamp(),
            None,
            AuditEventType::new("run.created").unwrap(),
            AuditSubjectType::new("run").unwrap(),
            None,
            None,
            None,
            None,
            hash.clone(),
            hash.clone(),
        )
        .is_err()
    );
    assert!(
        AuditRecord::new(
            UUID.parse().unwrap(),
            UUID.parse().unwrap(),
            AuditSequence::new(2).unwrap(),
            timestamp(),
            None,
            AuditEventType::new("run.completed").unwrap(),
            AuditSubjectType::new("run").unwrap(),
            None,
            None,
            None,
            None,
            zero,
            hash,
        )
        .is_err()
    );
}

fn hardening_approval(
    state: ApprovalState,
    decision: PolicyDecision,
    required: ApprovalAssurance,
    achieved: ApprovalAssurance,
    metadata: u8,
    note: Option<String>,
) -> Result<Approval, DomainError> {
    Approval::new(
        UUID.parse().unwrap(),
        UUID.parse().unwrap(),
        UUID.parse().unwrap(),
        PolicyId::new("policy").unwrap(),
        state,
        timestamp(),
        (metadata & 2 != 0).then(timestamp),
        (metadata & 1 != 0).then(|| ActorRef::new("actor").unwrap()),
        note,
        Sha256Hex::new("a".repeat(64)).unwrap(),
        Version::new(1).unwrap(),
        None,
        SurfaceId::new("desktop").unwrap(),
        (metadata & 4 != 0).then(|| SurfaceId::new("teams").unwrap()),
        Version::new(3).unwrap(),
        Sha256Hex::new("b".repeat(64)).unwrap(),
        decision,
        required,
        achieved,
    )
}

#[test]
fn f2_f3_policy_snapshot_state_assurance_and_metadata_matrix() {
    for &decision in PolicyDecision::ALL {
        for state in [
            ApprovalState::NotRequired,
            ApprovalState::Pending,
            ApprovalState::Approved,
            ApprovalState::Rejected,
        ] {
            for &required in ApprovalAssurance::ALL {
                for &achieved in ApprovalAssurance::ALL {
                    for metadata in 0..8 {
                        let expected = match (decision, state) {
                            (PolicyDecision::Allow, ApprovalState::NotRequired) => {
                                required == ApprovalAssurance::None
                                    && achieved == ApprovalAssurance::None
                                    && metadata == 0
                            }
                            (PolicyDecision::Confirm | PolicyDecision::ElevatedConfirm, _) => {
                                let correct_required = if decision == PolicyDecision::Confirm {
                                    ApprovalAssurance::Confirm
                                } else {
                                    ApprovalAssurance::ElevatedConfirm
                                };
                                required == correct_required
                                    && match state {
                                        ApprovalState::Pending => {
                                            achieved == ApprovalAssurance::None && metadata == 0
                                        }
                                        ApprovalState::Approved => {
                                            achieved >= correct_required && metadata == 7
                                        }
                                        ApprovalState::Rejected => {
                                            achieved >= ApprovalAssurance::Confirm && metadata == 7
                                        }
                                        _ => false,
                                    }
                            }
                            _ => false,
                        };
                        let result =
                            hardening_approval(state, decision, required, achieved, metadata, None);
                        assert_eq!(
                            result.is_ok(),
                            expected,
                            "{decision:?} {state:?} {required:?} {achieved:?} metadata={metadata}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn f3_pending_has_no_decision_and_cannot_transition_without_evidence() {
    assert!(
        hardening_approval(
            ApprovalState::Pending,
            PolicyDecision::Confirm,
            ApprovalAssurance::Confirm,
            ApprovalAssurance::None,
            0,
            Some(String::new())
        )
        .is_err()
    );
    let mut pending = approval_record(ApprovalState::Pending, 7, false).unwrap();
    let before = pending.clone();
    for next in [ApprovalState::Approved, ApprovalState::Rejected] {
        assert_eq!(
            pending.transition_to(next),
            Err(DomainError::InvalidApprovalMetadata)
        );
        assert_eq!(pending, before);
    }
    pending.transition_to(ApprovalState::Expired).unwrap();
    assert_eq!(pending.version().get(), 8);
    assert!(pending.decided_by().is_none());
}

#[test]
fn f3_confirmed_rejection_of_elevated_action_preserves_snapshot() {
    let rejected = hardening_approval(
        ApprovalState::Rejected,
        PolicyDecision::ElevatedConfirm,
        ApprovalAssurance::ElevatedConfirm,
        ApprovalAssurance::Confirm,
        7,
        Some(String::new()),
    )
    .unwrap();
    assert_eq!(rejected.policy_decision(), &PolicyDecision::ElevatedConfirm);
    assert_eq!(
        rejected.required_assurance(),
        &ApprovalAssurance::ElevatedConfirm
    );
    assert_eq!(rejected.achieved_assurance(), &ApprovalAssurance::Confirm);
    assert_eq!(rejected.decision_note().as_deref(), Some(""));
    let mut historical = rejected.clone();
    assert!(historical.transition_to(ApprovalState::Revoked).is_err());
    assert_eq!(historical, rejected);
    // Current allow/deny does not rewrite a historical human policy snapshot.
    let approved = hardening_approval(
        ApprovalState::Approved,
        PolicyDecision::Confirm,
        ApprovalAssurance::Confirm,
        ApprovalAssurance::Confirm,
        7,
        None,
    )
    .unwrap();
    assert_eq!(approved.policy_decision(), &PolicyDecision::Confirm);
}
