//! Bounded local executable inspection (M0.15.7d-prep.1). Development-evolution only.
//!
//! The preflight runs `<claude-exe> --version`. That is not a consultation and not
//! a model call, but it is still a child process, so it must be bounded: one
//! end-to-end deadline, capped capture, no wait for end-of-file that a descendant
//! can hold open.
//!
//! Since M0.15.7e.1 this is a thin adapter over the SAME bounded primitive that
//! runs the real Claude consultation, [`maia_claude_code::runner::StdProcessRunner`]
//! (see that module for the design: scratch files instead of pipes and reader
//! threads, size polling with kill-at-cap, a worker thread with a hard outer
//! timeout). There is one bounded process implementation, not two. The adapter only
//! translates between this module's spec and error types and the runner's, and
//! sends an empty stdin payload, so no prompt can reach the child.
//!
//! No shell is involved: the program is exactly the supplied path and the
//! arguments are separate strings. Credential names are removed from the child's
//! environment and only the supplied variables are set; values are never logged.
//!
//! Strict UTF-8 (M0.15.7e.2): the runner validates the original stdout bytes, so
//! malformed output is `InspectError::InvalidUtf8`, never a lossy repair.
//!
//! Residual (shared with the runner): killing the immediate child does not kill
//! its descendants, which needs `unsafe` job objects or process groups. A leftover
//! descendant cannot delay this call or grow its memory; it can only outlive it.

use maia_claude_code::runner::{
    CancellationToken, Invocation, OutputStream, ProcessError, ProcessRunner, StdProcessRunner,
};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// End to end: spawn, exit, capture and cleanup.
    pub deadline: Duration,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectSpec {
    /// Exactly this file; never resolved through `PATH`, never through a shell.
    pub program: PathBuf,
    pub args: Vec<String>,
    pub working_directory: PathBuf,
    /// A directory that can hold two short-lived capture files (created if missing).
    pub scratch_dir: PathBuf,
    /// Variables removed from the child's environment (credential scrub).
    pub env_remove: Vec<String>,
    /// Variables set on the child. Values are never logged.
    pub env_set: BTreeMap<String, String>,
    pub limits: Limits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspected {
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InspectError {
    ExecutableNotFound,
    SpawnFailed,
    /// The end-to-end deadline passed. The child was killed if it could be reached.
    Timeout,
    /// The child wrote more than its cap to this stream. It was killed.
    OutputTooLarge(Stream),
    /// The capture files could not be created or read.
    Scratch,
    /// The child exited in time but this stream was not valid UTF-8 (the original
    /// bytes are never repaired with replacement characters).
    InvalidUtf8(Stream),
    Io,
}

impl std::fmt::Display for InspectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutableNotFound => write!(f, "the executable was not found"),
            Self::SpawnFailed => write!(f, "the executable could not be started"),
            Self::Timeout => write!(f, "it did not finish before the deadline and was stopped"),
            Self::OutputTooLarge(Stream::Stdout) => {
                write!(f, "it wrote more standard output than the limit allows")
            }
            Self::OutputTooLarge(Stream::Stderr) => {
                write!(f, "it wrote more standard error than the limit allows")
            }
            Self::Scratch => write!(f, "the capture files could not be used"),
            Self::InvalidUtf8(Stream::Stdout) => {
                write!(f, "it printed standard output that is not valid UTF-8")
            }
            Self::InvalidUtf8(Stream::Stderr) => {
                write!(f, "it printed standard error that is not valid UTF-8")
            }
            Self::Io => write!(f, "waiting for it failed"),
        }
    }
}

/// Runs one bounded inspection. A trait so the live builder and the preflight can
/// be driven by a fake in tests; production uses [`StdInspector`].
pub trait Inspector: Send + Sync {
    fn inspect(&self, spec: &InspectSpec) -> Result<Inspected, InspectError>;
}

pub struct StdInspector;

