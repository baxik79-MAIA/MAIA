//! M0.16.3 candidate-only mutation policy and Tier 1 orchestration.
//!
//! This module has no filesystem, process, Git, persistence, or provider
//! authority. Those effects are ports implemented by the development host.
use crate::tier0::{Decision as Tier0Decision, Outcome as Tier0Outcome, Step as Tier0Step};
use crate::workspace::{Candidate, Identity, PortError, State, TerminalOutcome, WorkspacePort};

pub const EVOLVABLE_PATHS: &[&str] = &["apps/local-intelligence-host/src/lib.rs"];
pub const MAX_REPLACEMENT_BYTES: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    FunctionRewrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationRequest {
    pub candidate_workspace_id: String,
    pub generation_id: String,
    pub hypothesis_id: String,
    pub approval_reference: String,
    pub path: String,
    pub expected_sha256: String,
    pub expected_text: String,
    pub replacement_text: String,
    pub operation: Operation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckKind {
    CandidateIdentity,
    SyntaxStatic,
    FormattingLint,
    ComponentBuild,
    TargetedTests,
    ProtectedSurfaceIntegrity,
}

pub const REQUIRED_TIER1_CHECKS: &[CheckKind] = &[
    CheckKind::CandidateIdentity,
    CheckKind::SyntaxStatic,
    CheckKind::FormattingLint,
    CheckKind::ComponentBuild,
    CheckKind::TargetedTests,
    CheckKind::ProtectedSurfaceIntegrity,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckOutcome {
    Passed,
    Failed,
    ResourceLimit,
    Cancelled,
    InfraError,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckEvidence {
    pub check: CheckKind,
    pub outcome: CheckOutcome,
    pub evidence_ref: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Tier1Outcome {
    Passed,
    Failed,
    ResourceLimit,
    Cancelled,
    #[default]
    InfraError,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tier1Evidence {
    pub outcome: Tier1Outcome,
    pub verifier_identity: String,
    pub checks: Vec<CheckEvidence>,
}

impl Tier1Evidence {
    fn is_complete(&self) -> bool {
        !self.verifier_identity.trim().is_empty()
            && self.checks.len() == REQUIRED_TIER1_CHECKS.len()
            && REQUIRED_TIER1_CHECKS.iter().all(|required| {
                self.checks
                    .iter()
                    .filter(|item| item.check == *required)
                    .count()
                    == 1
                    && self
                        .checks
                        .iter()
                        .any(|item| item.check == *required && !item.evidence_ref.trim().is_empty())
            })
    }

    fn checks_pass(&self) -> bool {
        self.checks
            .iter()
            .all(|item| item.outcome == CheckOutcome::Passed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedChange {
    pub path: String,
    pub before_sha256: String,
    pub after_sha256: String,
    pub changed_lines: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptOutcome {
    Requested,
    Rejected,
    Applied,
    Tier1Passed,
    Tier1Failed,
    ResourceLimit,
    Cancelled,
    IsolationUnavailable,
    InfraError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    CandidateNotActive,
    Tier0NotAdmitted,
    CandidateMismatch,
    SupervisorNotReady,
    WrongCapabilityProfile,
    ApprovalMissingOrInvalid,
    PathNotEvolvable,
    OperatorNotSupported,
    InvalidPatch,
    WorkspaceMismatch,
    ProtectedTarget,
    MutationRejected,
    Tier1Failed,
    Tier1ResourceLimit,
    Tier1EvidenceIncomplete,
}

/// Content-free append-only evidence snapshot. Implementations must persist
/// each call before returning success; replacement text is never included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptEvidence {
    pub candidate_id: String,
    pub workspace_id: String,
    pub generation_id: String,
    pub hypothesis_id: String,
    pub tier0_outcome: Tier0Outcome,
    pub tier0_evidence_refs: Vec<String>,
    pub approval_reference: String,
    pub requested_path: String,
    pub operation: Operation,
    pub outcome: AttemptOutcome,
    pub reason: Option<RejectReason>,
    pub changed_file: Option<AppliedChange>,
    pub tier1: Option<Tier1Evidence>,
    pub terminal_state: Option<State>,
    pub terminal_reason: Option<TerminalOutcome>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortFailure {
    Rejected,
    Infrastructure,
    ProtectedTarget,
    Cancelled,
    IsolationUnavailable,
}

pub trait MutationPorts: WorkspacePort {
    fn cancellation_requested(&mut self) -> Result<bool, PortError>;
    fn approval_current(
        &mut self,
        identity: &Identity,
        approval_reference: &str,
    ) -> Result<bool, PortError>;
    fn candidate_workspace_current(&mut self, identity: &Identity) -> Result<bool, PortError>;
    fn record_attempt(&mut self, evidence: &AttemptEvidence) -> Result<(), PortError>;
    /// Prevents reuse after evidence or cleanup infrastructure becomes uncertain.
    fn quarantine_candidate(&mut self, identity: &Identity);
    fn apply_candidate_patch(
        &mut self,
        identity: &Identity,
        request: &MutationRequest,
    ) -> Result<AppliedChange, PortFailure>;
    fn verify_tier1(
        &mut self,
        identity: &Identity,
        changed_path: &str,
    ) -> Result<Tier1Evidence, PortFailure>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultState {
    Tier1Passed,
    Rejected,
    InfraError,
    Cancelled,
    ResourceLimit,
    IsolationUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationResult {
    pub state: ResultState,
    pub reason: Option<RejectReason>,
    pub changed_file: Option<AppliedChange>,
    pub tier1: Option<Tier1Evidence>,
}

fn tier0_matches(decision: &Tier0Decision, identity: &Identity) -> bool {
    decision.outcome == Tier0Outcome::Admitted
        && decision.step == Tier0Step::ScratchStaticApply
        && decision.reason.is_none()
        && decision.generation_id == identity.generation_id
        && decision.hypothesis_id == identity.hypothesis_id
        && decision.semantic_fingerprint == identity.semantic_fingerprint
        && decision.implementation_fingerprint == identity.implementation_fingerprint
        && decision.profiling_fingerprint == identity.profiling_fingerprint
        && decision.evidence_refs == identity.evidence_refs
        && decision.reservation_id.as_deref() == Some(identity.reservation_id.as_str())
}

fn deny(
    candidate: &mut Candidate,
    ports: &mut impl MutationPorts,
    evidence: &mut AttemptEvidence,
    reason: RejectReason,
) -> MutationResult {
    if evidence.outcome == AttemptOutcome::Requested {
        evidence.outcome = AttemptOutcome::Rejected;
    }
    evidence.reason = Some(reason);
    evidence.terminal_state = None;
    evidence.terminal_reason = Some(TerminalOutcome::Rejected);
    if ports.record_attempt(evidence).is_err() {
        ports.quarantine_candidate(candidate.identity());
        return MutationResult {
            state: ResultState::InfraError,
            reason: Some(RejectReason::MutationRejected),
            changed_file: evidence.changed_file.clone(),
            tier1: evidence.tier1.clone(),
        };
    }
    let discarded = candidate.discard(ports, TerminalOutcome::Rejected).is_ok();
    if !discarded {
        ports.quarantine_candidate(candidate.identity());
    }
    if candidate.state() == State::Closed {
        evidence.terminal_state = Some(State::Closed);
        if ports.record_attempt(evidence).is_err() {
            return MutationResult {
                state: ResultState::InfraError,
                reason: Some(RejectReason::MutationRejected),
                changed_file: evidence.changed_file.clone(),
                tier1: evidence.tier1.clone(),
            };
        }
    }
    MutationResult {
        state: if discarded {
            ResultState::Rejected
        } else {
            ResultState::InfraError
        },
        reason: Some(if discarded {
            reason
        } else {
            RejectReason::MutationRejected
        }),
        changed_file: evidence.changed_file.clone(),
        tier1: evidence.tier1.clone(),
    }
}

fn infra(
    candidate: &mut Candidate,
    ports: &mut impl MutationPorts,
    evidence: &mut AttemptEvidence,
) -> MutationResult {
    evidence.outcome = AttemptOutcome::InfraError;
    evidence.terminal_state = None;
    evidence.terminal_reason = Some(TerminalOutcome::InfraError);
    if ports.record_attempt(evidence).is_err() {
        ports.quarantine_candidate(candidate.identity());
        return MutationResult {
            state: ResultState::InfraError,
            reason: None,
            changed_file: evidence.changed_file.clone(),
            tier1: evidence.tier1.clone(),
        };
    }
    let _discarded = candidate
        .discard(ports, TerminalOutcome::InfraError)
        .is_ok();
    if !_discarded {
        ports.quarantine_candidate(candidate.identity());
    }
    if candidate.state() == State::Closed {
        evidence.terminal_state = Some(State::Closed);
        let _ = ports.record_attempt(evidence);
    }
    MutationResult {
        state: ResultState::InfraError,
        reason: None,
        changed_file: evidence.changed_file.clone(),
        tier1: evidence.tier1.clone(),
    }
}

fn cancelled(
    candidate: &mut Candidate,
    ports: &mut impl MutationPorts,
    evidence: &mut AttemptEvidence,
) -> MutationResult {
    evidence.outcome = AttemptOutcome::Cancelled;
    evidence.reason = None;
    evidence.terminal_state = None;
    evidence.terminal_reason = Some(TerminalOutcome::Cancelled);
    if ports.record_attempt(evidence).is_err() {
        ports.quarantine_candidate(candidate.identity());
        return MutationResult {
            state: ResultState::InfraError,
            reason: None,
            changed_file: evidence.changed_file.clone(),
            tier1: evidence.tier1.clone(),
        };
    }
    if candidate
        .discard(ports, TerminalOutcome::Cancelled)
        .is_err()
    {
        ports.quarantine_candidate(candidate.identity());
        return MutationResult {
            state: ResultState::InfraError,
            reason: None,
            changed_file: evidence.changed_file.clone(),
            tier1: evidence.tier1.clone(),
        };
    }
    evidence.terminal_state = Some(State::Closed);
    if ports.record_attempt(evidence).is_err() {
        return MutationResult {
            state: ResultState::InfraError,
            reason: None,
            changed_file: evidence.changed_file.clone(),
            tier1: evidence.tier1.clone(),
        };
    }
    MutationResult {
        state: ResultState::Cancelled,
        reason: None,
        changed_file: evidence.changed_file.clone(),
        tier1: evidence.tier1.clone(),
    }
}

fn isolation_unavailable(
    candidate: &mut Candidate,
    ports: &mut impl MutationPorts,
    evidence: &mut AttemptEvidence,
) -> MutationResult {
    evidence.outcome = AttemptOutcome::IsolationUnavailable;
    evidence.reason = None;
    evidence.terminal_state = None;
    evidence.terminal_reason = Some(TerminalOutcome::IsolationUnavailable);
    if ports.record_attempt(evidence).is_err()
        || candidate
            .discard(ports, TerminalOutcome::IsolationUnavailable)
            .is_err()
    {
        ports.quarantine_candidate(candidate.identity());
        return MutationResult {
            state: ResultState::InfraError,
            reason: None,
            changed_file: evidence.changed_file.clone(),
            tier1: evidence.tier1.clone(),
        };
    }
    evidence.terminal_state = Some(State::Closed);
    if ports.record_attempt(evidence).is_err() {
        return MutationResult {
            state: ResultState::InfraError,
            reason: None,
            changed_file: evidence.changed_file.clone(),
            tier1: evidence.tier1.clone(),
        };
    }
    MutationResult {
        state: ResultState::IsolationUnavailable,
        reason: None,
        changed_file: evidence.changed_file.clone(),
        tier1: evidence.tier1.clone(),
    }
}

fn resource_limit(
    candidate: &mut Candidate,
    ports: &mut impl MutationPorts,
    evidence: &mut AttemptEvidence,
) -> MutationResult {
    evidence.outcome = AttemptOutcome::ResourceLimit;
    evidence.reason = Some(RejectReason::Tier1ResourceLimit);
    evidence.terminal_state = None;
    evidence.terminal_reason = Some(TerminalOutcome::Rejected);
    if ports.record_attempt(evidence).is_err()
        || candidate.discard(ports, TerminalOutcome::Rejected).is_err()
    {
        ports.quarantine_candidate(candidate.identity());
        return MutationResult {
            state: ResultState::InfraError,
            reason: Some(RejectReason::Tier1ResourceLimit),
            changed_file: evidence.changed_file.clone(),
            tier1: evidence.tier1.clone(),
        };
    }
    evidence.terminal_state = Some(State::Closed);
    if ports.record_attempt(evidence).is_err() {
        return MutationResult {
            state: ResultState::InfraError,
            reason: Some(RejectReason::Tier1ResourceLimit),
            changed_file: evidence.changed_file.clone(),
            tier1: evidence.tier1.clone(),
        };
    }
    MutationResult {
        state: ResultState::ResourceLimit,
        reason: Some(RejectReason::Tier1ResourceLimit),
        changed_file: evidence.changed_file.clone(),
        tier1: evidence.tier1.clone(),
    }
}

/// Applies one human-approved, exact-path replacement to an active candidate
/// after revalidating its Tier 0 evidence and host-owned identity. Tier 1 is
/// run only after a successful mutation. A pass leaves the candidate active;
/// it never grants screening, promotion, Git, or release authority.
pub fn mutate_and_verify(
    candidate: &mut Candidate,
    tier0: &Tier0Decision,
    request: &MutationRequest,
    ports: &mut impl MutationPorts,
) -> MutationResult {
    let identity = candidate.identity().clone();
    let mut evidence = AttemptEvidence {
        // The allocated workspace is this milestone's unique candidate identity.
        candidate_id: identity.workspace_id.clone(),
        workspace_id: identity.workspace_id.clone(),
        generation_id: identity.generation_id.clone(),
        hypothesis_id: identity.hypothesis_id.clone(),
        tier0_outcome: tier0.outcome,
        tier0_evidence_refs: tier0.evidence_refs.clone(),
        approval_reference: request.approval_reference.clone(),
        requested_path: request.path.clone(),
        operation: request.operation,
        outcome: AttemptOutcome::Requested,
        reason: None,
        changed_file: None,
        tier1: None,
        terminal_state: None,
        terminal_reason: None,
    };

    if ports.record_attempt(&evidence).is_err() {
        return infra(candidate, ports, &mut evidence);
    }
    match ports.cancellation_requested() {
        Ok(true) => return cancelled(candidate, ports, &mut evidence),
        Ok(false) => {}
        Err(_) => return infra(candidate, ports, &mut evidence),
    }
    if candidate.state() != State::Active {
        return deny(
            candidate,
            ports,
            &mut evidence,
            RejectReason::CandidateNotActive,
        );
    }
    if tier0.outcome != Tier0Outcome::Admitted {
        return deny(
            candidate,
            ports,
            &mut evidence,
            RejectReason::Tier0NotAdmitted,
        );
    }
    if !tier0_matches(tier0, &identity)
        || request.candidate_workspace_id != identity.workspace_id
        || request.generation_id != identity.generation_id
        || request.hypothesis_id != identity.hypothesis_id
    {
        return deny(
            candidate,
            ports,
            &mut evidence,
            RejectReason::CandidateMismatch,
        );
    }
    match ports.supervisor_ready() {
        Ok(true) => {}
        Ok(false) => {
            return deny(
                candidate,
                ports,
                &mut evidence,
                RejectReason::SupervisorNotReady,
            );
        }
        Err(_) => return infra(candidate, ports, &mut evidence),
    }
    match ports.development_profile() {
        Ok(true) => {}
        Ok(false) => {
            return deny(
                candidate,
                ports,
                &mut evidence,
                RejectReason::WrongCapabilityProfile,
            );
        }
        Err(_) => return infra(candidate, ports, &mut evidence),
    }
    if request.approval_reference.trim().is_empty() {
        return deny(
            candidate,
            ports,
            &mut evidence,
            RejectReason::ApprovalMissingOrInvalid,
        );
    }
    match ports.approval_current(&identity, &request.approval_reference) {
        Ok(true) => {}
        Ok(false) => {
            return deny(
                candidate,
                ports,
                &mut evidence,
                RejectReason::ApprovalMissingOrInvalid,
            );
        }
        Err(_) => return infra(candidate, ports, &mut evidence),
    }
    if request.operation != Operation::FunctionRewrite || identity.operator != "FUNCTION_REWRITE" {
        return deny(
            candidate,
            ports,
            &mut evidence,
            RejectReason::OperatorNotSupported,
        );
    }
    if !EVOLVABLE_PATHS.contains(&request.path.as_str())
        || !identity
            .proposed_paths
            .iter()
            .any(|path| path == &request.path)
    {
        return deny(
            candidate,
            ports,
            &mut evidence,
            RejectReason::PathNotEvolvable,
        );
    }
    if request.expected_sha256.len() != 64
        || !request
            .expected_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || request.expected_text.is_empty()
        || request.replacement_text.len() > MAX_REPLACEMENT_BYTES
        || request.expected_text == request.replacement_text
    {
        return deny(candidate, ports, &mut evidence, RejectReason::InvalidPatch);
    }
    match ports.candidate_workspace_current(&identity) {
        Ok(true) => {}
        Ok(false) => {
            return deny(
                candidate,
                ports,
                &mut evidence,
                RejectReason::WorkspaceMismatch,
            );
        }
        Err(_) => return infra(candidate, ports, &mut evidence),
    }
    let changed = match ports.apply_candidate_patch(&identity, request) {
        Ok(change) => change,
        Err(PortFailure::Rejected) => {
            return deny(
                candidate,
                ports,
                &mut evidence,
                RejectReason::MutationRejected,
            );
        }
        Err(PortFailure::ProtectedTarget) => {
            return deny(
                candidate,
                ports,
                &mut evidence,
                RejectReason::ProtectedTarget,
            );
        }
        Err(PortFailure::Infrastructure) => return infra(candidate, ports, &mut evidence),
        Err(PortFailure::Cancelled) => return cancelled(candidate, ports, &mut evidence),
        Err(PortFailure::IsolationUnavailable) => {
            return isolation_unavailable(candidate, ports, &mut evidence);
        }
    };
    if changed.path != request.path
        || changed.changed_lines == 0
        || changed.changed_lines > identity.max_changed_lines
        || identity.max_files < 1
    {
        evidence.changed_file = Some(changed);
        return deny(
            candidate,
            ports,
            &mut evidence,
            RejectReason::MutationRejected,
        );
    }
    evidence.changed_file = Some(changed.clone());
    evidence.outcome = AttemptOutcome::Applied;
    if ports.record_attempt(&evidence).is_err() {
        return infra(candidate, ports, &mut evidence);
    }
    evidence.tier1 = match ports.verify_tier1(&identity, &changed.path) {
        Ok(report) => Some(report),
        Err(PortFailure::Infrastructure) => return infra(candidate, ports, &mut evidence),
        Err(PortFailure::Cancelled) => return cancelled(candidate, ports, &mut evidence),
        Err(PortFailure::IsolationUnavailable) => {
            return isolation_unavailable(candidate, ports, &mut evidence);
        }
        Err(PortFailure::Rejected) => {
            evidence.outcome = AttemptOutcome::Tier1Failed;
            return deny(candidate, ports, &mut evidence, RejectReason::Tier1Failed);
        }
        Err(PortFailure::ProtectedTarget) => {
            return deny(
                candidate,
                ports,
                &mut evidence,
                RejectReason::ProtectedTarget,
            );
        }
    };
    let report = evidence.tier1.as_ref().expect("set above");
    match report.outcome {
        Tier1Outcome::Cancelled => return cancelled(candidate, ports, &mut evidence),
        Tier1Outcome::ResourceLimit => return resource_limit(candidate, ports, &mut evidence),
        Tier1Outcome::InfraError => return infra(candidate, ports, &mut evidence),
        Tier1Outcome::Passed | Tier1Outcome::Failed => {}
    }
    if !report.is_complete() {
        evidence.outcome = AttemptOutcome::InfraError;
        return infra(candidate, ports, &mut evidence);
    }
    match report.outcome {
        Tier1Outcome::Passed if report.checks_pass() => {
            evidence.outcome = AttemptOutcome::Tier1Passed;
            evidence.terminal_state = None;
            evidence.terminal_reason = None;
            if ports.record_attempt(&evidence).is_err() {
                return infra(candidate, ports, &mut evidence);
            }
            MutationResult {
                state: ResultState::Tier1Passed,
                reason: None,
                changed_file: Some(changed),
                tier1: evidence.tier1,
            }
        }
        Tier1Outcome::Failed => {
            evidence.outcome = AttemptOutcome::Tier1Failed;
            deny(candidate, ports, &mut evidence, RejectReason::Tier1Failed)
        }
        Tier1Outcome::ResourceLimit => resource_limit(candidate, ports, &mut evidence),
        Tier1Outcome::InfraError => infra(candidate, ports, &mut evidence),
        Tier1Outcome::Cancelled => cancelled(candidate, ports, &mut evidence),
        Tier1Outcome::Passed => {
            evidence.outcome = AttemptOutcome::InfraError;
            infra(candidate, ports, &mut evidence)
        }
    }
}
