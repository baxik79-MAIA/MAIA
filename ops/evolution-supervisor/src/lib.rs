//! M0.16.0 Evolution Supervisor safety contract.
//!
//! This development-operations crate is not the host-level Supervisor. It
//! performs a pure review of caller-supplied facts and never grants mutation,
//! snapshot, worktree, promotion, rollback, or release authority. In
//! particular, a diagnostic hypothesis or qualification is only advisory.
//! A future host must independently verify facts under a separate OS trust
//! boundary before protected host adapters or a worker lifecycle are wired.
#![forbid(unsafe_code)]

/// A fact supplied for contract review. Unknown is never treated as success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    Verified,
    Failed,
    Unknown,
}

/// Inputs are claims for review, not attestations. Only a future protected
/// host can verify OS identity, ACLs, snapshots, and the kill switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewFacts {
    pub development_evolution_profile: Check,
    pub kill_switch_disengaged: Check,
    pub supervisor_integrity: Check,
    pub host_isolation: Check,
    pub hypothesis_admitted: Check,
    pub protected_path_dry_run: Check,
    pub blast_radius_within_limit: Check,
    pub budget_and_recovery_reserve: Check,
    pub parent_recovery_point: Check,
    pub scratch_static_apply: Check,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    WrongProfile,
    KillSwitch,
    SupervisorIntegrity,
    HostIsolation,
    HypothesisAdmission,
    ProtectedPath,
    BlastRadius,
    BudgetOrRecoveryReserve,
    ParentRecoveryPoint,
    ScratchStaticApply,
    /// M0.16.0 has no positive EVOLVABLE allowlist or mutation capability.
    MutationUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewVerdict {
    /// A halt is a control requirement for a future host, not a host action.
    HaltRequired(RejectReason),
    Rejected(RejectReason),
}

/// Pure, ordered review. It cannot return an authorization token or invoke a
/// worker. Passing every claimed check still rejects mutation in M0.16.0.
pub fn review(facts: ReviewFacts) -> ReviewVerdict {
    use Check::Verified;
    use RejectReason as R;
    use ReviewVerdict as V;

    if facts.kill_switch_disengaged != Verified {
        return V::HaltRequired(R::KillSwitch);
    }
    if facts.supervisor_integrity != Verified {
        return V::HaltRequired(R::SupervisorIntegrity);
    }
    if facts.development_evolution_profile != Verified {
        return V::Rejected(R::WrongProfile);
    }
    if facts.host_isolation != Verified {
        return V::HaltRequired(R::HostIsolation);
    }
    for (check, reason) in [
        (facts.hypothesis_admitted, R::HypothesisAdmission),
        (facts.protected_path_dry_run, R::ProtectedPath),
        (facts.blast_radius_within_limit, R::BlastRadius),
        (
            facts.budget_and_recovery_reserve,
            R::BudgetOrRecoveryReserve,
        ),
        (facts.parent_recovery_point, R::ParentRecoveryPoint),
        (facts.scratch_static_apply, R::ScratchStaticApply),
    ] {
        if check != Verified {
            return V::Rejected(reason);
        }
    }
    V::Rejected(R::MutationUnavailable)
}

pub mod tier0;
