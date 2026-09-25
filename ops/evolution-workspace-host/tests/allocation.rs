use maia_evolution_supervisor::mutation::{
    AttemptEvidence, CheckEvidence, CheckKind, CheckOutcome, MutationRequest, Operation,
    PortFailure, REQUIRED_TIER1_CHECKS, ResultState, Tier1Evidence, Tier1Outcome,
    mutate_and_verify,
};
use maia_evolution_supervisor::tier0::{Decision, Outcome, ParentKind, Plan, Step};
use maia_evolution_supervisor::workspace::{
    Failure, Identity, PortError, Request, State, TerminalOutcome, allocate,
};
use maia_evolution_workspace_host::{
    Evidence, Gate, GitWorkspaceHost, evidence::FileEvidenceJournal, tier1::Tier1Verifier,
};
use sha2::Digest;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Default)]
struct Checks {
    ready: bool,
    profile: bool,
    admission: bool,
    parent: bool,
    reservation: bool,
    paths: bool,
    approval: bool,
    admitted_plan: Option<Plan>,
}
impl Checks {
    fn all() -> Self {
        Self {
            ready: true,
            profile: true,
            admission: true,
            parent: true,
            reservation: true,
            paths: true,
            approval: true,
            admitted_plan: None,
        }
    }
}
impl Gate for Checks {
    fn supervisor_ready(&mut self) -> Result<bool, PortError> {
        Ok(self.ready)
    }
    fn development_profile(&mut self) -> Result<bool, PortError> {
        Ok(self.profile)
    }
    fn admission_current(&mut self, request: &Request) -> Result<bool, PortError> {
        Ok(self.admission
            && self
                .admitted_plan
                .as_ref()
                .is_none_or(|plan| plan == &request.plan))
    }
    fn parent_current(&mut self, _: &Identity) -> Result<bool, PortError> {
        Ok(self.parent)
    }
    fn reservation_current(&mut self, _: &Identity) -> Result<bool, PortError> {
        Ok(self.reservation)
    }
    fn paths_allowed(&mut self, _: &Identity) -> Result<bool, PortError> {
        Ok(self.paths)
    }
    fn approval_current(&mut self, _: &Identity, reference: &str) -> Result<bool, PortError> {
        Ok(self.approval && reference == "human-approval-1")
    }
}
#[derive(Default)]
struct Journal {
    states: Vec<(String, State)>,
    outcomes: Vec<(String, TerminalOutcome)>,
    attempts: Vec<AttemptEvidence>,
}
impl Evidence for Journal {
    fn record_state(&mut self, identity: &Identity, state: State) -> Result<(), PortError> {
        self.states.push((identity.workspace_id.clone(), state));
        Ok(())
    }
    fn preserve_terminal(
        &mut self,
        identity: &Identity,
        outcome: TerminalOutcome,
    ) -> Result<(), PortError> {
        self.outcomes.push((identity.workspace_id.clone(), outcome));
        Ok(())
    }
    fn record_mutation_attempt(&mut self, evidence: &AttemptEvidence) -> Result<(), PortError> {
        self.attempts.push(evidence.clone());
        Ok(())
    }
}

fn tier1_report(outcome: Tier1Outcome) -> Tier1Evidence {
    Tier1Evidence {
        outcome,
        verifier_identity: "synthetic-fixed-verifier-v1".into(),
        checks: REQUIRED_TIER1_CHECKS
            .iter()
            .copied()
            .filter(|check| *check != CheckKind::ProtectedSurfaceIntegrity)
            .map(|check| CheckEvidence {
                check,
                outcome: match outcome {
                    Tier1Outcome::Passed => CheckOutcome::Passed,
                    Tier1Outcome::Failed => CheckOutcome::Failed,
                    Tier1Outcome::InfraError => CheckOutcome::InfraError,
                },
                evidence_ref: format!("tier1:{check:?}"),
            })
            .collect(),
    }
}

struct ScriptedTier1(Tier1Outcome);
impl Tier1Verifier for ScriptedTier1 {
    fn verify(
        &mut self,
        _: &Identity,
        candidate_workspace: &Path,
        _: &str,
    ) -> Result<Tier1Evidence, PortFailure> {
        let changed =
            fs::read_to_string(candidate_workspace.join("apps/local-intelligence-host/src/lib.rs"))
                .map_err(|_| PortFailure::Infrastructure)?;
        if !changed
            .replace("\r\n", "\n")
            .contains("marker() -> u8 {\n    2\n}")
        {
            return Ok(tier1_report(Tier1Outcome::Failed));
        }
        Ok(tier1_report(self.0))
    }
}

