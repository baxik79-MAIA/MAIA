//! Pure Tier 0 orchestration. Ports belong to a future protected host. This
//! module creates no worktree, snapshot, process, provider call, or mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    SupervisorState,
    HypothesisAdmission,
    LedgerHit,
    ProtectedPath,
    BlastRadius,
    BudgetReservation,
    ParentRecoveryPoint,
    ScratchStaticApply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    KillSwitch,
    SupervisorUnavailable,
    MissingAdmissionField,
    RefutedOrRecentlyRejected,
    ProtectedOrUnclassifiedPath,
    RadiusExceeded,
    BudgetUnavailable,
    ParentUnverified,
    ScratchInvalid,
    AdapterFailure,
    RecordingFailure,
    ReleaseFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Admitted,
    Rejected,
    InfraError,
    HaltRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub outcome: Outcome,
    pub step: Step,
    pub reason: Option<Reason>,
    pub hypothesis_id: String,
    pub generation_id: String,
    pub semantic_fingerprint: String,
    pub implementation_fingerprint: String,
    pub profiling_fingerprint: String,
    pub reservation_id: Option<String>,
    pub evidence_refs: Vec<String>,
}

/// A plan is descriptive input, never a permission or a path to write.
#[derive(Debug, Clone)]
pub struct Plan {
    pub generation_id: String,
    pub hypothesis_id: String,
    pub objective_class: String,
    pub operator: String,
    pub semantic_fingerprint: String,
    pub implementation_fingerprint: String,
    pub profiling_fingerprint: String,
    pub evidence_refs: Vec<String>,
    pub parent_id: String,
    pub parent_snapshot_id: String,
    pub parent_kind: ParentKind,
    pub proposed_paths: Vec<String>,
    pub proposed_symbols: Vec<String>,
    pub estimated_changed_lines: u32,
    pub max_changed_lines: u32,
    pub max_files: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParentKind {
    KnownGoodBaseline,
    SteppingStone,
}

/// The protected host must implement these ports. No implementation is wired
/// in M0.16.1. A worker cannot use this API to acquire mutation authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortError;

pub trait Tier0Ports {
    fn supervisor_ready(&mut self) -> Result<bool, PortError>;
    fn ledger_clear(&mut self, plan: &Plan) -> Result<bool, PortError>;
    fn paths_allowed(&mut self, plan: &Plan) -> Result<bool, PortError>;
    fn reserve_budget(&mut self, plan: &Plan) -> Result<Option<String>, PortError>;
    fn release_budget(&mut self, reservation: &str) -> Result<(), PortError>;
    fn parent_verified(&mut self, plan: &Plan) -> Result<bool, PortError>;
    fn scratch_apply_valid(&mut self, plan: &Plan) -> Result<bool, PortError>;
    fn record(&mut self, decision: &Decision) -> Result<(), PortError>;
}

fn complete(plan: &Plan) -> bool {
    [
        &plan.generation_id,
        &plan.hypothesis_id,
        &plan.objective_class,
        &plan.operator,
        &plan.semantic_fingerprint,
        &plan.implementation_fingerprint,
        &plan.profiling_fingerprint,
        &plan.parent_id,
        &plan.parent_snapshot_id,
    ]
    .iter()
    .all(|s| !s.trim().is_empty())
        && !plan.evidence_refs.is_empty()
        && plan.evidence_refs.iter().all(|s| !s.trim().is_empty())
        && !plan.proposed_paths.is_empty()
        && plan.proposed_paths.iter().all(|s| !s.trim().is_empty())
        && plan.max_changed_lines > 0
        && plan.max_files > 0
        && plan.estimated_changed_lines > 0
}

fn decide(
    ports: &mut impl Tier0Ports,
    plan: &Plan,
    outcome: Outcome,
    step: Step,
    reason: Option<Reason>,
    reservation: Option<&str>,
) -> Decision {
    let mut decision = Decision {
        outcome,
        step,
        reason,
        hypothesis_id: plan.hypothesis_id.clone(),
        generation_id: plan.generation_id.clone(),
        semantic_fingerprint: plan.semantic_fingerprint.clone(),
        implementation_fingerprint: plan.implementation_fingerprint.clone(),
        profiling_fingerprint: plan.profiling_fingerprint.clone(),
        reservation_id: if outcome == Outcome::Admitted {
            reservation.map(str::to_owned)
        } else {
            None
        },
        evidence_refs: plan.evidence_refs.clone(),
    };
    if outcome != Outcome::Admitted {
        if let Some(id) = reservation {
            if ports.release_budget(id).is_err() {
                decision.outcome = Outcome::InfraError;
                decision.reason = Some(Reason::ReleaseFailure);
            }
        }
    }
    if ports.record(&decision).is_err() {
        if outcome == Outcome::Admitted {
            if let Some(id) = reservation {
                let _ = ports.release_budget(id);
            }
        }
        decision.reservation_id = None;
        decision.outcome = Outcome::InfraError;
        decision.reason = Some(Reason::RecordingFailure);
    }
    decision
}

/// Checks run in v5.1 order. An admitted result is only an advisory result:
/// it contains no worker, worktree, snapshot, or promotion capability.
pub fn admit(plan: &Plan, ports: &mut impl Tier0Ports) -> Decision {
    use Outcome::{HaltRequired as Halt, InfraError as Infra, Rejected as Reject};
    use Reason as R;
    use Step as S;
    match ports.supervisor_ready() {
        Ok(true) => {}
        Ok(false) => {
            return decide(
                ports,
                plan,
                Halt,
                S::SupervisorState,
                Some(R::KillSwitch),
                None,
            );
        }
        Err(PortError) => {
            return decide(
                ports,
                plan,
                Halt,
                S::SupervisorState,
                Some(R::SupervisorUnavailable),
                None,
            );
        }
    }
    if !complete(plan) {
        return decide(
            ports,
            plan,
            Reject,
            S::HypothesisAdmission,
            Some(R::MissingAdmissionField),
            None,
        );
    }
    match ports.ledger_clear(plan) {
        Ok(true) => {}
        Ok(false) => {
            return decide(
                ports,
                plan,
                Reject,
                S::LedgerHit,
                Some(R::RefutedOrRecentlyRejected),
                None,
            );
        }
        Err(PortError) => {
            return decide(
                ports,
                plan,
                Infra,
                S::LedgerHit,
                Some(R::AdapterFailure),
                None,
            );
        }
    }
    match ports.paths_allowed(plan) {
        Ok(true) => {}
        Ok(false) => {
            return decide(
                ports,
                plan,
                Reject,
                S::ProtectedPath,
                Some(R::ProtectedOrUnclassifiedPath),
                None,
            );
        }
        Err(PortError) => {
            return decide(
                ports,
                plan,
                Infra,
                S::ProtectedPath,
                Some(R::AdapterFailure),
                None,
            );
        }
    }
    if plan.proposed_paths.len() > plan.max_files as usize
        || plan.estimated_changed_lines > plan.max_changed_lines
    {
        return decide(
            ports,
            plan,
            Reject,
            S::BlastRadius,
            Some(R::RadiusExceeded),
            None,
        );
    }
    let reservation = match ports.reserve_budget(plan) {
        Ok(Some(id)) if !id.trim().is_empty() => id,
        Ok(_) => {
            return decide(
                ports,
                plan,
                Reject,
                S::BudgetReservation,
                Some(R::BudgetUnavailable),
                None,
            );
        }
        Err(PortError) => {
            return decide(
                ports,
                plan,
                Infra,
                S::BudgetReservation,
                Some(R::AdapterFailure),
                None,
            );
        }
    };
    match ports.parent_verified(plan) {
        Ok(true) => {}
        Ok(false) => {
            return decide(
                ports,
                plan,
                Reject,
                S::ParentRecoveryPoint,
                Some(R::ParentUnverified),
                Some(&reservation),
            );
        }
        Err(PortError) => {
            return decide(
                ports,
                plan,
                Infra,
                S::ParentRecoveryPoint,
                Some(R::AdapterFailure),
                Some(&reservation),
            );
        }
    }
    match ports.scratch_apply_valid(plan) {
        Ok(true) => {}
        Ok(false) => {
            return decide(
                ports,
                plan,
                Reject,
                S::ScratchStaticApply,
                Some(R::ScratchInvalid),
                Some(&reservation),
            );
        }
        Err(PortError) => {
            return decide(
                ports,
                plan,
                Infra,
                S::ScratchStaticApply,
                Some(R::AdapterFailure),
                Some(&reservation),
            );
        }
    }
    decide(
        ports,
        plan,
        Outcome::Admitted,
        S::ScratchStaticApply,
        None,
        Some(&reservation),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        calls: Vec<&'static str>,
        stop_at: Option<&'static str>,
        fail_at: Option<&'static str>,
        recorded: Vec<Decision>,
    }
    impl Fake {
        fn new() -> Self {
            Self {
                calls: vec![],
                stop_at: None,
                fail_at: None,
                recorded: vec![],
            }
        }
        fn check(&mut self, name: &'static str) -> Result<bool, PortError> {
            self.calls.push(name);
            if self.fail_at == Some(name) {
                Err(PortError)
            } else {
                Ok(self.stop_at != Some(name))
            }
        }
    }
    impl Tier0Ports for Fake {
        fn supervisor_ready(&mut self) -> Result<bool, PortError> {
            self.check("state")
        }
        fn ledger_clear(&mut self, _: &Plan) -> Result<bool, PortError> {
            self.check("ledger")
        }
        fn paths_allowed(&mut self, _: &Plan) -> Result<bool, PortError> {
            self.check("paths")
        }
        fn reserve_budget(&mut self, _plan: &Plan) -> Result<Option<String>, PortError> {
            self.calls.push("budget");
            if self.fail_at == Some("budget") {
                Err(PortError)
            } else if self.stop_at == Some("budget") {
                Ok(None)
            } else {
                Ok(Some("reserve-1".into()))
            }
        }
        fn release_budget(&mut self, _: &str) -> Result<(), PortError> {
            self.calls.push("release");
            if self.fail_at == Some("release") {
                Err(PortError)
            } else {
                Ok(())
            }
        }
        fn parent_verified(&mut self, _: &Plan) -> Result<bool, PortError> {
            self.check("parent")
        }
        fn scratch_apply_valid(&mut self, _: &Plan) -> Result<bool, PortError> {
            self.check("scratch")
        }
        fn record(&mut self, decision: &Decision) -> Result<(), PortError> {
            self.calls.push("record");
            if self.fail_at == Some("record") {
                Err(PortError)
            } else {
                self.recorded.push(decision.clone());
                Ok(())
            }
        }
    }
    fn plan() -> Plan {
        Plan {
            generation_id: "g".into(),
            hypothesis_id: "h".into(),
            objective_class: "speed".into(),
            operator: "replace".into(),
            semantic_fingerprint: "s".into(),
            implementation_fingerprint: "i".into(),
            profiling_fingerprint: "p".into(),
            evidence_refs: vec!["e".into()],
            parent_id: "parent".into(),
            parent_snapshot_id: "snapshot".into(),
            parent_kind: ParentKind::KnownGoodBaseline,
            proposed_paths: vec!["src/allowed.rs".into()],
            proposed_symbols: vec![],
            estimated_changed_lines: 2,
            max_changed_lines: 3,
            max_files: 1,
        }
    }
    #[test]
    fn ordered_success_is_advisory_and_retains_reservation() {
        let mut fake = Fake::new();
        let decision = admit(&plan(), &mut fake);
        assert_eq!(decision.outcome, Outcome::Admitted);
        assert_eq!(
            fake.calls,
            [
                "state", "ledger", "paths", "budget", "parent", "scratch", "record"
            ]
        );
        assert_eq!(decision.reservation_id.as_deref(), Some("reserve-1"));
    }
    #[test]
    fn each_failure_stops_later_checks_and_records_reason() {
        for (name, outcome) in [
            ("state", Outcome::HaltRequired),
            ("ledger", Outcome::Rejected),
            ("paths", Outcome::Rejected),
            ("budget", Outcome::Rejected),
            ("parent", Outcome::Rejected),
            ("scratch", Outcome::Rejected),
        ] {
            let mut fake = Fake::new();
            fake.stop_at = Some(name);
            let decision = admit(&plan(), &mut fake);
            assert_eq!(decision.outcome, outcome, "{name}");
            assert!(decision.reason.is_some());
            assert_eq!(fake.calls.last(), Some(&"record"));
            assert_eq!(fake.recorded.len(), 1);
            assert_eq!(
                fake.calls.contains(&"release"),
                name == "parent" || name == "scratch"
            );
            assert!(!fake.calls.contains(&"scratch") || name == "scratch");
        }
    }
    #[test]
    fn incomplete_and_radius_overflow_fail_before_budget() {
        let mut p = plan();
        p.operator.clear();
        let mut fake = Fake::new();
        assert_eq!(admit(&p, &mut fake).step, Step::HypothesisAdmission);
        assert_eq!(fake.calls, ["state", "record"]);
        let mut p = plan();
        p.estimated_changed_lines = 4;
        let mut fake = Fake::new();
        assert_eq!(admit(&p, &mut fake).step, Step::BlastRadius);
        assert_eq!(fake.calls, ["state", "ledger", "paths", "record"]);
    }
    #[test]
    fn infrastructure_and_recording_fail_closed() {
        let mut fake = Fake::new();
        fake.fail_at = Some("parent");
        assert_eq!(admit(&plan(), &mut fake).outcome, Outcome::InfraError);
        assert_eq!(fake.calls.last(), Some(&"record"));
        let mut fake = Fake::new();
        fake.fail_at = Some("record");
        assert_eq!(
            admit(&plan(), &mut fake).reason,
            Some(Reason::RecordingFailure)
        );
    }
}
