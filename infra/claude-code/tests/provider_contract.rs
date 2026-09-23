//! Deterministic contract tests for the Claude Code provider.
//!
//! No network, no credentials, no real subprocess: the process runner is faked
//! so every failure mode is exercised reproducibly.
//!
//! Acceptance: spec/acceptance.yaml#releases.m0_12_claude_code_subscription_provider

// The provider is a DEVELOPMENT_EVOLUTION capability. In a DEPLOYMENT_LOCKED
// build no constructor can produce one, so the invocation contract is only
// meaningful — and only compilable — with the feature enabled.
#![cfg(feature = "development-evolution")]

use maia_claude_code::{
    AUTH_MODE_SUBSCRIPTION, ChildMarker, ClaudeCodeConfig, ClaudeCodeError, ClaudeCodeProvider,
    DEFAULT_DENIED_TOOLS, RECURSION_MARKER, SCRUBBED_ENVIRONMENT,
    audit::{AuditError, AuditRecord, AuditSink},
    capability_present,
    contract::{ConsultationPacket, ContractViolation},
    runner::{
        CancellationToken, Invocation, OutputStream, ProcessError, ProcessOutcome, ProcessRunner,
    },
};
use std::{
    sync::{Mutex, OnceLock},
    time::Duration,
};

// ---------------------------------------------------------------- test doubles

struct FakeRunner {
    reply: Mutex<Option<Result<ProcessOutcome, ProcessError>>>,
    seen: Mutex<Option<Invocation>>,
}

impl FakeRunner {
    fn new(reply: Result<ProcessOutcome, ProcessError>) -> Self {
        Self {
            reply: Mutex::new(Some(reply)),
            seen: Mutex::new(None),
        }
    }
    fn ok(stdout: &str) -> Self {
        Self::new(Ok(outcome(0, stdout, "")))
    }
}

impl ProcessRunner for FakeRunner {
    fn run(
        &self,
        invocation: &Invocation,
        _cancel: &CancellationToken,
    ) -> Result<ProcessOutcome, ProcessError> {
        *self.seen.lock().unwrap() = Some(invocation.clone());
        self.reply
            .lock()
            .unwrap()
            .take()
            .expect("runner called twice")
    }
}

fn outcome(code: i32, stdout: &str, stderr: &str) -> ProcessOutcome {
    ProcessOutcome {
        exit_code: Some(code),
        stdout: stdout.into(),
        stderr: stderr.into(),
        duration: Duration::from_millis(12),
        timed_out: false,
        cancelled: false,
    }
}

#[derive(Default)]
struct RecordingAudit {
    records: Mutex<Vec<AuditRecord>>,
    fail: bool,
}

impl RecordingAudit {
    fn failing() -> Self {
        Self {
            fail: true,
            ..Default::default()
        }
    }
}

impl AuditSink for RecordingAudit {
    fn persist(&self, record: &AuditRecord) -> Result<(), AuditError> {
        self.records.lock().unwrap().push(record.clone());
        if self.fail {
            return Err(AuditError::WriteFailed);
        }
        Ok(())
    }
}

// ------------------------------------------------------------------- fixtures

fn config() -> ClaudeCodeConfig {
    let mut config = ClaudeCodeConfig::new("claude", "C:/MAIA/repo", "C:/MAIA/repo/target/audit");
    config.timeout = Duration::from_secs(30);
    config
}

fn packet() -> ConsultationPacket {
    ConsultationPacket::new("consult-1", "claude-code", "reviewer", "synthetic task", 30)
}

/// The recursion marker is process-global; serialize the tests that touch it.
fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn provider(
    runner: FakeRunner,
    audit: RecordingAudit,
) -> ClaudeCodeProvider<FakeRunner, RecordingAudit> {
    ClaudeCodeProvider::new_without_recursion_check(
        "claude-code",
        config(),
        "2.1.273".into(),
        runner,
        audit,
    )
    .expect("provider constructs")
}