struct ProtectedSurfaceMutator;
impl Tier1Verifier for ProtectedSurfaceMutator {
    fn verify(
        &mut self,
        _: &Identity,
        candidate_workspace: &Path,
        _: &str,
    ) -> Result<Tier1Evidence, PortFailure> {
        fs::write(
            candidate_workspace.join("baseline.txt"),
            "unexpected change",
        )
        .map_err(|_| PortFailure::Infrastructure)?;
        Ok(tier1_report(Tier1Outcome::Passed))
    }
}

struct IncompleteTier1;
impl Tier1Verifier for IncompleteTier1 {
    fn verify(&mut self, _: &Identity, _: &Path, _: &str) -> Result<Tier1Evidence, PortFailure> {
        Ok(Tier1Evidence {
            outcome: Tier1Outcome::Passed,
            verifier_identity: String::new(),
            checks: Vec::new(),
        })
    }
}
fn run(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn mutation_request(fixture: &Fixture, id: &str) -> MutationRequest {
    let path = fixture
        .root
        .join(id)
        .join("apps/local-intelligence-host/src/lib.rs");
    let digest = format!("{:x}", sha2::Sha256::digest(fs::read(path).unwrap()));
    MutationRequest {
        candidate_workspace_id: id.into(),
        generation_id: format!("generation-{id}"),
        hypothesis_id: format!("hypothesis-{id}"),
        approval_reference: "human-approval-1".into(),
        path: "apps/local-intelligence-host/src/lib.rs".into(),
        expected_sha256: digest,
        expected_text: "pub fn marker() -> u8 {\n    1\n}".into(),
        replacement_text: "pub fn marker() -> u8 {\n    2\n}".into(),
        operation: Operation::FunctionRewrite,
    }
}

fn read_candidate_text(path: &Path) -> String {
    fs::read_to_string(path).unwrap().replace("\r\n", "\n")
}

struct Fixture {
    base: PathBuf,
    repo: PathBuf,
    root: PathBuf,
    commit: String,
}
impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("maia-m0163-{}-{nonce}", std::process::id()));
        let repo = base.join("baseline");
        let root = base.join("evolution-candidates");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&root).unwrap();
        run(&repo, &["init"]);
        run(&repo, &["config", "core.autocrlf", "true"]);
        run(&repo, &["config", "user.name", "Synthetic Test"]);
        run(
            &repo,
            &["config", "user.email", "synthetic@example.invalid"],
        );
        fs::write(repo.join("baseline.txt"), "known good").unwrap();
        let evolvable = repo.join("apps/local-intelligence-host/src");
        fs::create_dir_all(&evolvable).unwrap();
        fs::write(
            evolvable.join("lib.rs"),
            "pub fn marker() -> u8 {\n    1\n}\n",
        )
        .unwrap();
        fs::write(
            repo.join("Cargo.toml"),
            "[package]\nname = \"maia-local-intelligence-host\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\npath = \"apps/local-intelligence-host/src/lib.rs\"\n",
        )
        .unwrap();
        fs::write(
            repo.join("Cargo.lock"),
            "version = 4\n\n[[package]]\nname = \"maia-local-intelligence-host\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(repo.join(".gitignore"), "target/\n").unwrap();
        run(&repo, &["add", "."]);
        run(&repo, &["commit", "-m", "fixture"]);
        let commit = run(&repo, &["rev-parse", "HEAD"]);
        Self {
            base,
            repo,
            root,
            commit,
        }
    }
    fn host(&self, checks: Checks) -> GitWorkspaceHost<Checks, Journal> {
        GitWorkspaceHost::new(&self.repo, &self.root, checks, Journal::default()).unwrap()
    }
    fn host_with_tier1(
        &self,
        checks: Checks,
        outcome: Tier1Outcome,
    ) -> GitWorkspaceHost<Checks, Journal, ScriptedTier1> {
        GitWorkspaceHost::new_with_verifier(
            &self.repo,
            &self.root,
            checks,
            Journal::default(),
            ScriptedTier1(outcome),
        )
        .unwrap()
    }
    fn request(&self, id: &str) -> Request {
        let plan = Plan {
            generation_id: format!("generation-{id}"),
            hypothesis_id: format!("hypothesis-{id}"),
            objective_class: "speed".into(),
            operator: "FUNCTION_REWRITE".into(),
            semantic_fingerprint: "semantic".into(),
            implementation_fingerprint: "implementation".into(),
            profiling_fingerprint: "profiling".into(),
            evidence_refs: vec!["evidence-1".into()],
            parent_id: "baseline".into(),
            parent_snapshot_id: "snapshot-1".into(),
            parent_kind: ParentKind::KnownGoodBaseline,
            proposed_paths: vec!["apps/local-intelligence-host/src/lib.rs".into()],
            proposed_symbols: vec![],
            estimated_changed_lines: 2,
            max_changed_lines: 3,
            max_files: 1,
        };
        let admission = Decision {
            outcome: Outcome::Admitted,
            step: Step::ScratchStaticApply,
            reason: None,
            hypothesis_id: plan.hypothesis_id.clone(),
            generation_id: plan.generation_id.clone(),
            semantic_fingerprint: plan.semantic_fingerprint.clone(),
            implementation_fingerprint: plan.implementation_fingerprint.clone(),
            profiling_fingerprint: plan.profiling_fingerprint.clone(),
            reservation_id: Some(format!("reservation-{id}")),
            evidence_refs: plan.evidence_refs.clone(),
        };
        Request {
            plan,
            admission,
            workspace_id: id.into(),
            parent_source_commit: self.commit.clone(),
            created_sequence: 1,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

#[test]
fn admission_and_volatile_checks_fail_before_workspace_creation() {
    let fixture = Fixture::new();
    let mut request = fixture.request("one");
    request.admission.outcome = Outcome::Rejected;
    let mut host = fixture.host(Checks::all());
    assert_eq!(
        allocate(&request, &mut host).unwrap_err(),
        Failure::NotAdmitted
    );
    assert!(!fixture.root.join("one").exists());

    let cases = [
        (
            Checks {
                ready: false,
                ..Checks::all()
            },
            Failure::SupervisorUnavailable,
        ),
        (
            Checks {
                profile: false,
                ..Checks::all()
            },
            Failure::WrongProfile,
        ),
        (
            Checks {
                admission: false,
                ..Checks::all()
            },
            Failure::AdmissionMismatch,
        ),
        (
            Checks {
                parent: false,
                ..Checks::all()
            },
            Failure::ParentChanged,
        ),
        (
            Checks {
                reservation: false,
                ..Checks::all()
            },
            Failure::ReservationInvalid,
        ),
        (
            Checks {
                paths: false,
                ..Checks::all()
            },
            Failure::ProtectedPath,
        ),
    ];
    for (checks, expected) in cases {
        let mut host = fixture.host(checks);
        assert_eq!(
            allocate(&fixture.request("one"), &mut host).unwrap_err(),
            expected
        );
        assert!(!fixture.root.join("one").exists());
    }
    let mut mismatched = fixture.request("one");
    mismatched.plan.operator.clear();
    let mut host = fixture.host(Checks::all());
    assert_eq!(
        allocate(&mismatched, &mut host).unwrap_err(),
        Failure::IncompleteEvidence
    );
    let mut mismatched = fixture.request("one");
    mismatched.plan.semantic_fingerprint = "changed".into();
    assert_eq!(
        allocate(&mismatched, &mut host).unwrap_err(),
        Failure::AdmissionMismatch
    );
}

#[test]
fn isolated_allocation_collision_and_discard_preserve_baseline_and_sibling() {
    let fixture = Fixture::new();
    let mut host = fixture.host(Checks::all());
    let mut first = allocate(&fixture.request("one"), &mut host).unwrap();
    let mut second = allocate(&fixture.request("two"), &mut host).unwrap();
    assert_eq!(first.state(), State::Allocated);
    assert_eq!(
        allocate(&fixture.request("one"), &mut host).unwrap_err(),
        Failure::IdentityCollision
    );
    first.activate(&mut host).unwrap();
    first.discard(&mut host, TerminalOutcome::Rejected).unwrap();
    first.discard(&mut host, TerminalOutcome::Rejected).unwrap();
    assert!(!fixture.root.join("one").exists());
    assert!(fixture.root.join("two").exists());
    assert_eq!(
        fs::read_to_string(fixture.repo.join("baseline.txt")).unwrap(),
        "known good"
    );
    assert_eq!(run(&fixture.repo, &["rev-parse", "HEAD"]), fixture.commit);
    second
        .discard(&mut host, TerminalOutcome::Cancelled)
        .unwrap();
    assert!(!fixture.root.join("two").exists());
    let journal = host.evidence();
    assert_eq!(journal.outcomes.len(), 2);
    assert_eq!(
        journal
            .states
            .iter()
            .filter(|(_, state)| *state == State::Closed)
            .count(),
        2
    );
}

#[test]
fn parent_commit_and_workspace_identity_are_scoped() {
    let fixture = Fixture::new();
    let mut host = fixture.host(Checks::all());
    let mut wrong = fixture.request("one");
    wrong.parent_source_commit = "0".repeat(40);
    assert_eq!(
        allocate(&wrong, &mut host).unwrap_err(),
        Failure::ParentChanged
    );
    let traversal = fixture.request("../other");
    assert_eq!(
        allocate(&traversal, &mut host).unwrap_err(),
        Failure::IdentityCollision
    );
    assert!(!fixture.base.join("other").exists());
}

#[test]
fn changed_protected_path_assumptions_fail_exact_admission_binding() {
    let fixture = Fixture::new();
    let mut checks = Checks::all();
    checks.admitted_plan = Some(fixture.request("one").plan);
    let mut host = fixture.host(checks);
    let mut changed = fixture.request("one");
    changed.plan.proposed_paths = vec!["ops/evolution-supervisor/src/lib.rs".into()];
    assert_eq!(
        allocate(&changed, &mut host).unwrap_err(),
        Failure::AdmissionMismatch
    );
    assert!(!fixture.root.join("one").exists());
}

#[test]
fn host_adapter_is_absent_from_core_and_shipped_apps() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for area in ["core", "infra", "composition", "apps", "roundtable"] {
        let directory = root.join(area);
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && matches!(area, "composition" | "roundtable") =>
            {
                continue;
            }
            Err(error) => panic!("cannot inspect {}: {error}", directory.display()),
        };
        for entry in entries {
            let manifest = entry.unwrap().path().join("Cargo.toml");
            if manifest.is_file() {
                let text = fs::read_to_string(&manifest).unwrap();
                assert!(
                    !text.contains("maia-evolution-workspace-host"),
                    "{}",
                    manifest.display()
                );
            }
        }
    }
    let host_source = include_str!("../src/lib.rs");
    for forbidden in [
        "\"commit\"",
        "\"push\"",
        "\"merge\"",
        "\"rebase\"",
        "\"update-ref\"",
        "\"reset\"",
        "\"branch\"",
        "fs::write",
    ] {
        assert!(!host_source.contains(forbidden));
    }
}

