//! Fixed, offline Tier 1 Cargo verification for the single M0.16.3 surface.
//! Candidate data cannot select executables, packages, arguments, or cwd.
use maia_evolution_process_host::{Completion, VerifierCommand, run_contained};
use maia_evolution_supervisor::mutation::{
    CheckEvidence, CheckKind, CheckOutcome, PortFailure, Tier1Evidence, Tier1Outcome,
};
use maia_evolution_supervisor::workspace::Identity;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

const COMMAND_BOUND: Duration = Duration::from_secs(120);

#[cfg(windows)]
fn local_msvc_paths() -> (Vec<std::path::PathBuf>, Vec<std::path::PathBuf>) {
    use std::path::PathBuf;
    let mut bins = Vec::new();
    let mut libs = Vec::new();
    let roots: Vec<_> = ["ProgramFiles", "ProgramFiles(x86)"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .collect();
    for root in roots {
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
    }
    bins.sort();
    bins.reverse();
    bins.dedup();
    libs.sort();
    libs.dedup();
    (bins, libs)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandResult {
    Passed,
    Failed(i32),
    ResourceLimit,
    Cancelled,
    Infrastructure,
}

fn command_env(command: &mut Command, target: &Path, _toolchain_bin: &Path) {
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
        "LIB",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    #[cfg(windows)]
    {
        let (bins, libs) = local_msvc_paths();
        let mut paths = bins;
        paths.insert(0, _toolchain_bin.to_owned());
        if let Some(original) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&original));
        }
        if let Ok(path) = std::env::join_paths(paths) {
            command.env("PATH", path);
        }
        command.env("RUSTC", _toolchain_bin.join("rustc.exe"));
        command.env("RUSTDOC", _toolchain_bin.join("rustdoc.exe"));
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

fn fixed_environment(
    target: &Path,
    toolchain_bin: &Path,
) -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
    let mut command = Command::new("cargo");
    command_env(&mut command, target, toolchain_bin);
    command
        .get_envs()
        .filter_map(|(key, value)| value.map(|value| (key.to_owned(), value.to_owned())))
        .collect::<Vec<_>>()
}

fn toolchain_executable(name: &str) -> Result<std::path::PathBuf, CommandResult> {
    let path = std::env::var_os("PATH").ok_or(CommandResult::Infrastructure)?;
    let rustup = std::env::split_paths(&path)
        .map(|directory| directory.join("rustup.exe"))
        .find(|candidate| candidate.is_file())
        .ok_or(CommandResult::Infrastructure)?;
    let output = Command::new(rustup)
        .args(["which", name])
        .output()
        .map_err(|_| CommandResult::Infrastructure)?;
    if !output.status.success() {
        return Err(CommandResult::Infrastructure);
    }
    let path = std::str::from_utf8(&output.stdout)
        .map_err(|_| CommandResult::Infrastructure)?
        .trim();
    let path = Path::new(path);
    if !path.is_absolute() || !path.is_file() {
        return Err(CommandResult::Infrastructure);
    }
    std::fs::canonicalize(path).map_err(|_| CommandResult::Infrastructure)
}

fn run(
    root: &Path,
    target: &Path,
    verifier_command: VerifierCommand,
    cancelled: &mut dyn FnMut() -> bool,
) -> CommandResult {
    let cargo = match toolchain_executable("cargo") {
        Ok(program) => program,
        Err(result) => return result,
    };
    let toolchain_bin = cargo.parent().ok_or(CommandResult::Infrastructure);
    let Ok(toolchain_bin) = toolchain_bin else {
        return CommandResult::Infrastructure;
    };
    match run_contained(
        verifier_command,
        toolchain_bin,
        root,
        target,
        &fixed_environment(target, toolchain_bin),
        COMMAND_BOUND,
        cancelled,
    ) {
        Ok(Completion::Exited(0)) => CommandResult::Passed,
        Ok(Completion::Exited(code)) => CommandResult::Failed(code),
        Ok(Completion::ResourceLimit) => CommandResult::ResourceLimit,
        Ok(Completion::TimedOut) => CommandResult::ResourceLimit,
        Ok(Completion::Cancelled) => CommandResult::Cancelled,
        Err(_) => CommandResult::Infrastructure,
    }
}

