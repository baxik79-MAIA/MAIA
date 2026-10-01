//! Fixed, offline Tier 1 Cargo verification for the single M0.16.3 surface.
//! Candidate data cannot select executables, packages, arguments, or cwd.
use maia_evolution_process_host::{
    CapturedRun, Completion, VerifierCommand, run_restricted_captured,
};
use maia_evolution_supervisor::mutation::{
    CheckEvidence, CheckKind, CheckOutcome, PortFailure, Tier1Evidence, Tier1Outcome,
};
use maia_evolution_supervisor::workspace::Identity;
use sha2::{Digest, Sha256};
#[cfg(windows)]
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

const COMMAND_BOUND: Duration = Duration::from_secs(900);

#[derive(Clone)]
struct HostResources {
    root: PathBuf,
    toolchain: PathBuf,
    toolchain_bin: PathBuf,
    vendor: PathBuf,
    msvc: PathBuf,
    msvc_bin: PathBuf,
    msvc_lib: PathBuf,
    sdk: PathBuf,
    sdk_um: PathBuf,
    sdk_ucrt: PathBuf,
    msvc_include: PathBuf,
    sdk_include_ucrt: PathBuf,
    sdk_include_um: PathBuf,
    sdk_include_shared: PathBuf,
    clippy_bundle: PathBuf,
    rustfmt_runtime: PathBuf,
    rustfmt_exe: PathBuf,
    run_state_root: PathBuf,
}

impl HostResources {
    fn load() -> Result<Self, CommandResult> {
        #[cfg(not(windows))]
        {
            return Err(CommandResult::Infrastructure);
        }
        #[cfg(windows)]
        {
            let local_app_data = std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var_os("USERPROFILE")
                        .map(|home| PathBuf::from(home).join("AppData/Local"))
                })
                .ok_or(CommandResult::Infrastructure)?;
            if !local_app_data.is_absolute() || contains_reparse_point(&local_app_data) {
                return Err(CommandResult::Infrastructure);
            }
            let host_config = local_app_data.join("MAIA/RestrictedVerifierHost/resources-root.txt");
            let config_meta = std::fs::symlink_metadata(&host_config)
                .map_err(|_| CommandResult::Infrastructure)?;
            if !config_meta.is_file() || config_meta.file_type().is_symlink() {
                return Err(CommandResult::Infrastructure);
            }
            let configured =
                std::fs::read_to_string(&host_config).map_err(|_| CommandResult::Infrastructure)?;
            let root_text = configured.trim();
            let configured_root = PathBuf::from(root_text);
            if !configured_root.is_absolute() || contains_reparse_point(&configured_root) {
                return Err(CommandResult::Infrastructure);
            }
            let canonical_root = std::fs::canonicalize(&configured_root)
                .map_err(|_| CommandResult::Infrastructure)?;
            if normalized_windows_path(&canonical_root) != normalized_windows_path(&configured_root)
            {
                return Err(CommandResult::Infrastructure);
            }
            #[cfg(test)]
            eprintln!("M0.16.5 host preflight: root passed");
            let root = configured_root;

            let toolchain = root.join("toolchains/stable-x86_64-pc-windows-msvc");
            let toolchain_bin = toolchain.join("bin");
            let vendor = root.join("vendor/m0165-r5v-generated-20260928");
            let msvc = root.join("msvc");
            let msvc_bin = msvc.join("bin/Hostx64/x64");
            let msvc_lib = msvc.join("lib/x64");
            let sdk = root.join("windows-sdk");
            let sdk_um = sdk.join("um/x64");
            let sdk_ucrt = sdk.join("ucrt/x64");
            let msvc_include = msvc.join("include");
            let sdk_include_ucrt = sdk.join("include/ucrt");
            let sdk_include_um = sdk.join("include/um");
            let sdk_include_shared = sdk.join("include/shared");
            let clippy_bundle =
                local_app_data.join("MAIA/RestrictedVerifierHost/resources/clippy-r5");
            let rustfmt_runtime =
                local_app_data.join("MAIA/RestrictedVerifierHost/resources/rustfmt-r5");
            let rustfmt_exe = rustfmt_runtime.join("rustfmt-r5-compat.exe");
            let run_state_root = root.join("run-state");