#[derive(Default)]
struct FlakyClosed {
    closed_attempts: u32,
    terminals: u32,
}
impl Evidence for FlakyClosed {
    fn record_state(&mut self, _: &Identity, state: State) -> Result<(), PortError> {
        if state == State::Closed {
            self.closed_attempts += 1;
            if self.closed_attempts == 1 {
                return Err(PortError);
            }
        }
        Ok(())
    }
    fn preserve_terminal(&mut self, _: &Identity, _: TerminalOutcome) -> Result<(), PortError> {
        self.terminals += 1;
        Ok(())
    }
    fn record_mutation_attempt(&mut self, _: &AttemptEvidence) -> Result<(), PortError> {
        Ok(())
    }
}
#[test]
fn closed_record_failure_retries_without_redeleting_candidate() {
    let fixture = Fixture::new();
    let mut host = GitWorkspaceHost::new(
        &fixture.repo,
        &fixture.root,
        Checks::all(),
        FlakyClosed::default(),
    )
    .unwrap();
    let mut candidate = allocate(&fixture.request("one"), &mut host).unwrap();
    assert_eq!(
        candidate.discard(&mut host, TerminalOutcome::InfraError),
        Err(Failure::EvidenceError)
    );
    assert_eq!(candidate.state(), State::Closed);
    assert!(!fixture.root.join("one").exists());
    candidate
        .discard(&mut host, TerminalOutcome::InfraError)
        .unwrap();
    assert_eq!(host.evidence().terminals, 1);
    assert_eq!(host.evidence().closed_attempts, 2);
    assert_eq!(run(&fixture.repo, &["rev-parse", "HEAD"]), fixture.commit);
}

