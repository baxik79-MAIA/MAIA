//! M0.15.7e.1: the REAL provider over the REAL runner, with a harmless stand-in
//! for the `claude` executable. Reproduces the shape of the first live failure:
//! a launcher that hangs past the consultation timeout while a descendant of it
//! keeps the inherited output handles open after the launcher is killed.
//!
//! The stand-in is a two-line script (a `.cmd` on Windows, `sh` elsewhere). It
//! ignores every argument, never contacts a model or the network, and its
//! descendants are `ping` / `sleep`. The provider must return `Timeout` within its
//! deadline plus a small grace, persist a truthful audit record, and never retry.
#![cfg(feature = "development-evolution")]

use maia_claude_code::{
    ClaudeCodeConfig, ClaudeCodeError, ClaudeCodeProvider,
    audit::FileAuditSink,
    contract::ConsultationPacket,
    runner::{CancellationToken, StdProcessRunner},
};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// How long the descendant keeps the inherited handles open. Much longer than the
/// deadline below, so waiting for it would be visible.
const HOLDER_SECS: u64 = 13;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("maia-provreal-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[cfg(windows)]
fn hanging_launcher(dir: &Path) -> PathBuf {
    let script = dir.join("fake-claude.cmd");
    std::fs::write(
        &script,
        format!(
            "@echo off\r\nstart \"\" /b ping -n {} 127.0.0.1\r\nping -n {} 127.0.0.1 > nul\r\n",
            HOLDER_SECS,
            HOLDER_SECS + 2
        ),
    )
    .unwrap();
    script
}

#[cfg(unix)]
fn hanging_launcher(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let script = dir.join("fake-claude.sh");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nsleep {} &\nsleep {}\n",
            HOLDER_SECS,
            HOLDER_SECS + 2
        ),
    )
    .unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();
    script
}

#[test]
fn a_hung_launcher_with_a_descendant_holding_the_handles_times_out_within_the_bound() {
    let dir = temp("hang");
    let audit_dir = dir.join("audit");
    let mut config = ClaudeCodeConfig::new(hanging_launcher(&dir), &dir, &audit_dir);
    config.timeout = Duration::from_secs(2);
    let provider = ClaudeCodeProvider::new_without_recursion_check(
        "claude-code",
        config,
        "2.1.278".into(),
        StdProcessRunner,
        FileAuditSink::new(&audit_dir),
    )
    .expect("provider constructs");
    let packet = ConsultationPacket::new("consult-1", "claude-code", "reviewer", "synthetic", 2);

    let started = Instant::now();
    let result = provider.consult(&packet, &CancellationToken::new());
    let took = started.elapsed();

    assert_eq!(result.err(), Some(ClaudeCodeError::Timeout));
    assert!(
        took < Duration::from_secs(2) + Duration::from_secs(4),
        "took {took:?} against a 2s timeout"
    );
    assert!(
        took < Duration::from_secs(HOLDER_SECS - 3),
        "took {took:?}; a descendant kept the handles for {HOLDER_SECS}s and must not be waited for"
    );

    // The audit record exists and is truthful: it says the consultation timed out.
    let records: Vec<_> = std::fs::read_dir(&audit_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    assert_eq!(records.len(), 1, "exactly one audit record: {records:?}");
    let record: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&records[0]).unwrap()).unwrap();
    assert_eq!(record["outcome"], "Timeout");
    assert_eq!(record["timed_out"], true);
    assert_eq!(record["consultation_id"], "consult-1");
    let _ = std::fs::remove_dir_all(&dir); // the descendant may still hold a handle
}
