//! Development-host Git worktree adapter. No worker process or capability is
//! exposed. Only a protected host may supply Gate and Evidence implementations.
#![forbid(unsafe_code)]

use crate::tier1::{FixedCargoTier1Verifier, Tier1Verifier};
use maia_evolution_supervisor::mutation::{
    AppliedChange, AttemptEvidence, CheckEvidence, CheckKind, CheckOutcome, EVOLVABLE_PATHS,
    MAX_REPLACEMENT_BYTES, MutationPorts, MutationRequest, PortFailure, Tier1Evidence,
};
use maia_evolution_supervisor::workspace::{
    Failure, Identity, PortError, Request, State, TerminalOutcome, WorkspacePort,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub trait Gate {
    fn supervisor_ready(&mut self) -> Result<bool, PortError>;
    fn development_profile(&mut self) -> Result<bool, PortError>;
    fn admission_current(&mut self, request: &Request) -> Result<bool, PortError>;
    fn parent_current(&mut self, identity: &Identity) -> Result<bool, PortError>;
    fn reservation_current(&mut self, identity: &Identity) -> Result<bool, PortError>;
    fn paths_allowed(&mut self, identity: &Identity) -> Result<bool, PortError>;
    /// Human approval is stricter than v5.1's unattended development profile
    /// and is required by the M0.16.3 operating contract.
    fn approval_current(
        &mut self,
        identity: &Identity,
        approval_reference: &str,
    ) -> Result<bool, PortError>;
}

pub trait Evidence {
    fn record_state(&mut self, identity: &Identity, state: State) -> Result<(), PortError>;
    fn preserve_terminal(
        &mut self,
        identity: &Identity,
        outcome: TerminalOutcome,
    ) -> Result<(), PortError>;
    /// Must append and durably flush a content-free evidence event before
    /// returning success. Replacement bytes and secret-bearing output are
    /// intentionally absent from this contract.
    fn record_mutation_attempt(&mut self, evidence: &AttemptEvidence) -> Result<(), PortError>;
}

pub struct GitWorkspaceHost<G, E, V = FixedCargoTier1Verifier> {
    repository: PathBuf,
    candidate_root: PathBuf,
    gate: G,
    evidence: E,
    verifier: V,
    owned: BTreeMap<String, Identity>,
    quarantined: BTreeSet<String>,
}

fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn git_path(path: &Path) -> Result<String, PortError> {
    let text = path.to_str().ok_or(PortError)?;
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        Ok(format!(r"\\{}", rest))
    } else {
        Ok(text.strip_prefix(r"\\?\").unwrap_or(text).to_owned())
    }
}

fn git(repository: &Path, args: &[&str]) -> Result<String, PortError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(args)
        .output()
        .map_err(|_| PortError)?;
    if !output.status.success() {
        return Err(PortError);
    }
    String::from_utf8(output.stdout).map_err(|_| PortError)
}

fn changed_line_count(before: &str, after: &str) -> Result<u32, PortFailure> {
    let old: Vec<_> = before.lines().collect();
    let new: Vec<_> = after.lines().collect();
    let mut prefix = 0;
    while prefix < old.len().min(new.len()) && old[prefix] == new[prefix] {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix
        < old
            .len()
            .saturating_sub(prefix)
            .min(new.len().saturating_sub(prefix))
        && old[old.len() - suffix - 1] == new[new.len() - suffix - 1]
    {
        suffix += 1;
    }
    u32::try_from((old.len() - prefix - suffix) + (new.len() - prefix - suffix))
        .map_err(|_| PortFailure::Rejected)
}

impl<G: Gate, E: Evidence> GitWorkspaceHost<G, E, FixedCargoTier1Verifier> {
    pub fn new(
        repository: &Path,
        candidate_root: &Path,
        gate: G,
        evidence: E,
    ) -> Result<Self, PortError> {
        Self::new_with_verifier(
            repository,
            candidate_root,
            gate,
            evidence,
            FixedCargoTier1Verifier,
        )
    }
}

impl<G: Gate, E: Evidence, V> GitWorkspaceHost<G, E, V> {
    /// Test and embedding seam for a trusted host-owned verifier. Production
    /// construction uses `FixedCargoTier1Verifier` and exposes no candidate
    /// command-selection API.
    pub fn new_with_verifier(
        repository: &Path,
        candidate_root: &Path,
        gate: G,
        evidence: E,
        verifier: V,
    ) -> Result<Self, PortError> {
        let repository = fs::canonicalize(repository).map_err(|_| PortError)?;
        let candidate_root = fs::canonicalize(candidate_root).map_err(|_| PortError)?;
        if candidate_root
            .file_name()
            .is_none_or(|name| name != "evolution-candidates")
            || repository == candidate_root
            || candidate_root.starts_with(&repository)
            || repository.starts_with(&candidate_root)
            || fs::symlink_metadata(&candidate_root)
                .map_err(|_| PortError)?
                .file_type()
                .is_symlink()
        {
            return Err(PortError);
        }
        let top = git(&repository, &["rev-parse", "--show-toplevel"])?;
        if fs::canonicalize(top.trim()).map_err(|_| PortError)? != repository {
            return Err(PortError);
        }
        Ok(Self {
            repository,
            candidate_root,
            gate,
            evidence,
            verifier,
            owned: BTreeMap::new(),
            quarantined: BTreeSet::new(),
        })
    }

    pub fn evidence(&self) -> &E {
        &self.evidence
    }

    fn path(&self, identity: &Identity) -> Result<PathBuf, PortError> {
        if !safe_component(&identity.workspace_id) {
            return Err(PortError);
        }
        Ok(self.candidate_root.join(&identity.workspace_id))
    }

    fn candidate_dir(&self, identity: &Identity) -> Result<PathBuf, PortFailure> {
        if self.owned.get(&identity.workspace_id) != Some(identity)
            || self.quarantined.contains(&identity.workspace_id)
        {
            return Err(PortFailure::Rejected);
        }
        let path = self.path(identity).map_err(|_| PortFailure::Rejected)?;
        let canonical = fs::canonicalize(&path).map_err(|_| PortFailure::Rejected)?;
        if !canonical.starts_with(&self.candidate_root)
            || canonical == self.repository
            || canonical.starts_with(&self.repository)
            || self.repository.starts_with(&canonical)
        {
            return Err(PortFailure::ProtectedTarget);
        }
        let candidate_top = git(&canonical, &["rev-parse", "--show-toplevel"])
            .map_err(|_| PortFailure::Infrastructure)?;
        let candidate_top =
            fs::canonicalize(candidate_top.trim()).map_err(|_| PortFailure::ProtectedTarget)?;
        let candidate_head =
            git(&canonical, &["rev-parse", "HEAD"]).map_err(|_| PortFailure::Infrastructure)?;
        if candidate_top != canonical || candidate_head.trim() != identity.parent_source_commit {
            return Err(PortFailure::ProtectedTarget);
        }
        let parent = git(&self.repository, &["rev-parse", "HEAD"])
            .map_err(|_| PortFailure::Infrastructure)?;
        let canonical_status = git(&self.repository, &["status", "--porcelain=v1", "-z"])
            .map_err(|_| PortFailure::Infrastructure)?;
        if parent.trim() != identity.parent_source_commit || !canonical_status.is_empty() {
            return Err(PortFailure::ProtectedTarget);
        }
        Ok(canonical)
    }

    fn candidate_file(&self, identity: &Identity, relative: &str) -> Result<PathBuf, PortFailure> {
        if !EVOLVABLE_PATHS.contains(&relative)
            || !identity.proposed_paths.iter().any(|path| path == relative)
        {
            return Err(PortFailure::Rejected);
        }
        let relative_path = Path::new(relative);
        if relative_path.is_absolute()
            || relative_path
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(PortFailure::ProtectedTarget);
        }
        let root = self.candidate_dir(identity)?;
        let mut current = root.clone();
        for component in relative_path.components() {
            let std::path::Component::Normal(part) = component else {
                return Err(PortFailure::ProtectedTarget);
            };
            current.push(part);
            let metadata = fs::symlink_metadata(&current).map_err(|_| PortFailure::Rejected)?;
            if metadata.file_type().is_symlink() {
                return Err(PortFailure::ProtectedTarget);
            }
        }
        let canonical = fs::canonicalize(&current).map_err(|_| PortFailure::Rejected)?;
        let metadata = fs::metadata(&canonical).map_err(|_| PortFailure::Rejected)?;
        if !canonical.starts_with(&root)
            || canonical.starts_with(&self.repository)
            || !metadata.is_file()
        {
            return Err(PortFailure::ProtectedTarget);
        }
        let protected_counterpart =
            fs::canonicalize(self.repository.join(relative)).map_err(|_| PortFailure::Rejected)?;
        if same_file::is_same_file(&canonical, &protected_counterpart)
            .map_err(|_| PortFailure::Infrastructure)?
        {
            return Err(PortFailure::ProtectedTarget);
        }
        for other in fs::read_dir(&self.candidate_root).map_err(|_| PortFailure::Infrastructure)? {
            let other = other.map_err(|_| PortFailure::Infrastructure)?.path();
            if other == root || !other.is_dir() {
                continue;
            }
            let sibling = other.join(relative);
            if sibling.exists()
                && same_file::is_same_file(&canonical, sibling)
                    .map_err(|_| PortFailure::Infrastructure)?
            {
                return Err(PortFailure::ProtectedTarget);
            }
        }
        Ok(canonical)
    }
}

impl<G: Gate, E: Evidence, V> WorkspacePort for GitWorkspaceHost<G, E, V> {
    fn supervisor_ready(&mut self) -> Result<bool, PortError> {
        self.gate.supervisor_ready()
    }
    fn development_profile(&mut self) -> Result<bool, PortError> {
        self.gate.development_profile()
    }
    fn admission_current(&mut self, request: &Request) -> Result<bool, PortError> {
        self.gate.admission_current(request)
    }
    fn parent_current(&mut self, identity: &Identity) -> Result<bool, PortError> {
        self.gate.parent_current(identity)
    }
    fn reservation_current(&mut self, identity: &Identity) -> Result<bool, PortError> {
        self.gate.reservation_current(identity)
    }
    fn paths_allowed(&mut self, identity: &Identity) -> Result<bool, PortError> {
        self.gate.paths_allowed(identity)
    }
    fn allocate_workspace(&mut self, identity: &Identity) -> Result<(), Failure> {
        let path = self
            .path(identity)
            .map_err(|_| Failure::IdentityCollision)?;
        if self.owned.contains_key(&identity.workspace_id) || path.exists() {
            return Err(Failure::IdentityCollision);
        }
        let commit = &identity.parent_source_commit;
        if commit.len() != 40 || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Failure::ParentChanged);
        }
        let actual = git(
            &self.repository,
            &["rev-parse", "--verify", &format!("{commit}^{{commit}}")],
        )
        .map_err(|_| Failure::ParentChanged)?;
        if actual.trim() != commit {
            return Err(Failure::ParentChanged);
        }
        let path_text = git_path(&path).map_err(|_| Failure::HostError)?;
        git(
            &self.repository,
            &["worktree", "add", "--detach", &path_text, commit],
        )
        .map_err(|_| Failure::HostError)?;
        self.owned
            .insert(identity.workspace_id.clone(), identity.clone());
        Ok(())
    }
    fn preserve_terminal(
        &mut self,
        identity: &Identity,
        outcome: TerminalOutcome,
    ) -> Result<(), PortError> {
        self.evidence.preserve_terminal(identity, outcome)
    }
    fn discard_workspace(&mut self, identity: &Identity) -> Result<(), PortError> {
        let Some(owned) = self.owned.get(&identity.workspace_id) else {
            return Err(PortError);
        };
        if owned != identity {
            return Err(PortError);
        }
        let path = self.path(identity)?;
        if path.exists() {
            let path_text = git_path(&path)?;
            git(
                &self.repository,
                &["worktree", "remove", "--force", &path_text],
            )?;
        }
        self.owned.remove(&identity.workspace_id);
        Ok(())
    }
    fn record_state(&mut self, identity: &Identity, state: State) -> Result<(), PortError> {
        self.evidence.record_state(identity, state)
    }
}