fn envelope(result_body: &str) -> String {
    serde_json::json!({
        "type": "result",
        "subtype": "success",
        "is_error": false,
        "session_id": "sess-abc",
        "result": result_body,
        "usage": {"input_tokens": 11, "output_tokens": 22},
        "modelUsage": {"claude-sonnet-5": {"inputTokens": 11}}
    })
    .to_string()
}

fn valid_body() -> String {
    serde_json::json!({
        "schema_version": "maia.consultation_result.v1",
        "consultation_id": "consult-1",
        "participant_id": "claude-code",
        "response": "The seam is sound.",
        "findings": [{"id": "F1", "summary": "no blocking defect", "severity": "low"}],
        "evidence": [{"id": "E1", "source_ref": "spec/round_table.yaml"}],
        "risks": ["host dependent"],
        "disagreements": [],
        "recommendation": "proceed",
        "confidence": "high"
    })
    .to_string()
}

// ------------------------------------------------------- C001 discovery / auth

#[test]
fn c001_missing_executable_fails_closed() {
    let provider = provider(
        FakeRunner::new(Err(ProcessError::ExecutableNotFound)),
        RecordingAudit::default(),
    );
    assert_eq!(
        provider
            .consult(&packet(), &CancellationToken::new())
            .unwrap_err(),
        ClaudeCodeError::ExecutableNotFound
    );
}

#[test]
fn c001_missing_authentication_is_reported_as_not_authenticated() {
    let provider = provider(
        FakeRunner::new(Ok(outcome(1, "", "Error: Not logged in. Run /login"))),
        RecordingAudit::default(),
    );
    assert_eq!(
        provider
            .consult(&packet(), &CancellationToken::new())
            .unwrap_err(),
        ClaudeCodeError::NotAuthenticated
    );
}

#[test]
fn c001_cli_version_is_captured_in_execution_metadata() {
    let provider = provider(
        FakeRunner::ok(&envelope(&valid_body())),
        RecordingAudit::default(),
    );
    let result = provider
        .consult(&packet(), &CancellationToken::new())
        .unwrap();
    assert_eq!(result.execution.cli_version, "2.1.273");
    assert_eq!(result.execution.auth_mode, AUTH_MODE_SUBSCRIPTION);
    assert_eq!(result.execution.model.as_deref(), Some("claude-sonnet-5"));
}

// ------------------------------------- C002 invocation shape / working directory

#[test]
fn c002_invocation_is_print_mode_json_with_explicit_working_directory() {
    let runner = FakeRunner::ok(&envelope(&valid_body()));
    let provider = provider(runner, RecordingAudit::default());
    provider
        .consult(&packet(), &CancellationToken::new())
        .unwrap();

    let seen = provider_invocation(&provider);
    assert!(seen.args.contains(&"-p".to_string()));
    assert_eq!(
        seen.args
            .windows(2)
            .find(|w| w[0] == "--output-format")
            .map(|w| w[1].clone()),
        Some("json".to_string())
    );
    assert_eq!(seen.working_directory.to_string_lossy(), "C:/MAIA/repo");
    // The packet travels on stdin, never inside a shell string.
    assert!(seen.stdin_payload.contains("synthetic task"));
    assert!(seen.program.to_string_lossy().contains("claude"));
}

fn provider_invocation(provider: &ClaudeCodeProvider<FakeRunner, RecordingAudit>) -> Invocation {
    // Reach through the provider to the fake it was built with.
    provider_runner(provider)
        .seen
        .lock()
        .unwrap()
        .clone()
        .expect("invocation captured")
}

fn provider_runner(provider: &ClaudeCodeProvider<FakeRunner, RecordingAudit>) -> &FakeRunner {
    provider.runner_for_test()
}

// ------------------------------------------------ C003 no API-key dependency

#[test]
fn c003_child_environment_scrubs_anthropic_api_key() {
    let provider = provider(
        FakeRunner::ok(&envelope(&valid_body())),
        RecordingAudit::default(),
    );
    provider
        .consult(&packet(), &CancellationToken::new())
        .unwrap();
    let seen = provider_invocation(&provider);

    for key in SCRUBBED_ENVIRONMENT {
        assert!(
            seen.env_remove.contains(&key.to_string()),
            "{key} must be scrubbed from the child environment"
        );
    }
    assert!(seen.env_remove.contains(&"ANTHROPIC_API_KEY".to_string()));
    // The adapter must never *inject* a key.
    assert!(
        seen.env_set.keys().all(|k| !k.starts_with("ANTHROPIC_")),
        "adapter must not set any ANTHROPIC_* variable"
    );
}