fn run_rustfmt(root: &Path, target: &Path, cancelled: &mut dyn FnMut() -> bool) -> CommandResult {
    let cargo = match toolchain_executable("cargo") {
        Ok(program) => program,
        Err(result) => return result,
    };
    let toolchain_bin = cargo.parent().ok_or(CommandResult::Infrastructure);
    let Ok(toolchain_bin) = toolchain_bin else {
        return CommandResult::Infrastructure;
    };
    match run_contained(
        VerifierCommand::RustfmtAllowlistedSource,
        toolchain_bin,
        root,
        target,
        &fixed_environment(target, toolchain_bin),
        COMMAND_BOUND,
        cancelled,
    ) {
        Ok(Completion::Exited(0)) => CommandResult::Passed,
        Ok(Completion::Exited(code)) => CommandResult::Failed(code),
        Ok(Completion::ResourceLimit) => CommandResult::ResourceLimit,
        Ok(Completion::TimedOut) => CommandResult::ResourceLimit,
        Ok(Completion::Cancelled) => CommandResult::Cancelled,
        Err(_) => CommandResult::Infrastructure,
    }
}

pub trait Tier1Verifier {
    fn verify(
        &mut self,
        identity: &Identity,
        candidate_workspace: &Path,
        changed_path: &str,
    ) -> Result<Tier1Evidence, PortFailure>;

    fn verify_cancellable(
        &mut self,
        identity: &Identity,
        candidate_workspace: &Path,
        changed_path: &str,
        cancelled: &mut dyn FnMut() -> bool,
    ) -> Result<Tier1Evidence, PortFailure> {
        if cancelled() {
            return Err(PortFailure::Cancelled);
        }
        self.verify(identity, candidate_workspace, changed_path)
    }
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
        self.verify_cancellable(identity, candidate_workspace, changed_path, &mut || false)
    }

    fn verify_cancellable(
        &mut self,
        identity: &Identity,
        candidate_workspace: &Path,
        changed_path: &str,
        cancelled: &mut dyn FnMut() -> bool,
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
            (CheckKind::SyntaxStatic, None, "rustfmt-fixed-file"),
            (
                CheckKind::FormattingLint,
                Some(VerifierCommand::CargoClippyHostComponent),
                "cargo-clippy",
            ),
            (
                CheckKind::ComponentBuild,
                Some(VerifierCommand::CargoCheckHostComponent),
                "cargo-check",
            ),
            (
                CheckKind::TargetedTests,
                Some(VerifierCommand::CargoTestHostComponent),
                "cargo-test",
            ),
        ];
        let mut outcome = Tier1Outcome::Passed;
        let mut evidence = vec![CheckEvidence {
            check: CheckKind::CandidateIdentity,
            outcome: CheckOutcome::Passed,
            evidence_ref: format!("candidate-workspace:{}", identity.workspace_id),
        }];
        for (check, command, label) in checks {
            let result = if cancelled() {
                CommandResult::Cancelled
            } else if let Some(command) = command {
                run(&root, &target, command, cancelled)
            } else {
                run_rustfmt(&root, &target, cancelled)
            };
            let check_outcome = match result {
                CommandResult::Passed => CheckOutcome::Passed,
                CommandResult::Failed(_) => {
                    outcome = Tier1Outcome::Failed;
                    CheckOutcome::Failed
                }
                CommandResult::ResourceLimit => {
                    outcome = Tier1Outcome::ResourceLimit;
                    CheckOutcome::ResourceLimit
                }
                CommandResult::Cancelled => {
                    outcome = Tier1Outcome::Cancelled;
                    CheckOutcome::Cancelled
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
            if matches!(
                result,
                CommandResult::ResourceLimit
                    | CommandResult::Cancelled
                    | CommandResult::Infrastructure
            ) {
                break;
            }
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