            for directory in [
                &toolchain,
                &toolchain_bin,
                &vendor,
                &msvc,
                &msvc_bin,
                &msvc_lib,
                &sdk,
                &sdk_um,
                &sdk_ucrt,
                &msvc_include,
                &sdk_include_ucrt,
                &sdk_include_um,
                &sdk_include_shared,
                &clippy_bundle,
                &rustfmt_runtime,
            ] {
                if !directory.is_dir() || contains_reparse_point(directory) {
                    return Err(CommandResult::Infrastructure);
                }
            }
            for file in [
                toolchain_bin.join("cargo.exe"),
                toolchain_bin.join("rustc.exe"),
                toolchain_bin.join("rustdoc.exe"),
                toolchain_bin.join("rustc_driver-573e106f78c6e3e0.dll"),
                toolchain_bin.join("std-44a584f44bc3dd65.dll"),
                toolchain_bin.join("rustfmt.exe"),
                msvc_bin.join("cl.exe"),
                msvc_bin.join("link.exe"),
                msvc_include.join("vcruntime.h"),
                sdk_include_ucrt.join("stdlib.h"),
                sdk_include_ucrt.join("stdio.h"),
                sdk_include_um.join("Windows.h"),
                sdk_include_shared.join("winapifamily.h"),
                vendor.join("ab_glyph/Cargo.toml"),
                vendor.join("ab_glyph/.cargo-checksum.json"),
            ] {
                if !file.is_file() || contains_reparse_point(&file) {
                    return Err(CommandResult::Infrastructure);
                }
            }
            if sha256_file(&toolchain_bin.join("cargo.exe"))?
                != "C37545EC61D48D31BDEFCE53280ECAB61C4EA54EAACE372FAC6A0316C6E165D9"
                || sha256_file(&rustfmt_exe)?
                    != "9F1CCDA00D9CC6321D529853ABC77733DF375C0087CE331AD27CD07E8BF69BFF"
                || sha256_file(&toolchain_bin.join("rustc_driver-573e106f78c6e3e0.dll"))?
                    != "50B9168C1D7BF98A7C4417EED0C8098DAAB7033B13EBB0BCA752945708EA9EC7"
                || sha256_file(&toolchain_bin.join("std-44a584f44bc3dd65.dll"))?
                    != "8DD8EA258A3561A499C438B322B218AF3CB2F4DB1029205ADC94D58FD5B0615C"
            {
                return Err(CommandResult::Infrastructure);
            }
            #[cfg(test)]
            eprintln!("M0.16.5 host preflight: base paths passed");
            let clippy_manifest_path = clippy_bundle.join("manifest.json");
            let clippy_manifest_bytes =
                std::fs::read(&clippy_manifest_path).map_err(|_| CommandResult::Infrastructure)?;
            let manifest: serde_json::Value = serde_json::from_slice(&clippy_manifest_bytes)
                .map_err(|_| CommandResult::Infrastructure)?;
            if manifest["purpose"].as_str() != Some("M0.16.5 proof-only Clippy path compatibility")
                || manifest["rust_source_commit"].as_str()
                    != Some("48a229ceaefd4985c50990b14116b6d856af0985")
                || manifest["config_source_sha256"].as_str()
                    != Some("DB8FC462B4542BBBB3C0429052C0E7A16A0A329CC6E7CFB011A6C6A5B23A9EA4")
                || manifest["patch_sha256"].as_str()
                    != Some("E42793BCE501923C2FD1FB964AF7F661AA181C5A9AEBE6629DFA56B2253159B9")
            {
                return Err(CommandResult::Infrastructure);
            }
            let files = manifest["files"]
                .as_array()
                .ok_or(CommandResult::Infrastructure)?;
            for entry in files {
                let name = entry["name"]
                    .as_str()
                    .ok_or(CommandResult::Infrastructure)?;
                let expected = entry["sha256"]
                    .as_str()
                    .ok_or(CommandResult::Infrastructure)?;
                let pinned = match name {
                    "cargo-clippy.exe" => {
                        "96A96C77099C22A1CF306C74626C7EC81EFD3A639B13AB485840063ECC22BCEB"
                    }
                    "clippy-driver.exe" => {
                        "A37DAF159E0CDC9B7A71875A4BD55904B6869E93D595EF155A2AB88734483CBC"
                    }
                    "compat.patch" => {
                        "E42793BCE501923C2FD1FB964AF7F661AA181C5A9AEBE6629DFA56B2253159B9"
                    }
                    "rustc_driver-573e106f78c6e3e0.dll" => {
                        "50B9168C1D7BF98A7C4417EED0C8098DAAB7033B13EBB0BCA752945708EA9EC7"
                    }
                    "std-44a584f44bc3dd65.dll" => {
                        "8DD8EA258A3561A499C438B322B218AF3CB2F4DB1029205ADC94D58FD5B0615C"
                    }
                    _ => return Err(CommandResult::Infrastructure),
                };
                if expected != pinned {
                    return Err(CommandResult::Infrastructure);
                }
                let path = clippy_bundle.join(name);
                if !path.is_file()
                    || contains_reparse_point(&path)
                    || sha256_file(&path)? != expected
                {
                    return Err(CommandResult::Infrastructure);
                }
            }

