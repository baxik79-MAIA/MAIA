//! Development-host Git worktree adapter. No worker process or capability is
//! exposed. Only a protected host may supply Gate and Evidence implementations.
#![forbid(unsafe_code)]

use maia_evolution_supervisor::workspace::{
    Failure, Identity, PortError, Request, State, TerminalOutcome, WorkspacePort,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub trait Gate {
    fn supervisor_ready(&mut self) -> Result<bool, PortError>;
    fn development_profile(&mut self) -> Result<bool, PortError>;
    fn admission_current(&mut self, request: &Request) -> Result<bool, PortError>;
    fn parent_current(&mut self, identity: &Identity) -> Result<bool, PortError>;
    fn reservation_current(&mut self, identity: &Identity) -> Result<bool, PortError>;
    fn paths_allowed(&mut self, identity: &Identity) -> Result<bool, PortError>;
}

pub trait Evidence {
    fn record_state(&mut self, identity: &Identity, state: State) -> Result<(), PortError>;
    fn preserve_terminal(
        &mut self,
        identity: &Identity,
        outcome: TerminalOutcome,
    ) -> Result<(), PortError>;
}

pub struct GitWorkspaceHost<G, E> {
    repository: PathBuf,
    candidate_root: PathBuf,
    gate: G,
    evidence: E,
    owned: BTreeMap<String, Identity>,
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

impl<G: Gate, E: Evidence> GitWorkspaceHost<G, E> {
    pub fn new(
        repository: &Path,
        candidate_root: &Path,
        gate: G,
        evidence: E,
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
            owned: BTreeMap::new(),
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
}

impl<G: Gate, E: Evidence> WorkspacePort for GitWorkspaceHost<G, E> {
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
