//! Protected host-owned runtime facts used by the development Gate.
//!
//! Candidate-facing APIs receive neither the controller nor its state handle.
//! Every change advances a version, so an operation must pass a fresh check.
use crate::{Gate, OperationSnapshot};
use maia_evolution_supervisor::tier0::{Decision, Plan};
use maia_evolution_supervisor::workspace::{Identity, PortError, Request};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProfile {
    DevelopmentEvolution,
    DeploymentLocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier1IsolationCapability {
    SameUserProcessTreeContained,
    RestrictedIdentityNetworkAndFilesystem,
}

#[derive(Debug, Clone)]
struct Admission {
    plan: Plan,
    decision: Decision,
    request_workspace_id: String,
    parent_commit: String,
    created_sequence: u64,
    approval: String,
    expires_at: Instant,
}

#[derive(Debug)]
struct State {
    profile: RuntimeProfile,
    enabled: bool,
    kill_switch: bool,
    supervisor_identity: String,
    expected_supervisor_identity: String,
    supervisor_integrity: bool,
    tier1_isolation: Tier1IsolationCapability,
    version: u64,
    admission: Option<Admission>,
}

#[derive(Clone)]
pub struct ProtectedRuntimeController(Arc<Mutex<State>>);

pub struct ProtectedRuntimeGate {
    state: Arc<Mutex<State>>,
}

impl ProtectedRuntimeGate {
    pub fn new(supervisor_identity: impl Into<String>) -> (ProtectedRuntimeController, Self) {
        let supervisor_identity = supervisor_identity.into();
        let state = Arc::new(Mutex::new(State {
            profile: RuntimeProfile::DeploymentLocked,
            enabled: false,
            kill_switch: true,
            supervisor_identity: String::new(),
            expected_supervisor_identity: supervisor_identity,
            supervisor_integrity: false,
            tier1_isolation: Tier1IsolationCapability::SameUserProcessTreeContained,
            version: 1,
            admission: None,
        }));
        (
            ProtectedRuntimeController(Arc::clone(&state)),
            Self { state },
        )
    }

    fn state(&self) -> Result<std::sync::MutexGuard<'_, State>, PortError> {
        self.state.lock().map_err(|_| PortError)
    }

    fn matches(&self, identity: &Identity, approval: Option<&str>) -> Result<bool, PortError> {
        let state = self.state()?;
        let Some(admission) = &state.admission else {
            return Ok(false);
        };
        Ok(state.profile == RuntimeProfile::DevelopmentEvolution
            && state.enabled
            && !state.kill_switch
            && state.supervisor_integrity
            && state.version > 0
            && !state.supervisor_identity.trim().is_empty()
            && state.supervisor_identity == state.expected_supervisor_identity
            && admission.expires_at > Instant::now()
            && admission.decision.outcome == maia_evolution_supervisor::tier0::Outcome::Admitted
            && admission.decision.step
                == maia_evolution_supervisor::tier0::Step::ScratchStaticApply
            && admission.decision.reason.is_none()
            && admission.request_workspace_id == identity.workspace_id
            && admission.parent_commit == identity.parent_source_commit
            && admission.created_sequence == identity.created_sequence
            && admission.plan.generation_id == identity.generation_id
            && admission.plan.hypothesis_id == identity.hypothesis_id
            && admission.plan.parent_id == identity.parent_id
            && admission.plan.parent_snapshot_id == identity.parent_snapshot_id
            && admission.plan.parent_kind == identity.parent_kind
            && admission.plan.proposed_paths == identity.proposed_paths
            && admission.plan.proposed_symbols == identity.proposed_symbols
            && admission.plan.evidence_refs == identity.evidence_refs
            && admission.plan.estimated_changed_lines == identity.estimated_changed_lines
            && admission.plan.semantic_fingerprint == identity.semantic_fingerprint
            && admission.plan.implementation_fingerprint == identity.implementation_fingerprint
            && admission.plan.profiling_fingerprint == identity.profiling_fingerprint
            && admission.plan.max_files == identity.max_files
            && admission.plan.max_changed_lines == identity.max_changed_lines
            && admission.plan.operator == identity.operator
            && admission.decision.reservation_id.as_deref()
                == Some(identity.reservation_id.as_str())
            && admission.decision.evidence_refs == identity.evidence_refs
            && approval.is_none_or(|reference| reference == admission.approval))
    }
}