#[test]
fn c003_provider_succeeds_while_an_api_key_is_present_in_the_parent_environment() {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY-equivalent note: single-threaded section guarded by env_lock.
    unsafe { std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-fixture-not-a-real-key") };
    let provider = provider(
        FakeRunner::ok(&envelope(&valid_body())),
        RecordingAudit::default(),
    );
    let result = provider.consult(&packet(), &CancellationToken::new());
    unsafe { std::env::remove_var("ANTHROPIC_API_KEY") };
    assert!(result.is_ok(), "provider must not depend on an API key");
}

// -------------------------------------------------------- C004 permission model

#[test]
fn c004_never_uses_bypass_permissions_and_denies_tools_explicitly() {
    let provider = provider(
        FakeRunner::ok(&envelope(&valid_body())),
        RecordingAudit::default(),
    );
    provider
        .consult(&packet(), &CancellationToken::new())
        .unwrap();
    let seen = provider_invocation(&provider);

    let joined = seen.args.join(" ");
    assert!(
        !joined.contains("bypassPermissions"),
        "bypassPermissions must never be passed: {joined}"
    );
    assert!(
        !seen
            .args
            .iter()
            .any(|a| a == "--allowedTools" || a == "--allowed-tools"),
        "an empty allowlist does not deny anything and must not be relied on"
    );
    assert!(seen.args.iter().any(|a| a == "--disallowed-tools"));
    for tool in DEFAULT_DENIED_TOOLS {
        assert!(
            seen.args.contains(&tool.to_string()),
            "{tool} must be denied"
        );
    }
    // Read-only tools are denied unless explicitly enabled.
    for tool in ["Read", "Glob", "Grep"] {
        assert!(seen.args.contains(&tool.to_string()));
    }
    // Egress is always denied.
    assert!(seen.args.contains(&"WebFetch".to_string()));
    assert!(seen.args.contains(&"WebSearch".to_string()));
}

// -------------------------------------------- C005 result validation / failures

#[test]
fn c005_happy_path_returns_validated_result() {
    let provider = provider(
        FakeRunner::ok(&envelope(&valid_body())),
        RecordingAudit::default(),
    );
    let result = provider
        .consult(&packet(), &CancellationToken::new())
        .unwrap();
    assert_eq!(result.consultation_id, "consult-1");
    assert_eq!(result.recommendation, "proceed");
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.execution.input_tokens, Some(11));
    // Unknown cost is never zero.
    assert!(!result.execution.cost_known);
}

#[test]
fn c005_timeout_fails_closed_and_is_transient() {
    let mut timed = outcome(0, "", "");
    timed.timed_out = true;
    timed.exit_code = None;
    let provider = provider(FakeRunner::new(Ok(timed)), RecordingAudit::default());
    assert_eq!(
        provider
            .consult(&packet(), &CancellationToken::new())
            .unwrap_err(),
        ClaudeCodeError::Timeout
    );
}

/// A timed-out consultation is a failed contribution, transient, audited as a
/// timeout, and asked exactly once: the provider never retries.
#[test]
fn c005_a_timeout_is_audited_once_and_never_retried() {
    let mut timed = outcome(0, "", "");
    timed.timed_out = true;
    timed.exit_code = None;
    let provider = provider(FakeRunner::new(Ok(timed)), RecordingAudit::default());
    let _ = provider.consult(&packet(), &CancellationToken::new());
    // `FakeRunner` panics on a second call ("runner called twice"), and the audit
    // sink holds exactly one record, labelled truthfully.
    let records = provider.audit_for_test().records.lock().unwrap().clone();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].outcome, "Timeout");
    assert!(records[0].timed_out);
}

