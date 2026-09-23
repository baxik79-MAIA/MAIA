//! Versioned consultation contracts carried by the Claude Code adapter.
//!
//! These types live in the adapter, never in MAIA Core. The result is validated
//! before Round Table consumes it; a violation is a permanent failure, never a
//! silently repaired or fabricated success.
//!
//! Canonical contract: `spec/claude_code_provider.yaml#contracts`.

use serde::{Deserialize, Serialize};

pub const CONSULTATION_PACKET_VERSION: &str = "maia.consultation_packet.v1";
pub const CONSULTATION_RESULT_VERSION: &str = "maia.consultation_result.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextReference {
    pub id: String,
    pub source_ref: String,
}

/// Input contract: what MAIA / Round Table asks a participant to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsultationPacket {
    pub schema_version: String,
    pub consultation_id: String,
    pub participant_id: String,
    pub role: String,
    pub task: String,
    pub context_references: Vec<ContextReference>,
    pub evidence_requirements: Vec<String>,
    pub constraints: Vec<String>,
    pub timeout_seconds: u64,
}

impl ConsultationPacket {
    pub fn new(
        consultation_id: impl Into<String>,
        participant_id: impl Into<String>,
        role: impl Into<String>,
        task: impl Into<String>,
        timeout_seconds: u64,
    ) -> Self {
        Self {
            schema_version: CONSULTATION_PACKET_VERSION.into(),
            consultation_id: consultation_id.into(),
            participant_id: participant_id.into(),
            role: role.into(),
            task: task.into(),
            context_references: Vec::new(),
            evidence_requirements: Vec::new(),
            constraints: Vec::new(),
            timeout_seconds,
        }
    }

    pub fn validate(&self) -> Result<(), ContractViolation> {
        if self.schema_version != CONSULTATION_PACKET_VERSION {
            return Err(ContractViolation::UnsupportedSchemaVersion);
        }
        for (field, value) in [
            ("consultation_id", &self.consultation_id),
            ("participant_id", &self.participant_id),
            ("role", &self.role),
            ("task", &self.task),
        ] {
            if value.trim().is_empty() {
                return Err(ContractViolation::MissingField(field));
            }
        }
        if self.timeout_seconds == 0 {
            return Err(ContractViolation::MissingField("timeout_seconds"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub summary: String,
    #[serde(default)]
    pub severity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceItem {
    pub id: String,
    pub source_ref: String,
}

/// Execution metadata captured by the adapter, not by the model. The model
/// cannot influence these values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionMetadata {
    pub correlation_id: String,
    pub session_id: Option<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub cli_version: String,
    pub model: Option<String>,
    pub auth_mode: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    /// Always false for a subscription-backed invocation: the CLI reports a
    /// list-price basis that is not actual spend. Unknown cost is never zero.
    pub cost_known: bool,
}

/// Output contract returned to Round Table after validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsultationResult {
    pub schema_version: String,
    pub consultation_id: String,
    pub participant_id: String,
    pub response: String,
    pub findings: Vec<Finding>,
    pub evidence: Vec<EvidenceItem>,
    pub risks: Vec<String>,
    pub disagreements: Vec<String>,
    pub recommendation: String,
    pub confidence: Confidence,
    pub execution: ExecutionMetadata,
}

/// The model-authored portion of the result. Execution metadata is attached by
/// the adapter afterwards so a model can never forge it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelAuthoredResult {
    pub schema_version: String,
    pub consultation_id: String,
    pub participant_id: String,
    pub response: String,
    #[serde(default)]
    pub findings: Vec<Finding>,
    #[serde(default)]
    pub evidence: Vec<EvidenceItem>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub disagreements: Vec<String>,
    pub recommendation: String,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractViolation {
    UnsupportedSchemaVersion,
    MissingField(&'static str),
    ConsultationIdMismatch,
    ParticipantIdMismatch,
}

impl std::fmt::Display for ContractViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ContractViolation {}

impl ModelAuthoredResult {
    /// Fail-closed validation against the originating packet.
    pub fn validate_against(&self, packet: &ConsultationPacket) -> Result<(), ContractViolation> {
        if self.schema_version != CONSULTATION_RESULT_VERSION {
            return Err(ContractViolation::UnsupportedSchemaVersion);
        }
        if self.consultation_id != packet.consultation_id {
            return Err(ContractViolation::ConsultationIdMismatch);
        }
        if self.participant_id != packet.participant_id {
            return Err(ContractViolation::ParticipantIdMismatch);
        }
        if self.response.trim().is_empty() {
            return Err(ContractViolation::MissingField("response"));
        }
        if self.recommendation.trim().is_empty() {
            return Err(ContractViolation::MissingField("recommendation"));
        }
        Ok(())
    }

    pub fn into_result(self, execution: ExecutionMetadata) -> ConsultationResult {
        ConsultationResult {
            schema_version: self.schema_version,
            consultation_id: self.consultation_id,
            participant_id: self.participant_id,
            response: self.response,
            findings: self.findings,
            evidence: self.evidence,
            risks: self.risks,
            disagreements: self.disagreements,
            recommendation: self.recommendation,
            confidence: self.confidence,
            execution,
        }
    }
}
