//! M0.15.7e.1: the real consultation runner must be bounded.
//!
//! The first live Round Table run hung for over fifteen minutes on a 180 second
//! timeout. `StdProcessRunner` joined its stdout/stderr reader threads after the
//! child was killed, so a descendant that inherited the pipe handles held the
//! caller for the descendant's whole lifetime, and the capture was unbounded.
//!
//! These tests reproduce that failure mode with REAL, HARMLESS subprocesses: the
//! test binary itself is re-run as a helper child (mode chosen through the
//! invocation's own `env_set`). No Claude, no model, no network, no shell.
//!
//! Measured against the previous implementation with the same helpers: a child
//! that hung past a 2 second timeout with a descendant holding the handles made
//! `run` return after 8.03 seconds (the descendant's lifetime), and a stdout
//! flood was buffered whole (268 MB). Here every case returns within its
//! deadline plus a small grace.
#![cfg(feature = "development-evolution")]

use maia_claude_code::runner::{
    CancellationToken, Invocation, OutputStream, ProcessError, ProcessRunner, StdProcessRunner,
};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// Long enough to outlast every deadline below with a wide margin, short enough
/// that a leftover helper ends by itself soon after the test run.
const HOLD_SECS: u64 = 10;

/// The helper child. Without the mode variable this is an ordinary, instantly
/// passing test; with it, the test binary behaves as the chosen harmless child.
#[test]
fn helper_child() {
    let Ok(mode) = std::env::var("MAIA_RUNNER_TEST_MODE") else {
        return;
    };
    let sleep = |secs: u64| std::thread::sleep(Duration::from_secs(secs));
    let spawn_holder = || {
        // A descendant that inherits stdout and stderr and keeps them open.
        let _ = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "helper_child", "--nocapture", "--test-threads=1"])
            .env("MAIA_RUNNER_TEST_MODE", "sleep")
            .stdin(Stdio::null())
            .spawn();
    };
    match mode.as_str() {
        "ok" => println!("runner-test-ok"),
        "exit3" => std::process::exit(3),
        // Exits at once, leaving a descendant that holds the inherited handles.
        "hold" => {
            spawn_holder();
            println!("held-open-by-a-descendant");
        }
        // Never finishes within any test deadline AND leaves such a descendant.
        "hang_with_holder" => {
            spawn_holder();
            sleep(HOLD_SECS + 15);
        }
        "hang" | "sleep" => sleep(HOLD_SECS),
        "flood_stdout" | "flood_stderr" => {
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
        "stdin_len" => {
            let mut input = Vec::new();
            let _ = std::io::stdin().read_to_end(&mut input);
            println!("stdin_len={}", input.len());
        }
        "env_probe" => {
            let present = |k: &str| std::env::var_os(k).is_some();
            println!(
                "username_present={} user_present={} marker={}",
                present("USERNAME"),
                present("USER"),
                std::env::var("MAIA_TEST_MARKER").unwrap_or_default()
            );
        }
        _ => {}
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("maia-runner-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn invocation(mode: &str, scratch_dir: &Path, timeout: Duration) -> Invocation {
    let mut env_set = BTreeMap::new();
    env_set.insert("MAIA_RUNNER_TEST_MODE".to_owned(), mode.to_owned());
    Invocation {
        program: std::env::current_exe().unwrap(),
        args: ["--exact", "helper_child", "--nocapture", "--test-threads=1"]
            .map(str::to_owned)
            .to_vec(),
        working_directory: std::env::temp_dir(),
        env_remove: vec![],
        env_set,
        stdin_payload: String::new(),
        timeout,
        scratch_directory: scratch_dir.to_path_buf(),
        max_stdout_bytes: 4096,
        max_stderr_bytes: 4096,
    }
}

fn listing(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect()
}

/// The slack a deadline may legitimately take: poll interval, reap, and the
/// runner's own grace, with room for a loaded machine.
const SLACK: Duration = Duration::from_secs(3);

#[test]
fn a_normal_child_is_captured_and_leaves_no_capture_files() {
    let dir = scratch("normal");
    let out = StdProcessRunner
        .run(
            &invocation("ok", &dir, Duration::from_secs(20)),
            &CancellationToken::new(),
        )
        .unwrap();
    assert_eq!(out.exit_code, Some(0));
    assert!(out.stdout.contains("runner-test-ok"));
    assert!(!out.timed_out && !out.cancelled);
    assert_eq!(listing(&dir), Vec::<String>::new(), "capture files removed");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A. The immediate child exits at once; a descendant keeps stdout and stderr
/// open. The runner must return when the child exits, not when the holder does.
#[test]
fn a_immediate_child_exits_while_a_descendant_keeps_the_output_handles_open() {
    let dir = scratch("hold");
    let started = Instant::now();
    let out = StdProcessRunner
        .run(
            &invocation("hold", &dir, Duration::from_secs(5)),
            &CancellationToken::new(),
        )
        .expect("the child exited, so this is a normal outcome");
    let took = started.elapsed();
    assert_eq!(out.exit_code, Some(0));
    assert!(out.stdout.contains("held-open-by-a-descendant"));
    assert!(!out.timed_out);
    assert!(
        took < Duration::from_secs(HOLD_SECS - 4),
        "returned after {took:?}; it must not wait for the descendant ({HOLD_SECS}s)"
    );
    let _ = std::fs::remove_dir_all(&dir); // the holder may still have a handle
}

/// B. The child exceeds the timeout. It is stopped and reported as timed out,
/// within the deadline plus grace.
#[test]
fn b_a_child_that_exceeds_the_timeout_is_stopped_and_reported_within_the_bound() {
    let dir = scratch("timeout");
    let timeout = Duration::from_millis(1500);
    let started = Instant::now();
    let out = StdProcessRunner
        .run(
            &invocation("hang", &dir, timeout),
            &CancellationToken::new(),
        )
        .expect("a timeout is an outcome, not an error");
    let took = started.elapsed();
    assert!(out.timed_out);
    assert_eq!(out.exit_code, None);
    assert!(took >= timeout, "stopped before its deadline: {took:?}");
    assert!(
        took < timeout + SLACK,
        "took {took:?} against a {timeout:?} timeout"
    );
    assert_eq!(listing(&dir), Vec::<String>::new(), "capture files removed");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// C. The exact failure of the live run: the child hangs past the timeout AND a
/// descendant retains the inherited output handles after the child is killed. The
/// old runner waited for the descendant; this one returns inside the bound.
#[test]
fn c_a_descendant_retaining_the_handles_cannot_hold_a_timed_out_call_open() {
    let dir = scratch("hang-holder");
    let timeout = Duration::from_secs(2);
    let started = Instant::now();
    let out = StdProcessRunner
        .run(
            &invocation("hang_with_holder", &dir, timeout),
            &CancellationToken::new(),
        )
        .expect("a timeout is an outcome, not an error");
    let took = started.elapsed();
    assert!(out.timed_out);
    assert!(
        took < timeout + SLACK,
        "took {took:?} against a {timeout:?} timeout"
    );
    assert!(
        took < Duration::from_secs(HOLD_SECS - 4),
        "returned after {took:?}; a descendant kept the handles for {HOLD_SECS}s"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// D and E. A flood on either stream is rejected, the child is stopped quickly,
/// nothing near the flood size is held in memory, and nothing is left on disk.
#[test]
fn d_e_output_floods_are_rejected_and_stopped_without_unbounded_capture() {
    for (mode, stream) in [
        ("flood_stdout", OutputStream::Stdout),
        ("flood_stderr", OutputStream::Stderr),
    ] {
        let dir = scratch(mode);
        let started = Instant::now();
        let got = StdProcessRunner.run(
            &invocation(mode, &dir, Duration::from_secs(20)),
            &CancellationToken::new(),
        );
        assert_eq!(got, Err(ProcessError::OutputTooLarge(stream)), "{mode}");
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "{mode}: stopped promptly, not run to the deadline"
        );
        assert_eq!(listing(&dir), Vec::<String>::new(), "{mode}: files removed");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[test]
fn a_small_exact_cap_is_enforced_not_rounded_up() {
    let dir = scratch("cap");
    // The helper prints far more than 16 bytes.
    let mut inv = invocation("ok", &dir, Duration::from_secs(20));
    inv.max_stdout_bytes = 4;
    let got = StdProcessRunner.run(&inv, &CancellationToken::new());
    assert_eq!(got, Err(ProcessError::OutputTooLarge(OutputStream::Stdout)));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn cancellation_stops_a_running_child_promptly() {
    let dir = scratch("cancel");
    let cancel = CancellationToken::new();
    let flag = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        flag.cancel();
    });
    let started = Instant::now();
    let out = StdProcessRunner
        .run(&invocation("hang", &dir, Duration::from_secs(20)), &cancel)
        .unwrap();
    assert!(out.cancelled && !out.timed_out);
    assert!(started.elapsed() < SLACK);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_prompt_is_delivered_on_stdin_in_full_even_beyond_the_pipe_buffer() {
    let dir = scratch("stdin");
    let mut inv = invocation("stdin_len", &dir, Duration::from_secs(20));
    inv.stdin_payload = "p".repeat(300_000);
    let out = StdProcessRunner
        .run(&inv, &CancellationToken::new())
        .unwrap();
    assert!(out.stdout.contains("stdin_len=300000"), "{}", out.stdout);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_credential_scrub_and_the_supplied_variables_reach_the_child() {
    let dir = scratch("env");
    let mut inv = invocation("env_probe", &dir, Duration::from_secs(20));
    inv.env_set
        .insert("MAIA_TEST_MARKER".to_owned(), "1".to_owned());
    inv.env_remove = vec!["USERNAME".to_owned(), "USER".to_owned()];
    let out = StdProcessRunner
        .run(&inv, &CancellationToken::new())
        .unwrap();
    assert!(
        out.stdout.contains("username_present=false"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("user_present=false"), "{}", out.stdout);
    assert!(out.stdout.contains("marker=1"), "{}", out.stdout);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_nonzero_exit_and_the_classified_failures_are_reported() {
    let dir = scratch("classify");
    let out = StdProcessRunner
        .run(
            &invocation("exit3", &dir, Duration::from_secs(20)),
            &CancellationToken::new(),
        )
        .unwrap();
    assert_eq!(out.exit_code, Some(3));

    let mut missing = invocation("ok", &dir, Duration::from_secs(5));
    missing.program = dir.join("no-such-program.exe");
    assert_eq!(
        StdProcessRunner.run(&missing, &CancellationToken::new()),
        Err(ProcessError::ExecutableNotFound)
    );
    assert_eq!(listing(&dir), Vec::<String>::new(), "capture files removed");

    // A regular file where the scratch directory has to be.
    let blocker = dir.join("blocker");
    std::fs::write(&blocker, "x").unwrap();
    let mut unusable = invocation("ok", &dir, Duration::from_secs(5));
    unusable.scratch_directory = blocker.join("sub");
    assert_eq!(
        StdProcessRunner.run(&unusable, &CancellationToken::new()),
        Err(ProcessError::ScratchFailed)
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
