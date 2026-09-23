//! Host/runtime configuration for the live builder (M0.15.7d-prep).
//!
//! The participant registry stays identity and routing only: id, adapter,
//! provider, model, roles, assurance, enabled. Everything about *this machine*
//! that the existing provider constructors need is explicit command-line
//! configuration, parsed here, with no default, no search and no environment
//! fallback:
//!
//! | option | needed by |
//! |---|---|
//! | `--claude-exe <absolute path>` | `ClaudeCodeConfig::executable` |
//! | `--claude-audit-dir <absolute dir>` | `ClaudeCodeConfig::audit_directory` |
//! | `--working-dir <absolute dir>` | `ClaudeCodeConfig::working_directory` |
//! | `--local-endpoint <ip:port>` | `LoopbackLocalProvider::new` |
//!
//! The participant model is never configured here: it is the registry
//! `model_ref`.
//!
//! Parsing is purely syntactic and touches no file, process or socket, so it is
//! safe in every build. Whether a path exists is checked later, in
//! [`HostConfig::local_problem`], still without running anything.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostConfig {
    pub claude_exe: PathBuf,
    pub claude_audit_dir: PathBuf,
    pub working_dir: PathBuf,
    pub local_endpoint: SocketAddr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostConfigError {
    /// A path was relative. An absolute path is required so nothing is searched
    /// for (`PATH`) or resolved against a directory the person did not name.
    NotAbsolute(&'static str),
    /// Not `ip:port`. A host name is refused rather than resolved.
    EndpointUnparsable,
    /// The address is not a loopback address. There is no override.
    EndpointNotLoopback,
    EndpointPortZero,
}

impl std::fmt::Display for HostConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAbsolute(opt) => write!(
                f,
                "{opt} must be an absolute path: nothing is searched for and nothing is \
                 resolved against a directory you did not name"
            ),
            Self::EndpointUnparsable => write!(
                f,
                "--local-endpoint must be a literal ip:port such as 127.0.0.1:11434 (a host \
                 name is not resolved)"
            ),
            Self::EndpointNotLoopback => write!(
                f,
                "--local-endpoint must be a loopback address; the local participant never \
                 leaves this machine and there is no override"
            ),
            Self::EndpointPortZero => write!(f, "--local-endpoint needs a non-zero port"),
        }
    }
}

impl HostConfig {
    pub fn parse(
        claude_exe: &str,
        claude_audit_dir: &str,
        working_dir: &str,
        local_endpoint: &str,
    ) -> Result<Self, HostConfigError> {
        let absolute = |value: &str, option: &'static str| {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                Ok(path)
            } else {
                Err(HostConfigError::NotAbsolute(option))
            }
        };
        let endpoint = SocketAddr::from_str(local_endpoint)
            .map_err(|_| HostConfigError::EndpointUnparsable)?;
        if !endpoint.ip().is_loopback() {
            return Err(HostConfigError::EndpointNotLoopback);
        }
        if endpoint.port() == 0 {
            return Err(HostConfigError::EndpointPortZero);
        }
        Ok(Self {
            claude_exe: absolute(claude_exe, "--claude-exe")?,
            claude_audit_dir: absolute(claude_audit_dir, "--claude-audit-dir")?,
            working_dir: absolute(working_dir, "--working-dir")?,
            local_endpoint: endpoint,
        })
    }

    /// What can be found wrong from metadata alone: the executable is not a
    /// file, or the working directory is not a directory. Runs nothing, opens
    /// nothing, creates nothing.
    pub fn local_problem(&self) -> Option<String> {
        if !is_file(&self.claude_exe) {
            return Some(format!(
                "--claude-exe `{}` is not an existing file",
                self.claude_exe.display()
            ));
        }
        match std::fs::metadata(&self.working_dir) {
            Ok(m) if m.is_dir() => None,
            _ => Some(format!(
                "--working-dir `{}` is not an existing directory",
                self.working_dir.display()
            )),
        }
    }
}

fn is_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    const ABS: &str = "C:\\tools\\claude.exe";
    #[cfg(not(windows))]
    const ABS: &str = "/opt/claude";

    fn parse(endpoint: &str) -> Result<HostConfig, HostConfigError> {
        HostConfig::parse(ABS, ABS, ABS, endpoint)
    }

    #[test]
    fn a_literal_loopback_endpoint_parses() {
        for ok in ["127.0.0.1:11434", "127.0.0.2:1", "[::1]:11434"] {
            let cfg = parse(ok).unwrap_or_else(|e| panic!("{ok}: {e}"));
            assert!(cfg.local_endpoint.ip().is_loopback());
        }
    }

    #[test]
    fn non_loopback_endpoints_are_rejected_with_no_override() {
        for bad in [
            "0.0.0.0:11434",
            "192.168.1.10:11434",
            "10.0.0.1:11434",
            "8.8.8.8:53",
            "[::]:11434",
            "[2001:db8::1]:11434",
            "[::ffff:127.0.0.1]:11434",
        ] {
            assert_eq!(
                parse(bad),
                Err(HostConfigError::EndpointNotLoopback),
                "{bad} must be refused"
            );
        }
    }

    #[test]
    fn names_and_malformed_endpoints_are_rejected_not_resolved() {
        for bad in [
            "localhost:11434",
            "127.0.0.1",
            ":11434",
            "",
            "http://127.0.0.1:1",
            "x:y",
        ] {
            assert_eq!(
                parse(bad),
                Err(HostConfigError::EndpointUnparsable),
                "{bad:?}"
            );
        }
        assert_eq!(parse("127.0.0.1:0"), Err(HostConfigError::EndpointPortZero));
    }

    #[test]
    fn every_path_must_be_absolute_so_nothing_is_searched_for() {
        for (exe, audit, wd, option) in [
            ("claude", ABS, ABS, "--claude-exe"),
            (ABS, "audit", ABS, "--claude-audit-dir"),
            (ABS, ABS, "work", "--working-dir"),
            (ABS, ABS, "", "--working-dir"),
            (ABS, ABS, ".", "--working-dir"),
        ] {
            assert_eq!(
                HostConfig::parse(exe, audit, wd, "127.0.0.1:11434"),
                Err(HostConfigError::NotAbsolute(option)),
                "{option}"
            );
        }
    }

    #[test]
    fn local_problem_reports_a_missing_executable_and_directory_without_creating_anything() {
        let root = std::env::temp_dir().join(format!("maia-invoke-host-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut cfg = parse("127.0.0.1:11434").unwrap();
        cfg.claude_exe = root.join("claude.exe");
        cfg.working_dir = root.clone();
        assert!(cfg.local_problem().unwrap().contains("--claude-exe"));
        assert!(!root.exists(), "checking creates nothing");

        std::fs::create_dir_all(&root).unwrap();
        // A directory is not an executable.
        cfg.claude_exe = root.clone();
        assert!(cfg.local_problem().unwrap().contains("--claude-exe"));
        cfg.claude_exe = root.join("claude.exe");
        std::fs::write(&cfg.claude_exe, "x").unwrap();
        assert_eq!(cfg.local_problem(), None);
        cfg.working_dir = root.join("nowhere");
        assert!(cfg.local_problem().unwrap().contains("--working-dir"));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
