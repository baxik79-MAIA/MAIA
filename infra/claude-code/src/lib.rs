//! Claude Code subscription-backed provider for the Round Table `Participant` port.
//!
//! Invokes the locally authenticated Claude Code CLI as a child process in
//! non-interactive print mode. Subscription-backed: the adapter introduces no
//! API key and unconditionally scrubs `ANTHROPIC_API_KEY` and equivalent
//! credential/endpoint overrides from the child environment, so an API key
//! present on the host cannot silently redirect the call onto metered billing.
//!
//! This adapter has no execution authority. It restricts child tool authority
//! with an explicit deny list and never passes `bypassPermissions`.
//!
//! Canonical contract: `spec/claude_code_provider.yaml`. Rationale: ADR-0047.
#![forbid(unsafe_code)]

pub mod audit;
pub mod contract;
pub mod runner;

use audit::{AuditError, AuditRecord, AuditSink};
use contract::{
    CONSULTATION_RESULT_VERSION, ConsultationPacket, ConsultationResult, ContractViolation,
    ExecutionMetadata, ModelAuthoredResult,
};
use maia_roundtable::{
    ModelProvider, ModelRef, Participant, ParticipantDescriptor, ParticipantFailure,
    ParticipantFailureKind, ParticipantId, ParticipantRequest, ParticipantResponse,
    UsageCostMetadata,
};
use runner::{
    CancellationToken, DEFAULT_MAX_STDERR_BYTES, DEFAULT_MAX_STDOUT_BYTES, Invocation,
    OutputStream, ProcessError, ProcessRunner,
};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

/// Environment marker preventing a Claude Code invocation from spawning another.
pub const RECURSION_MARKER: &str = "MAIA_CLAUDE_CODE_PROVIDER_ACTIVE";

/// A recursion marker the embedding host asks the provider to set on its own
/// child, in addition to [`RECURSION_MARKER`].
///
/// This is deliberately not a general environment pass-through. The name must
/// look like a MAIA recursion marker (`MAIA_<...>_ACTIVE`, upper-case ASCII,
/// digits and underscores), the value is always `"1"`, and it can never name a
/// credential or endpoint override that the provider scrubs. It exists so a host
/// process can mark the child it caused without mutating its own, process-global
/// environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildMarker(String);

impl ChildMarker {
    pub fn new(name: &str) -> Result<Self, ClaudeCodeError> {
        let shaped = name.len() > "MAIA__ACTIVE".len()
            && name.starts_with("MAIA_")
            && name.ends_with("_ACTIVE")
            && name
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_');
        if !shaped || SCRUBBED_ENVIRONMENT.contains(&name) {
            return Err(ClaudeCodeError::InvalidConfiguration("child_marker"));
        }
        Ok(Self(name.to_owned()))
    }

    pub fn name(&self) -> &str {
        &self.0
    }
}

/// Removed from the child environment on every invocation. Presence of any of
/// these would move the child off subscription-backed authentication.
pub const SCRUBBED_ENVIRONMENT: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_BASE_URL",
    "ANTHROPIC_CUSTOM_HEADERS",
    "ANTHROPIC_MODEL",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "AWS_BEARER_TOKEN_BEDROCK",
];

/// Measured on Claude Code 2.1.273: an empty `--allowed-tools` does not deny
/// anything, so authority is restricted with an explicit deny list. Egress
/// tools are always denied.
pub const DEFAULT_DENIED_TOOLS: &[&str] = &[
    "Bash",
    "BashOutput",
    "KillShell",
    "Edit",
    "Write",
    "NotebookEdit",
    "WebFetch",
    "WebSearch",
    "Task",
    "SlashCommand",
];

pub const AUTH_MODE_SUBSCRIPTION: &str = "subscription_cli_session";

