//! Durable, content-free candidate lifecycle and mutation evidence journal.
//! The journal is kept outside both the canonical repository and candidates.
use maia_evolution_supervisor::mutation::{AttemptEvidence, CheckKind, CheckOutcome, Operation};
use maia_evolution_supervisor::workspace::{Identity, PortError, State, TerminalOutcome};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

pub struct FileEvidenceJournal {
    path: PathBuf,
    file: File,
    sequence: u64,
    previous_hash: String,
    poisoned: bool,
}

fn load_chain(path: &Path) -> Result<(u64, String, Vec<Value>), PortError> {
    let file = File::open(path).map_err(|_| PortError)?;
    let reader = BufReader::new(file);
    let mut sequence = 0u64;
    let mut previous_hash = String::new();
    let mut events = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|_| PortError)?;
        let entry: Value = serde_json::from_str(&line).map_err(|_| PortError)?;
        let body = entry.get("body").ok_or(PortError)?;
        let stored_hash = entry
            .get("entry_sha256")
            .and_then(Value::as_str)
            .ok_or(PortError)?;
        let bytes = serde_json::to_vec(body).map_err(|_| PortError)?;
        let calculated = format!("{:x}", Sha256::digest(bytes));
        if calculated != stored_hash
            || body.get("sequence").and_then(Value::as_u64) != Some(sequence + 1)
            || body.get("previous_sha256").and_then(Value::as_str) != Some(previous_hash.as_str())
        {
            return Err(PortError);
        }
        events.push(body.get("event").ok_or(PortError)?.clone());
        sequence += 1;
        previous_hash = calculated;
    }
    Ok((sequence, previous_hash, events))
}

fn safe_token(value: &str) -> Value {
    if !value.is_empty()
        && value.len() <= 160
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/')
        })
    {
        json!(value)
    } else {
        json!({"sha256": format!("{:x}", Sha256::digest(value.as_bytes()))})
    }
}

impl FileEvidenceJournal {
    /// Opens a single-writer append-only journal whose location must be
    /// disjoint from the canonical repository and candidate workspace root.
    pub fn open(path: &Path, repository: &Path, candidate_root: &Path) -> Result<Self, PortError> {
        let repository = fs::canonicalize(repository).map_err(|_| PortError)?;
        let candidate_root = fs::canonicalize(candidate_root).map_err(|_| PortError)?;
        let parent = path.parent().ok_or(PortError)?;
        let canonical_parent = fs::canonicalize(parent).map_err(|_| PortError)?;
        let canonical_path = canonical_parent.join(path.file_name().ok_or(PortError)?);
        if canonical_path.starts_with(&repository)
            || canonical_path.starts_with(&candidate_root)
            || repository.starts_with(&canonical_path)
            || candidate_root.starts_with(&canonical_path)
        {
            return Err(PortError);
        }
        if let Ok(metadata) = fs::symlink_metadata(&canonical_path)
            && metadata.file_type().is_symlink()
        {
            return Err(PortError);
        }
        let (sequence, previous_hash) = if canonical_path.exists() {
            let (sequence, previous_hash, _) = load_chain(&canonical_path)?;
            (sequence, previous_hash)
        } else {
            (0, String::new())
        };
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&canonical_path)
            .map_err(|_| PortError)?;
        Ok(Self {
            path: canonical_path,
            file,
            sequence,
            previous_hash,
            poisoned: false,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read_events(path: &Path) -> Result<Vec<Value>, PortError> {
        Ok(load_chain(path)?.2)
    }

    fn append(&mut self, event: Value) -> Result<(), PortError> {
        if self.poisoned {
            return Err(PortError);
        }
        let body = json!({
            "sequence": self.sequence + 1,
            "previous_sha256": self.previous_hash,
            "event": event,
        });
        let encoded = serde_json::to_vec(&body).map_err(|_| PortError)?;
        let digest = format!("{:x}", Sha256::digest(&encoded));
        let mut line = serde_json::to_vec(&json!({
            "body": body,
            "entry_sha256": digest,
        }))
        .map_err(|_| PortError)?;
        line.push(b'\n');
        if self.file.write_all(&line).is_err() || self.file.sync_all().is_err() {
            self.poisoned = true;
            return Err(PortError);
        }
        self.sequence += 1;
        self.previous_hash = digest;
        Ok(())
    }
}

fn state_name(state: State) -> &'static str {
    match state {
        State::Requested => "REQUESTED",
        State::Allocated => "ALLOCATED",
        State::Active => "ACTIVE",
        State::Closed => "CLOSED",
    }
}

fn terminal_name(outcome: TerminalOutcome) -> &'static str {
    match outcome {
        TerminalOutcome::Rejected => "REJECTED",
        TerminalOutcome::Cancelled => "CANCELLED",
        TerminalOutcome::InfraError => "INFRA_ERROR",
    }
}