impl Inspector for StdInspector {
    fn inspect(&self, spec: &InspectSpec) -> Result<Inspected, InspectError> {
        let invocation = Invocation {
            program: spec.program.clone(),
            args: spec.args.clone(),
            working_directory: spec.working_directory.clone(),
            env_remove: spec.env_remove.clone(),
            env_set: spec.env_set.clone(),
            // Empty: nothing, and in particular no consultation prompt, is sent.
            stdin_payload: String::new(),
            timeout: spec.limits.deadline,
            scratch_directory: spec.scratch_dir.clone(),
            max_stdout_bytes: spec.limits.max_stdout_bytes,
            max_stderr_bytes: spec.limits.max_stderr_bytes,
        };
        match StdProcessRunner.run(&invocation, &CancellationToken::new()) {
            Ok(outcome) if outcome.timed_out || outcome.cancelled => Err(InspectError::Timeout),
            Ok(outcome) => Ok(Inspected {
                exit_code: outcome.exit_code,
                stdout: outcome.stdout.into_bytes(),
                stderr: outcome.stderr.into_bytes(),
            }),
            Err(ProcessError::ExecutableNotFound) => Err(InspectError::ExecutableNotFound),
            Err(ProcessError::SpawnFailed) => Err(InspectError::SpawnFailed),
            Err(ProcessError::OutputTooLarge(OutputStream::Stdout)) => {
                Err(InspectError::OutputTooLarge(Stream::Stdout))
            }
            Err(ProcessError::OutputTooLarge(OutputStream::Stderr)) => {
                Err(InspectError::OutputTooLarge(Stream::Stderr))
            }
            Err(ProcessError::InvalidUtf8(OutputStream::Stdout)) => {
                Err(InspectError::InvalidUtf8(Stream::Stdout))
            }
            Err(ProcessError::InvalidUtf8(OutputStream::Stderr)) => {
                Err(InspectError::InvalidUtf8(Stream::Stderr))
            }
            Err(ProcessError::ScratchFailed) => Err(InspectError::Scratch),
            Err(ProcessError::IoFailure) => Err(InspectError::Io),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::time::Instant;

    /// The test binary is re-run as a harmless helper child: a real subprocess
    /// that never touches Claude, a model, the network or a shell. Its behaviour
    /// is chosen by an environment variable set through `InspectSpec::env_set`
    /// (the production mechanism), so the parent's own environment is untouched.
    /// Without the variable this test is an ordinary, instantly passing test.
    #[test]
    fn helper_child() {
        let Ok(mode) = std::env::var("MAIA_INSPECT_TEST_MODE") else {
            return;
        };
        let sleep = |secs: u64| std::thread::sleep(Duration::from_secs(secs));
        match mode.as_str() {
            "version" => println!("9.9.9 (inspect-test)"),
            "exit3" => std::process::exit(3),
            "echo_env" => println!(
                "marker={}",
                std::env::var("MAIA_ROUNDTABLE_INVOKE_ACTIVE").unwrap_or_default()
            ),
            "stdin_len" => {
                let mut input = Vec::new();
                let _ = std::io::stdin().read_to_end(&mut input);
                println!("stdin_len={}", input.len());
            }
            // Exits at once, but leaves a descendant that inherited stdout and
            // stderr and keeps them open for a while.
            "hold" => {
                let exe = std::env::current_exe().unwrap();
                let _ = Command::new(exe)
                    .args(["--exact", "inspect::tests::helper_child", "--nocapture"])
                    .env("MAIA_INSPECT_TEST_MODE", "sleep")
                    .stdin(Stdio::null())
                    .spawn();
                println!("9.9.9 (holder)");
            }
            // Malformed UTF-8 on stdout: a lone 0xff is never valid.
            "bad_utf8" => {
                use std::io::Write;
                let _ = std::io::stdout().write_all(&[b'9', 0xff, 0xfe, b'\n']);
            }
            "sleep" => sleep(HOLD_SECS),
            // Never finishes on its own within any test deadline.
            "hang" => sleep(HOLD_SECS),
            "flood_stdout" | "flood_stderr" => {
                use std::io::Write;
                let chunk = [b'x'; 64 * 1024];
                let to_err = mode == "flood_stderr";
                for _ in 0..4096 {
                    let ok = if to_err {
                        std::io::stderr().write_all(&chunk).is_ok()
                    } else {
                        std::io::stdout().write_all(&chunk).is_ok()
                    };
                    if !ok {
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    /// Long enough to outlast every deadline used below, short enough that a
    /// leftover helper ends by itself soon after the test run.
    const HOLD_SECS: u64 = 8;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("maia-inspect-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn spec(mode: &str, scratch_dir: &Path, deadline: Duration) -> InspectSpec {
        let mut env_set = BTreeMap::new();
        env_set.insert("MAIA_INSPECT_TEST_MODE".to_owned(), mode.to_owned());
        InspectSpec {
            program: std::env::current_exe().unwrap(),
            args: [
                "--exact",
                "inspect::tests::helper_child",
                "--nocapture",
                "--test-threads=1",
            ]
            .map(str::to_owned)
            .to_vec(),
            working_directory: std::env::temp_dir(),
            scratch_dir: scratch_dir.to_path_buf(),
            env_remove: vec!["ANTHROPIC_API_KEY".to_owned()],
            env_set,
            limits: Limits {
                deadline,
                max_stdout_bytes: 4096,
                max_stderr_bytes: 4096,
            },
        }
    }

    fn text(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).into_owned()
    }

    fn listing(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn a_normal_child_is_captured_with_its_exit_code_and_nothing_is_left_behind() {
        let dir = scratch("normal");
        let started = Instant::now();
        let got = StdInspector
            .inspect(&spec("version", &dir, Duration::from_secs(20)))
            .unwrap();
        assert_eq!(got.exit_code, Some(0));
        assert!(text(&got.stdout).contains("9.9.9 (inspect-test)"));
        assert!(started.elapsed() < Duration::from_secs(20));
        assert_eq!(listing(&dir), Vec::<String>::new(), "capture files removed");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// M0.15.7e.2: the strict UTF-8 validation of the `--version` output must
    /// survive the shared runner; malformed bytes are a classified failure, not
    /// U+FFFD text that then looks valid.
    #[test]
    fn malformed_utf8_in_version_stdout_is_a_classified_failure_not_a_lossy_success() {
        let dir = scratch("bad-utf8");
        let got = StdInspector.inspect(&spec("bad_utf8", &dir, Duration::from_secs(20)));
        assert_eq!(got, Err(InspectError::InvalidUtf8(Stream::Stdout)));
        assert_eq!(listing(&dir), Vec::<String>::new(), "capture files removed");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_failing_exit_code_is_reported_not_hidden() {
        let dir = scratch("exit3");
        let got = StdInspector
            .inspect(&spec("exit3", &dir, Duration::from_secs(20)))
            .unwrap();
        assert_eq!(got.exit_code, Some(3));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn only_the_supplied_variables_are_set_and_stdin_is_empty() {
        let dir = scratch("env");
        let mut s = spec("echo_env", &dir, Duration::from_secs(20));
        s.env_set
            .insert("MAIA_ROUNDTABLE_INVOKE_ACTIVE".to_owned(), "1".to_owned());
        let got = StdInspector.inspect(&s).unwrap();
        assert!(
            text(&got.stdout).contains("marker=1"),
            "{}",
            text(&got.stdout)
        );

        // Nothing can be written to the child: its stdin is the null device.
        let got = StdInspector
            .inspect(&spec("stdin_len", &dir, Duration::from_secs(20)))
            .unwrap();
        assert!(text(&got.stdout).contains("stdin_len=0"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The regression the review asked for. The helper exits immediately but a
    /// descendant it started inherits stdout and stderr and holds them open for
    /// `HOLD_SECS`. With pipes and a joined reader thread that holds the caller
    /// for the whole time; here the call must return as soon as the child exits.
    #[test]
    fn a_descendant_holding_the_inherited_handles_cannot_hold_the_deadline_open() {
        let dir = scratch("hold");
        let started = Instant::now();
        let got = StdInspector
            .inspect(&spec("hold", &dir, Duration::from_secs(5)))
            .expect("the immediate child exited, so this must succeed");
        let took = started.elapsed();
        assert_eq!(got.exit_code, Some(0));
        assert!(text(&got.stdout).contains("9.9.9 (holder)"));
        assert!(
            took < Duration::from_secs(HOLD_SECS - 3),
            "returned after {took:?}; it must not wait for the descendant ({HOLD_SECS}s)"
        );
        // Best effort: the descendant may still hold a handle, so no listing check.
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_child_that_never_finishes_is_stopped_at_the_deadline() {
        let dir = scratch("hang");
        let deadline = Duration::from_millis(1500);
        let started = Instant::now();
        let got = StdInspector.inspect(&spec("hang", &dir, deadline));
        let took = started.elapsed();
        assert_eq!(got, Err(InspectError::Timeout));
        assert!(
            took < deadline + Duration::from_secs(3),
            "took {took:?} against a {deadline:?} deadline"
        );
        assert_eq!(listing(&dir), Vec::<String>::new(), "capture files removed");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn output_beyond_the_cap_is_rejected_and_the_child_stopped_quickly() {
        for (mode, stream) in [
            ("flood_stdout", Stream::Stdout),
            ("flood_stderr", Stream::Stderr),
        ] {
            let dir = scratch(mode);
            let started = Instant::now();
            let got = StdInspector.inspect(&spec(mode, &dir, Duration::from_secs(20)));
            assert_eq!(got, Err(InspectError::OutputTooLarge(stream)), "{mode}");
            assert!(
                started.elapsed() < Duration::from_secs(15),
                "{mode}: stopped promptly, not run to the deadline"
            );
            assert_eq!(listing(&dir), Vec::<String>::new(), "{mode}: files removed");
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }

    #[test]
    fn a_missing_executable_and_an_unusable_scratch_directory_are_classified() {
        let dir = scratch("classify");
        let mut s = spec("version", &dir, Duration::from_secs(5));
        s.program = dir.join("no-such-program.exe");
        assert_eq!(
            StdInspector.inspect(&s),
            Err(InspectError::ExecutableNotFound)
        );
        assert_eq!(listing(&dir), Vec::<String>::new(), "capture files removed");

        let mut s = spec("version", &dir, Duration::from_secs(5));
        // A regular file where the scratch directory has to be is unusable. (A
        // merely missing directory is created, as the shared runner does.)
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, "x").unwrap();
        s.scratch_dir = blocker.join("sub");
        assert_eq!(StdInspector.inspect(&s), Err(InspectError::Scratch));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn no_shell_is_involved_metacharacters_are_plain_arguments() {
        // A program name with shell metacharacters is just a name that is not
        // found; nothing is interpreted.
        let dir = scratch("noshell");
        let mut s = spec("version", &dir, Duration::from_secs(5));
        s.program = dir.join("a & echo pwned & b.exe");
        assert_eq!(
            StdInspector.inspect(&s),
            Err(InspectError::ExecutableNotFound)
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
