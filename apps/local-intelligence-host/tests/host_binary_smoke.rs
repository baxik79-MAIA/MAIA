//! M0.15.14 §14: the actual compiled `local-intelligence-host` binary, run
//! as a real subprocess, proving HOST START -> RECORDER START ->
//! AUTOMATIC OBSERVATION ACCUMULATION -> GRACEFUL STOP end to end. Uses the
//! binary's `--seconds` bounded-run mode (matching
//! `local-intelligence-recorder run --seconds N`'s existing convention) so
//! no signal needs to be sent to the child process, and lives here (rather
//! than in `src/main.rs`'s own inline tests) because `CARGO_BIN_EXE_*` is
//! only set by Cargo for integration tests under `tests/`.
//!
//! No provider/model call, no Round Table call, no service/autostart
//! installation.
use maia_local_intelligence_hypothesis_ledger::{Ledger, RetentionPolicy as LedgerRetentionPolicy};
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_test_dir(label: &str) -> std::path::PathBuf {
    let n = TEST_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "maia-li-host-binary-smoke-{label}-{}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp test dir");
    dir
}

#[test]
fn actual_host_binary_smoke_test_start_record_stop() {
    let dir = temp_test_dir("smoke");
    let health_path = dir.join("health.sqlite3");
    let ledger_path = dir.join("ledger.sqlite3");
    let exe = env!("CARGO_BIN_EXE_maia-local-intelligence-host");
    let output = std::process::Command::new(exe)
        .arg("--seconds")
        .arg("1")
        .env("MAIA_LOCAL_INTELLIGENCE_HEALTH_DB", &health_path)
        .env("MAIA_LOCAL_INTELLIGENCE_LEDGER_DB", &ledger_path)
        .env("MAIA_LOCAL_INTELLIGENCE_HOST_CADENCE_MS", "50")
        .env("MAIA_LOCAL_INTELLIGENCE_HOST_WINDOW_MS", "5000")
        .env(
            "MAIA_LOCAL_INTELLIGENCE_HOST_RECORDER_ID",
            "binary-smoke-test",
        )
        .output()
        .expect("run the actual host binary as a subprocess");
    assert!(
        output.status.success(),
        "host binary must exit cleanly\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Recorder started."), "stdout: {stdout}");
    assert!(stdout.contains("Recorder stopped:"), "stdout: {stdout}");
    let ledger = Ledger::open(&ledger_path, LedgerRetentionPolicy::default()).expect("open ledger");
    assert!(
        !ledger.latest_n(10).unwrap().is_empty(),
        "the actual host binary must have automatically recorded at least one observation \
         within its bounded run -- stdout was:\n{stdout}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