            #[cfg(test)]
            eprintln!("M0.16.5 host preflight: clippy pins passed");
            let packages = std::fs::read_dir(&vendor)
                .map_err(|_| CommandResult::Infrastructure)?
                .filter_map(Result::ok)
                .filter(|entry| entry.path().is_dir())
                .count();
            let checksums = std::fs::read_dir(&vendor)
                .map_err(|_| CommandResult::Infrastructure)?
                .filter_map(Result::ok)
                .filter(|entry| entry.path().is_dir())
                .try_fold(0usize, |count, entry| {
                    let count = count
                        + std::fs::read_dir(entry.path())
                            .map_err(|_| CommandResult::Infrastructure)?
                            .filter_map(Result::ok)
                            .filter(|child| child.file_name() == ".cargo-checksum.json")
                            .count();
                    Ok::<usize, CommandResult>(count)
                })?;
            if packages != 388 || checksums != 388 {
                return Err(CommandResult::Infrastructure);
            }
            #[cfg(test)]
            eprintln!("M0.16.5 host preflight: vendor counts passed");
            Ok(Self {
                root,
                toolchain,
                toolchain_bin,
                vendor,
                msvc,
                msvc_bin,
                msvc_lib,
                sdk,
                sdk_um,
                sdk_ucrt,
                msvc_include,
                sdk_include_ucrt,
                sdk_include_um,
                sdk_include_shared,
                clippy_bundle,
                rustfmt_runtime,
                rustfmt_exe,
                run_state_root,
            })
        }
    }

    fn readonly_roots(&self) -> Vec<PathBuf> {
        vec![
            self.toolchain.clone(),
            self.msvc.clone(),
            self.sdk.clone(),
            self.vendor.clone(),
            self.clippy_bundle.clone(),
            self.rustfmt_runtime.clone(),
        ]
    }
}

fn normalized_windows_path(path: &Path) -> String {
    let text = path.to_string_lossy().replace('/', "\\");
    let text = text
        .strip_prefix(r"\\?\UNC\")
        .map(|tail| format!(r"\\{tail}"))
        .or_else(|| text.strip_prefix(r"\\?\").map(str::to_owned))
        .unwrap_or(text);
    text.trim_end_matches('\\').to_ascii_lowercase()
}

fn contains_reparse_point(path: &Path) -> bool {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        return true;
    };
    let mut current = PathBuf::new();
    for component in absolute.components() {
        current.push(component.as_os_str());
        if let Ok(metadata) = std::fs::symlink_metadata(&current) {
            if metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0 {
                return true;
            }
        }
    }
    false
}

fn sha256_file(path: &Path) -> Result<String, CommandResult> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|_| CommandResult::Infrastructure)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| CommandResult::Infrastructure)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:X}", digest.finalize()))
}

