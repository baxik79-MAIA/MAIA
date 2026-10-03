//! M0.16.2 policy for candidate allocation. Host I/O is behind WorkspacePort.
//! ADMITTED is advisory: the protected host must attest the exact admission
//! and repeat volatile checks before the port may create a workspace.
use crate::tier0::{Decision, Outcome, Plan, Step};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Requested,
    Allocated,
    Active,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalOutcome {
    Rejected,
    Cancelled,
    IsolationUnavailable,
    InfraError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    SupervisorUnavailable,
    WrongProfile,
    NotAdmitted,
    AdmissionMismatch,
    IncompleteEvidence,
    ParentChanged,
    ReservationInvalid,
    ProtectedPath,
    IdentityCollision,
    HostError,
    EvidenceError,
    DiscardError,
    InvalidTransition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub workspace_id: String,
    pub generation_id: String,
    pub hypothesis_id: String,
    pub parent_id: String,
    pub parent_snapshot_id: String,
    pub parent_source_commit: String,
    pub parent_kind: crate::tier0::ParentKind,
    pub operator: String,
    pub proposed_paths: Vec<String>,
    pub proposed_symbols: Vec<String>,
    pub estimated_changed_lines: u32,
    pub max_changed_lines: u32,
    pub max_files: u32,
    pub semantic_fingerprint: String,
    pub implementation_fingerprint: String,
    pub profiling_fingerprint: String,
    pub evidence_refs: Vec<String>,
    pub reservation_id: String,
    pub created_sequence: u64,
}

