//! Narrow Windows process-tree containment primitive for trusted host commands.
//! This provides Job Object containment only; it does not claim network,
//! filesystem, or restricted-token isolation.
use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    Exited(i32),
    ResourceLimit,
    TimedOut,
    Cancelled,
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

#[cfg(windows)]
mod windows_job {
    use super::*;
    use std::ffi::OsStr;
    use std::fs;
    use std::iter;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::{null, null_mut};
    use std::sync::{Mutex, OnceLock};
    use std::thread;
    use std::time::Instant;
    use windows_sys::Win32::Foundation::{
        CloseHandle, GENERIC_READ, GENERIC_WRITE, WAIT_ABANDONED, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Environment::{
        FreeEnvironmentStringsW, GetEnvironmentStringsW,
    };
    use windows_sys::Win32::System::IO::CreateIoCompletionPort;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_CPU_RATE_CONTROL_ENABLE,
        JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
        JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_ASSOCIATE_COMPLETION_PORT, JOBOBJECT_BASIC_LIMIT_INFORMATION,
        JOBOBJECT_CPU_RATE_CONTROL_INFORMATION, JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectAssociateCompletionPortInformation,
        JobObjectCpuRateControlInformation, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    };
    use windows_sys::Win32::System::SystemServices::{
        JOB_OBJECT_MSG_ACTIVE_PROCESS_LIMIT, JOB_OBJECT_MSG_JOB_MEMORY_LIMIT,
        JOB_OBJECT_MSG_PROCESS_MEMORY_LIMIT,
    };
    use windows_sys::Win32::System::Threading::{
        CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateMutexW,
        CreateProcessW, DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT,
        GetExitCodeProcess, InitializeProcThreadAttributeList, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
        PROCESS_INFORMATION, ReleaseMutex, ResumeThread, STARTF_USESTDHANDLES, STARTUPINFOEXW,
        UpdateProcThreadAttribute, WaitForSingleObject,
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
        ) -> Result<Self, ContainmentUnavailable> {
            let mut required = 0usize;
            unsafe { InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut required) };
            if required == 0 {
                return Err(ContainmentUnavailable);
            }
            let mut result = Self {
                storage: vec![0; required.div_ceil(std::mem::size_of::<usize>())],
                initialized: false,
            };
            let list = result.storage.as_mut_ptr().cast();
            if unsafe { InitializeProcThreadAttributeList(list, 1, 0, &mut required) } == 0 {
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
                    working_directory
                        .join("apps/local-intelligence-host/src/lib.rs")
                        .to_string_lossy()
                        .into_owned(),
                ],
            ),
            VerifierCommand::CargoClippyHostComponent => (
                "cargo.exe",
                vec![
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
                ],
            ),
            VerifierCommand::CargoCheckHostComponent => (
                "cargo.exe",
                vec![
                    "check".into(),
                    "-p".into(),
                    "maia-local-intelligence-host".into(),
                    "--all-targets".into(),
                    "--all-features".into(),
                    "--locked".into(),
                    "--offline".into(),
                ],
            ),
            VerifierCommand::CargoTestHostComponent => (
                "cargo.exe",
                vec![
                    "test".into(),
                    "-p".into(),
                    "maia-local-intelligence-host".into(),
                    "--locked".into(),
                    "--offline".into(),
                    "--no-fail-fast".into(),
                ],
            ),
        };
        let program = toolchain_bin.join(program_name);
        if !program.is_file() {
            return Err(ContainmentUnavailable);
        }
        run_process(
            &program,
            &arguments,
            working_directory,
            environment,
            timeout.min(Duration::from_secs(120)),
            cancelled,
        )
    }

    pub(super) fn run_process(
        program: &Path,
        arguments: &[String],
        working_directory: &Path,
        environment: &[(OsString, OsString)],
        timeout: Duration,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<Completion, ContainmentUnavailable> {
        static VERIFIER_SLOT: OnceLock<Mutex<()>> = OnceLock::new();
        let _slot = VERIFIER_SLOT
            .get_or_init(|| Mutex::new(()))
            .try_lock()
            .map_err(|_| ContainmentUnavailable)?;
        let _cross_process_slot =
            NamedVerifierSlot::acquire().map_err(|_| ContainmentUnavailable)?;
        let job = unsafe { CreateJobObjectW(null(), null()) };
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

        let mut line = command_line(program, arguments);
        let mut cwd = wide(working_directory.as_os_str());
        let mut env_block = Vec::new();
        let inherited_environment = unsafe { GetEnvironmentStringsW() };
        if !inherited_environment.is_null() {
            let mut cursor = inherited_environment;
            loop {
                let mut end = cursor;
                while unsafe { *end } != 0 {
                    end = unsafe { end.add(1) };
                }
                if end == cursor {
                    break;
                }
                if unsafe { *cursor } == b'=' as u16 {
                    let entry = unsafe {
                        std::slice::from_raw_parts(cursor, end.offset_from(cursor) as usize)
                    };
                    env_block.extend_from_slice(entry);
                    env_block.push(0);
                }
                cursor = unsafe { end.add(1) };
            }
            unsafe { FreeEnvironmentStringsW(inherited_environment) };
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
        let security = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: null_mut(),
            bInheritHandle: 1,
        };
        let nul = wide(OsStr::new("NUL"));
        let stdin = unsafe {
            CreateFileW(
                nul.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                &security,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        };
        let stdout = unsafe {
            CreateFileW(
                nul.as_ptr(),
                GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                &security,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        };
        let stderr = unsafe {
            CreateFileW(
                nul.as_ptr(),
                GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                &security,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        };
        if stdin == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE
            || stdout == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE
            || stderr == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE
        {
            return Err(ContainmentUnavailable);
        }
        if cancelled() {
            return Ok(Completion::Cancelled);
        }
        let standard_handles = [stdin, stdout, stderr];
        let inherited_handles = Handles(standard_handles.to_vec());
        let mut attributes = AttributeList::with_handle_list(&standard_handles)?;
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
        let mut process = PROCESS_INFORMATION::default();
        let created = unsafe {
            CreateProcessW(
                null(),
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
            return Err(ContainmentUnavailable);
        }
        drop(attributes);
        drop(inherited_handles);
        let child_handles = Handles(vec![process.hProcess, process.hThread]);
        if unsafe { AssignProcessToJobObject(job, process.hProcess) } == 0 {
            unsafe { TerminateJobObject(job, 1) };
            return Err(ContainmentUnavailable);
        }
        if cancelled() {
            unsafe { TerminateJobObject(job, 2) };
            let _ = unsafe { WaitForSingleObject(process.hProcess, 5_000) };
            drop(child_handles);
            return Ok(Completion::Cancelled);
        }
        if unsafe { ResumeThread(process.hThread) } == u32::MAX {
            unsafe { TerminateJobObject(job, 1) };
            return Err(ContainmentUnavailable);
        }
        let started = Instant::now();
        loop {
            if cancelled() {
                unsafe { TerminateJobObject(job, 2) };
                let _ = unsafe { WaitForSingleObject(process.hProcess, 5_000) };
                drop(child_handles);
                return Ok(Completion::Cancelled);
            }
            if started.elapsed() >= timeout {
                unsafe { TerminateJobObject(job, 3) };
                let _ = unsafe { WaitForSingleObject(process.hProcess, 5_000) };
                drop(child_handles);
                return Ok(Completion::TimedOut);
            }
            if unsafe { WaitForSingleObject(process.hProcess, 20) } == WAIT_OBJECT_0 {
                if resource_limit_notification(completion_port)? {
                    unsafe { TerminateJobObject(job, 4) };
                    drop(child_handles);
                    return Ok(Completion::ResourceLimit);
                }
                let mut exit = 1u32;
                if unsafe { GetExitCodeProcess(process.hProcess, &mut exit) } == 0 {
                    unsafe { TerminateJobObject(job, 1) };
                    return Err(ContainmentUnavailable);
                }
                if matches!(exit, 0xC000_012D | 0xC000_0017) {
                    unsafe { TerminateJobObject(job, 4) };
                    drop(child_handles);
                    return Ok(Completion::ResourceLimit);
                }
                // Closing the job owner also kills any descendants that outlive
                // the direct process, even on its ordinary successful exit.
                let _ = unsafe { TerminateJobObject(job, exit) };
                drop(child_handles);
                return Ok(Completion::Exited(exit as i32));
            }
            if resource_limit_notification(completion_port)? {
                unsafe { TerminateJobObject(job, 4) };
                let _ = unsafe { WaitForSingleObject(process.hProcess, 5_000) };
                drop(child_handles);
                return Ok(Completion::ResourceLimit);
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

#[cfg(windows)]
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