/// M0.15.7e.2: a result that completed after the deadline reaches the provider as
/// a timed-out outcome (the runner never reports late success). The provider gives
/// it exactly the timeout treatment even if the outcome still carries a valid
/// envelope: no response, one audit record labelled Timeout, one runner call.
#[test]
fn c005_a_late_completion_gets_exactly_the_timeout_treatment_and_no_late_success_audit() {
    let mut late = outcome(0, &envelope(&valid_body()), "");
    late.timed_out = true;
    let provider = provider(FakeRunner::new(Ok(late)), RecordingAudit::default());
    let err = provider
        .consult(&packet(), &CancellationToken::new())
        .unwrap_err();
    assert_eq!(err, ClaudeCodeError::Timeout);
    assert_eq!(
        err.failure_kind(),
        maia_roundtable::ParticipantFailureKind::Transient
    );
    let records = provider.audit_for_test().records.lock().unwrap().clone();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].outcome, "Timeout");
    assert!(records[0].timed_out);
    assert_ne!(records[0].outcome, "success");
}

#[test]
fn c005_invalid_utf8_stdout_fails_closed_permanently_and_is_audited() {
    let provider = provider(
        FakeRunner::new(Err(ProcessError::InvalidUtf8(OutputStream::Stdout))),
        RecordingAudit::default(),
    );
    let err = provider
        .consult(&packet(), &CancellationToken::new())
        .unwrap_err();
    assert_eq!(err, ClaudeCodeError::InvalidUtf8Output);
    assert_eq!(
        err.failure_kind(),
        maia_roundtable::ParticipantFailureKind::Permanent
    );
    let records = provider.audit_for_test().records.lock().unwrap().clone();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].outcome, "invalid_utf8_stdout");
    assert!(records[0].raw_stdout.is_empty());
}

#[test]
fn c005_output_over_the_cap_fails_closed_permanently_and_is_audited() {
    for (stream, label) in [
        (OutputStream::Stdout, "output_too_large_stdout"),
        (OutputStream::Stderr, "output_too_large_stderr"),
    ] {
        let provider = provider(
            FakeRunner::new(Err(ProcessError::OutputTooLarge(stream))),
            RecordingAudit::default(),
        );
        let err = provider
            .consult(&packet(), &CancellationToken::new())
            .unwrap_err();
        assert_eq!(err, ClaudeCodeError::OutputTooLarge);
        assert_eq!(
            err.failure_kind(),
            maia_roundtable::ParticipantFailureKind::Permanent,
            "an oversized answer is not retried into a different one"
        );
        let records = provider.audit_for_test().records.lock().unwrap().clone();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].outcome, label);
        assert!(
            records[0].raw_stdout.is_empty(),
            "nothing partial is presented"
        );
    }
}

#[test]
fn c005_an_unusable_scratch_directory_is_a_configuration_failure_not_a_result() {
    let provider = provider(
        FakeRunner::new(Err(ProcessError::ScratchFailed)),
        RecordingAudit::default(),
    );
    assert_eq!(
        provider
            .consult(&packet(), &CancellationToken::new())
            .unwrap_err(),
        ClaudeCodeError::InvalidConfiguration("scratch_directory")
    );
    let records = provider.audit_for_test().records.lock().unwrap().clone();
    assert_eq!(records[0].outcome, "scratch_failed");
}

#[test]
fn c005_the_invocation_carries_the_deadline_the_caps_and_the_audit_directory_as_scratch() {
    let provider = provider(
        FakeRunner::ok(&envelope(&valid_body())),
        RecordingAudit::default(),
    );
    provider
        .consult(&packet(), &CancellationToken::new())
        .unwrap();
    let seen = provider_invocation(&provider);
    let config = config();
    assert_eq!(seen.scratch_directory, config.audit_directory);
    assert_eq!(seen.max_stdout_bytes, config.max_stdout_bytes);
    assert_eq!(seen.max_stderr_bytes, config.max_stderr_bytes);
    assert!(seen.max_stdout_bytes > 0 && seen.max_stderr_bytes > 0);
    assert!(
        seen.timeout <= config.timeout,
        "one real end-to-end timeout"
    );
    assert!(
        seen.stdin_payload.contains("consult-1"),
        "the prompt is still on stdin"
    );
}