#[derive(Debug, Clone)]
pub struct Request {
    pub plan: Plan,
    pub admission: Decision,
    pub workspace_id: String,
    pub parent_source_commit: String,
    pub created_sequence: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortError;

pub trait WorkspacePort {
    /// Includes kill switch, Supervisor integrity, host isolation and profile.
    fn supervisor_ready(&mut self) -> Result<bool, PortError>;
    fn development_profile(&mut self) -> Result<bool, PortError>;
    /// Exact plan, decision and evidence must match the protected admission record.
    fn admission_current(&mut self, request: &Request) -> Result<bool, PortError>;
    fn parent_current(&mut self, identity: &Identity) -> Result<bool, PortError>;
    fn reservation_current(&mut self, identity: &Identity) -> Result<bool, PortError>;
    fn paths_allowed(&mut self, identity: &Identity) -> Result<bool, PortError>;
    fn allocate_workspace(&mut self, identity: &Identity) -> Result<(), Failure>;
    /// Persists evidence before candidate-scoped cleanup. No baseline rollback.
    fn preserve_terminal(
        &mut self,
        identity: &Identity,
        outcome: TerminalOutcome,
    ) -> Result<(), PortError>;
    fn discard_workspace(&mut self, identity: &Identity) -> Result<(), PortError>;
    fn record_state(&mut self, identity: &Identity, state: State) -> Result<(), PortError>;
}

fn nonempty(value: &str) -> bool {
    !value.trim().is_empty()
}

fn identity(request: &Request) -> Result<Identity, Failure> {
    let p = &request.plan;
    let d = &request.admission;
    if d.outcome != Outcome::Admitted || d.step != Step::ScratchStaticApply || d.reason.is_some() {
        return Err(Failure::NotAdmitted);
    }
    if d.generation_id != p.generation_id
        || d.hypothesis_id != p.hypothesis_id
        || d.semantic_fingerprint != p.semantic_fingerprint
        || d.implementation_fingerprint != p.implementation_fingerprint
        || d.profiling_fingerprint != p.profiling_fingerprint
        || d.evidence_refs != p.evidence_refs
    {
        return Err(Failure::AdmissionMismatch);
    }
    let reservation_id = d
        .reservation_id
        .as_deref()
        .ok_or(Failure::IncompleteEvidence)?;
    if ![
        &request.workspace_id,
        &request.parent_source_commit,
        &p.generation_id,
        &p.hypothesis_id,
        &p.parent_id,
        &p.parent_snapshot_id,
        &p.operator,
        &p.semantic_fingerprint,
        &p.implementation_fingerprint,
        &p.profiling_fingerprint,
        reservation_id,
    ]
    .iter()
    .all(|value| nonempty(value))
        || request.created_sequence == 0
        || p.evidence_refs.is_empty()
        || p.evidence_refs.iter().any(|value| !nonempty(value))
        || p.proposed_paths.is_empty()
        || p.proposed_paths.iter().any(|value| !nonempty(value))
        || p.max_changed_lines == 0
        || p.max_files == 0
        || p.estimated_changed_lines == 0
        || p.estimated_changed_lines > p.max_changed_lines
        || p.proposed_paths.len() > p.max_files as usize
    {
        return Err(Failure::IncompleteEvidence);
    }
    Ok(Identity {
        workspace_id: request.workspace_id.clone(),
        generation_id: p.generation_id.clone(),
        hypothesis_id: p.hypothesis_id.clone(),
        parent_id: p.parent_id.clone(),
        parent_snapshot_id: p.parent_snapshot_id.clone(),
        parent_source_commit: request.parent_source_commit.clone(),
        parent_kind: p.parent_kind,
        operator: p.operator.clone(),
        proposed_paths: p.proposed_paths.clone(),
        proposed_symbols: p.proposed_symbols.clone(),
        estimated_changed_lines: p.estimated_changed_lines,
        max_changed_lines: p.max_changed_lines,
        max_files: p.max_files,
        semantic_fingerprint: p.semantic_fingerprint.clone(),
        implementation_fingerprint: p.implementation_fingerprint.clone(),
        profiling_fingerprint: p.profiling_fingerprint.clone(),
        evidence_refs: p.evidence_refs.clone(),
        reservation_id: reservation_id.to_owned(),
        created_sequence: request.created_sequence,
    })
}

fn verified(result: Result<bool, PortError>, failure: Failure) -> Result<(), Failure> {
    match result {
        Ok(true) => Ok(()),
        Ok(false) => Err(failure),
        Err(PortError) => Err(Failure::HostError),
    }
}

#[derive(Debug, Clone)]
pub struct Candidate {
    identity: Identity,
    state: State,
    outcome: Option<TerminalOutcome>,
    close_recorded: bool,
}

impl Candidate {
    pub fn identity(&self) -> &Identity {
        &self.identity
    }
    pub fn state(&self) -> State {
        self.state
    }
    pub fn outcome(&self) -> Option<TerminalOutcome> {
        self.outcome
    }
    pub fn activate(&mut self, port: &mut impl WorkspacePort) -> Result<(), Failure> {
        if self.state != State::Allocated {
            return Err(Failure::InvalidTransition);
        }
        verified(port.supervisor_ready(), Failure::SupervisorUnavailable)?;
        verified(port.development_profile(), Failure::WrongProfile)?;
        port.record_state(&self.identity, State::Active)
            .map_err(|_| Failure::EvidenceError)?;
        self.state = State::Active;
        Ok(())
    }
    /// Evidence is preserved before removing only this candidate's workspace.
    pub fn discard(
        &mut self,
        port: &mut impl WorkspacePort,
        outcome: TerminalOutcome,
    ) -> Result<(), Failure> {
        if self.state == State::Closed {
            if self.outcome != Some(outcome) {
                return Err(Failure::InvalidTransition);
            }
            if !self.close_recorded {
                port.record_state(&self.identity, State::Closed)
                    .map_err(|_| Failure::EvidenceError)?;
                self.close_recorded = true;
            }
            return Ok(());
        }
        port.preserve_terminal(&self.identity, outcome)
            .map_err(|_| Failure::EvidenceError)?;
        port.discard_workspace(&self.identity)
            .map_err(|_| Failure::DiscardError)?;
        self.state = State::Closed;
        self.outcome = Some(outcome);
        port.record_state(&self.identity, State::Closed)
            .map_err(|_| Failure::EvidenceError)?;
        self.close_recorded = true;
        Ok(())
    }
}

pub fn allocate(request: &Request, port: &mut impl WorkspacePort) -> Result<Candidate, Failure> {
    verified(port.supervisor_ready(), Failure::SupervisorUnavailable)?;
    verified(port.development_profile(), Failure::WrongProfile)?;
    let identity = identity(request)?;
    verified(port.admission_current(request), Failure::AdmissionMismatch)?;
    verified(port.parent_current(&identity), Failure::ParentChanged)?;
    verified(
        port.reservation_current(&identity),
        Failure::ReservationInvalid,
    )?;
    verified(port.paths_allowed(&identity), Failure::ProtectedPath)?;
    port.record_state(&identity, State::Requested)
        .map_err(|_| Failure::EvidenceError)?;
    port.allocate_workspace(&identity)?;
    if port.record_state(&identity, State::Allocated).is_err() {
        port.preserve_terminal(&identity, TerminalOutcome::InfraError)
            .map_err(|_| Failure::EvidenceError)?;
        return if port.discard_workspace(&identity).is_ok() {
            Err(Failure::EvidenceError)
        } else {
            Err(Failure::DiscardError)
        };
    }
    Ok(Candidate {
        identity,
        state: State::Allocated,
        outcome: None,
        close_recorded: false,
    })
}