impl<G: Gate, E: Evidence, V: Tier1Verifier> MutationPorts for GitWorkspaceHost<G, E, V> {
    fn approval_current(
        &mut self,
        identity: &Identity,
        approval_reference: &str,
    ) -> Result<bool, PortError> {
        self.gate.approval_current(identity, approval_reference)
    }

    fn candidate_workspace_current(&mut self, identity: &Identity) -> Result<bool, PortError> {
        Ok(self.candidate_dir(identity).is_ok())
    }

    fn record_attempt(&mut self, evidence: &AttemptEvidence) -> Result<(), PortError> {
        self.evidence.record_mutation_attempt(evidence)
    }

    fn quarantine_candidate(&mut self, identity: &Identity) {
        if self.owned.get(&identity.workspace_id) == Some(identity) {
            self.quarantined.insert(identity.workspace_id.clone());
        }
    }

    fn apply_candidate_patch(
        &mut self,
        identity: &Identity,
        request: &MutationRequest,
    ) -> Result<AppliedChange, PortFailure> {
        if request.candidate_workspace_id != identity.workspace_id
            || request.generation_id != identity.generation_id
            || request.hypothesis_id != identity.hypothesis_id
        {
            return Err(PortFailure::Rejected);
        }
        if request.expected_text.is_empty()
            || request.expected_text == request.replacement_text
            || request.expected_text.len() > MAX_REPLACEMENT_BYTES
            || request.replacement_text.len() > MAX_REPLACEMENT_BYTES
        {
            return Err(PortFailure::Rejected);
        }
        let target = self.candidate_file(identity, &request.path)?;
        let before = fs::read(&target).map_err(|_| PortFailure::Infrastructure)?;
        let before_digest = format!("{:x}", Sha256::digest(&before));
        if before_digest != request.expected_sha256 {
            return Err(PortFailure::Rejected);
        }
        let text = std::str::from_utf8(&before).map_err(|_| PortFailure::Rejected)?;
        if text.matches(&request.expected_text).count() != 1 {
            return Err(PortFailure::Rejected);
        }
        let changed_lines = changed_line_count(&request.expected_text, &request.replacement_text)?;
        if changed_lines == 0
            || changed_lines > identity.max_changed_lines
            || identity.max_files < 1
        {
            return Err(PortFailure::Rejected);
        }
        let replacement = text.replacen(&request.expected_text, &request.replacement_text, 1);
        let after = replacement.as_bytes();
        let after_digest = format!("{:x}", Sha256::digest(after));
        // Re-resolve immediately before opening. Candidate workspaces are only
        // writable through this host capability; symlink parents are refused.
        if self.candidate_file(identity, &request.path)? != target {
            return Err(PortFailure::ProtectedTarget);
        }
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&target)
            .map_err(|_| PortFailure::Infrastructure)?;
        file.write_all(after)
            .and_then(|()| file.sync_all())
            .map_err(|_| PortFailure::Infrastructure)?;
        Ok(AppliedChange {
            path: request.path.clone(),
            before_sha256: before_digest,
            after_sha256: after_digest,
            changed_lines,
        })
    }

    fn verify_tier1(
        &mut self,
        identity: &Identity,
        changed_path: &str,
    ) -> Result<Tier1Evidence, PortFailure> {
        let workspace = self.candidate_dir(identity)?;
        let mut evidence = self.verifier.verify(identity, &workspace, changed_path)?;
        // Revalidate both the candidate and the canonical host after the
        // verifier returns. The verifier's fixed Cargo checks run build tools.
        let _ = self.candidate_dir(identity)?;
        let status = git(
            &workspace,
            &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        )
        .map_err(|_| PortFailure::Infrastructure)?;
        let entries: Vec<_> = status
            .split('\0')
            .filter(|entry| !entry.is_empty())
            .collect();
        if entries.len() != 1 || !entries[0].starts_with(" M ") || &entries[0][3..] != changed_path
        {
            return Err(PortFailure::ProtectedTarget);
        }
        if evidence
            .checks
            .iter()
            .any(|item| item.check == CheckKind::ProtectedSurfaceIntegrity)
        {
            return Err(PortFailure::Infrastructure);
        }
        evidence.checks.push(CheckEvidence {
            check: CheckKind::ProtectedSurfaceIntegrity,
            outcome: CheckOutcome::Passed,
            evidence_ref: "git-status:exact-single-allowlisted-worktree-change".into(),
        });
        Ok(evidence)
    }
}

pub mod evidence;
pub mod tier1;