#[test]
fn c005_zero_capture_caps_are_refused_at_construction() {
    for (field, apply) in [
        (
            "max_stdout_bytes",
            (|c: &mut ClaudeCodeConfig| c.max_stdout_bytes = 0) as fn(&mut ClaudeCodeConfig),
        ),
        ("max_stderr_bytes", |c: &mut ClaudeCodeConfig| {
            c.max_stderr_bytes = 0
        }),
    ] {
        let mut bad = config();
        apply(&mut bad);
        let built = ClaudeCodeProvider::new_without_recursion_check(
            "claude-code",
            bad,
            "2.1.273".into(),
            FakeRunner::ok("{}"),
            RecordingAudit::default(),
        );
        assert_eq!(
            built.err(),
            Some(ClaudeCodeError::InvalidConfiguration(match field {
                "max_stdout_bytes" => "max_stdout_bytes",
                _ => "max_stderr_bytes",
            }))
        );
    }
}

#[test]
fn c005_cancellation_fails_closed() {
    let mut cancelled = outcome(0, "", "");
    cancelled.cancelled = true;
    cancelled.exit_code = None;
    let provider = provider(FakeRunner::new(Ok(cancelled)), RecordingAudit::default());
    let token = CancellationToken::new();
    token.cancel();
    assert_eq!(
        provider.consult(&packet(), &token).unwrap_err(),
        ClaudeCodeError::Cancelled
    );
}

#[test]
fn c005_non_zero_exit_fails_closed() {
    let provider = provider(
        FakeRunner::new(Ok(outcome(2, "", "boom"))),
        RecordingAudit::default(),
    );
    assert_eq!(
        provider
            .consult(&packet(), &CancellationToken::new())
            .unwrap_err(),
        ClaudeCodeError::NonZeroExit(2)
    );
}

#[test]
fn c005_malformed_envelope_json_fails_closed() {
    let provider = provider(FakeRunner::ok("{not json"), RecordingAudit::default());
    assert_eq!(
        provider
            .consult(&packet(), &CancellationToken::new())
            .unwrap_err(),
        ClaudeCodeError::MalformedJson
    );
}

#[test]
fn c005_malformed_inner_payload_fails_closed() {
    let provider = provider(
        FakeRunner::ok(&envelope("I decline to produce JSON.")),
        RecordingAudit::default(),
    );
    assert_eq!(
        provider
            .consult(&packet(), &CancellationToken::new())
            .unwrap_err(),
        ClaudeCodeError::MalformedJson
    );
}

#[test]
fn c005_empty_response_fails_closed() {
    let blank_body = provider(FakeRunner::ok(&envelope("   ")), RecordingAudit::default());
    assert_eq!(
        blank_body
            .consult(&packet(), &CancellationToken::new())
            .unwrap_err(),
        ClaudeCodeError::EmptyResponse
    );
    let empty_stdout = provider(FakeRunner::ok(""), RecordingAudit::default());
    assert_eq!(
        empty_stdout
            .consult(&packet(), &CancellationToken::new())
            .unwrap_err(),
        ClaudeCodeError::EmptyResponse
    );
}

#[test]
fn c005_schema_violation_fails_closed() {
    // Missing `recommendation` entirely.
    let body = serde_json::json!({
        "schema_version": "maia.consultation_result.v1",
        "consultation_id": "consult-1",
        "participant_id": "claude-code",
        "response": "text",
        "confidence": "high"
    })
    .to_string();
    let provider = provider(FakeRunner::ok(&envelope(&body)), RecordingAudit::default());
    assert_eq!(
        provider
            .consult(&packet(), &CancellationToken::new())
            .unwrap_err(),
        ClaudeCodeError::MalformedJson
    );
}

