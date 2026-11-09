//! Read-only process-presence observation for the desktop status projection.
//!
//! A successful query identifies one kernel process object. It does not prove
//! that the process is an authorized worker, that a particular executable image
//! ran, or that any task succeeded. This crate never starts or controls a process.

#![deny(unsafe_op_in_unsafe_fn)]

/// Process ID and exact UTC creation time expressed as .NET DateTime ticks.
///
/// `birth_ticks` is a lossless count of 100 ns units since 0001-01-01. Consumers
/// must compare the whole tuple, not PID alone, and must not round through f64.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub birth_ticks: u64,
}

/// A point-in-time observation, without execution or semantic-result authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessObservation {
    Alive(ProcessIdentity),
    Exited {
        identity: ProcessIdentity,
        exit_code: Option<u32>,
    },
    /// Unsupported platform, denied access, invalid identity, or query failure.
    /// This is not evidence that the requested process is absent or dead.
    Unavailable,
}

/// Observe only the requested PID, using an owned query/synchronize handle.
///
/// On Windows all identity and state queries refer to that same handle. The
/// handle is closed on every return path by `OwnedHandle`. The returned value
/// is already historical when this function returns; callers own freshness and
/// matching against the exact reported run identity. Non-Windows is unavailable.
pub fn observe(pid: u32) -> ProcessObservation {
    if pid == 0 {
        return ProcessObservation::Unavailable;
    }

    #[cfg(windows)]
    {
        platform::observe(pid)
    }
    #[cfg(not(windows))]
    {
        ProcessObservation::Unavailable
    }
}

#[cfg(any(windows, test))]
const FILETIME_TO_DOTNET_TICKS: u64 = 504_911_232_000_000_000;
#[cfg(any(windows, test))]
const WAIT_SIGNALED: u32 = 0;
#[cfg(any(windows, test))]
const WAIT_TIMED_OUT: u32 = 258;

#[cfg(any(windows, test))]
fn creation_ticks(low: u32, high: u32) -> Option<u64> {
    let filetime = (u64::from(high) << 32) | u64::from(low);
    if filetime == 0 {
        return None;
    }
    filetime.checked_add(FILETIME_TO_DOTNET_TICKS)
}

#[cfg(any(windows, test))]
fn classify_wait(
    identity: ProcessIdentity,
    wait_result: u32,
    exit_code: Option<u32>,
) -> ProcessObservation {
    match wait_result {
        WAIT_TIMED_OUT => ProcessObservation::Alive(identity),
        WAIT_SIGNALED => ProcessObservation::Exited {
            identity,
            exit_code,
        },
        _ => ProcessObservation::Unavailable,
    }
}

