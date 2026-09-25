//! Fixed, offline Tier 1 Cargo verification for the single M0.16.3 surface.
//! Candidate data cannot select executables, packages, arguments, or cwd.
use maia_evolution_supervisor::mutation::{
    CheckEvidence, CheckKind, CheckOutcome, EVOLVABLE_PATHS, PortFailure, Tier1Evidence,
    Tier1Outcome,
};
use maia_evolution_supervisor::workspace::Identity;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const PACKAGE: &str = "maia-local-intelligence-host";
const COMMAND_BOUND: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

#[cfg(windows)]
fn local_msvc_paths() -> (Vec<std::path::PathBuf>, Vec<std::path::PathBuf>) {
    use std::path::PathBuf;
    let Some(program_files) = std::env::var_os("ProgramFiles(x86)") else {
        return (Vec::new(), Vec::new());
    };
    let root = PathBuf::from(program_files);
    let mut bins = Vec::new();
    let mut libs = Vec::new();
    let visual_studio = root.join("Microsoft Visual Studio");
    if let Ok(years) = std::fs::read_dir(visual_studio) {
        for year in years.flatten() {
            if let Ok(editions) = std::fs::read_dir(year.path()) {
                for edition in editions.flatten() {
                    let msvc = edition.path().join("VC/Tools/MSVC");
                    if let Ok(versions) = std::fs::read_dir(msvc) {
                        for version in versions.flatten() {
                            let base = version.path();
                            let bin = base.join("bin/Hostx64/x64");
                            let lib = base.join("lib/x64");
                            if bin.join("link.exe").is_file() && lib.is_dir() {
                                bins.push(bin);
                                libs.push(lib);
                            }
                        }
                    }
                }
            }
        }
    }
    let sdk = root.join("Windows Kits/10/Lib");
    if let Ok(versions) = std::fs::read_dir(sdk) {
        let mut version_paths: Vec<_> = versions.flatten().map(|entry| entry.path()).collect();
        version_paths.sort();
        for version in version_paths.into_iter().rev() {
            for component in ["ucrt/x64", "um/x64"] {
                let lib = version.join(component);
                if lib.is_dir() {
                    libs.push(lib);
                }
            }
        }
    }
    bins.sort();
    bins.reverse();
    (bins, libs)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandResult {
    Passed,
    Failed,
    Infrastructure,
}

fn command_env(command: &mut Command, target: &Path) {
    command.env_clear();
    for key in [
        "PATH",
        "USERPROFILE",
        "SYSTEMROOT",
        "WINDIR",
        "TEMP",
        "TMP",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "RUSTUP_TOOLCHAIN",
        "HOME",
        "HOMEDRIVE",
        "HOMEPATH",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    #[cfg(windows)]
    {
        let (bins, libs) = local_msvc_paths();
        if !bins.is_empty() {
            let mut paths = bins;
            if let Some(original) = std::env::var_os("PATH") {
                paths.extend(std::env::split_paths(&original));
            }
            if let Ok(path) = std::env::join_paths(paths) {
                command.env("PATH", path);
            }
        }
        if !libs.is_empty()
            && let Ok(path) = std::env::join_paths(libs)
        {
            command.env("LIB", path);
        }
    }
    command
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TERM_COLOR", "never")
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_TARGET_DIR", target)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
}

fn linker_available() -> bool {
    #[cfg(windows)]
    {
        if !local_msvc_paths().0.is_empty() {
            return true;
        }
        std::env::var_os("PATH").is_some_and(|path| {
            std::env::split_paths(&path).any(|dir| dir.join("link.exe").is_file())
        })
    }
    #[cfg(not(windows))]
    {
        true
    }
}

fn run(root: &Path, target: &Path, arguments: &[&str]) -> CommandResult {
    let mut command = Command::new("cargo");
    command.current_dir(root).args(arguments);
    command_env(&mut command, target);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => return CommandResult::Infrastructure,
    };
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    CommandResult::Passed
                } else {
                    CommandResult::Failed
                };
            }
            Ok(None) if started.elapsed() < COMMAND_BOUND => {
                std::thread::sleep(POLL_INTERVAL);
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return CommandResult::Infrastructure;
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return CommandResult::Infrastructure;
            }
        }
    }
}