#[test]
fn c005_correlation_id_mismatch_is_rejected_not_repaired() {
    let body = serde_json::json!({
        "schema_version": "maia.consultation_result.v1",
        "consultation_id": "some-other-consultation",
        "participant_id": "claude-code",
        "response": "text",
        "recommendation": "proceed",
        "confidence": "high"
    })
    .to_string();
    let provider = provider(FakeRunner::ok(&envelope(&body)), RecordingAudit::default());
    assert_eq!(
        provider
            .consult(&packet(), &CancellationToken::new())
            .unwrap_err(),
        ClaudeCodeError::SchemaViolation(ContractViolation::ConsultationIdMismatch)
    );
}

#[test]
fn c005_wrong_schema_version_is_rejected() {
    let body = serde_json::json!({
        "schema_version": "maia.consultation_result.v2",
        "consultation_id": "consult-1",
        "participant_id": "claude-code",
        "response": "text",
        "recommendation": "proceed",
        "confidence": "high"
    })
    .to_string();
    let provider = provider(FakeRunner::ok(&envelope(&body)), RecordingAudit::default());
    assert_eq!(
        provider
            .consult(&packet(), &CancellationToken::new())
            .unwrap_err(),
        ClaudeCodeError::SchemaViolation(ContractViolation::UnsupportedSchemaVersion)
    );
}

#[test]
fn c005_cli_reported_error_is_never_treated_as_success() {
    let body = serde_json::json!({
        "subtype": "error_max_turns",
        "is_error": true,
        "result": ""
    })
    .to_string();
    let provider = provider(FakeRunner::ok(&body), RecordingAudit::default());
    assert!(matches!(
        provider
            .consult(&packet(), &CancellationToken::new())
            .unwrap_err(),
        ClaudeCodeError::CliReportedError(_)
    ));
}

// ----------------------------------------------------------------- C006 audit

#[test]
fn c006_audit_is_persisted_with_raw_response_and_correlation_id() {
    let audit = RecordingAudit::default();
    let provider = provider(FakeRunner::ok(&envelope(&valid_body())), audit);
    let result = provider
        .consult(&packet(), &CancellationToken::new())
        .unwrap();

    let records = provider.audit_for_test().records.lock().unwrap().clone();
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.correlation_id, result.execution.correlation_id);
    assert_eq!(record.consultation_id, "consult-1");
    assert_eq!(record.outcome, "success");
    assert!(record.raw_stdout.contains("maia.consultation_result.v1"));
    assert_eq!(record.cli_version, "2.1.273");
    // Names are recorded, values never are.
    assert!(
        record
            .scrubbed_environment_variables
            .contains(&"ANTHROPIC_API_KEY".to_string())
    );
    let serialized = serde_json::to_string(record).unwrap();
    assert!(!serialized.contains("sk-ant-"));
}

#[test]
fn c006_failures_are_audited_too() {
    let provider = provider(
        FakeRunner::new(Ok(outcome(2, "", "boom"))),
        RecordingAudit::default(),
    );
    let _ = provider.consult(&packet(), &CancellationToken::new());
    let records = provider.audit_for_test().records.lock().unwrap().clone();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].outcome, "NonZeroExit(2)");
}

#[test]
fn c006_audit_write_failure_fails_the_invocation() {
    let provider = provider(
        FakeRunner::ok(&envelope(&valid_body())),
        RecordingAudit::failing(),
    );
    assert_eq!(
        provider
            .consult(&packet(), &CancellationToken::new())
            .unwrap_err(),
        ClaudeCodeError::AuditWriteFailed,
        "an unaudited consultation must not be reported as successful"
    );
}

// ------------------------------------------------------------- C007 recursion

#[test]
fn c007_recursion_marker_is_set_on_the_child() {
    let provider = provider(
        FakeRunner::ok(&envelope(&valid_body())),
        RecordingAudit::default(),
    );
    provider
        .consult(&packet(), &CancellationToken::new())
        .unwrap();
    let seen = provider_invocation(&provider);
    assert_eq!(
        seen.env_set.get(RECURSION_MARKER).map(String::as_str),
        Some("1")
    );
}