#[cfg(windows)]
mod platform {
    use super::{
        ProcessIdentity, ProcessObservation, WAIT_SIGNALED, classify_wait, creation_ticks,
    };
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, GetProcessId, GetProcessTimes, OpenProcess,
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };

    fn empty_filetime() -> FILETIME {
        FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        }
    }

    pub(super) fn observe(pid: u32) -> ProcessObservation {
        // SAFETY: OpenProcess receives only a PID and read/query + synchronize
        // rights, with handle inheritance disabled. No process-control rights
        // or caller-owned pointers are supplied. Failure is a null handle.
        let raw = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                0,
                pid,
            )
        };
        if raw.is_null() {
            return ProcessObservation::Unavailable;
        }
        // SAFETY: this is the non-null handle freshly returned by OpenProcess.
        // Ownership is transferred once; no raw handle escapes this function.
        let owned = unsafe { OwnedHandle::from_raw_handle(raw) };
        let handle = owned.as_raw_handle();

        // SAFETY: the owned process handle remains open for every query below.
        let actual_pid = unsafe { GetProcessId(handle) };
        if actual_pid == 0 || actual_pid != pid {
            return ProcessObservation::Unavailable;
        }
        let mut creation = empty_filetime();
        let mut exit_time = empty_filetime();
        let mut kernel_time = empty_filetime();
        let mut user_time = empty_filetime();
        // SAFETY: all four output pointers refer to distinct initialized
        // FILETIME values, alive for the duration of this synchronous call.
        let times_ok = unsafe {
            GetProcessTimes(
                handle,
                &mut creation,
                &mut exit_time,
                &mut kernel_time,
                &mut user_time,
            )
        };
        if times_ok == 0 {
            return ProcessObservation::Unavailable;
        }
        let Some(birth_ticks) = creation_ticks(creation.dwLowDateTime, creation.dwHighDateTime)
        else {
            return ProcessObservation::Unavailable;
        };
        let identity = ProcessIdentity {
            pid: actual_pid,
            birth_ticks,
        };

        // SAFETY: this is the same open process handle with SYNCHRONIZE rights.
        // Zero timeout never waits for process completion or changes its state.
        let waited = unsafe { WaitForSingleObject(handle, 0) };
        let exit_code = if waited == WAIT_SIGNALED {
            let mut code = 0;
            // SAFETY: the process object was signaled on this same retained
            // handle. `code` is a valid output pointer. Query failure means
            // unknown exit code, not a running process or semantic success.
            if unsafe { GetExitCodeProcess(handle, &mut code) } != 0 {
                Some(code)
            } else {
                None
            }
        } else {
            None
        };
        classify_wait(identity, waited, exit_code)
        // `owned` drops here, or on any earlier return, closing the query handle.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> ProcessIdentity {
        ProcessIdentity {
            pid: 42,
            birth_ticks: 638_980_123_456_789_012,
        }
    }

    #[test]
    fn converts_windows_unix_epoch_to_exact_dotnet_epoch() {
        let filetime = 116_444_736_000_000_000_u64;
        assert_eq!(
            creation_ticks(filetime as u32, (filetime >> 32) as u32),
            Some(621_355_968_000_000_000)
        );
    }

    #[test]
    fn creation_time_keeps_low_bits_above_javascript_integer_precision() {
        let filetime = 0x0123_4567_89ab_cdef_u64;
        assert_eq!(
            creation_ticks(0x89ab_cdef, 0x0123_4567),
            Some(FILETIME_TO_DOTNET_TICKS + filetime)
        );
    }

    #[test]
    fn invalid_or_overflowing_creation_time_is_unavailable() {
        assert_eq!(creation_ticks(0, 0), None);
        assert_eq!(creation_ticks(u32::MAX, u32::MAX), None);
    }

    #[test]
    fn signaled_process_with_exit_259_is_exited_not_alive() {
        assert_eq!(
            classify_wait(identity(), WAIT_SIGNALED, Some(259)),
            ProcessObservation::Exited {
                identity: identity(),
                exit_code: Some(259)
            }
        );
    }

    #[test]
    fn signaled_process_without_exit_code_remains_observed_exited() {
        assert_eq!(
            classify_wait(identity(), WAIT_SIGNALED, None),
            ProcessObservation::Exited {
                identity: identity(),
                exit_code: None
            }
        );
    }

    #[test]
    fn only_wait_timeout_establishes_the_alive_sample() {
        assert_eq!(
            classify_wait(identity(), WAIT_TIMED_OUT, None),
            ProcessObservation::Alive(identity())
        );
        // A supplied numeric exit value cannot override the wait observation.
        assert_eq!(
            classify_wait(identity(), WAIT_TIMED_OUT, Some(0)),
            ProcessObservation::Alive(identity())
        );
    }

    #[test]
    fn failed_or_unexpected_wait_does_not_mean_dead_or_alive() {
        for wait in [u32::MAX, 0x80, 1] {
            assert_eq!(
                classify_wait(identity(), wait, Some(0)),
                ProcessObservation::Unavailable
            );
        }
    }

    #[test]
    fn zero_pid_is_rejected_without_a_native_query() {
        assert_eq!(observe(0), ProcessObservation::Unavailable);
    }

    #[cfg(windows)]
    #[test]
    fn self_query_observes_the_same_live_identity_without_process_control() {
        let pid = std::process::id();
        let ProcessObservation::Alive(first) = observe(pid) else {
            panic!("querying this test process did not produce a live observation");
        };
        assert_eq!(first.pid, pid);
        assert!(first.birth_ticks > FILETIME_TO_DOTNET_TICKS);
        assert_eq!(observe(pid), ProcessObservation::Alive(first));
    }

    #[cfg(windows)]
    #[test]
    fn native_exit_259_child_helper() {
        use std::io::Read;

        // In the ordinary suite this helper is inert. Only the owned child
        // selected by the exact filter and private test flag may exit 259.
        const HELPER: &str = "tests::native_exit_259_child_helper";
        const FLAG: &str = "MAIA_SUPERVISOR_OBSERVATION_EXIT259_HELPER";
        let args: Vec<_> = std::env::args().collect();
        let exact_helper =
            args.iter().any(|arg| arg == "--exact") && args.iter().any(|arg| arg == HELPER);
        if !exact_helper || std::env::var(FLAG).as_deref() != Ok("owned-child-v1") {
            return;
        }
        // The gate lets the parent record the live PID/birth tuple first.
        // Parent teardown closes the pipe; EOF returns without a special exit.
        let mut gate = [0_u8; 1];
        if std::io::stdin().read_exact(&mut gate).is_ok() && gate == [0xA5] {
            std::process::exit(259);
        }
    }

    #[cfg(windows)]
    #[test]
    fn observes_owned_native_child_exit_259_as_exited_with_unchanged_identity() {
        use std::io::Write;
        use std::process::{Child, Command, Stdio};
        use std::time::{Duration, Instant};

        // This regression alone owns a child. The production API has no
        // process-control operation. Cleanup never looks up or kills a PID.
        struct OwnedRegressionChild(Child);
        impl Drop for OwnedRegressionChild {
            fn drop(&mut self) {
                if matches!(self.0.try_wait(), Ok(Some(_))) {
                    return;
                }
                let _ = self.0.kill();
                let deadline = Instant::now() + Duration::from_secs(2);
                while Instant::now() < deadline {
                    match self.0.try_wait() {
                        Ok(Some(_)) | Err(_) => break,
                        Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                    }
                }
                // Dropping ChildStdin also gives an un-released helper EOF.
                // No unbounded wait is introduced on an exceptional path.
            }
        }

        let child = Command::new(std::env::current_exe().expect("current test executable"))
            .arg("--exact")
            .arg("tests::native_exit_259_child_helper")
            .arg("--nocapture")
            .arg("--test-threads=1")
            .env(
                "MAIA_SUPERVISOR_OBSERVATION_EXIT259_HELPER",
                "owned-child-v1",
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn the exact owned regression helper");
        let mut child = OwnedRegressionChild(child);
        let pid = child.0.id();
        let ProcessObservation::Alive(initial_identity) = observe(pid) else {
            panic!("owned helper was not observed alive before releasing its gate");
        };
        assert_eq!(initial_identity.pid, pid);
        assert!(initial_identity.birth_ticks > FILETIME_TO_DOTNET_TICKS);
        let mut gate = child.0.stdin.take().expect("owned helper input pipe");
        gate.write_all(&[0xA5]).expect("release the owned helper");
        drop(gate);

        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            match child.0.try_wait().expect("poll the owned helper") {
                Some(_) => break,
                None => {
                    assert!(Instant::now() < deadline, "owned helper exit deadline");
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
        // A known terminal child makes wait nonblocking. Keep Child itself
        // alive through the following observation, retaining its process
        // handle so the just-exited kernel object and PID remain owned.
        let status = child.0.wait().expect("wait on the known terminal helper");
        assert_eq!(status.code(), Some(259));
        assert_eq!(
            observe(pid),
            ProcessObservation::Exited {
                identity: initial_identity,
                exit_code: Some(259),
            }
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_platform_does_not_infer_process_absence() {
        assert_eq!(observe(42), ProcessObservation::Unavailable);
    }
}