fn run_rustfmt(root: &Path, target: &Path) -> CommandResult {
    let source = root.join(EVOLVABLE_PATHS[0]);
    let mut command = Command::new("rustfmt");
    command
        .current_dir(root)
        .arg("--check")
        .arg("--edition")
        .arg("2024")
        .arg(source);
    command_env(&mut command, target);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => return CommandResult::Infrastructure,
    };
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    CommandResult::Passed
                } else {
                    CommandResult::Failed
                };
            }
            Ok(None) if started.elapsed() < COMMAND_BOUND => {
                std::thread::sleep(POLL_INTERVAL);
            }
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return CommandResult::Infrastructure;
            }
        }
    }
}

pub trait Tier1Verifier {
    fn verify(
        &mut self,
        identity: &Identity,
        candidate_workspace: &Path,
        changed_path: &str,
    ) -> Result<Tier1Evidence, PortFailure>;
}

/// The verifier has a fixed package, command list, offline mode, isolated
/// candidate target directory, null output, and a per-command wall bound.
/// It cannot run candidate-supplied commands or access credentials through
/// inherited environment variables outside the explicit toolchain allowlist.
pub struct FixedCargoTier1Verifier;

impl Tier1Verifier for FixedCargoTier1Verifier {
    fn verify(
        &mut self,
        identity: &Identity,
        candidate_workspace: &Path,
        changed_path: &str,
    ) -> Result<Tier1Evidence, PortFailure> {
        if !linker_available() {
            return Err(PortFailure::Infrastructure);
        }
        if identity
            .proposed_paths
            .iter()
            .filter(|path| path.as_str() == changed_path)
            .count()
            != 1
        {
            return Err(PortFailure::Rejected);
        }
        let root =
            std::fs::canonicalize(candidate_workspace).map_err(|_| PortFailure::Infrastructure)?;
        let target = root.join("target/m0163-tier1");
        let checks = [
            (
                CheckKind::SyntaxStatic,
                run_rustfmt(&root, &target),
                "rustfmt-fixed-file",
            ),
            (
                CheckKind::FormattingLint,
                run(
                    &root,
                    &target,
                    &[
                        "clippy",
                        "-p",
                        PACKAGE,
                        "--all-targets",
                        "--all-features",
                        "--locked",
                        "--offline",
                        "--",
                        "-D",
                        "warnings",
                    ],
                ),
                "cargo-clippy",
            ),
            (
                CheckKind::ComponentBuild,
                run(
                    &root,
                    &target,
                    &[
                        "check",
                        "-p",
                        PACKAGE,
                        "--all-targets",
                        "--all-features",
                        "--locked",
                        "--offline",
                    ],
                ),
                "cargo-check",
            ),
            (
                CheckKind::TargetedTests,
                run(
                    &root,
                    &target,
                    &[
                        "test",
                        "-p",
                        PACKAGE,
                        "--locked",
                        "--offline",
                        "--no-fail-fast",
                    ],
                ),
                "cargo-test",
            ),
        ];
        let mut outcome = Tier1Outcome::Passed;
        let mut evidence = vec![CheckEvidence {
            check: CheckKind::CandidateIdentity,
            outcome: CheckOutcome::Passed,
            evidence_ref: format!("candidate-workspace:{}", identity.workspace_id),
        }];
        for (check, result, label) in checks {
            let check_outcome = match result {
                CommandResult::Passed => CheckOutcome::Passed,
                CommandResult::Failed => {
                    outcome = Tier1Outcome::Failed;
                    CheckOutcome::Failed
                }
                CommandResult::Infrastructure => {
                    outcome = Tier1Outcome::InfraError;
                    CheckOutcome::InfraError
                }
            };
            evidence.push(CheckEvidence {
                check,
                outcome: check_outcome,
                evidence_ref: format!("{label}:fixed-command-result"),
            });
        }
        // Git cleanliness/protected-surface integrity is checked after this
        // verifier returns, by the owning workspace adapter.
        Ok(Tier1Evidence {
            outcome,
            verifier_identity: "fixed-cargo-tier1-v1".into(),
            checks: evidence,
        })
    }
}