#[test]
fn allowed_candidate_mutation_runs_tier1_and_does_not_promote_or_touch_baseline() {
    let fixture = Fixture::new();
    let mut host = fixture.host(Checks::all());
    let request = fixture.request("allowed");
    let sibling_request = fixture.request("sibling");
    let tier0 = request.admission.clone();
    let mut candidate = allocate(&request, &mut host).unwrap();
    let mut sibling = allocate(&sibling_request, &mut host).unwrap();
    candidate.activate(&mut host).unwrap();
    sibling.activate(&mut host).unwrap();
    let checkout_bytes = fs::read(
        fixture
            .root
            .join("allowed/apps/local-intelligence-host/src/lib.rs"),
    )
    .unwrap();
    assert!(checkout_bytes.windows(2).any(|bytes| bytes == b"\r\n"));
    let result = mutate_and_verify(
        &mut candidate,
        &tier0,
        &mutation_request(&fixture, "allowed"),
        &mut host,
    );
    assert_eq!(result.state, ResultState::Tier1Passed, "result={result:?}");
    assert_eq!(candidate.state(), State::Active);
    assert_eq!(
        read_candidate_text(
            &fixture
                .root
                .join("allowed/apps/local-intelligence-host/src/lib.rs"),
        ),
        "pub fn marker() -> u8 {\n    2\n}\n"
    );
    assert_eq!(
        fs::read_to_string(fixture.repo.join("apps/local-intelligence-host/src/lib.rs")).unwrap(),
        "pub fn marker() -> u8 {\n    1\n}\n"
    );
    assert_eq!(
        read_candidate_text(
            &fixture
                .root
                .join("sibling/apps/local-intelligence-host/src/lib.rs"),
        ),
        "pub fn marker() -> u8 {\n    1\n}\n"
    );
    assert_eq!(run(&fixture.repo, &["rev-parse", "HEAD"]), fixture.commit);
    let events = &host.evidence().attempts;
    let final_event = events.last().unwrap();
    assert_eq!(final_event.workspace_id, "allowed");
    assert_eq!(final_event.generation_id, "generation-allowed");
    assert_eq!(final_event.hypothesis_id, "hypothesis-allowed");
    assert_eq!(final_event.approval_reference, "human-approval-1");
    assert_eq!(
        final_event.changed_file.as_ref().unwrap().path,
        "apps/local-intelligence-host/src/lib.rs"
    );
    assert!(final_event.tier1.is_some());
    assert_eq!(final_event.terminal_state, None);
    assert!(!format!("{final_event:?}").contains("marker() -> u8 {\n    2\n}"));
    candidate
        .discard(&mut host, TerminalOutcome::Cancelled)
        .unwrap();
    sibling
        .discard(&mut host, TerminalOutcome::Cancelled)
        .unwrap();
    assert_eq!(run(&fixture.repo, &["rev-parse", "HEAD"]), fixture.commit);
}