fn check_name(check: CheckKind) -> &'static str {
    match check {
        CheckKind::CandidateIdentity => "post_mutation_candidate_identity",
        CheckKind::SyntaxStatic => "syntax_static_validation",
        CheckKind::FormattingLint => "formatting_and_lint",
        CheckKind::ComponentBuild => "touched_component_build",
        CheckKind::TargetedTests => "targeted_unit_and_contract_tests",
        CheckKind::ProtectedSurfaceIntegrity => "protected_surface_integrity",
    }
}

fn check_outcome_name(outcome: CheckOutcome) -> &'static str {
    match outcome {
        CheckOutcome::Passed => "PASS",
        CheckOutcome::Failed => "FAIL",
        CheckOutcome::InfraError => "INFRA_ERROR",
    }
}

impl super::Evidence for FileEvidenceJournal {
    fn record_state(&mut self, identity: &Identity, state: State) -> Result<(), PortError> {
        self.append(json!({
            "kind": "candidate_state",
            "workspace_id": safe_token(&identity.workspace_id),
            "generation_id": safe_token(&identity.generation_id),
            "hypothesis_id": safe_token(&identity.hypothesis_id),
            "state": state_name(state),
        }))
    }

    fn preserve_terminal(
        &mut self,
        identity: &Identity,
        outcome: TerminalOutcome,
    ) -> Result<(), PortError> {
        self.append(json!({
            "kind": "candidate_terminal",
            "workspace_id": safe_token(&identity.workspace_id),
            "generation_id": safe_token(&identity.generation_id),
            "hypothesis_id": safe_token(&identity.hypothesis_id),
            "outcome": terminal_name(outcome),
            "operation": "discard_candidate",
        }))
    }

    fn record_mutation_attempt(&mut self, evidence: &AttemptEvidence) -> Result<(), PortError> {
        let changed_file = evidence.changed_file.as_ref().map(|change| {
            json!({
                "path": safe_token(&change.path),
                "before_sha256": change.before_sha256,
                "after_sha256": change.after_sha256,
                "changed_lines": change.changed_lines,
            })
        });
        let tier1 = evidence.tier1.as_ref().map(|report| {
            json!({
                "outcome": format!("{:?}", report.outcome).to_ascii_uppercase(),
                "verifier_identity": safe_token(&report.verifier_identity),
                "checks": report.checks.iter().map(|item| json!({
                    "check": check_name(item.check),
                    "outcome": check_outcome_name(item.outcome),
                    "evidence_ref": safe_token(&item.evidence_ref),
                })).collect::<Vec<_>>(),
            })
        });
        self.append(json!({
            "kind": "mutation_attempt",
            "candidate_id": safe_token(&evidence.candidate_id),
            "workspace_id": safe_token(&evidence.workspace_id),
            "generation_id": safe_token(&evidence.generation_id),
            "hypothesis_id": safe_token(&evidence.hypothesis_id),
            "tier0_outcome": format!("{:?}", evidence.tier0_outcome).to_ascii_uppercase(),
            "tier0_evidence_refs": evidence.tier0_evidence_refs.iter().map(|item| safe_token(item)).collect::<Vec<_>>(),
            "approval_reference": safe_token(&evidence.approval_reference),
            "requested_path": safe_token(&evidence.requested_path),
            "operation": match evidence.operation { Operation::FunctionRewrite => "FUNCTION_REWRITE" },
            "outcome": format!("{:?}", evidence.outcome).to_ascii_uppercase(),
            "reason": evidence.reason.map(|reason| format!("{reason:?}").to_ascii_uppercase()),
            "changed_file": changed_file,
            "tier1": tier1,
            "terminal_state": evidence.terminal_state.map(state_name),
            "terminal_reason": evidence.terminal_reason.map(terminal_name),
            "content_recorded": false,
        }))
    }
}