#[test]
fn c007_additional_child_markers_are_set_alongside_the_provider_marker() {
    let mut config = config();
    config
        .additional_child_markers
        .push(ChildMarker::new("MAIA_ROUNDTABLE_INVOKE_ACTIVE").unwrap());
    let provider = ClaudeCodeProvider::new_without_recursion_check(
        "claude-code",
        config,
        "2.1.273".into(),
        FakeRunner::ok(&envelope(&valid_body())),
        RecordingAudit::default(),
    )
    .expect("provider constructs");
    provider
        .consult(&packet(), &CancellationToken::new())
        .unwrap();
    let seen = provider_invocation(&provider);
    assert_eq!(
        seen.env_set.get(RECURSION_MARKER).map(String::as_str),
        Some("1")
    );
    assert_eq!(
        seen.env_set
            .get("MAIA_ROUNDTABLE_INVOKE_ACTIVE")
            .map(String::as_str),
        Some("1")
    );
    assert_eq!(seen.env_set.len(), 2, "nothing else is injected");
}

#[test]
fn c007_a_child_marker_is_not_an_arbitrary_environment_variable() {
    for bad in [
        "",
        "PATH",
        "ANTHROPIC_API_KEY",
        "MAIA_ACTIVE",
        "MAIA__ACTIVE",
        "maia_roundtable_invoke_active",
        "MAIA_X_ACTIVE ",
        "MAIA_X=1_ACTIVE",
        "MAIA_X_ACTIVEX",
        "MAIA_X-Y_ACTIVE",
    ] {
        assert!(ChildMarker::new(bad).is_err(), "{bad:?} must be rejected");
    }
    assert!(ChildMarker::new("MAIA_ROUNDTABLE_INVOKE_ACTIVE").is_ok());
}

#[test]
fn c007_default_configuration_adds_no_extra_marker() {
    let provider = provider(
        FakeRunner::ok(&envelope(&valid_body())),
        RecordingAudit::default(),
    );
    provider
        .consult(&packet(), &CancellationToken::new())
        .unwrap();
    assert_eq!(provider_invocation(&provider).env_set.len(), 1);
}

#[test]
fn c007_nested_invocation_is_refused() {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::set_var(RECURSION_MARKER, "1") };
    let built = ClaudeCodeProvider::new(
        "claude-code",
        config(),
        "2.1.273".into(),
        FakeRunner::ok(&envelope(&valid_body())),
        RecordingAudit::default(),
    );
    unsafe { std::env::remove_var(RECURSION_MARKER) };

    match built.err() {
        Some(ClaudeCodeError::RecursionGuard) => {}
        // Without the feature the capability check fires first; both are fail-closed.
        Some(ClaudeCodeError::CapabilityAbsent) => assert!(!capability_present()),
        other => panic!("nested invocation must be refused, got {other:?}"),
    }
}

// --------------------------------------------------------- C008 deployment lock

#[test]
fn c008_capability_absent_without_development_evolution_feature() {
    assert_eq!(
        capability_present(),
        cfg!(feature = "development-evolution")
    );

    let built = ClaudeCodeProvider::new(
        "claude-code",
        config(),
        "2.1.273".into(),
        FakeRunner::ok(&envelope(&valid_body())),
        RecordingAudit::default(),
    );
    if !capability_present() {
        assert_eq!(
            built.err(),
            Some(ClaudeCodeError::CapabilityAbsent),
            "a DEPLOYMENT_LOCKED build must not construct the provider"
        );
    }
}

// ------------------------------------------------------------ packet validation

#[test]
fn invalid_packet_is_rejected_before_any_process_is_spawned() {
    let provider = provider(
        FakeRunner::ok(&envelope(&valid_body())),
        RecordingAudit::default(),
    );
    let mut bad = packet();
    bad.task = "   ".into();
    assert_eq!(
        provider
            .consult(&bad, &CancellationToken::new())
            .unwrap_err(),
        ClaudeCodeError::InvalidPacket(ContractViolation::MissingField("task"))
    );
    assert!(
        provider.runner_for_test().seen.lock().unwrap().is_none(),
        "no process may be spawned for an invalid packet"
    );
}