#[test]
fn non_allowlisted_traversal_and_absolute_paths_are_rejected() {
    for (id, path) in [
        ("other", "apps/local-intelligence-host/src/main.rs"),
        ("traversal", "../baseline.txt"),
        ("absolute", "C:/MAIA/repo/src/lib.rs"),
    ] {
        let fixture = Fixture::new();
        let mut host = fixture.host(Checks::all());
        let request = fixture.request(id);
        let tier0 = request.admission.clone();
        let mut candidate = allocate(&request, &mut host).unwrap();
        candidate.activate(&mut host).unwrap();
        let mut mutation = mutation_request(&fixture, id);
        mutation.path = path.into();
        let result = mutate_and_verify(&mut candidate, &tier0, &mutation, &mut host);
        assert_eq!(result.state, ResultState::Rejected, "{path}");
        assert_eq!(candidate.state(), State::Closed);
        assert_eq!(run(&fixture.repo, &["rev-parse", "HEAD"]), fixture.commit);
        assert_eq!(
            fs::read_to_string(fixture.repo.join("baseline.txt")).unwrap(),
            "known good"
        );
    }
}

#[test]
fn candidate_hardlink_to_canonical_host_file_is_refused() {
    let fixture = Fixture::new();
    let mut host = fixture.host(Checks::all());
    let request = fixture.request("hardlink");
    let tier0 = request.admission.clone();
    let mut candidate = allocate(&request, &mut host).unwrap();
    candidate.activate(&mut host).unwrap();
    let canonical = fixture.repo.join("apps/local-intelligence-host/src/lib.rs");
    let candidate_file = fixture
        .root
        .join("hardlink/apps/local-intelligence-host/src/lib.rs");
    fs::remove_file(&candidate_file).unwrap();
    fs::hard_link(&canonical, &candidate_file).unwrap();
    let result = mutate_and_verify(
        &mut candidate,
        &tier0,
        &mutation_request(&fixture, "hardlink"),
        &mut host,
    );
    assert_eq!(result.state, ResultState::Rejected);
    assert_eq!(
        fs::read_to_string(canonical).unwrap(),
        "pub fn marker() -> u8 {\n    1\n}\n"
    );
    assert_eq!(run(&fixture.repo, &["rev-parse", "HEAD"]), fixture.commit);
}

