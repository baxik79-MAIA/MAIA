//! Fixed-command Windows containment adapters for the evolution host.
//! Restricted runs use a per-run AppContainer whose only capability is the
//! exact null-stdin resource capability, plus Job Object containment.
use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;
#[cfg(windows)]
mod windows_restricted;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    Exited(i32),
    ResourceLimit,
    TimedOut,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedRun {
    pub completion: Completion,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub cleanup_verified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainmentUnavailable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifierCommand {
    RustfmtAllowlistedSource,
    CargoClippyHostComponent,
    CargoCheckHostComponent,
    CargoTestHostComponent,
}

#[cfg(not(windows))]
pub fn appcontainer_profile_available() -> bool {
    false
}

pub fn appcontainer_profile_available() -> bool {
    windows_restricted::probe_profile_creation()
}

#[cfg(not(windows))]
pub fn appcontainer_filesystem_acl_available(_path: &Path) -> bool {
    false
}

#[cfg(windows)]
pub fn appcontainer_filesystem_acl_available(path: &Path) -> bool {
    windows_restricted::probe_directory_acl(path)
}

#[cfg(not(windows))]
pub fn run_restricted(
    _command: VerifierCommand,
    _toolchain_bin: &Path,
    _working_directory: &Path,
    _target_directory: &Path,
    _readonly_roots: &[std::path::PathBuf],
    _rustfmt_executable: Option<&Path>,
    _cargo_vendor: Option<&Path>,
    _rustfmt_config: Option<&Path>,
    _environment: &[(OsString, OsString)],
    _timeout: Duration,
    _cancelled: impl FnMut() -> bool,
) -> Result<Completion, ContainmentUnavailable> {
    Err(ContainmentUnavailable)
}

pub fn run_restricted(
    command: VerifierCommand,
    toolchain_bin: &Path,
    working_directory: &Path,
    target_directory: &Path,
    readonly_roots: &[std::path::PathBuf],
    rustfmt_executable: Option<&Path>,
    cargo_vendor: Option<&Path>,
    rustfmt_config: Option<&Path>,
    environment: &[(OsString, OsString)],
    timeout: Duration,
    cancelled: impl FnMut() -> bool,
) -> Result<Completion, ContainmentUnavailable> {
    Ok(run_restricted_captured(
        command,
        toolchain_bin,
        working_directory,
        target_directory,
        readonly_roots,
        rustfmt_executable,
        cargo_vendor,
        rustfmt_config,
        environment,
        timeout,
        cancelled,
    )?
    .completion)
}

#[cfg(not(windows))]
pub fn run_restricted_captured(
    _command: VerifierCommand,
    _toolchain_bin: &Path,
    _working_directory: &Path,
    _target_directory: &Path,
    _readonly_roots: &[std::path::PathBuf],
    _rustfmt_executable: Option<&Path>,
    _cargo_vendor: Option<&Path>,
    _rustfmt_config: Option<&Path>,
    _environment: &[(OsString, OsString)],
    _timeout: Duration,
    _cancelled: impl FnMut() -> bool,
) -> Result<CapturedRun, ContainmentUnavailable> {
    Err(ContainmentUnavailable)
}

#[cfg(windows)]
pub fn run_restricted_captured(
    command: VerifierCommand,
    toolchain_bin: &Path,
    working_directory: &Path,
    target_directory: &Path,
    readonly_roots: &[std::path::PathBuf],
    rustfmt_executable: Option<&Path>,
    cargo_vendor: Option<&Path>,
    rustfmt_config: Option<&Path>,
    environment: &[(OsString, OsString)],
    timeout: Duration,
    cancelled: impl FnMut() -> bool,
) -> Result<CapturedRun, ContainmentUnavailable> {
    windows_job::run_restricted_captured(
        command,
        toolchain_bin,
        working_directory,
        target_directory,
        readonly_roots,
        rustfmt_executable,
        cargo_vendor,
        rustfmt_config,
        environment,
        timeout,
        cancelled,
    )
}
#[cfg(not(windows))]
pub fn run_contained(
    _command: VerifierCommand,
    _toolchain_bin: &Path,
    _working_directory: &Path,
    _target_directory: &Path,
    _environment: &[(OsString, OsString)],
    _timeout: Duration,
    _cancelled: impl FnMut() -> bool,
) -> Result<Completion, ContainmentUnavailable> {
    Err(ContainmentUnavailable)
}

mod windows_job {
    use super::*;
    use std::ffi::OsStr;
    use std::fs;
    use std::io::Read;
    use std::iter;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::FromRawHandle;
    use std::ptr::{null, null_mut};
    use std::sync::{Mutex, OnceLock};
    use std::thread;
    use std::time::Instant;
    use windows_sys::Win32::Foundation::{
        CloseHandle, GENERIC_READ, GetLastError, HANDLE_FLAG_INHERIT, SetHandleInformation,
        WAIT_ABANDONED, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::Security::{
        ACL_REVISION, AddAccessAllowedAce, EqualSid, GetTokenInformation, InitializeAcl,
        InitializeSecurityDescriptor, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR,
        SetSecurityDescriptorDacl, TOKEN_APPCONTAINER_INFORMATION, TOKEN_QUERY, TOKEN_USER,
        TokenAppContainerSid, TokenCapabilities, TokenIsAppContainer, TokenUser,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::IO::CreateIoCompletionPort;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, IsProcessInJob,
        JOB_OBJECT_CPU_RATE_CONTROL_ENABLE, JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
        JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_JOB_MEMORY,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_ASSOCIATE_COMPLETION_PORT,
        JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_BASIC_LIMIT_INFORMATION,
        JOBOBJECT_CPU_RATE_CONTROL_INFORMATION, JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectAssociateCompletionPortInformation,
        JobObjectBasicAccountingInformation, JobObjectCpuRateControlInformation,
        JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
        TerminateJobObject,
    };
    use windows_sys::Win32::System::Pipes::CreatePipe;
    use windows_sys::Win32::System::SystemServices::{
        JOB_OBJECT_MSG_ACTIVE_PROCESS_LIMIT, JOB_OBJECT_MSG_JOB_MEMORY_LIMIT,
        JOB_OBJECT_MSG_PROCESS_MEMORY_LIMIT,
    };
    use windows_sys::Win32::System::Threading::{
        CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateMutexW,
        CreateProcessW, DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT,
        GetCurrentProcess, GetExitCodeProcess, InitializeProcThreadAttributeList, OpenProcessToken,
        PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROCESS_INFORMATION, ReleaseMutex, ResumeThread,
        STARTF_USESTDHANDLES, STARTUPINFOEXW, TerminateProcess, UpdateProcThreadAttribute,
        WaitForSingleObject,
    };

    struct Handles(Vec<*mut core::ffi::c_void>);
    impl Drop for Handles {
        fn drop(&mut self) {
            for handle in self.0.drain(..) {
                if !handle.is_null() {
                    unsafe { CloseHandle(handle) };
                }
            }
        }
    }

    pub(super) const CAPTURE_LIMIT: usize = 256 * 1024;
    type Reader = thread::JoinHandle<(Vec<u8>, bool)>;
    struct ReaderSet(Option<(Reader, Reader)>);
    impl Drop for ReaderSet {
        fn drop(&mut self) {
            if let Some((stdout, stderr)) = self.0.take() {
                let _ = stdout.join();
                let _ = stderr.join();
            }
        }
    }
    fn start_reader(handle: *mut core::ffi::c_void) -> Reader {
        let handle = handle as usize;
        thread::spawn(move || {
            let mut file =
                unsafe { std::fs::File::from_raw_handle(handle as *mut core::ffi::c_void) };
            let mut kept = Vec::with_capacity(CAPTURE_LIMIT.min(16 * 1024));
            let mut truncated = false;
            let mut chunk = [0u8; 16 * 1024];
            loop {
                match file.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => {
                        let remaining = CAPTURE_LIMIT.saturating_sub(kept.len());
                        let take = remaining.min(count);
                        kept.extend_from_slice(&chunk[..take]);
                        truncated |= take != count;
                    }
                }
            }
            (kept, truncated)
        })
    }
    fn join_capture(completion: Completion, stdout: Reader, stderr: Reader) -> CapturedRun {
        let (stdout, stdout_truncated) = stdout.join().unwrap_or_default();
        let (stderr, stderr_truncated) = stderr.join().unwrap_or_default();
        CapturedRun {
            completion,
            stdout,
            stderr,
            stdout_truncated,
            stderr_truncated,
            cleanup_verified: true,
        }
    }
    fn finish_capture(completion: Completion, readers: &mut ReaderSet) -> CapturedRun {
        match readers.0.take() {
            Some((stdout, stderr)) => join_capture(completion, stdout, stderr),
            None => CapturedRun {
                completion,
                stdout: vec![],
                stderr: vec![],
                stdout_truncated: false,
                stderr_truncated: false,
                cleanup_verified: true,
            },
        }
    }

    pub(super) struct NamedVerifierSlot(*mut core::ffi::c_void);
    impl NamedVerifierSlot {
        pub(super) fn acquire() -> Result<Self, ContainmentUnavailable> {
            let name: Vec<u16> = "Local\\MAIA-Evolution-Tier1-Verifier-v1"
                .encode_utf16()
                .chain(iter::once(0))
                .collect();
            let handle = unsafe { CreateMutexW(null(), 0, name.as_ptr()) };
            if handle.is_null() {
                return Err(ContainmentUnavailable);
            }
            let wait = unsafe { WaitForSingleObject(handle, 0) };
            if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
                unsafe { CloseHandle(handle) };
                return Err(ContainmentUnavailable);
            }
            Ok(Self(handle))
        }
    }

    impl Drop for NamedVerifierSlot {
        fn drop(&mut self) {
            unsafe {
                ReleaseMutex(self.0);
                CloseHandle(self.0);
            }
        }
    }

    struct AttributeList {
        storage: Vec<usize>,
        initialized: bool,
    }

    impl AttributeList {
        fn with_handle_list(
            handles: &[*mut core::ffi::c_void],
            security: Option<&windows_sys::Win32::Security::SECURITY_CAPABILITIES>,
        ) -> Result<Self, ContainmentUnavailable> {
            let count = if security.is_some() { 2 } else { 1 };
            let mut required = 0usize;
            unsafe { InitializeProcThreadAttributeList(null_mut(), count, 0, &mut required) };
            if required == 0 {
                return Err(ContainmentUnavailable);
            }
            let mut result = Self {
                storage: vec![0; required.div_ceil(std::mem::size_of::<usize>())],
                initialized: false,
            };
            let list = result.storage.as_mut_ptr().cast();
            if unsafe { InitializeProcThreadAttributeList(list, count, 0, &mut required) } == 0 {
                return Err(ContainmentUnavailable);
            }
            result.initialized = true;
            if unsafe {
                UpdateProcThreadAttribute(
                    list,
                    0,
                    PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                    handles.as_ptr().cast(),
                    std::mem::size_of_val(handles),
                    null_mut(),
                    null(),
                )
            } == 0
            {
                return Err(ContainmentUnavailable);
            }
            if let Some(security) = security {
                if unsafe {
                    UpdateProcThreadAttribute(
                        list,
                        0,
                        windows_sys::Win32::System::Threading::PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
                        (security as *const windows_sys::Win32::Security::SECURITY_CAPABILITIES).cast(),
                        std::mem::size_of_val(security),
                        null_mut(),
                        null(),
                    )
                } == 0
                {
                    return Err(ContainmentUnavailable);
                }
            }
            Ok(result)
        }

        fn as_ptr(&mut self) -> *mut core::ffi::c_void {
            self.storage.as_mut_ptr().cast()
        }
    }

    impl Drop for AttributeList {
        fn drop(&mut self) {
            if self.initialized {
                unsafe { DeleteProcThreadAttributeList(self.as_ptr()) };
            }
        }
    }

    fn verify_restricted_child(
        process: *mut core::ffi::c_void,
        job: *mut core::ffi::c_void,
        expected_sid: *mut core::ffi::c_void,
        expected_capability_sid: *mut core::ffi::c_void,
    ) -> bool {
        let mut in_job = 0;
        if unsafe { IsProcessInJob(process, job, &mut in_job) } == 0 || in_job == 0 {
            eprintln!(
                "Tier-1 child Job membership verification failed: Win32 {}",
                unsafe { GetLastError() }
            );
            return false;
        }
        let mut token = null_mut();
        if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 || token.is_null() {
            eprintln!("Tier-1 child token open failed: Win32 {}", unsafe {
                GetLastError()
            });
            return false;
        }
        let _token = Handles(vec![token]);
        let mut appcontainer = 0i32;
        let mut returned = 0u32;
        if unsafe {
            GetTokenInformation(
                token,
                TokenIsAppContainer,
                (&mut appcontainer as *mut i32).cast(),
                std::mem::size_of_val(&appcontainer) as u32,
                &mut returned,
            )
        } == 0
            || appcontainer != 1
        {
            eprintln!(
                "Tier-1 child is not in AppContainer: Win32 {}, value={appcontainer}",
                unsafe { GetLastError() }
            );
            return false;
        }
        let mut required = 0u32;
        unsafe {
            GetTokenInformation(token, TokenAppContainerSid, null_mut(), 0, &mut required);
        }
        if required < std::mem::size_of::<TOKEN_APPCONTAINER_INFORMATION>() as u32 {
            eprintln!(
                "Tier-1 child AppContainer SID size query failed: Win32 {} size={required}",
                unsafe { GetLastError() }
            );
            return false;
        }
        let mut app_info = vec![0u8; required as usize];
        if unsafe {
            GetTokenInformation(
                token,
                TokenAppContainerSid,
                app_info.as_mut_ptr().cast(),
                required,
                &mut returned,
            )
        } == 0
        {
            eprintln!(
                "Tier-1 child AppContainer SID read failed: Win32 {}",
                unsafe { GetLastError() }
            );
            return false;
        }
        let app_info = app_info.as_ptr().cast::<TOKEN_APPCONTAINER_INFORMATION>();
        let actual_sid = unsafe { (*app_info).TokenAppContainer };
        if actual_sid.is_null() || unsafe { EqualSid(actual_sid, expected_sid) } == 0 {
            eprintln!("Tier-1 child AppContainer SID mismatch");
            return false;
        }
        let mut capabilities = vec![0usize; 4096];
        if unsafe {
            GetTokenInformation(
                token,
                TokenCapabilities,
                capabilities.as_mut_ptr().cast(),
                (capabilities.len() * std::mem::size_of::<usize>()) as u32,
                &mut returned,
            )
        } == 0
            || returned < std::mem::size_of::<u32>() as u32
        {
            eprintln!(
                "Tier-1 child has unexpected or unreadable capabilities: Win32 {}",
                unsafe { GetLastError() }
            );
            return false;
        }
        let capability_count = unsafe { *(capabilities.as_ptr().cast::<u32>()) };
        let capability_offset =
            std::mem::size_of::<u32>().div_ceil(std::mem::align_of::<
                windows_sys::Win32::Security::SID_AND_ATTRIBUTES,
            >()) * std::mem::align_of::<windows_sys::Win32::Security::SID_AND_ATTRIBUTES>();
        let capability = unsafe {
            capabilities
                .as_ptr()
                .cast::<u8>()
                .add(capability_offset)
                .cast::<windows_sys::Win32::Security::SID_AND_ATTRIBUTES>()
        };
        if capability_count != 1
            || expected_capability_sid.is_null()
            || unsafe {
                (*capability).Sid.is_null()
                    || EqualSid((*capability).Sid, expected_capability_sid) == 0
                    || (*capability).Attributes & 0x0000_0004 == 0
            }
        {
            eprintln!(
                "Tier-1 child capability allowlist mismatch: count={capability_count}; expected={}",
                super::windows_restricted::NULL_STDIN_CAPABILITY_NAME
            );
            return false;
        }
        true
    }

    fn job_active_processes(job: *mut core::ffi::c_void) -> Result<u32, ContainmentUnavailable> {
        let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        let mut returned = 0u32;
        if unsafe {
            QueryInformationJobObject(
                job,
                JobObjectBasicAccountingInformation,
                (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                std::mem::size_of_val(&accounting) as u32,
                &mut returned,
            )
        } == 0
        {
            eprintln!("Tier-1 Job accounting query failed: Win32 {}", unsafe {
                GetLastError()
            });
            return Err(ContainmentUnavailable);
        }
        Ok(accounting.ActiveProcesses)
    }

    fn wait_for_job_empty(job: *mut core::ffi::c_void) -> Result<(), ContainmentUnavailable> {
        loop {
            if job_active_processes(job)? == 0 {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn terminate_job_and_wait(
        job: *mut core::ffi::c_void,
        exit_code: u32,
    ) -> Result<(), ContainmentUnavailable> {
        if unsafe { TerminateJobObject(job, exit_code) } == 0 {
            let active = job_active_processes(job)?;
            if active != 0 {
                eprintln!(
                    "Tier-1 Job termination failed with Win32 {}; active={active}",
                    unsafe { GetLastError() }
                );
                return Err(ContainmentUnavailable);
            }
            return Ok(());
        }
        wait_for_job_empty(job)
    }

    fn resource_limit_notification(
        completion_port: *mut core::ffi::c_void,
    ) -> Result<bool, ContainmentUnavailable> {
        let mut resource_limit_seen = false;
        loop {
            let mut message = 0u32;
            let mut key = 0usize;
            let mut overlapped = null_mut();
            let received = unsafe {
                windows_sys::Win32::System::IO::GetQueuedCompletionStatus(
                    completion_port,
                    &mut message,
                    &mut key,
                    &mut overlapped,
                    0,
                )
            };
            if received == 0 {
                let error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
                if error == 258 {
                    return Ok(resource_limit_seen);
                }
                eprintln!("Tier-1 completion-port query failed: Win32 {error}");
                return Err(ContainmentUnavailable);
            }
            resource_limit_seen |= matches!(
                message,
                JOB_OBJECT_MSG_ACTIVE_PROCESS_LIMIT
                    | JOB_OBJECT_MSG_JOB_MEMORY_LIMIT
                    | JOB_OBJECT_MSG_PROCESS_MEMORY_LIMIT
            );
        }
    }

    fn wide(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(iter::once(0)).collect()
    }

    fn command_line(program: &Path, arguments: &[String]) -> Vec<u16> {
        let mut line = format!("\"{}\"", program.display());
        for argument in arguments {
            line.push(' ');
            line.push_str(&format!("\"{}\"", argument.replace('"', "\\\"")));
        }
        line.encode_utf16().chain(iter::once(0)).collect()
    }

    fn target_is_candidate_scoped(working_directory: &Path, target_directory: &Path) -> bool {
        let Ok(root) = fs::canonicalize(working_directory) else {
            return false;
        };
        let Ok(relative) = target_directory.strip_prefix(&root) else {
            return false;
        };
        let mut current = root.clone();
        for component in relative.components() {
            let std::path::Component::Normal(part) = component else {
                return false;
            };
            current.push(part);
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => return false,
                Ok(_) => {
                    let Ok(canonical) = fs::canonicalize(&current) else {
                        return false;
                    };
                    if !canonical.starts_with(&root) {
                        return false;
                    }
                    current = canonical;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                Err(_) => return false,
            }
        }
        true
    }

    pub fn run(
        command: VerifierCommand,
        toolchain_bin: &Path,
        working_directory: &Path,
        target_directory: &Path,
        environment: &[(OsString, OsString)],
        timeout: Duration,
        cancelled: impl FnMut() -> bool,
    ) -> Result<Completion, ContainmentUnavailable> {
        run_mode(
            command,
            toolchain_bin,
            working_directory,
            target_directory,
            &[],
            None,
            None,
            None,
            environment,
            timeout,
            cancelled,
            false,
        )
        .map(|run| run.completion)
    }

    pub fn run_restricted_captured(
        command: VerifierCommand,
        toolchain_bin: &Path,
        working_directory: &Path,
        target_directory: &Path,
        readonly_roots: &[std::path::PathBuf],
        rustfmt_executable: Option<&Path>,
        cargo_vendor: Option<&Path>,
        rustfmt_config: Option<&Path>,
        environment: &[(OsString, OsString)],
        timeout: Duration,
        cancelled: impl FnMut() -> bool,
    ) -> Result<CapturedRun, ContainmentUnavailable> {
        run_mode(
            command,
            toolchain_bin,
            working_directory,
            target_directory,
            readonly_roots,
            rustfmt_executable,
            cargo_vendor,
            rustfmt_config,
            environment,
            timeout,
            cancelled,
            true,
        )
    }

    fn run_mode(
        command: VerifierCommand,
        toolchain_bin: &Path,
        working_directory: &Path,
        target_directory: &Path,
        readonly_roots: &[std::path::PathBuf],
        rustfmt_executable: Option<&Path>,
        cargo_vendor: Option<&Path>,
        rustfmt_config: Option<&Path>,
        environment: &[(OsString, OsString)],
        timeout: Duration,
        cancelled: impl FnMut() -> bool,
        restricted: bool,
    ) -> Result<CapturedRun, ContainmentUnavailable> {
        if !toolchain_bin.is_absolute()
            || !working_directory.is_absolute()
            || !fs::canonicalize(working_directory)
                .is_ok_and(|canonical| canonical == working_directory)
            || !target_is_candidate_scoped(working_directory, target_directory)
            || timeout.is_zero()
        {
            return Err(ContainmentUnavailable);
        }
        let (program_name, arguments): (&str, Vec<String>) = match command {
            VerifierCommand::RustfmtAllowlistedSource => (
                "rustfmt.exe",
                vec![
                    "--check".into(),
                    "--edition".into(),
                    "2024".into(),
                    "--config-path".into(),
                    rustfmt_config
                        .ok_or(ContainmentUnavailable)?
                        .to_string_lossy()
                        .into_owned(),
                    working_directory
                        .join("apps/local-intelligence-host/src/lib.rs")
                        .to_string_lossy()
                        .into_owned(),
                ],
            ),
            VerifierCommand::CargoClippyHostComponent => ("cargo.exe", {
                let vendor = cargo_vendor.ok_or(ContainmentUnavailable)?;
                let mut args = vec![
                    "--config".into(),
                    format!("source.crates-io.replace-with=\"m0165-host-vendor\""),
                    "--config".into(),
                    format!(
                        "source.m0165-host-vendor.directory={:?}",
                        vendor.to_string_lossy()
                    ),
                ];
                args.extend([
                    "clippy".into(),
                    "-p".into(),
                    "maia-local-intelligence-host".into(),
                    "--all-targets".into(),
                    "--all-features".into(),
                    "--locked".into(),
                    "--offline".into(),
                    "--".into(),
                    "-D".into(),
                    "warnings".into(),
                ]);
                args
            }),
            VerifierCommand::CargoCheckHostComponent => ("cargo.exe", {
                let vendor = cargo_vendor.ok_or(ContainmentUnavailable)?;
                let mut args = vec![
                    "--config".into(),
                    "source.crates-io.replace-with=\"m0165-host-vendor\"".into(),
                    "--config".into(),
                    format!(
                        "source.m0165-host-vendor.directory={:?}",
                        vendor.to_string_lossy()
                    ),
                ];
                args.extend([
                    "check".into(),
                    "-p".into(),
                    "maia-local-intelligence-host".into(),
                    "--all-targets".into(),
                    "--all-features".into(),
                    "--locked".into(),
                    "--offline".into(),
                ]);
                args
            }),
            VerifierCommand::CargoTestHostComponent => ("cargo.exe", {
                let vendor = cargo_vendor.ok_or(ContainmentUnavailable)?;
                let mut args = vec![
                    "--config".into(),
                    "source.crates-io.replace-with=\"m0165-host-vendor\"".into(),
                    "--config".into(),
                    format!(
                        "source.m0165-host-vendor.directory={:?}",
                        vendor.to_string_lossy()
                    ),
                ];
                args.extend([
                    "test".into(),
                    "--all-targets".into(),
                    "--all-features".into(),
                    "-p".into(),
                    "maia-local-intelligence-host".into(),
                    "--locked".into(),
                    "--offline".into(),
                    "--no-fail-fast".into(),
                ]);
                args
            }),
        };
        let program = if matches!(command, VerifierCommand::RustfmtAllowlistedSource) && restricted
        {
            rustfmt_executable.ok_or(ContainmentUnavailable)?.to_owned()
        } else {
            toolchain_bin.join(program_name)
        };
        if !program.is_absolute() || !program.is_file() {
            return Err(ContainmentUnavailable);
        }
        if restricted {
            run_restricted_process(
                &program,
                &arguments,
                working_directory,
                target_directory,
                readonly_roots,
                environment,
                timeout.min(Duration::from_secs(900)),
                cancelled,
            )
        } else {
            run_process(
                &program,
                &arguments,
                working_directory,
                environment,
                timeout.min(Duration::from_secs(900)),
                cancelled,
            )
            .map(|completion| CapturedRun {
                completion,
                stdout: vec![],
                stderr: vec![],
                stdout_truncated: false,
                stderr_truncated: false,
                cleanup_verified: true,
            })
        }
    }

    pub(super) fn run_restricted_process(
        program: &Path,
        arguments: &[String],
        working_directory: &Path,
        target_directory: &Path,
        readonly_roots: &[std::path::PathBuf],
        environment: &[(OsString, OsString)],
        timeout: Duration,
        cancelled: impl FnMut() -> bool,
    ) -> Result<CapturedRun, ContainmentUnavailable> {
        static ACL_SLOT: OnceLock<Mutex<()>> = OnceLock::new();
        let _acl_slot = ACL_SLOT
            .get_or_init(|| Mutex::new(()))
            .try_lock()
            .map_err(|_| ContainmentUnavailable)?;
        let resource_capability = super::windows_restricted::NullStdinCapability::derive()
            .map_err(|_| {
                eprintln!("Tier-1 null-stdin capability derivation failed");
                ContainmentUnavailable
            })?;
        if !super::windows_restricted::null_stdin_capability_acl_available(
            resource_capability.sid(),
        ) {
            eprintln!(
                "Tier-1 null-stdin capability host ACL is absent or mismatched; run the elevated host preparation"
            );
            return Err(ContainmentUnavailable);
        }
        let mut profile = super::windows_restricted::AppContainerProfile::create()
            .map_err(|_| ContainmentUnavailable)?;
        let mut acl = super::windows_restricted::TemporaryAclGrants::new();
        let mut traversal_paths = std::collections::HashSet::new();
        for path in readonly_roots {
            if acl.grant_read(path, profile.sid()).is_err() {
                eprintln!(
                    "Tier-1 host ACL setup failed: read grant on {}",
                    path.display()
                );
                return Err(ContainmentUnavailable);
            }
        }
        if acl.grant_read(working_directory, profile.sid()).is_err() {
            eprintln!("Tier-1 host ACL setup failed: candidate read grant");
            return Err(ContainmentUnavailable);
        }
        if acl.grant_modify(target_directory, profile.sid()).is_err() {
            eprintln!("Tier-1 host ACL setup failed: target modify grant");
            return Err(ContainmentUnavailable);
        }
        let mut executable_files = vec![program.to_owned()];
        for key in ["RUSTC", "RUSTDOC"] {
            if let Some((_, value)) = environment
                .iter()
                .find(|(name, _)| name.to_string_lossy().eq_ignore_ascii_case(key))
            {
                executable_files.push(Path::new(value).to_owned());
            }
        }
        for root in readonly_roots {
            for name in ["cargo-clippy.exe", "clippy-driver.exe"] {
                let path = root.join(name);
                if path.is_file() {
                    executable_files.push(path);
                }
            }
        }
        for root in readonly_roots {
            for name in [
                "rustc_driver-573e106f78c6e3e0.dll",
                "std-44a584f44bc3dd65.dll",
            ] {
                for path in [root.join(name), root.join("bin").join(name)] {
                    if path.is_file() {
                        executable_files.push(path);
                    }
                }
            }
        }
        executable_files.sort();
        executable_files.dedup();
        for root in readonly_roots {
            if root
                .to_string_lossy()
                .to_ascii_lowercase()
                .contains("\\restricted-verifier-depot\\")
            {
                let mut parent = root.parent();
                while let Some(directory) = parent {
                    if !directory
                        .to_string_lossy()
                        .to_ascii_lowercase()
                        .contains("\\restricted-verifier-depot")
                    {
                        break;
                    }
                    if traversal_paths.insert(directory.to_owned())
                        && acl.grant_traverse(directory, profile.sid()).is_err()
                    {
                        eprintln!(
                            "Tier-1 host ACL setup failed: depot traversal grant on {}",
                            directory.display()
                        );
                        return Err(ContainmentUnavailable);
                    }
                    parent = directory.parent();
                }
            }
        }
        for path in executable_files {
            if acl.grant_read_file(&path, profile.sid()).is_err() {
                eprintln!(
                    "Tier-1 host ACL setup failed: runtime file read/execute grant on {}",
                    path.display()
                );
                return Err(ContainmentUnavailable);
            }
        }
        let mut capability = windows_sys::Win32::Security::SID_AND_ATTRIBUTES {
            Sid: resource_capability.sid(),
            Attributes: 0x0000_0004 | 0x0000_0002,
        };
        let security = windows_sys::Win32::Security::SECURITY_CAPABILITIES {
            AppContainerSid: profile.sid(),
            Capabilities: &mut capability,
            CapabilityCount: 1,
            Reserved: 0,
        };
        let run_result = run_process_with_security(
            program,
            arguments,
            working_directory,
            environment,
            timeout,
            cancelled,
            Some(&security),
        );
        match &run_result {
            Ok(captured) => eprintln!(
                "Tier-1 restricted child completion={:?}; stdout_bytes={}; stdout_truncated={}; stderr_bytes={}; stderr_truncated={}",
                captured.completion,
                captured.stdout.len(),
                captured.stdout_truncated,
                captured.stderr.len(),
                captured.stderr_truncated
            ),
            Err(_) => eprintln!("Tier-1 restricted child completion=containment-unavailable"),
        }
        let acl_result = acl.restore_all();
        eprintln!(
            "Tier-1 temporary ACL restoration: {}",
            if acl_result.is_ok() {
                "verified"
            } else {
                "FAILED"
            }
        );
        let profile_result = profile.delete();
        eprintln!(
            "Tier-1 AppContainer profile cleanup: {}",
            if profile_result.is_ok() {
                "verified"
            } else {
                "FAILED"
            }
        );
        match run_result {
            Ok(mut captured) => {
                captured.cleanup_verified = acl_result.is_ok() && profile_result.is_ok();
                Ok(captured)
            }
            Err(error) => Err(error),
        }
    }

    pub(super) fn run_process(
        program: &Path,
        arguments: &[String],
        working_directory: &Path,
        environment: &[(OsString, OsString)],
        timeout: Duration,
        cancelled: impl FnMut() -> bool,
    ) -> Result<Completion, ContainmentUnavailable> {
        run_process_with_security(
            program,
            arguments,
            working_directory,
            environment,
            timeout,
            cancelled,
            None,
        )
        .map(|run| run.completion)
    }

    pub(super) fn run_process_with_security(
        program: &Path,
        arguments: &[String],
        working_directory: &Path,
        environment: &[(OsString, OsString)],
        timeout: Duration,
        mut cancelled: impl FnMut() -> bool,
        security: Option<&windows_sys::Win32::Security::SECURITY_CAPABILITIES>,
    ) -> Result<CapturedRun, ContainmentUnavailable> {
        let mut readers = ReaderSet(None);
        static VERIFIER_SLOT: OnceLock<Mutex<()>> = OnceLock::new();
        let _slot = VERIFIER_SLOT
            .get_or_init(|| Mutex::new(()))
            .try_lock()
            .map_err(|_| ContainmentUnavailable)?;
        eprintln!("Tier-1 process runner: in-process slot acquired");
        let _cross_process_slot =
            NamedVerifierSlot::acquire().map_err(|_| ContainmentUnavailable)?;
        let job = unsafe { CreateJobObjectW(null(), null()) };
        eprintln!("Tier-1 process runner: cross-process slot acquired");
        if job.is_null() {
            return Err(ContainmentUnavailable);
        }
        let completion_port = unsafe {
            CreateIoCompletionPort(
                windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE,
                null_mut(),
                0,
                1,
            )
        };
        if completion_port.is_null() {
            return Err(ContainmentUnavailable);
        }
        let _completion_port_owner = Handles(vec![completion_port]);
        let _job_owner = Handles(vec![job]);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
            BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
                LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                    | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
                    | JOB_OBJECT_LIMIT_JOB_MEMORY,
                ActiveProcessLimit: 64,
                ..Default::default()
            },
            JobMemoryLimit: 4 * 1024 * 1024 * 1024usize,
            ..Default::default()
        };
        if unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
        } == 0
        {
            return Err(ContainmentUnavailable);
        }
        let mut cpu = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
            ControlFlags: JOB_OBJECT_CPU_RATE_CONTROL_ENABLE | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
            Anonymous: JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0 { CpuRate: 7_500 },
        };
        if unsafe {
            SetInformationJobObject(
                job,
                JobObjectCpuRateControlInformation,
                (&mut cpu as *mut JOBOBJECT_CPU_RATE_CONTROL_INFORMATION).cast(),
                std::mem::size_of_val(&cpu) as u32,
            )
        } == 0
        {
            return Err(ContainmentUnavailable);
        }

        let mut completion_association = JOBOBJECT_ASSOCIATE_COMPLETION_PORT {
            CompletionKey: job,
            CompletionPort: completion_port,
        };
        if unsafe {
            SetInformationJobObject(
                job,
                JobObjectAssociateCompletionPortInformation,
                (&mut completion_association as *mut JOBOBJECT_ASSOCIATE_COMPLETION_PORT).cast(),
                std::mem::size_of_val(&completion_association) as u32,
            )
        } == 0
        {
            return Err(ContainmentUnavailable);
        }

        eprintln!("Tier-1 process runner: Job limits configured");
        let mut line = command_line(program, arguments);
        let cwd_text = working_directory.as_os_str().to_string_lossy();
        let cwd_text = cwd_text.strip_prefix("\\\\?\\").unwrap_or(&cwd_text);
        let mut cwd = wide(std::ffi::OsStr::new(cwd_text));
        let mut env_block = Vec::new();
        let cwd_wide = &cwd[..cwd.len() - 1];
        if cwd_wide.len() >= 2 && cwd_wide[1] == b':' as u16 {
            env_block.extend([b'=' as u16, cwd_wide[0], b':' as u16, b'=' as u16]);
            env_block.extend_from_slice(cwd_wide);
            env_block.push(0);
        }
        let mut sorted_environment = environment.iter().collect::<Vec<_>>();
        sorted_environment.sort_by_key(|(key, _)| key.to_string_lossy().to_ascii_uppercase());
        for (key, value) in sorted_environment {
            env_block.extend(key.encode_wide());
            env_block.push(b'=' as u16);
            env_block.extend(value.encode_wide());
            env_block.push(0);
        }
        env_block.push(0);
        let first_env = env_block.iter().position(|unit| *unit == 0).unwrap_or(0);
        eprintln!(
            "Tier-1 child cwd={}, drive-entry={:?}",
            String::from_utf16_lossy(&cwd[..cwd.len() - 1]),
            String::from_utf16_lossy(&env_block[..first_env])
        );
        let mut pipe_acl_storage = [0usize; 16];
        let pipe_acl = pipe_acl_storage
            .as_mut_ptr()
            .cast::<windows_sys::Win32::Security::ACL>();
        let mut pipe_descriptor = SECURITY_DESCRIPTOR::default();
        let mut host_user_storage: Vec<usize>;
        let pipe_security = if let Some(security) = security {
            let mut host_token = null_mut();
            if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut host_token) } == 0 {
                eprintln!(
                    "Tier-1 host token open for pipe ACL failed: Win32 {}",
                    unsafe { GetLastError() }
                );
                return Err(ContainmentUnavailable);
            }
            let mut host_user_bytes = 0u32;
            unsafe {
                GetTokenInformation(host_token, TokenUser, null_mut(), 0, &mut host_user_bytes);
            }
            host_user_storage =
                vec![0usize; (host_user_bytes as usize).div_ceil(std::mem::size_of::<usize>())];
            let host_user_ok = host_user_bytes >= std::mem::size_of::<TOKEN_USER>() as u32
                && unsafe {
                    GetTokenInformation(
                        host_token,
                        TokenUser,
                        host_user_storage.as_mut_ptr().cast(),
                        host_user_bytes,
                        &mut host_user_bytes,
                    ) != 0
                };
            unsafe { CloseHandle(host_token) };
            if !host_user_ok {
                eprintln!(
                    "Tier-1 host user SID query for pipe ACL failed: Win32 {}",
                    unsafe { GetLastError() }
                );
                return Err(ContainmentUnavailable);
            }
            let host_user = host_user_storage.as_mut_ptr().cast::<TOKEN_USER>();
            let host_sid = unsafe { (*host_user).User.Sid };
            let acl_ok = unsafe {
                InitializeAcl(
                    pipe_acl,
                    std::mem::size_of_val(&pipe_acl_storage) as u32,
                    ACL_REVISION,
                ) != 0
                    && AddAccessAllowedAce(
                        pipe_acl,
                        ACL_REVISION,
                        GENERIC_READ | windows_sys::Win32::Foundation::GENERIC_WRITE,
                        host_sid,
                    ) != 0
                    && AddAccessAllowedAce(
                        pipe_acl,
                        ACL_REVISION,
                        GENERIC_READ | windows_sys::Win32::Foundation::GENERIC_WRITE,
                        security.AppContainerSid,
                    ) != 0
                    && InitializeSecurityDescriptor(
                        (&mut pipe_descriptor as *mut SECURITY_DESCRIPTOR).cast(),
                        windows_sys::Win32::System::SystemServices::SECURITY_DESCRIPTOR_REVISION,
                    ) != 0
                    && SetSecurityDescriptorDacl(
                        (&mut pipe_descriptor as *mut SECURITY_DESCRIPTOR).cast(),
                        1,
                        pipe_acl,
                        0,
                    ) != 0
            };
            if !acl_ok {
                eprintln!("Tier-1 pipe ACL setup failed: Win32 {}", unsafe {
                    GetLastError()
                });
                return Err(ContainmentUnavailable);
            }
            &mut pipe_descriptor as *mut SECURITY_DESCRIPTOR as *mut _
        } else {
            null_mut()
        };
        let stdio_security = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: pipe_security,
            bInheritHandle: 1,
        };
        let nul = wide(OsStr::new("NUL"));
        let stdin = unsafe {
            CreateFileW(
                nul.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                &stdio_security,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        };
        let mut stdout_read = null_mut();
        let mut stdout = null_mut();
        let mut stderr_read = null_mut();
        let mut stderr = null_mut();
        let stdout_pipe = unsafe { CreatePipe(&mut stdout_read, &mut stdout, &stdio_security, 0) };
        if stdout_pipe == 0 {
            eprintln!("Tier-1 stdout CreatePipe failed: Win32 {}", unsafe {
                GetLastError()
            });
        }
        let stderr_pipe = unsafe { CreatePipe(&mut stderr_read, &mut stderr, &stdio_security, 0) };
        if stderr_pipe == 0 {
            eprintln!("Tier-1 stderr CreatePipe failed: Win32 {}", unsafe {
                GetLastError()
            });
        }
        let stdout_read_inherit =
            unsafe { SetHandleInformation(stdout_read, HANDLE_FLAG_INHERIT, 0) };
        let stderr_read_inherit =
            unsafe { SetHandleInformation(stderr_read, HANDLE_FLAG_INHERIT, 0) };
        if stdout_pipe == 0
            || stderr_pipe == 0
            || stdout_read_inherit == 0
            || stderr_read_inherit == 0
        {
            if stdout_read_inherit == 0 || stderr_read_inherit == 0 {
                eprintln!(
                    "Tier-1 pipe inheritability setup failed: Win32 {}",
                    unsafe { GetLastError() }
                );
            }
            let mut owned = vec![stdin, stdout_read, stdout, stderr_read, stderr];
            for handle in owned.drain(..) {
                if !handle.is_null()
                    && handle != windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE
                {
                    unsafe { CloseHandle(handle) };
                }
            }
            return Err(ContainmentUnavailable);
        }
        if stdin == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            eprintln!("Tier-1 NUL stdin open failed: Win32 {}", unsafe {
                GetLastError()
            });
            unsafe {
                CloseHandle(stdout_read);
                CloseHandle(stdout);
                CloseHandle(stderr_read);
                CloseHandle(stderr);
            }
            return Err(ContainmentUnavailable);
        }
        if cancelled() {
            return Ok(finish_capture(Completion::Cancelled, &mut readers));
        }
        let standard_handles = [stdin, stdout, stderr];
        let inherited_handles = Handles(standard_handles.to_vec());
        let mut reader_handles = Handles(vec![stdout_read, stderr_read]);
        eprintln!("Tier-1 process runner: stdin NUL and separate output pipes opened");
        let mut attributes =
            AttributeList::with_handle_list(&standard_handles, security).map_err(|_| {
                eprintln!(
                    "Tier-1 process attribute-list setup failed: Win32 {}",
                    unsafe { GetLastError() }
                );
                ContainmentUnavailable
            })?;
        eprintln!("Tier-1 process runner: AppContainer attributes prepared");
        let mut startup = STARTUPINFOEXW {
            StartupInfo: windows_sys::Win32::System::Threading::STARTUPINFOW {
                cb: std::mem::size_of::<STARTUPINFOEXW>() as u32,
                dwFlags: STARTF_USESTDHANDLES,
                hStdInput: stdin,
                hStdOutput: stdout,
                hStdError: stderr,
                ..Default::default()
            },
            lpAttributeList: attributes.as_ptr(),
        };
        let application = wide(program.as_os_str());
        let mut process = PROCESS_INFORMATION::default();
        let created = unsafe {
            CreateProcessW(
                application.as_ptr(),
                line.as_mut_ptr(),
                null(),
                null(),
                1,
                CREATE_SUSPENDED
                    | CREATE_NO_WINDOW
                    | CREATE_UNICODE_ENVIRONMENT
                    | EXTENDED_STARTUPINFO_PRESENT,
                env_block.as_ptr().cast(),
                cwd.as_mut_ptr(),
                (&mut startup as *mut STARTUPINFOEXW).cast(),
                &mut process,
            )
        };
        if created == 0 {
            eprintln!("Tier-1 CreateProcessW failed: Win32 {}", unsafe {
                GetLastError()
            });
            return Err(ContainmentUnavailable);
        }
        eprintln!("Tier-1 process runner: process created suspended");
        drop(inherited_handles);
        let child_handles = Handles(vec![process.hProcess, process.hThread]);
        if unsafe { AssignProcessToJobObject(job, process.hProcess) } == 0 {
            unsafe { TerminateProcess(process.hProcess, 1) };
            unsafe { WaitForSingleObject(process.hProcess, 10_000) };
            let _ = terminate_job_and_wait(job, 1);
            drop(child_handles);
            return Err(ContainmentUnavailable);
        }
        if let Some(security) = security {
            if !verify_restricted_child(
                process.hProcess,
                job,
                security.AppContainerSid,
                if security.Capabilities.is_null() {
                    std::ptr::null_mut()
                } else {
                    unsafe { (*security.Capabilities).Sid }
                },
            ) {
                let _ = terminate_job_and_wait(job, 1);
                return Err(ContainmentUnavailable);
            }
        }
        let stdout_reader = start_reader(reader_handles.0.remove(0));
        let stderr_reader = start_reader(reader_handles.0.remove(0));
        readers.0 = Some((stdout_reader, stderr_reader));
        if cancelled() {
            terminate_job_and_wait(job, 2)?;
            drop(child_handles);
            return Ok(finish_capture(Completion::Cancelled, &mut readers));
        }
        if unsafe { ResumeThread(process.hThread) } == u32::MAX {
            let _ = terminate_job_and_wait(job, 1);
            let _ = finish_capture(Completion::ResourceLimit, &mut readers);
            return Err(ContainmentUnavailable);
        }
        eprintln!("Tier-1 process runner: resumed");
        let started = Instant::now();
        loop {
            if cancelled() {
                terminate_job_and_wait(job, 2)?;
                drop(child_handles);
                return Ok(finish_capture(Completion::Cancelled, &mut readers));
            }
            if started.elapsed() >= timeout {
                terminate_job_and_wait(job, 3)?;
                drop(child_handles);
                return Ok(finish_capture(Completion::TimedOut, &mut readers));
            }
            let wait_status = unsafe { WaitForSingleObject(process.hProcess, 20) };
            if wait_status == WAIT_OBJECT_0 {
                if resource_limit_notification(completion_port)? {
                    terminate_job_and_wait(job, 4)?;
                    drop(child_handles);
                    return Ok(finish_capture(Completion::ResourceLimit, &mut readers));
                }
                let mut exit = 1u32;
                if unsafe { GetExitCodeProcess(process.hProcess, &mut exit) } == 0 {
                    eprintln!("Tier-1 GetExitCodeProcess failed: Win32 {}", unsafe {
                        GetLastError()
                    });
                    let _ = terminate_job_and_wait(job, 1);
                    let _ = finish_capture(Completion::ResourceLimit, &mut readers);
                    return Err(ContainmentUnavailable);
                }
                if matches!(exit, 0xC000_012D | 0xC000_0017) {
                    terminate_job_and_wait(job, 4)?;
                    drop(child_handles);
                    return Ok(finish_capture(Completion::ResourceLimit, &mut readers));
                }
                eprintln!(
                    "Tier-1 root process exited with {exit}; active Job processes={:?}",
                    job_active_processes(job)
                );
                terminate_job_and_wait(job, exit)?;
                eprintln!("Tier-1 Job Object is empty after root exit");
                drop(child_handles);
                return Ok(finish_capture(
                    Completion::Exited(exit as i32),
                    &mut readers,
                ));
            }
            if wait_status == u32::MAX {
                eprintln!("Tier-1 process wait failed: Win32 {}", unsafe {
                    GetLastError()
                });
                let _ = terminate_job_and_wait(job, 1);
                let _ = finish_capture(Completion::ResourceLimit, &mut readers);
                return Err(ContainmentUnavailable);
            }
            if resource_limit_notification(completion_port)? {
                terminate_job_and_wait(job, 4)?;
                drop(child_handles);
                return Ok(finish_capture(Completion::ResourceLimit, &mut readers));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

pub use windows_job::run as run_contained;

#[cfg(all(test, windows))]
mod tests {
    use super::windows_job::run_process;
    use super::{Completion, ContainmentUnavailable};
    use std::ffi::OsString;
    use std::fs;
    use std::os::windows::io::IntoRawHandle;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::Mutex;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, WaitForSingleObject,
    };

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    const HELPER_MODE: &str = "MAIA_CONTAINED_TEST_MODE";
    const HELPER_PID_FILE: &str = "MAIA_CONTAINED_TEST_PID_FILE";

    /// The same test binary supplies tiny child and grandchild processes. This
    /// keeps the OS containment tests independent of shell startup and quoting.
    #[test]
    fn contained_process_tree_helper() {
        let Ok(mode) = std::env::var(HELPER_MODE) else {
            return;
        };
        if mode == "grandchild" || mode == "sleep" {
            loop {
                std::thread::sleep(Duration::from_secs(30));
            }
        }
        if mode == "exit" {
            return;
        }
        if mode == "fail" {
            std::process::exit(3);
        }
        if mode == "emit-output" {
            use std::io::Write;
            std::io::stdout().write_all(b"stdout-begin\n").unwrap();
            std::io::stderr().write_all(b"stderr-begin\n").unwrap();
            let block = vec![b'x'; super::windows_job::CAPTURE_LIMIT + 4096];
            std::io::stdout().write_all(&block).unwrap();
            std::io::stderr().write_all(&block).unwrap();
            std::io::stdout().write_all(b"\nstdout-end\n").unwrap();
            std::io::stderr().write_all(b"\nstderr-end\n").unwrap();
            return;
        }
        if mode == "pipe-diagnostic" {
            pipe_diagnostic_child();
            return;
        }
        if mode == "production-e4" {
            production_e4_child();
            return;
        }
        if matches!(mode.as_str(), "spawn-and-sleep" | "spawn-and-exit") {
            let pid_file = std::env::var_os(HELPER_PID_FILE).expect("helper pid path");
            let child = Command::new(std::env::current_exe().expect("test executable"))
                .args([
                    "--exact",
                    "tests::contained_process_tree_helper",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(HELPER_MODE, "grandchild")
                .spawn()
                .expect("spawn contained grandchild");
            fs::write(pid_file, child.id().to_string()).expect("write grandchild pid");
            let child_handle = child.into_raw_handle();
            assert_ne!(unsafe { CloseHandle(child_handle.cast()) }, 0);
            if mode == "spawn-and-exit" {
                return;
            }
            loop {
                std::thread::sleep(Duration::from_secs(30));
            }
        }
        panic!("unknown containment helper mode: {mode}");
    }

    fn pipe_diagnostic_child() {
        use windows_sys::Win32::Foundation::GENERIC_READ;
        use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
        use windows_sys::Win32::Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        };
        use windows_sys::Win32::System::Pipes::CreatePipe;

        let self_exe = std::env::current_exe().expect("current test executable");
        let child_args = [
            "--exact",
            "tests::contained_process_tree_helper",
            "--nocapture",
            "--test-threads=1",
        ];
        let mut inherited = Command::new(&self_exe);
        inherited.args(child_args).env(HELPER_MODE, "exit");
        match inherited.status() {
            Ok(status) => eprintln!("PROBE inherited-child: status={status}"),
            Err(error) => eprintln!(
                "PROBE inherited-child: spawn-error kind={:?} raw={:?} error={error}",
                error.kind(),
                error.raw_os_error()
            ),
        }

        let mut read = std::ptr::null_mut();
        let mut write = std::ptr::null_mut();
        let mut security = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: std::ptr::null_mut(),
            bInheritHandle: 1,
        };
        let created = unsafe { CreatePipe(&mut read, &mut write, &mut security, 0) };
        if created == 0 {
            let error = unsafe { GetLastError() };
            eprintln!("PROBE Win32 CreatePipe: failed immediately win32={error}");
        } else {
            eprintln!("PROBE Win32 CreatePipe: success read={read:p} write={write:p}");
            unsafe {
                CloseHandle(read);
                CloseHandle(write);
            }
        }

        let nul_name: Vec<u16> = "NUL".encode_utf16().chain(std::iter::once(0)).collect();
        let nul = unsafe {
            CreateFileW(
                nul_name.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                std::ptr::null_mut(),
            )
        };
        if nul == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            let error = unsafe { GetLastError() };
            eprintln!(
                "PROBE Win32 CreateFileW(NUL, GENERIC_READ): failed immediately win32={error}"
            );
        } else {
            eprintln!("PROBE Win32 CreateFileW(NUL, GENERIC_READ): success handle={nul:p}");
            unsafe { CloseHandle(nul) };
        }

        let mut spawned = Command::new(&self_exe);
        spawned.args(child_args).env(HELPER_MODE, "exit");
        match spawned.spawn() {
            Ok(mut child) => match child.wait() {
                Ok(status) => eprintln!("PROBE Rust Command::spawn inherited: status={status}"),
                Err(error) => eprintln!("PROBE Rust Command::spawn inherited: wait-error={error}"),
            },
            Err(error) => eprintln!(
                "PROBE Rust Command::spawn inherited: spawn-error kind={:?} raw={:?} error={error}",
                error.kind(),
                error.raw_os_error()
            ),
        }

        let mut null_stdin = Command::new(&self_exe);
        null_stdin
            .args(child_args)
            .env(HELPER_MODE, "exit")
            .stdin(std::process::Stdio::null());
        match null_stdin.status() {
            Ok(status) => {
                eprintln!("PROBE Rust Command::spawn stdin=null inherited outputs: status={status}")
            }
            Err(error) => eprintln!(
                "PROBE Rust Command::spawn stdin=null inherited outputs: spawn-error kind={:?} raw={:?} error={error}",
                error.kind(),
                error.raw_os_error()
            ),
        }

        let mut null_stdin_piped_outputs = Command::new(&self_exe);
        null_stdin_piped_outputs
            .args(child_args)
            .env(HELPER_MODE, "exit")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        match null_stdin_piped_outputs.spawn() {
            Ok(child) => match child.wait_with_output() {
                Ok(output) => eprintln!(
                    "PROBE Rust Command::spawn stdin=null stdout/stderr piped: status={} stdout={} stderr={}",
                    output.status,
                    output.stdout.len(),
                    output.stderr.len()
                ),
                Err(error) => eprintln!(
                    "PROBE Rust Command::spawn stdin=null wait_with_output: error={error}"
                ),
            },
            Err(error) => eprintln!(
                "PROBE Rust Command::spawn stdin=null stdout/stderr piped: spawn-error kind={:?} raw={:?} error={error}",
                error.kind(),
                error.raw_os_error()
            ),
        }

        let mut piped = Command::new(&self_exe);
        piped.args(child_args).env(HELPER_MODE, "exit");
        match piped.spawn() {
            Ok(child) => match child.wait_with_output() {
                Ok(output) => eprintln!(
                    "PROBE Rust Command::spawn stdout/stderr piped: status={} stdout={} stderr={}",
                    output.status,
                    output.stdout.len(),
                    output.stderr.len()
                ),
                Err(error) => {
                    eprintln!("PROBE Rust Command::spawn wait_with_output: error={error}")
                }
            },
            Err(error) => eprintln!(
                "PROBE Rust Command::spawn stdout/stderr piped: spawn-error kind={:?} raw={:?} error={error}",
                error.kind(),
                error.raw_os_error()
            ),
        }

        let mut output = Command::new(&self_exe);
        output.args(child_args).env(HELPER_MODE, "exit");
        match output.output() {
            Ok(output) => eprintln!(
                "PROBE Rust Command::output: status={} stdout={} stderr={}",
                output.status,
                output.stdout.len(),
                output.stderr.len()
            ),
            Err(error) => eprintln!(
                "PROBE Rust Command::output: spawn-error kind={:?} raw={:?} error={error}",
                error.kind(),
                error.raw_os_error()
            ),
        }

        nt_child_pipe_stages();
    }

    fn production_e4_child() {
        use std::net::{SocketAddr, TcpStream};

        let path = |key: &str| PathBuf::from(std::env::var_os(key).expect("E4 path"));
        let target = path("MAIA_E4_TARGET");
        let workspace_file = path("MAIA_E4_WORKSPACE_FILE");
        let protected_file = path("MAIA_E4_PROTECTED_FILE");
        let canonical_file = path("MAIA_E4_CANONICAL_FILE");
        let vendor_file = path("MAIA_E4_VENDOR_FILE");
        let port: u16 = std::env::var("MAIA_E4_PORT").unwrap().parse().unwrap();
        let writable = |file: &Path| fs::OpenOptions::new().write(true).open(file).is_ok();
        let target_write = fs::write(target.join("e4-target-write.txt"), b"synthetic").is_ok();
        let workspace_write = writable(&workspace_file);
        let workspace_create = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(workspace_file.with_file_name("e4-workspace-create.txt"))
            .is_ok();
        let protected_read = fs::read(&protected_file).is_ok();
        let protected_write = writable(&protected_file);
        let canonical_read = fs::read(&canonical_file).is_ok();
        let canonical_write = writable(&canonical_file);
        let vendor_read = fs::read(&vendor_file).is_ok();
        let vendor_write = writable(&vendor_file);
        let null_read = fs::File::open("NUL").is_ok();
        let loopback = SocketAddr::from(([127, 0, 0, 1], port));
        let loopback_result = TcpStream::connect_timeout(&loopback, Duration::from_secs(2));
        // TEST-NET-1 (RFC 5737) is never routed; a denied socket fails before
        // any reply could arrive, while an allowed one would only time out.
        let external = SocketAddr::from(([192, 0, 2, 1], 443));
        let external_result = TcpStream::connect_timeout(&external, Duration::from_secs(2));
        let network_connected = loopback_result.is_ok() || external_result.is_ok();
        let error_kind = |result: &std::io::Result<TcpStream>| match result {
            Ok(_) => "connected".to_owned(),
            Err(error) => format!("{:?}/{:?}", error.kind(), error.raw_os_error()),
        };
        let passed = target_write
            && !workspace_write
            && !workspace_create
            && !protected_read
            && !protected_write
            && !canonical_read
            && !canonical_write
            && vendor_read
            && !vendor_write
            && null_read
            && !network_connected;
        eprintln!(
            "PRODUCTION_E4 target_write={target_write} workspace_write={workspace_write} workspace_create={workspace_create} protected_read={protected_read} protected_write={protected_write} canonical_read={canonical_read} canonical_write={canonical_write} vendor_read={vendor_read} vendor_write={vendor_write} null_read={null_read} network_connected={network_connected} loopback={} external={} passed={passed}",
            error_kind(&loopback_result),
            error_kind(&external_result)
        );
        assert!(passed, "production E4 AppContainer isolation");
        fs::remove_file(target.join("e4-target-write.txt")).unwrap();
    }

    fn production_environment(target: &Path) -> Vec<(OsString, OsString)> {
        let system_root = std::env::var_os("SystemRoot")
            .or_else(|| std::env::var_os("WINDIR"))
            .expect("SystemRoot");
        let system32 = PathBuf::from(&system_root).join("System32");
        let mut environment = vec![
            ("SYSTEMDRIVE".into(), "C:".into()),
            ("COMSPEC".into(), system32.join("cmd.exe").into_os_string()),
            ("PATH".into(), system32.into_os_string()),
            ("SYSTEMROOT".into(), system_root.clone()),
            ("WINDIR".into(), system_root),
            ("TEMP".into(), target.as_os_str().to_owned()),
            ("TMP".into(), target.as_os_str().to_owned()),
            ("HOME".into(), target.as_os_str().to_owned()),
            ("USERPROFILE".into(), target.as_os_str().to_owned()),
            ("CARGO_HOME".into(), target.as_os_str().to_owned()),
            ("CARGO_TARGET_DIR".into(), target.as_os_str().to_owned()),
        ];
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
        environment
    }

    #[test]
    #[ignore = "explicit M0.16.5 production AppContainer E4 isolation probe"]
    fn production_appcontainer_e4_isolation() {
        use std::net::TcpListener;

        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let root = std::env::temp_dir().join(format!("maia-m0165-e4-{}", std::process::id()));
        let target = root.join("target");
        fs::create_dir_all(&target).unwrap();
        let workspace_file = root.join("workspace-read-only.txt");
        fs::write(&workspace_file, b"synthetic").unwrap();
        let protected_file = root.with_extension("protected.txt");
        fs::write(&protected_file, b"synthetic").unwrap();
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap()
            .to_path_buf();
        let canonical_file = repo.join("Cargo.toml");
        let vendor =
            PathBuf::from(r"C:\MAIA\restricted-verifier-depot\vendor\m0165-r5v-generated-20260928");
        let vendor_file = vendor.join("ab_glyph/Cargo.toml");
        assert!(vendor_file.is_file(), "prepared production vendor");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let exe = std::env::current_exe().unwrap();
        let mut environment = production_environment(&target);
        for (key, value) in [
            (HELPER_MODE, OsString::from("production-e4")),
            ("MAIA_E4_TARGET", target.as_os_str().to_owned()),
            (
                "MAIA_E4_WORKSPACE_FILE",
                workspace_file.as_os_str().to_owned(),
            ),
            (
                "MAIA_E4_PROTECTED_FILE",
                protected_file.as_os_str().to_owned(),
            ),
            (
                "MAIA_E4_CANONICAL_FILE",
                canonical_file.as_os_str().to_owned(),
            ),
            ("MAIA_E4_VENDOR_FILE", vendor_file.as_os_str().to_owned()),
            ("MAIA_E4_PORT", OsString::from(port.to_string())),
        ] {
            environment.push((OsString::from(key), value));
        }
        let run = super::windows_job::run_restricted_process(
            &exe,
            &[
                "--exact".into(),
                "tests::contained_process_tree_helper".into(),
                "--nocapture".into(),
                "--test-threads=1".into(),
            ],
            &root,
            &target,
            &[exe.parent().unwrap().to_path_buf(), vendor],
            &environment,
            Duration::from_secs(30),
            || false,
        )
        .expect("production containment");
        eprintln!(
            "PRODUCTION_E4_COMPLETION={:?} CLEANUP={} STDERR={} ",
            run.completion,
            run.cleanup_verified,
            String::from_utf8_lossy(&run.stderr)
        );
        assert!(run.cleanup_verified);
        assert_eq!(run.completion, Completion::Exited(0));
        fs::remove_file(protected_file).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    fn security_descriptor_sddl(path: &Path) -> String {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Foundation::LocalFree;
        use windows_sys::Win32::Security::Authorization::{
            ConvertSecurityDescriptorToStringSecurityDescriptorW, GetNamedSecurityInfoW,
            SE_FILE_OBJECT,
        };
        use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;

        let name = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let mut descriptor = std::ptr::null_mut();
        let status = unsafe {
            GetNamedSecurityInfoW(
                name.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut descriptor,
            )
        };
        assert_eq!(status, 0, "read DACL of {}", path.display());
        let mut text = std::ptr::null_mut();
        let mut length = 0u32;
        let converted = unsafe {
            ConvertSecurityDescriptorToStringSecurityDescriptorW(
                descriptor,
                1,
                DACL_SECURITY_INFORMATION,
                &mut text,
                &mut length,
            )
        };
        assert_ne!(converted, 0, "convert DACL of {}", path.display());
        let sddl =
            String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, length as usize) });
        unsafe {
            LocalFree(text.cast());
            LocalFree(descriptor);
        }
        sddl.trim_end_matches('\0').to_owned()
    }

    /// Exact DACL text for every entry below `root`, including `root` itself.
    fn tree_sddl(root: &Path) -> Vec<(PathBuf, String)> {
        let mut entries = vec![(root.to_owned(), security_descriptor_sddl(root))];
        if root.is_dir() {
            let mut children = fs::read_dir(root)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect::<Vec<_>>();
            children.sort();
            for child in children {
                entries.extend(tree_sddl(&child));
            }
        }
        entries
    }

    /// Per-profile AppContainer SIDs have eight subauthorities after S-1-15-2;
    /// the well-known package SIDs (S-1-15-2-1, S-1-15-2-2) have one.
    fn per_profile_package_sids(sddl: &str) -> Vec<String> {
        sddl.match_indices("S-1-15-2-")
            .map(|(index, _)| {
                sddl[index..]
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '-' || *c == 'S')
                    .collect::<String>()
            })
            .filter(|sid| sid.trim_end_matches('-').split('-').count() > 4)
            .collect()
    }

    fn tier1_profile_mappings_for_current_process() -> Vec<String> {
        let output = Command::new("reg")
            .args([
                "query",
                r"HKCU\Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppContainer\Mappings",
                "/s",
                "/v",
                "Moniker",
            ])
            .output()
            .expect("query AppContainer mappings");
        let prefix = format!("maia-tier1-{}-", std::process::id());
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| line.contains(&prefix))
            .map(str::to_owned)
            .collect()
    }

    /// Production-runtime E4 lifecycle: the real restricted runner (unique
    /// AppContainer profile, null-stdin capability, temporary ACLs, Job) must
    /// contain descendants and restore every temporary change after failure,
    /// timeout, cancellation, and a root that exits while a descendant lives.
    #[test]
    #[ignore = "explicit M0.16.5 production AppContainer E4 lifecycle probe"]
    fn production_appcontainer_e4_lifecycle_cleanup() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let root = std::env::temp_dir().join(format!("maia-m0165-e4l-{}", std::process::id()));
        fs::create_dir_all(root.join("target")).unwrap();
        let root = root.canonicalize().unwrap();
        let target = root.join("target");
        fs::write(root.join("workspace-read-only.txt"), b"synthetic").unwrap();
        let exe = std::env::current_exe().unwrap();
        let exe_dir = exe.parent().unwrap().to_path_buf();
        let before_root = tree_sddl(&root);
        let before_exe_dir = security_descriptor_sddl(&exe_dir);
        let before_exe = security_descriptor_sddl(&exe);
        assert!(tier1_profile_mappings_for_current_process().is_empty());

        let cases: [(&str, &str, Duration, bool, Completion); 4] = [
            (
                "failure",
                "fail",
                Duration::from_secs(60),
                false,
                Completion::Exited(3),
            ),
            (
                "timeout",
                "spawn-and-sleep",
                Duration::from_secs(20),
                false,
                Completion::TimedOut,
            ),
            (
                "cancellation",
                "spawn-and-sleep",
                Duration::from_secs(120),
                true,
                Completion::Cancelled,
            ),
            (
                "root-exit-with-descendant",
                "spawn-and-exit",
                Duration::from_secs(60),
                false,
                Completion::Exited(0),
            ),
        ];
        for (label, mode, timeout, cancel_on_descendant, expected) in cases {
            let pid_file = target.join(format!("{label}.pid"));
            let mut environment = production_environment(&target);
            environment.push((OsString::from(HELPER_MODE), OsString::from(mode)));
            environment.push((
                OsString::from(HELPER_PID_FILE),
                pid_file.as_os_str().to_owned(),
            ));
            let started = std::time::Instant::now();
            let run = super::windows_job::run_restricted_process(
                &exe,
                &[
                    "--exact".into(),
                    "tests::contained_process_tree_helper".into(),
                    "--nocapture".into(),
                    "--test-threads=1".into(),
                ],
                &root,
                &target,
                std::slice::from_ref(&exe_dir),
                &environment,
                timeout,
                || cancel_on_descendant && pid_file.exists(),
            )
            .expect("production containment");
            eprintln!(
                "PRODUCTION_E4_LIFECYCLE case={label} completion={:?} cleanup_verified={} seconds={:.1}",
                run.completion,
                run.cleanup_verified,
                started.elapsed().as_secs_f32()
            );
            assert_eq!(run.completion, expected, "{label}");
            assert!(run.cleanup_verified, "{label} cleanup");
            if mode != "fail" {
                assert_process_ended(&pid_file);
                eprintln!("PRODUCTION_E4_LIFECYCLE case={label} descendant_ended=true");
            }
            let after_root = tree_sddl(&root);
            assert_eq!(after_root, before_root, "{label} candidate ACL restoration");
            assert_eq!(
                security_descriptor_sddl(&exe_dir),
                before_exe_dir,
                "{label}"
            );
            assert_eq!(security_descriptor_sddl(&exe), before_exe, "{label}");
            for (path, sddl) in &after_root {
                assert!(
                    per_profile_package_sids(sddl).is_empty(),
                    "{label} residual package SID on {}",
                    path.display()
                );
            }
            assert!(
                tier1_profile_mappings_for_current_process().is_empty(),
                "{label} AppContainer profile mapping"
            );
            eprintln!(
                "PRODUCTION_E4_LIFECYCLE case={label} acl_exact=true residual_package_sid=0 profile_mapping=0"
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[repr(C)]
    struct NtUnicodeString {
        length: u16,
        maximum_length: u16,
        buffer: *mut u16,
    }
    #[repr(C)]
    struct NtObjectAttributes {
        length: u32,
        root_directory: *mut core::ffi::c_void,
        object_name: *mut NtUnicodeString,
        attributes: u32,
        security_descriptor: *mut core::ffi::c_void,
        security_quality_of_service: *mut core::ffi::c_void,
    }
    #[repr(C)]
    struct NtIoStatusBlock {
        status: *mut core::ffi::c_void,
        information: usize,
    }
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtOpenFile(
            file_handle: *mut *mut core::ffi::c_void,
            desired_access: u32,
            object_attributes: *mut NtObjectAttributes,
            io_status_block: *mut NtIoStatusBlock,
            share_access: u32,
            open_options: u32,
        ) -> i32;
        fn NtCreateNamedPipeFile(
            file_handle: *mut *mut core::ffi::c_void,
            desired_access: u32,
            object_attributes: *mut NtObjectAttributes,
            io_status_block: *mut NtIoStatusBlock,
            share_access: u32,
            create_disposition: u32,
            create_options: u32,
            named_pipe_type: u32,
            read_mode: u32,
            completion_mode: u32,
            maximum_instances: u32,
            inbound_quota: u32,
            outbound_quota: u32,
            default_timeout: *const i64,
        ) -> i32;
        fn RtlNtStatusToDosError(status: i32) -> u32;
    }

    fn nt_child_pipe_stages() {
        use windows_sys::Win32::Foundation::CloseHandle;

        let mut io = NtIoStatusBlock {
            status: std::ptr::null_mut(),
            information: 0,
        };
        let mut path: Vec<u16> = "\\Device\\NamedPipe\\".encode_utf16().collect();
        let mut name = NtUnicodeString {
            length: (path.len() * 2) as u16,
            maximum_length: (path.len() * 2) as u16,
            buffer: path.as_mut_ptr(),
        };
        let mut attributes = NtObjectAttributes {
            length: std::mem::size_of::<NtObjectAttributes>() as u32,
            root_directory: std::ptr::null_mut(),
            object_name: &mut name,
            attributes: 0x40,
            security_descriptor: std::ptr::null_mut(),
            security_quality_of_service: std::ptr::null_mut(),
        };
        let mut pipe_fs = std::ptr::null_mut();
        let status = unsafe {
            NtOpenFile(
                &mut pipe_fs,
                0x0010_0000 | 0x8000_0000,
                &mut attributes,
                &mut io,
                3,
                0x20,
            )
        };
        let dos = unsafe { RtlNtStatusToDosError(status) };
        eprintln!(
            "PROBE NT NtOpenFile(\\Device\\NamedPipe\\): status=0x{:08X} dos={dos}",
            status as u32
        );
        if status < 0 {
            return;
        }

        let empty = NtUnicodeString {
            length: 0,
            maximum_length: 0,
            buffer: std::ptr::null_mut(),
        };
        attributes.object_name = (&empty as *const NtUnicodeString).cast_mut();
        attributes.root_directory = pipe_fs;
        attributes.attributes = 0;
        let mut pipe = std::ptr::null_mut();
        let timeout = -500_000i64;
        let status = unsafe {
            NtCreateNamedPipeFile(
                &mut pipe,
                0x0010_0000 | 0x8000_0000,
                &mut attributes,
                &mut io,
                2,
                2,
                0,
                0,
                0,
                0,
                1,
                64 * 1024,
                64 * 1024,
                &timeout,
            )
        };
        let dos = unsafe { RtlNtStatusToDosError(status) };
        eprintln!(
            "PROBE NT NtCreateNamedPipeFile: status=0x{:08X} dos={dos}",
            status as u32
        );
        if status < 0 {
            unsafe { CloseHandle(pipe_fs) };
            return;
        }

        attributes.root_directory = pipe;
        attributes.attributes = 2;
        let mut peer = std::ptr::null_mut();
        let status = unsafe {
            NtOpenFile(
                &mut peer,
                0x0010_0000 | 0x4000_0000 | 0x80,
                &mut attributes,
                &mut io,
                0,
                0x40 | 0x20,
            )
        };
        let dos = unsafe { RtlNtStatusToDosError(status) };
        eprintln!(
            "PROBE NT NtOpenFile(anonymous peer): status=0x{:08X} dos={dos}",
            status as u32
        );
        if status >= 0 {
            unsafe { CloseHandle(peer) };
        }
        unsafe {
            CloseHandle(pipe);
            CloseHandle(pipe_fs);
        }
    }

    #[test]
    #[ignore = "explicit M0.16.5 AppContainer pipe diagnosis"]
    fn production_appcontainer_child_pipe_stage_diagnostic() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let root =
            std::env::temp_dir().join(format!("maia-m0165-pipe-probe-{}", std::process::id()));
        let target = root.join("target");
        let temp = target.join("tmp");
        let home = target.join("home");
        let cargo_home = target.join("cargo-home");
        for directory in [&target, &temp, &home, &cargo_home] {
            fs::create_dir_all(directory).expect("probe root");
        }
        let exe = std::env::current_exe().expect("test executable");
        let system_root = std::env::var_os("SystemRoot")
            .or_else(|| std::env::var_os("WINDIR"))
            .expect("SystemRoot");
        let system32 = PathBuf::from(&system_root).join("System32");
        let mut environment = vec![
            ("SYSTEMDRIVE".into(), "C:".into()),
            ("COMSPEC".into(), system32.join("cmd.exe").into_os_string()),
            ("PATH".into(), system32.into_os_string()),
            ("SYSTEMROOT".into(), system_root.clone()),
            ("WINDIR".into(), system_root),
            ("TEMP".into(), temp.clone().into_os_string()),
            ("TMP".into(), temp.into_os_string()),
            ("HOME".into(), home.clone().into_os_string()),
            ("USERPROFILE".into(), home.into_os_string()),
            ("CARGO_HOME".into(), cargo_home.into_os_string()),
            ("CARGO_TARGET_DIR".into(), target.as_os_str().to_owned()),
        ];
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
        environment.push((
            OsString::from(HELPER_MODE),
            OsString::from("pipe-diagnostic"),
        ));
        let run = super::windows_job::run_restricted_process(
            &exe,
            &[
                "--exact".into(),
                "tests::contained_process_tree_helper".into(),
                "--nocapture".into(),
                "--test-threads=1".into(),
            ],
            &root,
            &target,
            &[exe.parent().expect("exe parent").to_owned()],
            &environment,
            Duration::from_secs(20),
            || false,
        )
        .expect("production AppContainer runner");
        fs::remove_dir_all(&root).expect("probe fixture cleanup");
        eprintln!("PIPE_DIAGNOSTIC_COMPLETION={:?}", run.completion);
        eprintln!(
            "PIPE_DIAGNOSTIC_STDOUT_BEGIN\n{}\nPIPE_DIAGNOSTIC_STDOUT_END",
            String::from_utf8_lossy(&run.stdout)
        );
        eprintln!(
            "PIPE_DIAGNOSTIC_STDERR_BEGIN\n{}\nPIPE_DIAGNOSTIC_STDERR_END",
            String::from_utf8_lossy(&run.stderr)
        );
        assert!(
            run.cleanup_verified,
            "AppContainer and temporary ACL cleanup"
        );
        assert_eq!(run.completion, Completion::Exited(0));
        let stderr = String::from_utf8_lossy(&run.stderr);
        for evidence in [
            "PROBE inherited-child: status=exit code: 0",
            "PROBE Win32 CreatePipe: success",
            "PROBE Win32 CreateFileW(NUL, GENERIC_READ): success",
            "PROBE Rust Command::spawn inherited: status=exit code: 0",
            "PROBE Rust Command::spawn stdin=null inherited outputs: status=exit code: 0",
            "PROBE Rust Command::spawn stdout/stderr piped: status=exit code: 0",
            "PROBE Rust Command::spawn stdin=null stdout/stderr piped: status=exit code: 0",
            "PROBE Rust Command::output: status=exit code: 0",
            "PROBE NT NtOpenFile(\\Device\\NamedPipe\\): status=0x00000000 dos=0",
            "PROBE NT NtCreateNamedPipeFile: status=0x00000000 dos=0",
            "PROBE NT NtOpenFile(anonymous peer): status=0x00000000 dos=0",
        ] {
            assert!(
                stderr.contains(evidence),
                "missing probe evidence: {evidence}\n{stderr}"
            );
        }
    }

    fn run_helper(
        mode: &str,
        pid_file: Option<&Path>,
        timeout: Duration,
        cancelled: impl FnMut() -> bool,
    ) -> Result<Completion, ContainmentUnavailable> {
        let mut environment = ["PATH", "SYSTEMROOT", "WINDIR", "TEMP", "TMP"]
            .into_iter()
            .filter_map(|key| std::env::var_os(key).map(|value| (key.into(), value)))
            .collect::<Vec<_>>();
        environment.push((OsString::from(HELPER_MODE), OsString::from(mode)));
        if let Some(pid_file) = pid_file {
            environment.push((
                OsString::from(HELPER_PID_FILE),
                pid_file.as_os_str().to_owned(),
            ));
        }
        run_process(
            &std::env::current_exe().expect("test executable"),
            &[
                "--exact".into(),
                "tests::contained_process_tree_helper".into(),
                "--nocapture".into(),
                "--test-threads=1".into(),
            ],
            &std::env::temp_dir(),
            &environment,
            timeout,
            cancelled,
        )
    }

    fn child_pid_file() -> PathBuf {
        std::env::temp_dir().join(format!(
            "maia-job-child-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    fn assert_process_ended(id_file: &Path) {
        let child_id: u32 = std::fs::read_to_string(id_file)
            .expect("child pid evidence")
            .trim()
            .parse()
            .expect("numeric child pid");
        let _ = std::fs::remove_file(id_file);
        let process =
            unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | 0x0010_0000, 0, child_id) };
        if process.is_null() {
            assert_eq!(
                unsafe { GetLastError() },
                windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER,
                "grandchild process lookup failed for a reason other than process termination"
            );
            return;
        }
        let ended = unsafe { WaitForSingleObject(process, 5_000) } == WAIT_OBJECT_0;
        unsafe { CloseHandle(process) };
        assert!(ended, "grandchild survived containment owner");
    }

    #[test]
    fn timeout_terminates_descendants() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let id_file = child_pid_file();
        assert_eq!(
            run_helper(
                "spawn-and-sleep",
                Some(&id_file),
                Duration::from_secs(5),
                || false,
            )
            .expect("containment"),
            Completion::TimedOut
        );
        assert_process_ended(&id_file);
    }

    #[test]
    fn cancellation_terminates_descendants() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let id_file = child_pid_file();
        assert_eq!(
            run_helper(
                "spawn-and-sleep",
                Some(&id_file),
                Duration::from_secs(15),
                || id_file.exists(),
            )
            .expect("containment"),
            Completion::Cancelled
        );
        assert_process_ended(&id_file);
    }

    #[test]
    fn target_traversal_is_rejected_before_process_start() {
        let root = std::env::temp_dir().canonicalize().expect("temp root");
        let target = root.join("target").join("..").join("outside");
        let result = super::windows_job::run(
            super::VerifierCommand::CargoCheckHostComponent,
            std::env::temp_dir().as_path(),
            &root,
            &target,
            &[],
            Duration::from_secs(1),
            || false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn operating_system_mutex_allows_only_one_verifier_owner() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _owner = super::windows_job::NamedVerifierSlot::acquire().expect("first owner");
        let denied =
            std::thread::spawn(|| super::windows_job::NamedVerifierSlot::acquire().is_err())
                .join()
                .expect("second verifier thread");
        assert!(denied);
    }

    #[test]
    fn pre_start_cancellation_never_starts_verifier() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(
            run_helper("exit", None, Duration::from_secs(10), || true).expect("containment"),
            Completion::Cancelled
        );
    }

    #[test]
    fn captures_both_streams_with_bounded_storage_and_drains_overflow() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut environment = ["PATH", "SYSTEMROOT", "WINDIR", "TEMP", "TMP"]
            .into_iter()
            .filter_map(|key| std::env::var_os(key).map(|v| (key.into(), v)))
            .collect::<Vec<_>>();
        environment.push((OsString::from(HELPER_MODE), OsString::from("emit-output")));
        let run = super::windows_job::run_process_with_security(
            &std::env::current_exe().unwrap(),
            &[
                "--exact".into(),
                "tests::contained_process_tree_helper".into(),
                "--nocapture".into(),
                "--test-threads=1".into(),
            ],
            &std::env::temp_dir(),
            &environment,
            Duration::from_secs(10),
            || false,
            None,
        )
        .expect("captured process");
        assert_eq!(run.completion, Completion::Exited(0));
        assert!(run.stdout.windows(13).any(|v| v == b"stdout-begin\n"));
        assert!(run.stderr.windows(13).any(|v| v == b"stderr-begin\n"));
        assert_eq!(run.stdout.len(), super::windows_job::CAPTURE_LIMIT);
        assert_eq!(run.stderr.len(), super::windows_job::CAPTURE_LIMIT);
        assert!(run.stdout_truncated && run.stderr_truncated);
    }

    #[test]
    fn timeout_terminates_contained_process() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(
            run_helper("sleep", None, Duration::from_millis(100), || false).expect("containment"),
            Completion::TimedOut
        );
    }

    #[test]
    fn cancellation_terminates_contained_process() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut checks = 0;
        assert_eq!(
            run_helper("sleep", None, Duration::from_secs(10), || {
                checks += 1;
                checks >= 3
            })
            .expect("containment"),
            Completion::Cancelled
        );
    }

    #[test]
    fn job_owner_termination_kills_grandchild_after_root_exit() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let id_file = child_pid_file();
        let result = run_helper(
            "spawn-and-exit",
            Some(&id_file),
            Duration::from_secs(15),
            || false,
        )
        .expect("contained execution");
        assert!(matches!(result, Completion::Exited(0)));
        assert_process_ended(&id_file);
    }
}