/// True only when the crate is compiled for a DEVELOPMENT_EVOLUTION build.
/// A DEPLOYMENT_LOCKED artifact omits the crate; if it is nonetheless linked,
/// no subprocess code exists and this is `false`.
pub const fn capability_present() -> bool {
    cfg!(feature = "development-evolution")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeCodeConfig {
    pub executable: PathBuf,
    pub working_directory: PathBuf,
    pub model: Option<String>,
    pub timeout: Duration,
    pub denied_tools: Vec<String>,
    pub audit_directory: PathBuf,
    /// Read-only repository inspection. Off by default: a reasoning-only
    /// participant gets no filesystem authority unless explicitly granted.
    pub allow_read_only_repository_tools: bool,
    /// Extra recursion markers set (to `"1"`) on the child, beyond
    /// [`RECURSION_MARKER`], which is always set. Empty by default.
    pub additional_child_markers: Vec<ChildMarker>,
    /// Cap on captured standard output. A child that writes more is stopped and
    /// the consultation fails with `OutputTooLarge`; nothing is truncated into a
    /// result and nothing unbounded is buffered.
    pub max_stdout_bytes: usize,
    /// Cap on captured standard error, with the same consequence.
    pub max_stderr_bytes: usize,
}

impl ClaudeCodeConfig {
    pub fn new(
        executable: impl AsRef<Path>,
        working_directory: impl AsRef<Path>,
        audit_directory: impl AsRef<Path>,
    ) -> Self {
        Self {
            executable: executable.as_ref().to_path_buf(),
            working_directory: working_directory.as_ref().to_path_buf(),
            model: None,
            timeout: Duration::from_secs(180),
            denied_tools: DEFAULT_DENIED_TOOLS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            audit_directory: audit_directory.as_ref().to_path_buf(),
            allow_read_only_repository_tools: false,
            additional_child_markers: Vec::new(),
            max_stdout_bytes: DEFAULT_MAX_STDOUT_BYTES,
            max_stderr_bytes: DEFAULT_MAX_STDERR_BYTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeCodeError {
    /// The crate was built without `development-evolution`.
    CapabilityAbsent,
    /// A Claude Code invocation is already in progress in this process tree.
    RecursionGuard,
    ExecutableNotFound,
    NotAuthenticated,
    InvalidConfiguration(&'static str),
    Timeout,
    /// The child wrote more output than the configured cap and was stopped.
    OutputTooLarge,
    /// The child's standard output was not valid UTF-8. Never repaired.
    InvalidUtf8Output,
    Cancelled,
    NonZeroExit(i32),
    MalformedJson,
    EmptyResponse,
    SchemaViolation(ContractViolation),
    CliReportedError(String),
    AuditWriteFailed,
    InvalidPacket(ContractViolation),
}

impl std::fmt::Display for ClaudeCodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ClaudeCodeError {}

impl ClaudeCodeError {
    /// Only genuinely retryable conditions are transient. Everything else —
    /// including any schema or authentication problem — is permanent so the
    /// Round Table fails closed rather than retrying into a wrong answer.
    pub fn failure_kind(&self) -> ParticipantFailureKind {
        match self {
            ClaudeCodeError::Timeout => ParticipantFailureKind::Transient,
            _ => ParticipantFailureKind::Permanent,
        }
    }
}

/// The JSON envelope emitted by `claude -p --output-format json`.
#[derive(Debug, Deserialize)]
struct CliEnvelope {
    #[serde(default)]
    subtype: Option<String>,
    #[serde(default)]
    is_error: Option<bool>,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    usage: Option<CliUsage>,
    #[serde(default, rename = "modelUsage")]
    model_usage: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct CliUsage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
}

pub struct ClaudeCodeProvider<R: ProcessRunner, A: AuditSink> {
    descriptor: ParticipantDescriptor,
    config: ClaudeCodeConfig,
    cli_version: String,
    runner: R,
    audit: A,
}

impl<R: ProcessRunner, A: AuditSink> ClaudeCodeProvider<R, A> {
    /// Construct the provider. Fails closed when the capability is absent by
    /// construction, when a Claude Code invocation is already active in this
    /// process tree, or when configuration is incomplete.
    pub fn new(
        participant_id: &str,
        config: ClaudeCodeConfig,
        cli_version: String,
        runner: R,
        audit: A,
    ) -> Result<Self, ClaudeCodeError> {
        if !capability_present() {
            return Err(ClaudeCodeError::CapabilityAbsent);
        }
        if std::env::var_os(RECURSION_MARKER).is_some() {
            return Err(ClaudeCodeError::RecursionGuard);
        }
        Self::new_without_recursion_check(participant_id, config, cli_version, runner, audit)
    }

    /// Construction without the *ambient recursion* check, so the invocation
    /// contract can be exercised deterministically in tests.
    ///
    /// This is not a DEPLOYMENT_LOCKED bypass: the capability check is still
    /// enforced here, so no constructor in this crate can produce a provider in
    /// a build without `development-evolution`.
    #[doc(hidden)]
    pub fn new_without_recursion_check(
        participant_id: &str,
        config: ClaudeCodeConfig,
        cli_version: String,
        runner: R,
        audit: A,
    ) -> Result<Self, ClaudeCodeError> {
        if !capability_present() {
            return Err(ClaudeCodeError::CapabilityAbsent);
        }
        if config.executable.as_os_str().is_empty() {
            return Err(ClaudeCodeError::InvalidConfiguration("executable"));
        }
        if config.working_directory.as_os_str().is_empty() {
            return Err(ClaudeCodeError::InvalidConfiguration("working_directory"));
        }
        if config.audit_directory.as_os_str().is_empty() {
            return Err(ClaudeCodeError::InvalidConfiguration("audit_directory"));
        }
        if config.max_stdout_bytes == 0 {
            return Err(ClaudeCodeError::InvalidConfiguration("max_stdout_bytes"));
        }
        if config.max_stderr_bytes == 0 {
            return Err(ClaudeCodeError::InvalidConfiguration("max_stderr_bytes"));
        }
        if config.timeout.is_zero() {
            return Err(ClaudeCodeError::InvalidConfiguration("timeout"));
        }
        if cli_version.trim().is_empty() {
            return Err(ClaudeCodeError::InvalidConfiguration("cli_version"));
        }
        if config.denied_tools.is_empty() {
            return Err(ClaudeCodeError::InvalidConfiguration("denied_tools"));
        }
        let model = config
            .model
            .clone()
            .unwrap_or_else(|| "claude-code-default".to_string());
        Ok(Self {
            descriptor: ParticipantDescriptor {
                id: ParticipantId::new(participant_id)
                    .map_err(|_| ClaudeCodeError::InvalidConfiguration("participant_id"))?,
                provider: ModelProvider::new("anthropic")
                    .map_err(|_| ClaudeCodeError::InvalidConfiguration("provider"))?,
                model: ModelRef::new(model)
                    .map_err(|_| ClaudeCodeError::InvalidConfiguration("model"))?,
            },
            config,
            cli_version,
            runner,
            audit,
        })
    }

    pub fn cli_version(&self) -> &str {
        &self.cli_version
    }

    /// Inspection accessors so contract tests can assert on the exact
    /// invocation and audit record the provider produced.
    #[doc(hidden)]
    pub fn runner_for_test(&self) -> &R {
        &self.runner
    }

    #[doc(hidden)]
    pub fn audit_for_test(&self) -> &A {
        &self.audit
    }

    fn denied_tools(&self) -> Vec<String> {
        let mut denied = self.config.denied_tools.clone();
        if !self.config.allow_read_only_repository_tools {
            for tool in ["Read", "Glob", "Grep"] {
                if !denied.iter().any(|d| d == tool) {
                    denied.push(tool.to_string());
                }
            }
        }
        denied
    }

    fn build_invocation(&self, packet: &ConsultationPacket, prompt: String) -> Invocation {
        let mut args: Vec<String> = vec![
            "-p".into(),
            "--output-format".into(),
            "json".into(),
            // Sessions are not persisted: a consultation is stateless and must
            // not leak context between packets.
            "--no-session-persistence".into(),
        ];
        if let Some(model) = &self.config.model {
            args.push("--model".into());
            args.push(model.clone());
        }
        // Explicit deny list. `bypassPermissions` is never passed, and an empty
        // allowlist is never used because it does not deny anything.
        args.push("--disallowed-tools".into());
        args.extend(self.denied_tools());

        let mut env_set = BTreeMap::new();
        env_set.insert(RECURSION_MARKER.to_string(), "1".to_string());
        for marker in &self.config.additional_child_markers {
            env_set.insert(marker.name().to_owned(), "1".to_owned());
        }

        Invocation {
            program: self.config.executable.clone(),
            args,
            working_directory: self.config.working_directory.clone(),
            env_remove: SCRUBBED_ENVIRONMENT
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            env_set,
            stdin_payload: prompt,
            timeout: Duration::from_secs(
                packet
                    .timeout_seconds
                    .min(self.config.timeout.as_secs().max(1)),
            ),
            // The audit directory already has to be writable for the audit
            // record, so the short-lived capture files live there too.
            scratch_directory: self.config.audit_directory.clone(),
            max_stdout_bytes: self.config.max_stdout_bytes,
            max_stderr_bytes: self.config.max_stderr_bytes,
        }
    }

    /// Run one consultation. Every path that does not produce a validated
    /// result returns an error; no response is ever synthesized.
    pub fn consult(
        &self,
        packet: &ConsultationPacket,
        cancel: &CancellationToken,
    ) -> Result<ConsultationResult, ClaudeCodeError> {
        packet.validate().map_err(ClaudeCodeError::InvalidPacket)?;

        let correlation_id = correlation_id(&packet.consultation_id);
        let prompt = render_prompt(packet);
        let invocation = self.build_invocation(packet, prompt);

        let outcome = self.runner.run(&invocation, cancel);

        let (result, record) = match outcome {
            Err(ProcessError::ExecutableNotFound) => (
                Err(ClaudeCodeError::ExecutableNotFound),
                self.record(
                    packet,
                    &correlation_id,
                    &invocation,
                    None,
                    "executable_not_found",
                ),
            ),
            Err(ProcessError::OutputTooLarge(stream)) => (
                Err(ClaudeCodeError::OutputTooLarge),
                self.record(
                    packet,
                    &correlation_id,
                    &invocation,
                    None,
                    match stream {
                        OutputStream::Stdout => "output_too_large_stdout",
                        OutputStream::Stderr => "output_too_large_stderr",
                    },
                ),
            ),
            Err(ProcessError::InvalidUtf8(stream)) => (
                Err(ClaudeCodeError::InvalidUtf8Output),
                self.record(
                    packet,
                    &correlation_id,
                    &invocation,
                    None,
                    match stream {
                        OutputStream::Stdout => "invalid_utf8_stdout",
                        OutputStream::Stderr => "invalid_utf8_stderr",
                    },
                ),
            ),
            Err(ProcessError::ScratchFailed) => (
                Err(ClaudeCodeError::InvalidConfiguration("scratch_directory")),
                self.record(packet, &correlation_id, &invocation, None, "scratch_failed"),
            ),
            Err(_) => (
                Err(ClaudeCodeError::InvalidConfiguration("spawn")),
                self.record(packet, &correlation_id, &invocation, None, "spawn_failed"),
            ),
            Ok(outcome) => {
                let interpreted = self.interpret(packet, &correlation_id, &outcome);
                let label = match &interpreted {
                    Ok(_) => "success".to_string(),
                    Err(e) => format!("{e:?}"),
                };
                (
                    interpreted,
                    self.record(packet, &correlation_id, &invocation, Some(&outcome), &label),
                )
            }
        };

        // Audit is persisted for successes and failures alike. A failed audit
        // write fails the invocation.
        match self.audit.persist(&record) {
            Ok(()) => result,
            Err(AuditError::WriteFailed) | Err(AuditError::SerializeFailed) => {
                Err(ClaudeCodeError::AuditWriteFailed)
            }
        }
    }

    fn record(
        &self,
        packet: &ConsultationPacket,
        correlation_id: &str,
        invocation: &Invocation,
        outcome: Option<&runner::ProcessOutcome>,
        label: &str,
    ) -> AuditRecord {
        AuditRecord {
            schema_version: "maia.claude_code_audit.v1",
            correlation_id: correlation_id.to_string(),
            consultation_id: packet.consultation_id.clone(),
            participant_id: packet.participant_id.clone(),
            executable: self.config.executable.display().to_string(),
            cli_version: self.cli_version.clone(),
            auth_mode: AUTH_MODE_SUBSCRIPTION.to_string(),
            working_directory: self.config.working_directory.display().to_string(),
            args: invocation.args.clone(),
            scrubbed_environment_variables: invocation.env_remove.clone(),
            denied_tools: self.denied_tools(),
            exit_code: outcome.and_then(|o| o.exit_code),
            duration_ms: outcome.map(|o| o.duration.as_millis() as u64).unwrap_or(0),
            timed_out: outcome.map(|o| o.timed_out).unwrap_or(false),
            cancelled: outcome.map(|o| o.cancelled).unwrap_or(false),
            raw_stdout: outcome.map(|o| o.stdout.clone()).unwrap_or_default(),
            raw_stderr: outcome.map(|o| o.stderr.clone()).unwrap_or_default(),
            outcome: label.to_string(),
        }
    }

    fn interpret(
        &self,
        packet: &ConsultationPacket,
        correlation_id: &str,
        outcome: &runner::ProcessOutcome,
    ) -> Result<ConsultationResult, ClaudeCodeError> {
        if outcome.cancelled {
            return Err(ClaudeCodeError::Cancelled);
        }
        if outcome.timed_out {
            return Err(ClaudeCodeError::Timeout);
        }
        match outcome.exit_code {
            Some(0) => {}
            Some(code) => {
                if looks_unauthenticated(&outcome.stderr) {
                    return Err(ClaudeCodeError::NotAuthenticated);
                }
                return Err(ClaudeCodeError::NonZeroExit(code));
            }
            None => return Err(ClaudeCodeError::NonZeroExit(-1)),
        }
        if outcome.stdout.trim().is_empty() {
            return Err(ClaudeCodeError::EmptyResponse);
        }

        let envelope: CliEnvelope =
            serde_json::from_str(&outcome.stdout).map_err(|_| ClaudeCodeError::MalformedJson)?;
        if envelope.is_error.unwrap_or(false) || envelope.subtype.as_deref() != Some("success") {
            return Err(ClaudeCodeError::CliReportedError(
                envelope.subtype.unwrap_or_else(|| "unknown".into()),
            ));
        }
        let body = envelope.result.unwrap_or_default();
        if body.trim().is_empty() {
            return Err(ClaudeCodeError::EmptyResponse);
        }

        let payload = extract_json_object(&body).ok_or(ClaudeCodeError::MalformedJson)?;
        let authored: ModelAuthoredResult =
            serde_json::from_str(payload).map_err(|_| ClaudeCodeError::MalformedJson)?;
        authored
            .validate_against(packet)
            .map_err(ClaudeCodeError::SchemaViolation)?;

        let execution = ExecutionMetadata {
            correlation_id: correlation_id.to_string(),
            session_id: envelope.session_id,
            exit_code: outcome.exit_code,
            duration_ms: outcome.duration.as_millis() as u64,
            cli_version: self.cli_version.clone(),
            model: resolved_model(envelope.model_usage.as_ref())
                .or_else(|| self.config.model.clone()),
            auth_mode: AUTH_MODE_SUBSCRIPTION.to_string(),
            input_tokens: envelope.usage.as_ref().and_then(|u| u.input_tokens),
            output_tokens: envelope.usage.as_ref().and_then(|u| u.output_tokens),
            // Subscription: the CLI's list-price figure is not actual spend.
            cost_known: false,
        };
        Ok(authored.into_result(execution))
    }
}

/// Round Table port implementation. A participant failure never carries a
/// fabricated response.
impl<R: ProcessRunner, A: AuditSink> Participant for ClaudeCodeProvider<R, A> {
    fn descriptor(&self) -> ParticipantDescriptor {
        self.descriptor.clone()
    }

    fn invoke(
        &self,
        request: ParticipantRequest,
    ) -> Result<ParticipantResponse, ParticipantFailure> {
        let mut packet = ConsultationPacket::new(
            request.decision.id.clone(),
            self.descriptor.id.as_str(),
            "round_table_participant",
            request.decision.prompt.clone(),
            self.config.timeout.as_secs().max(1),
        );
        packet.context_references = request
            .decision
            .evidence
            .iter()
            .map(|e| contract::ContextReference {
                id: e.id.clone(),
                source_ref: e.source_ref.clone(),
            })
            .collect();
        packet.constraints = vec![
            format!("subject: {}", request.decision.subject),
            format!("max_output_tokens: {}", request.max_output_tokens),
        ];

        let cancel = CancellationToken::new();
        let result = self
            .consult(&packet, &cancel)
            .map_err(|error| ParticipantFailure {
                kind: error.failure_kind(),
                provider_request_id: None,
            })?;

        Ok(ParticipantResponse {
            participant: self.descriptor(),
            response_text: result.response.clone(),
            evidence: result
                .evidence
                .iter()
                .map(|e| maia_roundtable::EvidenceReference {
                    id: e.id.clone(),
                    source_ref: e.source_ref.clone(),
                })
                .collect(),
            provider_request_id: result.execution.session_id.clone(),
            // The CLI reports which model actually served the request, which may
            // differ from the configured one. Reported as-is, or left unknown
            // when the CLI did not say, never backfilled from configuration.
            model_ref_used: result
                .execution
                .model
                .as_deref()
                .and_then(|m| ModelRef::new(m).ok()),
            usage: Some(UsageCostMetadata {
                input_tokens: result.execution.input_tokens,
                output_tokens: result.execution.output_tokens,
                cost_known: false,
                cost_minor: None,
                currency: None,
            }),
        })
    }
}

fn looks_unauthenticated(stderr: &str) -> bool {
    let lowered = stderr.to_ascii_lowercase();
    [
        "not logged in",
        "unauthorized",
        "authentication",
        "/login",
        "invalid api key",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
}

fn resolved_model(model_usage: Option<&serde_json::Value>) -> Option<String> {
    model_usage?
        .as_object()?
        .keys()
        .next()
        .map(|k| k.to_string())
}

/// Correlation IDs are adapter-generated and monotonic within a process.
fn correlation_id(consultation_id: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = COUNTER.fetch_add(1, Ordering::SeqCst);
    let short: String = consultation_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(24)
        .collect();
    format!("maia-cc-{short}-{nanos}-{}-{seq}", std::process::id())
}

/// Tolerate a fenced or prose-wrapped JSON object without accepting garbage:
/// the extracted span must still parse as the declared contract.
fn extract_json_object(body: &str) -> Option<&str> {
    let trimmed = body.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed);
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(&trimmed[start..=end])
}

fn render_prompt(packet: &ConsultationPacket) -> String {
    let packet_json = serde_json::to_string_pretty(packet)
        .unwrap_or_else(|_| "{\"error\":\"packet_serialization_failed\"}".into());
    format!(
        "You are participating in a MAIA Round Table consultation as an independent \
reviewer. You have no execution authority: do not attempt to modify any file or \
run any command.\n\n\
Consultation packet (JSON):\n{packet_json}\n\n\
Respond with a SINGLE JSON object and nothing else. No prose before or after, no \
markdown fence. The object MUST match exactly this contract:\n\
{{\n\
  \"schema_version\": \"{CONSULTATION_RESULT_VERSION}\",\n\
  \"consultation_id\": \"{consultation_id}\",\n\
  \"participant_id\": \"{participant_id}\",\n\
  \"response\": \"<your substantive answer>\",\n\
  \"findings\": [{{\"id\": \"F1\", \"summary\": \"...\", \"severity\": \"low|medium|high\"}}],\n\
  \"evidence\": [{{\"id\": \"E1\", \"source_ref\": \"<file or contract reference>\"}}],\n\
  \"risks\": [\"...\"],\n\
  \"disagreements\": [\"...\"],\n\
  \"recommendation\": \"<clear recommended next step>\",\n\
  \"confidence\": \"low|medium|high\"\n\
}}\n\n\
Every field is required. Use empty arrays where you have nothing to report. Do not \
invent evidence you did not actually verify; state uncertainty in `risks` instead.",
        CONSULTATION_RESULT_VERSION = CONSULTATION_RESULT_VERSION,
        consultation_id = packet.consultation_id,
        participant_id = packet.participant_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fenced_or_wrapped_json_is_extracted_but_garbage_is_not() {
        assert_eq!(extract_json_object("{\"a\":1}"), Some("{\"a\":1}"));
        assert_eq!(
            extract_json_object("```json\n{\"a\":1}\n```"),
            Some("{\"a\":1}")
        );
        assert_eq!(extract_json_object("no json here"), None);
        assert_eq!(extract_json_object("} {"), None);
    }

    #[test]
    fn correlation_ids_are_unique_and_path_safe() {
        let a = correlation_id("decision/../1");
        let b = correlation_id("decision/../1");
        assert_ne!(a, b);
        assert!(!a.contains('/'));
        assert!(!a.contains('.'));
    }

    #[test]
    fn unauthenticated_stderr_is_recognised() {
        assert!(looks_unauthenticated("Error: Not logged in. Run /login"));
        assert!(!looks_unauthenticated("some unrelated warning"));
    }

    #[test]
    fn only_timeout_is_transient() {
        assert_eq!(
            ClaudeCodeError::Timeout.failure_kind(),
            ParticipantFailureKind::Transient
        );
        for error in [
            ClaudeCodeError::NotAuthenticated,
            ClaudeCodeError::MalformedJson,
            ClaudeCodeError::EmptyResponse,
            ClaudeCodeError::ExecutableNotFound,
            ClaudeCodeError::RecursionGuard,
            ClaudeCodeError::CapabilityAbsent,
            ClaudeCodeError::AuditWriteFailed,
            ClaudeCodeError::OutputTooLarge,
            ClaudeCodeError::InvalidUtf8Output,
        ] {
            assert_eq!(error.failure_kind(), ParticipantFailureKind::Permanent);
        }
    }
}
