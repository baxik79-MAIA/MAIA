use maia_evolution_supervisor::{Check, RejectReason, ReviewFacts, ReviewVerdict, review};

fn verified() -> ReviewFacts {
    ReviewFacts {
        development_evolution_profile: Check::Verified,
        kill_switch_disengaged: Check::Verified,
        supervisor_integrity: Check::Verified,
        host_isolation: Check::Verified,
        hypothesis_admitted: Check::Verified,
        protected_path_dry_run: Check::Verified,
        blast_radius_within_limit: Check::Verified,
        budget_and_recovery_reserve: Check::Verified,
        parent_recovery_point: Check::Verified,
        scratch_static_apply: Check::Verified,
    }
}

#[test]
fn no_claimed_evidence_can_authorize_mutation() {
    assert_eq!(
        review(verified()),
        ReviewVerdict::Rejected(RejectReason::MutationUnavailable)
    );
}

#[test]
fn unknown_stop_and_integrity_require_halt() {
    let mut facts = verified();
    facts.kill_switch_disengaged = Check::Unknown;
    assert_eq!(
        review(facts),
        ReviewVerdict::HaltRequired(RejectReason::KillSwitch)
    );
    facts.kill_switch_disengaged = Check::Verified;
    facts.supervisor_integrity = Check::Unknown;
    assert_eq!(
        review(facts),
        ReviewVerdict::HaltRequired(RejectReason::SupervisorIntegrity)
    );
    facts.supervisor_integrity = Check::Verified;
    facts.host_isolation = Check::Failed;
    assert_eq!(
        review(facts),
        ReviewVerdict::HaltRequired(RejectReason::HostIsolation)
    );
}

#[test]
fn missing_recovery_and_protected_path_evidence_reject() {
    let mut facts = verified();
    facts.protected_path_dry_run = Check::Unknown;
    assert_eq!(
        review(facts),
        ReviewVerdict::Rejected(RejectReason::ProtectedPath)
    );
    facts.protected_path_dry_run = Check::Verified;
    facts.parent_recovery_point = Check::Failed;
    assert_eq!(
        review(facts),
        ReviewVerdict::Rejected(RejectReason::ParentRecoveryPoint)
    );
}

#[test]
fn wrong_profile_and_budget_reject() {
    let mut facts = verified();
    facts.development_evolution_profile = Check::Failed;
    assert_eq!(
        review(facts),
        ReviewVerdict::Rejected(RejectReason::WrongProfile)
    );
    facts.development_evolution_profile = Check::Verified;
    facts.budget_and_recovery_reserve = Check::Unknown;
    assert_eq!(
        review(facts),
        ReviewVerdict::Rejected(RejectReason::BudgetOrRecoveryReserve)
    );
}
