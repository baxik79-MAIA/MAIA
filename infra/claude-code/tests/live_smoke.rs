//! Opt-in live smoke test against the locally authenticated Claude Code CLI.
//!
//! Skipped by default. Enable with `MAIA_LIVE_CLAUDE_CODE=1` and the
//! `development-evolution` feature. Read-only: repository tools are denied, so
//! the invocation cannot modify any canonical project file.
//!
//! Canonical contract: spec/claude_code_provider.yaml#live_smoke

#![cfg(feature = "development-evolution")]

use maia_claude_code::{
    ClaudeCodeConfig, ClaudeCodeProvider,
    audit::FileAuditSink,
    contract::ConsultationPacket,
    runner::{CancellationToken, StdProcessRunner},
};
use std::time::Duration;

fn enabled() -> bool {
    std::env::var("MAIA_LIVE_CLAUDE_CODE").as_deref() == Ok("1")
}

fn cli_version() -> String {
    // Discovery: the adapter records the version it actually invoked.
    let out = std::process::Command::new("claude")
        .arg("--version")
        .output()
        .expect("claude --version");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn live_consultation_returns_a_schema_valid_result() {
    if !enabled() {
        eprintln!("skipped: set MAIA_LIVE_CLAUDE_CODE=1 to run the live smoke test");
        return;
    }

    let repo = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let audit_dir = std::path::Path::new(repo).join("target/claude-code-audit");

    let mut config = ClaudeCodeConfig::new("claude", repo, &audit_dir);
    config.timeout = Duration::from_secs(180);
    config.model = Some("claude-haiku-4-5-20251001".into());

    let provider = ClaudeCodeProvider::new(
        "claude-code",
        config,
        cli_version(),
        StdProcessRunner,
        FileAuditSink::new(&audit_dir),
    )
    .expect("provider constructs on an authenticated development host");

    let mut packet = ConsultationPacket::new(
        "live-smoke-1",
        "claude-code",
        "reviewer",
        "State in one sentence whether a reasoning-only consultation participant \
         should ever be granted unattended file-write authority. This is a synthetic, \
         non-sensitive question; do not read or modify any file.",
        180,
    );
    packet.constraints = vec!["read_only".into(), "no_file_modification".into()];

    let result = provider
        .consult(&packet, &CancellationToken::new())
        .expect("live consultation succeeds");

    assert_eq!(result.consultation_id, "live-smoke-1");
    assert_eq!(result.participant_id, "claude-code");
    assert!(!result.response.trim().is_empty());
    assert!(!result.recommendation.trim().is_empty());
    // Subscription-backed: token counts may be known, monetary cost is not.
    assert!(!result.execution.cost_known);
    assert!(!result.execution.correlation_id.is_empty());
    assert!(result.execution.session_id.is_some());

    // Audit record exists on disk and preserves the raw envelope.
    let audit_file = audit_dir.join(format!("{}.json", result.execution.correlation_id));
    let persisted = std::fs::read_to_string(&audit_file).expect("audit record written");
    assert!(persisted.contains("maia.consultation_result.v1"));
    assert!(persisted.contains("ANTHROPIC_API_KEY")); // recorded as a scrubbed *name*
    assert!(!persisted.contains("sk-ant-")); // never a value

    eprintln!(
        "live smoke ok: model={:?} session={:?} duration_ms={}",
        result.execution.model, result.execution.session_id, result.execution.duration_ms
    );
}