impl ProtectedRuntimeController {
    fn update(&self, update: impl FnOnce(&mut State)) -> Result<(), PortError> {
        let mut state = self.0.lock().map_err(|_| PortError)?;
        update(&mut state);
        state.version = state.version.checked_add(1).ok_or(PortError)?;
        Ok(())
    }

    pub fn set_profile(&self, profile: RuntimeProfile) -> Result<(), PortError> {
        self.update(|state| state.profile = profile)
    }

    pub fn set_evolution_enabled(&self, enabled: bool) -> Result<(), PortError> {
        self.update(|state| state.enabled = enabled)
    }

    pub fn set_kill_switch(&self, enabled: bool) -> Result<(), PortError> {
        self.update(|state| state.kill_switch = enabled)
    }

    pub fn set_supervisor_integrity(&self, valid: bool) -> Result<(), PortError> {
        self.update(|state| state.supervisor_integrity = valid)
    }

    pub fn set_observed_supervisor_identity(
        &self,
        observed_identity: impl Into<String>,
    ) -> Result<(), PortError> {
        let observed_identity = observed_identity.into();
        self.update(|state| state.supervisor_identity = observed_identity)
    }

    /// Protected host code calls this when the approval source revokes or
    /// expires the human authorization; every outstanding snapshot is invalidated.
    pub fn revoke_admission(&self) -> Result<(), PortError> {
        self.update(|state| state.admission = None)
    }

    /// The protected host records approval and exact admitted facts. Candidate
    /// request fields alone cannot create this record.
    pub fn bind_admission(
        &self,
        request: &Request,
        approval_reference: &str,
        validity: Duration,
    ) -> Result<(), PortError> {
        if approval_reference.trim().is_empty() || validity.is_zero() {
            return Err(PortError);
        }
        self.update(|state| {
            state.admission = Some(Admission {
                plan: request.plan.clone(),
                decision: request.admission.clone(),
                request_workspace_id: request.workspace_id.clone(),
                parent_commit: request.parent_source_commit.clone(),
                created_sequence: request.created_sequence,
                approval: approval_reference.to_owned(),
                expires_at: Instant::now() + validity,
            });
        })
    }
}

impl Gate for ProtectedRuntimeGate {
    fn supervisor_ready(&mut self) -> Result<bool, PortError> {
        let state = self.state()?;
        Ok(state.supervisor_integrity
            && !state.supervisor_identity.trim().is_empty()
            && state.supervisor_identity == state.expected_supervisor_identity
            && !state.kill_switch)
    }

    fn development_profile(&mut self) -> Result<bool, PortError> {
        let state = self.state()?;
        Ok(state.profile == RuntimeProfile::DevelopmentEvolution && state.enabled)
    }

    fn admission_current(&mut self, request: &Request) -> Result<bool, PortError> {
        let state = self.state()?;
        Ok(state.profile == RuntimeProfile::DevelopmentEvolution
            && state.enabled
            && !state.kill_switch
            && state.supervisor_integrity
            && !state.supervisor_identity.trim().is_empty()
            && state.supervisor_identity == state.expected_supervisor_identity
            && state.admission.as_ref().is_some_and(|admission| {
                admission.expires_at > Instant::now()
                    && admission.plan == request.plan
                    && admission.decision == request.admission
                    && admission.request_workspace_id == request.workspace_id
                    && admission.parent_commit == request.parent_source_commit
                    && admission.created_sequence == request.created_sequence
                    && admission.decision.outcome
                        == maia_evolution_supervisor::tier0::Outcome::Admitted
                    && admission.decision.step
                        == maia_evolution_supervisor::tier0::Step::ScratchStaticApply
                    && admission.decision.reason.is_none()
                    && request.admission.outcome
                        == maia_evolution_supervisor::tier0::Outcome::Admitted
            }))
    }

    fn parent_current(&mut self, identity: &Identity) -> Result<bool, PortError> {
        self.matches(identity, None)
    }

    fn reservation_current(&mut self, identity: &Identity) -> Result<bool, PortError> {
        self.matches(identity, None)
    }

    fn paths_allowed(&mut self, identity: &Identity) -> Result<bool, PortError> {
        self.matches(identity, None)
    }

    fn approval_current(
        &mut self,
        identity: &Identity,
        approval_reference: &str,
    ) -> Result<bool, PortError> {
        self.matches(identity, Some(approval_reference))
    }

    fn operation_current(
        &mut self,
        identity: &Identity,
        approval_reference: &str,
    ) -> Result<bool, PortError> {
        self.matches(identity, Some(approval_reference))
    }