#[test]
fn tier0_failure_and_identity_mismatch_never_run_tier1() {
    let fixture = Fixture::new();
    let mut host = fixture.host_with_tier1(Checks::all(), Tier1Outcome::InfraError);
    let request = fixture.request("tier0-fail");
    let mut failed = request.admission.clone();
    failed.outcome = Outcome::Rejected;
    let mut candidate = allocate(&request, &mut host).unwrap();
    candidate.activate(&mut host).unwrap();
    let result = mutate_and_verify(
        &mut candidate,
        &failed,
        &mutation_request(&fixture, "tier0-fail"),
        &mut host,
    );
    assert_eq!(result.state, ResultState::Rejected);
    assert!(
        host.evidence()
            .attempts
            .iter()
            .all(|event| event.tier1.is_none())
    );
    assert_eq!(candidate.state(), State::Closed);

    let request = fixture.request("identity-mismatch");
    let tier0 = request.admission.clone();
    let mut candidate = allocate(&request, &mut host).unwrap();
    candidate.activate(&mut host).unwrap();
    let mut mutation = mutation_request(&fixture, "identity-mismatch");
    mutation.hypothesis_id = "different-hypothesis".into();
    let result = mutate_and_verify(&mut candidate, &tier0, &mutation, &mut host);
    assert_eq!(result.state, ResultState::Rejected);
    assert!(host.evidence().attempts.last().unwrap().tier1.is_none());
}

#[test]
fn tier1_failure_and_infrastructure_error_discard_candidate_without_rollback() {
    for outcome in [Tier1Outcome::Failed, Tier1Outcome::InfraError] {
        let fixture = Fixture::new();
        let mut host = fixture.host_with_tier1(Checks::all(), outcome);
        let request = fixture.request("tier1-outcome");
        let tier0 = request.admission.clone();
        let mut candidate = allocate(&request, &mut host).unwrap();
        candidate.activate(&mut host).unwrap();
        let result = mutate_and_verify(
            &mut candidate,
            &tier0,
            &mutation_request(&fixture, "tier1-outcome"),
            &mut host,
        );
        assert_eq!(
            result.state,
            if outcome == Tier1Outcome::Failed {
                ResultState::Rejected
            } else {
                ResultState::InfraError
            },
            "outcome={outcome:?}; result={result:?}; attempt={:?}",
            host.evidence().attempts.last()
        );
        assert_eq!(candidate.state(), State::Closed);
        assert_eq!(
            candidate.outcome(),
            Some(if outcome == Tier1Outcome::Failed {
                TerminalOutcome::Rejected
            } else {
                TerminalOutcome::InfraError
            })
        );
        assert!(!fixture.root.join("tier1-outcome").exists());
        assert_eq!(run(&fixture.repo, &["rev-parse", "HEAD"]), fixture.commit);
    }
}