pub(crate) fn host_isolation_ready() -> bool {
    let Ok(resources) = HostResources::load() else {
        #[cfg(test)]
        eprintln!("M0.16.5 host preflight: resource validation failed");
        return false;
    };
    #[cfg(test)]
    eprintln!("M0.16.5 host preflight: resource validation passed");
    if std::fs::create_dir_all(&resources.run_state_root).is_err() {
        #[cfg(test)]
        eprintln!("M0.16.5 host preflight: run-state creation failed");
        return false;
    }
    let profile = maia_evolution_process_host::appcontainer_profile_available();
    #[cfg(test)]
    eprintln!("M0.16.5 host preflight: profile create/delete={profile}");
    if !profile {
        return false;
    }
    let acl = maia_evolution_process_host::appcontainer_filesystem_acl_available(
        &resources.run_state_root,
    );
    #[cfg(test)]
    eprintln!("M0.16.5 host preflight: reversible ACL probe={acl}");
    acl
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandResult {
    Passed,
    Failed(i32),
    ResourceLimit,
    Cancelled,
    Infrastructure,
}

struct ConfigSnapshot {
    directory: PathBuf,
    rustfmt_config: PathBuf,
    clippy_evidence: String,
    rustfmt_evidence: String,
}

impl ConfigSnapshot {
    fn create(resources: &HostResources, workspace: &Path) -> Result<Self, CommandResult> {
        let nonce = format!(
            "{}-{:x}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| CommandResult::Infrastructure)?
                .as_nanos()
        );
        let directory = resources.run_state_root.join(nonce);
        std::fs::create_dir_all(&directory).map_err(|_| CommandResult::Infrastructure)?;
        let result = (|| {
            let package = workspace.join("apps/local-intelligence-host");
            let source = package.join("src/lib.rs");
            let clippy = copy_effective_config(
                &package,
                workspace,
                &["clippy.toml", ".clippy.toml"],
                &directory,
                "clippy.toml",
            )?;
            let rustfmt = copy_effective_config(
                source.parent().ok_or(CommandResult::Infrastructure)?,
                workspace,
                &["rustfmt.toml", ".rustfmt.toml"],
                &directory,
                "rustfmt.toml",
            )?;
            if rustfmt.is_none() {
                // Isolated host snapshot of rustfmt defaults only. Candidate
                // configuration files are detected first and are never edited.
                std::fs::write(directory.join("rustfmt.toml"), [])
                    .map_err(|_| CommandResult::Infrastructure)?;
            }
            let clippy_evidence = clippy.map_or_else(
                || "absent-defaults".to_owned(),
                |hash| format!("sha256:{hash}"),
            );
            let rustfmt_evidence = rustfmt.map_or_else(
                || "absent-defaults".to_owned(),
                |hash| format!("sha256:{hash}"),
            );
            Ok(Self {
                directory: directory.clone(),
                rustfmt_config: directory.join("rustfmt.toml"),
                clippy_evidence,
                rustfmt_evidence,
            })
        })();
        if result.is_err() {
            let _ = std::fs::remove_dir_all(&directory);
        }
        result
    }

    fn remove(self) -> Result<(), CommandResult> {
        std::fs::remove_dir_all(self.directory).map_err(|_| CommandResult::Infrastructure)
    }
}

fn copy_effective_config(
    start: &Path,
    workspace: &Path,
    names: &[&str],
    destination: &Path,
    output: &str,
) -> Result<Option<String>, CommandResult> {
    let mut current = start.to_owned();
    loop {
        let mut found = Vec::new();
        for name in names {
            let candidate = current.join(name);
            match std::fs::symlink_metadata(&candidate) {
                Ok(meta)
                    if meta.is_file()
                        && !meta.file_type().is_symlink()
                        && !contains_reparse_point(&candidate) =>
                {
                    found.push(candidate)
                }
                Ok(_) => return Err(CommandResult::Infrastructure),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(CommandResult::Infrastructure),
            }
        }
        if found.len() > 1 {
            return Err(CommandResult::Infrastructure);
        }
        if let Some(source) = found.first() {
            let copied = destination.join(output);
            std::fs::copy(source, &copied).map_err(|_| CommandResult::Infrastructure)?;
            return Ok(Some(sha256_file(&copied)?));
        }
        if current == workspace {
            return Ok(None);
        }
        let parent = current.parent().ok_or(CommandResult::Infrastructure)?;
        if !parent.starts_with(workspace) {
            return Err(CommandResult::Infrastructure);
        }
        current = parent.to_owned();
    }
}

fn fixed_environment(
    resources: &HostResources,
    target: &Path,
    config: &ConfigSnapshot,
) -> Result<Vec<(std::ffi::OsString, std::ffi::OsString)>, CommandResult> {
    let cargo_home = target.join("cargo-home");
    let temp = target.join("tmp");
    let home = target.join("home");
    for path in [&cargo_home, &temp, &home] {
        std::fs::create_dir_all(path).map_err(|_| CommandResult::Infrastructure)?;
    }
    let cargo_config = format!(
        "[source.crates-io]\nreplace-with = \"m0165-host-vendor\"\n\n[source.m0165-host-vendor]\ndirectory = {:?}\n",
        resources.vendor.to_string_lossy()
    );
    std::fs::write(cargo_home.join("config.toml"), cargo_config)
        .map_err(|_| CommandResult::Infrastructure)?;
    let system_root = std::env::var_os("SystemRoot")
        .or_else(|| std::env::var_os("WINDIR"))
        .ok_or(CommandResult::Infrastructure)?;
    let system32 = PathBuf::from(&system_root).join("System32");
    let path = std::env::join_paths([
        resources.clippy_bundle.as_path(),
        resources.rustfmt_runtime.as_path(),
        resources.toolchain_bin.as_path(),
        resources.msvc_bin.as_path(),
        system32.as_path(),
    ])
    .map_err(|_| CommandResult::Infrastructure)?;
    let include = std::env::join_paths([
        resources.msvc_include.as_path(),
        resources.sdk_include_ucrt.as_path(),
        resources.sdk_include_um.as_path(),
        resources.sdk_include_shared.as_path(),
    ])
    .map_err(|_| CommandResult::Infrastructure)?;
    let lib = std::env::join_paths([
        resources.msvc_lib.as_path(),
        resources.sdk_um.as_path(),
        resources.sdk_ucrt.as_path(),
    ])
    .map_err(|_| CommandResult::Infrastructure)?;
    let system_drive: String = target.to_string_lossy().chars().take(2).collect();
    let comspec = PathBuf::from(system_root.clone()).join("System32/cmd.exe");
    let values = [
        ("SYSTEMDRIVE", system_drive.into()),
        ("COMSPEC", comspec.into_os_string()),
        ("PATH", path),
        ("SYSTEMROOT", system_root.clone()),
        ("WINDIR", system_root),
        ("TEMP", temp.clone().into_os_string()),
        ("TMP", temp.into_os_string()),
        ("HOME", home.clone().into_os_string()),
        ("USERPROFILE", home.into_os_string()),
        ("CARGO_HOME", cargo_home.into_os_string()),
        ("CARGO_TARGET_DIR", target.as_os_str().to_owned()),
        ("RUSTUP_HOME", resources.root.as_os_str().to_owned()),
        ("RUSTUP_TOOLCHAIN", "stable-x86_64-pc-windows-msvc".into()),
        (
            "RUSTC",
            resources.toolchain_bin.join("rustc.exe").into_os_string(),
        ),
        (
            "RUSTDOC",
            resources.toolchain_bin.join("rustdoc.exe").into_os_string(),
        ),
        ("SYSROOT", resources.toolchain.as_os_str().to_owned()),
        ("CLIPPY_CONF_DIR", config.directory.as_os_str().to_owned()),
        ("INCLUDE", include),
        ("LIB", lib),
        ("CARGO_NET_OFFLINE", "true".into()),
        ("CARGO_TERM_COLOR", "never".into()),
        ("CARGO_INCREMENTAL", "0".into()),
    ];
    let mut environment = values
        .into_iter()
        .map(|(key, value)| (key.into(), value))
        .collect::<Vec<_>>();
    for key in [
        "HOMEDRIVE",
        "HOMEPATH",
        "OS",
        "PROCESSOR_ARCHITECTURE",
        "PROCESSOR_LEVEL",
        "PROCESSOR_REVISION",
        "PROCESSOR_IDENTIFIER",
        "ALLUSERSPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "PUBLIC",
        "PROGRAMDATA",
        "PROGRAMFILES",
        "PROGRAMFILES(X86)",
        "PROGRAMW6432",
        "COMMONPROGRAMFILES",
        "COMMONPROGRAMFILES(X86)",
        "COMMONPROGRAMW6432",
        "COMPUTERNAME",
        "USERNAME",
        "SESSIONNAME",
        "NUMBER_OF_PROCESSORS",
        "PATHEXT",
    ] {
        if let Some(value) = std::env::var_os(key) {
            environment.push((key.into(), value));
        }
    }
    Ok(environment)
}

fn run(
    resources: &HostResources,
    config: &ConfigSnapshot,
    root: &Path,
    target: &Path,
    command: VerifierCommand,
    cancelled: &mut dyn FnMut() -> bool,
) -> CommandResult {
    let env = match fixed_environment(resources, target, config) {
        Ok(env) => env,
        Err(error) => return error,
    };
    let mut readonly = resources.readonly_roots();
    readonly.push(config.directory.clone());
    let label = match command {
        VerifierCommand::CargoClippyHostComponent => "Clippy",
        VerifierCommand::CargoCheckHostComponent => "component build",
        VerifierCommand::CargoTestHostComponent => "targeted tests",
        _ => "fixed verifier",
    };
    eprintln!("Tier-1 {label}: start (offline AppContainer; limit 900s)");
    let started = std::time::Instant::now();
    let result = run_restricted_captured(
        command,
        &resources.toolchain_bin,
        root,
        target,
        &readonly,
        Some(&resources.rustfmt_exe),
        Some(&resources.vendor),
        Some(&config.rustfmt_config),
        &env,
        COMMAND_BOUND,
        cancelled,
    );
    eprintln!(
        "Tier-1 {label}: finished after {:.1}s",
        started.elapsed().as_secs_f32()
    );
    match result {
        Ok(captured) => {
            if persist_diagnostics(resources, config, label, &captured).is_err() {
                return CommandResult::Infrastructure;
            }
            if !captured.cleanup_verified {
                return CommandResult::Infrastructure;
            }
            match captured.completion {
                Completion::Exited(0) => CommandResult::Passed,
                Completion::Exited(code) => CommandResult::Failed(code),
                Completion::ResourceLimit | Completion::TimedOut => CommandResult::ResourceLimit,
                Completion::Cancelled => CommandResult::Cancelled,
            }
        }
        Err(_) => CommandResult::Infrastructure,
    }
}

fn run_rustfmt(
    resources: &HostResources,
    config: &ConfigSnapshot,
    root: &Path,
    target: &Path,
    cancelled: &mut dyn FnMut() -> bool,
) -> CommandResult {
    let env = match fixed_environment(resources, target, config) {
        Ok(env) => env,
        Err(error) => return error,
    };
    let mut readonly = resources.readonly_roots();
    readonly.push(config.directory.clone());
    eprintln!("Tier-1 Rustfmt: start (AppContainer; limit 900s)");
    let started = std::time::Instant::now();
    let result = run_restricted_captured(
        VerifierCommand::RustfmtAllowlistedSource,
        &resources.toolchain_bin,
        root,
        target,
        &readonly,
        Some(&resources.rustfmt_exe),
        Some(&resources.vendor),
        Some(&config.rustfmt_config),
        &env,
        COMMAND_BOUND,
        cancelled,
    );
    eprintln!(
        "Tier-1 Rustfmt: finished after {:.1}s",
        started.elapsed().as_secs_f32()
    );
    match result {
        Ok(captured) => {
            if persist_diagnostics(resources, config, "Rustfmt", &captured).is_err() {
                return CommandResult::Infrastructure;
            }
            if !captured.cleanup_verified {
                return CommandResult::Infrastructure;
            }
            match captured.completion {
                Completion::Exited(0) => CommandResult::Passed,
                Completion::Exited(code) => CommandResult::Failed(code),
                Completion::ResourceLimit | Completion::TimedOut => CommandResult::ResourceLimit,
                Completion::Cancelled => CommandResult::Cancelled,
            }
        }
        Err(_) => CommandResult::Infrastructure,
    }
}

fn persist_diagnostics(
    resources: &HostResources,
    config: &ConfigSnapshot,
    stage: &str,
    run: &CapturedRun,
) -> std::io::Result<()> {
    let run_name = config
        .directory
        .file_name()
        .ok_or_else(|| std::io::Error::other("missing verifier run id"))?;
    let directory = resources.run_state_root.join("diagnostics").join(run_name);
    std::fs::create_dir_all(&directory)?;
    let slug = stage.to_ascii_lowercase().replace(' ', "-");
    std::fs::write(directory.join(format!("{slug}.stdout.bin")), &run.stdout)?;
    std::fs::write(directory.join(format!("{slug}.stderr.bin")), &run.stderr)?;
    let metadata = format!(
        "completion={:?}\ncleanup_verified={}\nstdout_bytes={}\nstderr_bytes={}\nstdout_truncated={}\nstderr_truncated={}\n",
        run.completion,
        run.cleanup_verified,
        run.stdout.len(),
        run.stderr.len(),
        run.stdout_truncated,
        run.stderr_truncated
    );
    std::fs::write(directory.join(format!("{slug}.meta.txt")), metadata)?;
    eprintln!(
        "Tier-1 {stage}: host-captured diagnostics={} (outside candidate ACL)",
        directory.display()
    );
    Ok(())
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
        if contains_reparse_point(&root) {
            return Err(PortFailure::Infrastructure);
        }
        let resources = HostResources::load().map_err(|_| PortFailure::Infrastructure)?;
        if !maia_evolution_process_host::appcontainer_profile_available() {
            return Err(PortFailure::IsolationUnavailable);
        }
        std::fs::create_dir_all(&resources.run_state_root)
            .map_err(|_| PortFailure::Infrastructure)?;
        let config =
            ConfigSnapshot::create(&resources, &root).map_err(|_| PortFailure::Infrastructure)?;
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
                run(&resources, &config, &root, &target, command, cancelled)
            } else {
                run_rustfmt(&resources, &config, &root, &target, cancelled)
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
                evidence_ref: match check {
                    CheckKind::SyntaxStatic => {
                        format!("{label}:compat-rustfmt;config={}", config.rustfmt_evidence)
                    }
                    CheckKind::FormattingLint => {
                        format!("{label}:compat-clippy;config={}", config.clippy_evidence)
                    }
                    _ => format!("{label}:fixed-offline-command"),
                },
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
        // Remove the host-owned snapshot only after all commands have exited
        // and AppContainer ACL restoration has completed.
        config.remove().map_err(|_| PortFailure::Infrastructure)?;
        // Git cleanliness/protected-surface integrity is checked after this
        // verifier returns, by the owning workspace adapter.
        Ok(Tier1Evidence {
            outcome,
            verifier_identity: "fixed-cargo-tier1-v1".into(),
            checks: evidence,
        })
    }
}