    fn operation_snapshot(
        &mut self,
        identity: &Identity,
        approval_reference: &str,
    ) -> Result<Option<OperationSnapshot>, PortError> {
        let version = self.state()?.version;
        let allowed = self.matches(identity, Some(approval_reference))?;
        Ok(allowed.then(|| OperationSnapshot::for_identity(identity, approval_reference, version)))
    }

    fn operation_snapshot_current(
        &mut self,
        identity: &Identity,
        approval_reference: &str,
        snapshot: &OperationSnapshot,
    ) -> Result<bool, PortError> {
        if !snapshot.matches(identity, approval_reference) {
            return Ok(false);
        }
        let version = self.state()?.version;
        Ok(
            version == snapshot.state_version
                && self.matches(identity, Some(approval_reference))?,
        )
    }

    fn cancellation_requested(&mut self) -> Result<bool, PortError> {
        let state = self.state()?;
        Ok(state.kill_switch
            || !state.enabled
            || state.profile != RuntimeProfile::DevelopmentEvolution
            || !state.supervisor_integrity
            || state.supervisor_identity.trim().is_empty()
            || state.supervisor_identity != state.expected_supervisor_identity)
    }

    fn tier1_isolation_ready(&mut self) -> Result<bool, PortError> {
        let state = self.state()?;
        Ok(state.tier1_isolation
            == Tier1IsolationCapability::RestrictedIdentityNetworkAndFilesystem)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maia_evolution_supervisor::tier0::{Decision, Outcome, ParentKind, Plan, Step};

    fn development_runtime() -> (ProtectedRuntimeController, ProtectedRuntimeGate) {
        let (controller, gate) = ProtectedRuntimeGate::new("supervisor-v1");
        controller
            .set_profile(RuntimeProfile::DevelopmentEvolution)
            .unwrap();
        controller.set_evolution_enabled(true).unwrap();
        controller.set_kill_switch(false).unwrap();
        controller.set_supervisor_integrity(true).unwrap();
        controller
            .set_observed_supervisor_identity("supervisor-v1")
            .unwrap();
        (controller, gate)
    }

    fn request() -> Request {
        let plan = Plan {
            generation_id: "g1".into(),
            hypothesis_id: "h1".into(),
            objective_class: "test".into(),
            operator: "FUNCTION_REWRITE".into(),
            semantic_fingerprint: "s".into(),
            implementation_fingerprint: "i".into(),
            profiling_fingerprint: "p".into(),
            evidence_refs: vec!["e1".into()],
            parent_id: "parent".into(),
            parent_snapshot_id: "snapshot".into(),
            parent_kind: ParentKind::KnownGoodBaseline,
            proposed_paths: vec!["apps/local-intelligence-host/src/lib.rs".into()],
            proposed_symbols: vec![],
            estimated_changed_lines: 1,
            max_changed_lines: 1,
            max_files: 1,
        };
        Request {
            admission: Decision {
                outcome: Outcome::Admitted,
                step: Step::ScratchStaticApply,
                reason: None,
                hypothesis_id: plan.hypothesis_id.clone(),
                generation_id: plan.generation_id.clone(),
                semantic_fingerprint: plan.semantic_fingerprint.clone(),
                implementation_fingerprint: plan.implementation_fingerprint.clone(),
                profiling_fingerprint: plan.profiling_fingerprint.clone(),
                reservation_id: Some("r1".into()),
                evidence_refs: plan.evidence_refs.clone(),
            },
            plan,
            workspace_id: "w1".into(),
            parent_source_commit: "0123456789012345678901234567890123456789".into(),
            created_sequence: 1,
        }
    }

    fn identity(request: &Request) -> Identity {
        let plan = &request.plan;
        Identity {
            workspace_id: request.workspace_id.clone(),
            generation_id: plan.generation_id.clone(),
            hypothesis_id: plan.hypothesis_id.clone(),
            parent_id: plan.parent_id.clone(),
            parent_snapshot_id: plan.parent_snapshot_id.clone(),
            parent_source_commit: request.parent_source_commit.clone(),
            parent_kind: plan.parent_kind,
            operator: plan.operator.clone(),
            proposed_paths: plan.proposed_paths.clone(),
            proposed_symbols: plan.proposed_symbols.clone(),
            estimated_changed_lines: plan.estimated_changed_lines,
            max_changed_lines: plan.max_changed_lines,
            max_files: plan.max_files,
            semantic_fingerprint: plan.semantic_fingerprint.clone(),
            implementation_fingerprint: plan.implementation_fingerprint.clone(),
            profiling_fingerprint: plan.profiling_fingerprint.clone(),
            evidence_refs: plan.evidence_refs.clone(),
            reservation_id: request.admission.reservation_id.clone().unwrap(),
            created_sequence: request.created_sequence,
        }
    }

    #[test]
    fn kill_switch_profile_and_expiry_revoke_bound_admission() {
        let request = request();
        let (controller, mut gate) = development_runtime();
        controller
            .bind_admission(&request, "approval-1", Duration::from_secs(30))
            .unwrap();
        assert!(gate.admission_current(&request).unwrap());
        controller.set_kill_switch(true).unwrap();
        assert!(!gate.supervisor_ready().unwrap());
        assert!(!gate.admission_current(&request).unwrap());
        controller.set_kill_switch(false).unwrap();
        controller
            .set_profile(RuntimeProfile::DeploymentLocked)
            .unwrap();
        assert!(!gate.development_profile().unwrap());
        assert!(!gate.admission_current(&request).unwrap());
    }

    #[test]
    fn unknown_startup_runtime_state_is_closed_until_host_attests_it() {
        let (_, mut gate) = ProtectedRuntimeGate::new("supervisor-v1");
        assert!(!gate.supervisor_ready().unwrap());
        assert!(!gate.development_profile().unwrap());
        assert!(gate.cancellation_requested().unwrap());
        assert!(!gate.tier1_isolation_ready().unwrap());
    }

    #[test]
    fn candidate_facts_cannot_fabricate_protected_admission() {
        let request = request();
        let (_, mut gate) = development_runtime();
        assert!(!gate.admission_current(&request).unwrap());
    }

    #[test]
    fn production_capability_defaults_closed_and_identity_drift_revokes_gate() {
        let request = request();
        let (controller, mut gate) = development_runtime();
        assert!(!gate.tier1_isolation_ready().unwrap());
        controller
            .bind_admission(&request, "approval-1", Duration::from_secs(30))
            .unwrap();
        assert!(gate.admission_current(&request).unwrap());
        controller
            .set_observed_supervisor_identity("candidate-controlled-value")
            .unwrap();
        assert!(!gate.admission_current(&request).unwrap());
    }

    #[test]
    fn protected_gate_rejects_non_admitted_decision_even_if_bound() {
        let mut request = request();
        request.admission.outcome = maia_evolution_supervisor::tier0::Outcome::Rejected;
        let (controller, mut gate) = development_runtime();
        controller
            .bind_admission(&request, "approval-1", Duration::from_secs(30))
            .unwrap();
        assert!(!gate.admission_current(&request).unwrap());
        assert!(
            gate.operation_snapshot(&identity(&request), "approval-1")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn approval_revocation_invalidates_the_bound_decision() {
        let request = request();
        let (controller, mut gate) = development_runtime();
        controller
            .bind_admission(&request, "approval-1", Duration::from_secs(30))
            .unwrap();
        let identity = identity(&request);
        let snapshot = gate
            .operation_snapshot(&identity, "approval-1")
            .unwrap()
            .unwrap();
        controller.revoke_admission().unwrap();
        assert!(!gate.approval_current(&identity, "approval-1").unwrap());
        assert!(
            !gate
                .operation_snapshot_current(&identity, "approval-1", &snapshot)
                .unwrap()
        );
    }

    #[test]
    fn immutable_operation_snapshot_is_invalidated_by_state_change() {
        let request = request();
        let (controller, mut gate) = development_runtime();
        controller
            .bind_admission(&request, "approval-1", Duration::from_secs(30))
            .unwrap();
        let identity = identity(&request);
        let snapshot = gate
            .operation_snapshot(&identity, "approval-1")
            .unwrap()
            .unwrap();
        assert!(
            gate.operation_snapshot_current(&identity, "approval-1", &snapshot)
                .unwrap()
        );
        controller
            .set_profile(RuntimeProfile::DeploymentLocked)
            .unwrap();
        assert!(
            !gate
                .operation_snapshot_current(&identity, "approval-1", &snapshot)
                .unwrap()
        );
        assert!(gate.cancellation_requested().unwrap());
        controller.set_evolution_enabled(false).unwrap();
        assert!(gate.cancellation_requested().unwrap());
    }
}