#[test]
fn verifier_side_effect_on_protected_candidate_file_fails_tier1_closed() {
    let fixture = Fixture::new();
    let mut host = GitWorkspaceHost::new_with_verifier(
        &fixture.repo,
        &fixture.root,
        Checks::all(),
        Journal::default(),
        ProtectedSurfaceMutator,
    )
    .unwrap();
    let request = fixture.request("unexpected-protected-change");
    let tier0 = request.admission.clone();
    let mut candidate = allocate(&request, &mut host).unwrap();
    candidate.activate(&mut host).unwrap();
    let result = mutate_and_verify(
        &mut candidate,
        &tier0,
        &mutation_request(&fixture, "unexpected-protected-change"),
        &mut host,
    );
    assert_eq!(result.state, ResultState::Rejected);
    assert_eq!(candidate.outcome(), Some(TerminalOutcome::Rejected));
    assert_eq!(
        fs::read_to_string(fixture.repo.join("baseline.txt")).unwrap(),
        "known good"
    );
    assert_eq!(run(&fixture.repo, &["rev-parse", "HEAD"]), fixture.commit);
}

#[test]
fn incomplete_tier1_evidence_fails_closed_and_discards_candidate() {
    let fixture = Fixture::new();
    let mut host = GitWorkspaceHost::new_with_verifier(
        &fixture.repo,
        &fixture.root,
        Checks::all(),
        Journal::default(),
        IncompleteTier1,
    )
    .unwrap();
    let request = fixture.request("incomplete-tier1");
    let tier0 = request.admission.clone();
    let mut candidate = allocate(&request, &mut host).unwrap();
    candidate.activate(&mut host).unwrap();
    let result = mutate_and_verify(
        &mut candidate,
        &tier0,
        &mutation_request(&fixture, "incomplete-tier1"),
        &mut host,
    );
    assert_eq!(
        result.state,
        ResultState::InfraError,
        "result={result:?}; attempt={:?}",
        host.evidence().attempts.last()
    );
    assert_eq!(candidate.state(), State::Closed);
    assert_eq!(candidate.outcome(), Some(TerminalOutcome::InfraError));
    assert!(!fixture.root.join("incomplete-tier1").exists());
    assert_eq!(run(&fixture.repo, &["rev-parse", "HEAD"]), fixture.commit);
}

#[test]
fn durable_evidence_is_queryable_hash_chained_and_disjoint_from_candidate_and_host() {
    let fixture = Fixture::new();
    let journal_path = fixture.base.join("evolution-state/evidence.jsonl");
    fs::create_dir_all(journal_path.parent().unwrap()).unwrap();
    let journal = FileEvidenceJournal::open(&journal_path, &fixture.repo, &fixture.root).unwrap();
    let mut host =
        GitWorkspaceHost::new(&fixture.repo, &fixture.root, Checks::all(), journal).unwrap();
    let request = fixture.request("durable");
    let tier0 = request.admission.clone();
    let mut candidate = allocate(&request, &mut host).unwrap();
    candidate.activate(&mut host).unwrap();
    let result = mutate_and_verify(
        &mut candidate,
        &tier0,
        &mutation_request(&fixture, "durable"),
        &mut host,
    );
    assert_eq!(result.state, ResultState::Tier1Passed, "result={result:?}");
    drop(candidate);
    drop(host);

    let events = FileEvidenceJournal::read_events(&journal_path).unwrap();
    assert!(events.iter().any(|event| {
        event["kind"] == "mutation_attempt"
            && event["workspace_id"] == "durable"
            && event["tier0_outcome"] == "ADMITTED"
            && event["approval_reference"] == "human-approval-1"
            && event["tier1"]["outcome"] == "PASSED"
            && event["changed_file"]["path"] == "apps/local-intelligence-host/src/lib.rs"
    }));
    let contents = fs::read_to_string(&journal_path).unwrap();
    assert!(!contents.contains("marker() -> u8 {\n    2\n}"));
    assert!(FileEvidenceJournal::open(&journal_path, &fixture.repo, &fixture.root).is_ok());
    assert!(
        FileEvidenceJournal::open(
            &fixture.repo.join(".evolution-evidence.jsonl"),
            &fixture.repo,
            &fixture.root,
        )
        .is_err()
    );

    let mut tampered = contents;
    tampered.push(' ');
    fs::write(&journal_path, tampered).unwrap();
    assert!(FileEvidenceJournal::open(&journal_path, &fixture.repo, &fixture.root).is_err());
}
