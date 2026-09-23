//! File-backed persistence for Round Table session history.
//!
//! Implements `maia_roundtable::SessionStore` outside MAIA Core, with state
//! owned by the Round Table module.
//!
//! This is a separate crate rather than part of `infra/sqlite` for an
//! architectural reason, not a stylistic one: both shipped applications depend
//! on `maia-sqlite`, so implementing Round Table persistence there would pull
//! `maia-roundtable` into their dependency graphs and break the ADR-0046
//! boundary that `core_independence.rs` enforces.
//!
//! Records carry an explicit schema version. `maia-roundtable` deliberately has
//! no serde dependency, so the wire shapes live here as versioned DTOs that map
//! to and from the domain types. That keeps Core free of a serialization
//! contract and makes schema evolution this crate's problem, which is where it
//! belongs.
#![forbid(unsafe_code)]

use maia_domain::ReasoningAssuranceLevel;
use maia_roundtable::{
    ContributionFailureReason, ContributionResult, ContributionRole, DecisionRequest, Disagreement,
    EvidenceReference, InsufficientAssurance, LeaderSelection, LeaderSelectionFailure,
    ModelProvider, ModelRef, ParticipantDescriptor, ParticipantFailureKind, ParticipantId,
    ParticipantOutcome, ParticipantResponse, QuorumReasonCode, RoundTableDecision,
    SessionFailureKind, SessionQuery, SessionQueryError, SessionRecord, SessionStore,
    SessionStoreError, SessionSummary, SessionView, Timestamp, UsageCostMetadata,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Current on-disk schema version. A record written by a future version is
/// rejected as corrupt rather than guessed at.
pub const SCHEMA_VERSION: u32 = 1;

// ----------------------------------------------------------------- wire shapes

#[derive(Serialize, Deserialize)]
struct SessionDto {
    schema_version: u32,
    id: String,
    assurance: String,
    decision: DecisionDto,
    outcomes: Vec<OutcomeDto>,
    leader: Option<LeaderDto>,
    disagreements: Vec<DisagreementDto>,
    adjudication: Option<DecisionResultDto>,
    failure: Option<FailureDto>,
}

#[derive(Serialize, Deserialize)]
struct DecisionDto {
    id: String,
    subject: String,
    prompt: String,
    evidence: Vec<EvidenceDto>,
}

#[derive(Serialize, Deserialize)]
struct EvidenceDto {
    id: String,
    source_ref: String,
}

#[derive(Serialize, Deserialize)]
struct DescriptorDto {
    id: String,
    provider: String,
    model: String,
}

#[derive(Serialize, Deserialize)]
struct UsageDto {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cost_known: bool,
    cost_minor: Option<u64>,
    currency: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ResponseDto {
    participant: DescriptorDto,
    response_text: String,
    evidence: Vec<EvidenceDto>,
    provider_request_id: Option<String>,
    usage: Option<UsageDto>,
    model_ref_used: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct OutcomeDto {
    participant: DescriptorDto,
    role: String,
    sequence: u32,
    started_at: u64,
    finished_at: u64,
    first_round_isolated: bool,
    /// "responded" or "failed" — failed attempts are persisted too.
    result: String,
    response: Option<ResponseDto>,
    failure_kind: Option<String>,
    failure_reason: Option<String>,
    provider_request_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct LeaderDto {
    leader: DescriptorDto,
    considered: usize,
    eligible: usize,
    assurance: String,
}

#[derive(Serialize, Deserialize)]
struct DisagreementDto {
    id: String,
    participants: Vec<String>,
    summary: String,
    evidence: Vec<EvidenceDto>,
}

#[derive(Serialize, Deserialize)]
struct DecisionResultDto {
    session_id: String,
    conclusion: String,
    evidence: Vec<EvidenceDto>,
    disagreement_ids: Vec<String>,
    execution_authority: bool,
}

#[derive(Serialize, Deserialize)]
struct FailureDto {
    kind: String,
    leader_failure: Option<String>,
    quorum: Option<QuorumDto>,
}

#[derive(Serialize, Deserialize)]
struct QuorumDto {
    assurance: String,
    required_responses: usize,
    achieved_responses: usize,
    attempted_participants: usize,
    reason_code: String,
}

// ------------------------------------------------------------------- mapping

fn assurance_to_str(level: ReasoningAssuranceLevel) -> &'static str {
    match level {
        ReasoningAssuranceLevel::A0 => "A0",
        ReasoningAssuranceLevel::A1 => "A1",
        ReasoningAssuranceLevel::A2 => "A2",
        ReasoningAssuranceLevel::A3 => "A3",
        ReasoningAssuranceLevel::A4 => "A4",
    }
}

fn assurance_from_str(value: &str) -> Result<ReasoningAssuranceLevel, SessionStoreError> {
    match value {
        "A0" => Ok(ReasoningAssuranceLevel::A0),
        "A1" => Ok(ReasoningAssuranceLevel::A1),
        "A2" => Ok(ReasoningAssuranceLevel::A2),
        "A3" => Ok(ReasoningAssuranceLevel::A3),
        "A4" => Ok(ReasoningAssuranceLevel::A4),
        _ => Err(SessionStoreError::Corrupt),
    }
}

fn descriptor_to_dto(d: &ParticipantDescriptor) -> DescriptorDto {
    DescriptorDto {
        id: d.id.as_str().to_owned(),
        provider: d.provider.as_str().to_owned(),
        model: d.model.as_str().to_owned(),
    }
}

fn descriptor_from_dto(d: &DescriptorDto) -> Result<ParticipantDescriptor, SessionStoreError> {
    Ok(ParticipantDescriptor {
        id: ParticipantId::new(d.id.clone()).map_err(|_| SessionStoreError::Corrupt)?,
        provider: ModelProvider::new(d.provider.clone()).map_err(|_| SessionStoreError::Corrupt)?,
        model: ModelRef::new(d.model.clone()).map_err(|_| SessionStoreError::Corrupt)?,
    })
}

fn evidence_to_dto(e: &[EvidenceReference]) -> Vec<EvidenceDto> {
    e.iter()
        .map(|e| EvidenceDto {
            id: e.id.clone(),
            source_ref: e.source_ref.clone(),
        })
        .collect()
}

fn evidence_from_dto(e: &[EvidenceDto]) -> Vec<EvidenceReference> {
    e.iter()
        .map(|e| EvidenceReference {
            id: e.id.clone(),
            source_ref: e.source_ref.clone(),
        })
        .collect()
}

fn reason_to_str(r: ContributionFailureReason) -> &'static str {
    match r {
        ContributionFailureReason::ParticipantUnavailable => "participant_unavailable",
        ContributionFailureReason::InvalidResponse => "invalid_response",
        ContributionFailureReason::ResolutionFailed => "resolution_failed",
    }
}

fn reason_from_str(v: &str) -> Result<ContributionFailureReason, SessionStoreError> {
    match v {
        "participant_unavailable" => Ok(ContributionFailureReason::ParticipantUnavailable),
        "invalid_response" => Ok(ContributionFailureReason::InvalidResponse),
        "resolution_failed" => Ok(ContributionFailureReason::ResolutionFailed),
        _ => Err(SessionStoreError::Corrupt),
    }
}

fn outcome_to_dto(o: &ParticipantOutcome) -> OutcomeDto {
    let (result, response, failure_kind, failure_reason, request_id) = match &o.result {
        ContributionResult::Responded(r) => (
            "responded",
            Some(ResponseDto {
                participant: descriptor_to_dto(&r.participant),
                response_text: r.response_text.clone(),
                evidence: evidence_to_dto(&r.evidence),
                provider_request_id: r.provider_request_id.clone(),
                usage: r.usage.as_ref().map(|u| UsageDto {
                    input_tokens: u.input_tokens,
                    output_tokens: u.output_tokens,
                    cost_known: u.cost_known,
                    cost_minor: u.cost_minor,
                    currency: u.currency.clone(),
                }),
                model_ref_used: r.model_ref_used.as_ref().map(|m| m.as_str().to_owned()),
            }),
            None,
            None,
            r.provider_request_id.clone(),
        ),
        ContributionResult::Failed {
            kind,
            reason,
            provider_request_id,
        } => (
            "failed",
            None,
            Some(match kind {
                ParticipantFailureKind::Transient => "transient".to_owned(),
                ParticipantFailureKind::Permanent => "permanent".to_owned(),
            }),
            Some(reason_to_str(*reason).to_owned()),
            provider_request_id.clone(),
        ),
    };
    OutcomeDto {
        participant: descriptor_to_dto(&o.participant),
        role: match o.role {
            ContributionRole::FirstRound => "first_round".to_owned(),
            ContributionRole::Adjudication => "adjudication".to_owned(),
        },
        sequence: o.sequence,
        started_at: o.started_at.0,
        finished_at: o.finished_at.0,
        first_round_isolated: o.first_round_isolated,
        result: result.to_owned(),
        response,
        failure_kind,
        failure_reason,
        provider_request_id: request_id,
    }
}

fn outcome_from_dto(o: &OutcomeDto) -> Result<ParticipantOutcome, SessionStoreError> {
    let result = match o.result.as_str() {
        "responded" => {
            let r = o.response.as_ref().ok_or(SessionStoreError::Corrupt)?;
            ContributionResult::Responded(ParticipantResponse {
                participant: descriptor_from_dto(&r.participant)?,
                response_text: r.response_text.clone(),
                evidence: evidence_from_dto(&r.evidence),
                provider_request_id: r.provider_request_id.clone(),
                usage: r.usage.as_ref().map(|u| UsageCostMetadata {
                    input_tokens: u.input_tokens,
                    output_tokens: u.output_tokens,
                    cost_known: u.cost_known,
                    cost_minor: u.cost_minor,
                    currency: u.currency.clone(),
                }),
                model_ref_used: match &r.model_ref_used {
                    Some(m) => {
                        Some(ModelRef::new(m.clone()).map_err(|_| SessionStoreError::Corrupt)?)
                    }
                    None => None,
                },
            })
        }
        "failed" => ContributionResult::Failed {
            kind: match o.failure_kind.as_deref() {
                Some("transient") => ParticipantFailureKind::Transient,
                Some("permanent") => ParticipantFailureKind::Permanent,
                _ => return Err(SessionStoreError::Corrupt),
            },
            reason: reason_from_str(
                o.failure_reason
                    .as_deref()
                    .ok_or(SessionStoreError::Corrupt)?,
            )?,
            provider_request_id: o.provider_request_id.clone(),
        },
        _ => return Err(SessionStoreError::Corrupt),
    };
    Ok(ParticipantOutcome {
        participant: descriptor_from_dto(&o.participant)?,
        role: match o.role.as_str() {
            "first_round" => ContributionRole::FirstRound,
            "adjudication" => ContributionRole::Adjudication,
            _ => return Err(SessionStoreError::Corrupt),
        },
        sequence: o.sequence,
        started_at: Timestamp(o.started_at),
        finished_at: Timestamp(o.finished_at),
        first_round_isolated: o.first_round_isolated,
        result,
    })
}

fn to_dto(record: &SessionRecord) -> SessionDto {
    SessionDto {
        schema_version: SCHEMA_VERSION,
        id: record.id.clone(),
        assurance: assurance_to_str(record.assurance).to_owned(),
        decision: DecisionDto {
            id: record.decision.id.clone(),
            subject: record.decision.subject.clone(),
            prompt: record.decision.prompt.clone(),
            evidence: evidence_to_dto(&record.decision.evidence),
        },
        outcomes: record.outcomes.iter().map(outcome_to_dto).collect(),
        leader: record.leader.as_ref().map(|l| LeaderDto {
            leader: descriptor_to_dto(&l.leader),
            considered: l.considered,
            eligible: l.eligible,
            assurance: assurance_to_str(l.assurance).to_owned(),
        }),
        disagreements: record
            .disagreements
            .iter()
            .map(|d| DisagreementDto {
                id: d.id.clone(),
                participants: d
                    .participants
                    .iter()
                    .map(|p| p.as_str().to_owned())
                    .collect(),
                summary: d.summary.clone(),
                evidence: evidence_to_dto(&d.evidence),
            })
            .collect(),
        adjudication: record.adjudication.as_ref().map(|a| DecisionResultDto {
            session_id: a.session_id.clone(),
            conclusion: a.conclusion.clone(),
            evidence: evidence_to_dto(&a.evidence),
            disagreement_ids: a.disagreement_ids.clone(),
            execution_authority: a.execution_authority,
        }),
        failure: record.failure.as_ref().map(|f| match f {
            SessionFailureKind::NotARoundTableLevel => FailureDto {
                kind: "not_a_round_table_level".into(),
                leader_failure: None,
                quorum: None,
            },
            SessionFailureKind::NoLeader(l) => FailureDto {
                kind: "no_leader".into(),
                leader_failure: Some(
                    match l {
                        LeaderSelectionFailure::NoCandidates => "no_candidates",
                        LeaderSelectionFailure::NoEligibleCandidate => "no_eligible_candidate",
                    }
                    .into(),
                ),
                quorum: None,
            },
            SessionFailureKind::QuorumNotMet(i) => FailureDto {
                kind: "quorum_not_met".into(),
                leader_failure: None,
                quorum: Some(QuorumDto {
                    assurance: assurance_to_str(i.assurance).to_owned(),
                    required_responses: i.required_responses,
                    achieved_responses: i.achieved_responses,
                    attempted_participants: i.attempted_participants,
                    reason_code: match i.reason_code {
                        QuorumReasonCode::ParticipantUnavailable => "participant_unavailable",
                        QuorumReasonCode::RequiredAssuranceUnachievable => {
                            "required_assurance_unachievable"
                        }
                    }
                    .into(),
                }),
            },
            SessionFailureKind::AdjudicationFailed => FailureDto {
                kind: "adjudication_failed".into(),
                leader_failure: None,
                quorum: None,
            },
        }),
    }
}

fn from_dto(dto: &SessionDto) -> Result<SessionRecord, SessionStoreError> {
    if dto.schema_version != SCHEMA_VERSION {
        // A record from another schema version is refused, never guessed at.
        return Err(SessionStoreError::Corrupt);
    }
    let failure = match &dto.failure {
        None => None,
        Some(f) => Some(match f.kind.as_str() {
            "not_a_round_table_level" => SessionFailureKind::NotARoundTableLevel,
            "adjudication_failed" => SessionFailureKind::AdjudicationFailed,
            "no_leader" => SessionFailureKind::NoLeader(match f.leader_failure.as_deref() {
                Some("no_candidates") => LeaderSelectionFailure::NoCandidates,
                Some("no_eligible_candidate") => LeaderSelectionFailure::NoEligibleCandidate,
                _ => return Err(SessionStoreError::Corrupt),
            }),
            "quorum_not_met" => {
                let q = f.quorum.as_ref().ok_or(SessionStoreError::Corrupt)?;
                SessionFailureKind::QuorumNotMet(InsufficientAssurance {
                    assurance: assurance_from_str(&q.assurance)?,
                    required_responses: q.required_responses,
                    achieved_responses: q.achieved_responses,
                    attempted_participants: q.attempted_participants,
                    reason_code: match q.reason_code.as_str() {
                        "participant_unavailable" => QuorumReasonCode::ParticipantUnavailable,
                        "required_assurance_unachievable" => {
                            QuorumReasonCode::RequiredAssuranceUnachievable
                        }
                        _ => return Err(SessionStoreError::Corrupt),
                    },
                })
            }
            _ => return Err(SessionStoreError::Corrupt),
        }),
    };
    Ok(SessionRecord {
        id: dto.id.clone(),
        assurance: assurance_from_str(&dto.assurance)?,
        decision: DecisionRequest {
            id: dto.decision.id.clone(),
            subject: dto.decision.subject.clone(),
            prompt: dto.decision.prompt.clone(),
            evidence: evidence_from_dto(&dto.decision.evidence),
        },
        outcomes: dto
            .outcomes
            .iter()
            .map(outcome_from_dto)
            .collect::<Result<_, _>>()?,
        leader: match &dto.leader {
            None => None,
            Some(l) => Some(LeaderSelection {
                leader: descriptor_from_dto(&l.leader)?,
                considered: l.considered,
                eligible: l.eligible,
                assurance: assurance_from_str(&l.assurance)?,
            }),
        },
        disagreements: dto
            .disagreements
            .iter()
            .map(|d| {
                Ok(Disagreement {
                    id: d.id.clone(),
                    participants: d
                        .participants
                        .iter()
                        .map(|p| {
                            ParticipantId::new(p.clone()).map_err(|_| SessionStoreError::Corrupt)
                        })
                        .collect::<Result<_, _>>()?,
                    summary: d.summary.clone(),
                    evidence: evidence_from_dto(&d.evidence),
                })
            })
            .collect::<Result<_, SessionStoreError>>()?,
        adjudication: dto.adjudication.as_ref().map(|a| RoundTableDecision {
            session_id: a.session_id.clone(),
            conclusion: a.conclusion.clone(),
            evidence: evidence_from_dto(&a.evidence),
            disagreement_ids: a.disagreement_ids.clone(),
            execution_authority: a.execution_authority,
        }),
        failure,
    })
}

// ------------------------------------------------------------------- the store

/// One JSON file per session, named by session id.
pub struct FileSessionStore {
    directory: PathBuf,
}

impl FileSessionStore {
    pub fn new(directory: impl AsRef<Path>) -> Self {
        Self {
            directory: directory.as_ref().to_path_buf(),
        }
    }

    /// Session ids come from callers, so never build a path from one unchecked.
    fn key(session_id: &str) -> Result<String, SessionStoreError> {
        if session_id.trim().is_empty() {
            return Err(SessionStoreError::InvalidId);
        }
        let safe: String = session_id
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        Ok(safe)
    }

    fn path_for(&self, session_id: &str) -> Result<PathBuf, SessionStoreError> {
        Ok(self
            .directory
            .join(format!("{}.json", Self::key(session_id)?)))
    }
}

impl SessionStore for FileSessionStore {
    fn save(&self, record: &SessionRecord) -> Result<(), SessionStoreError> {
        let path = self.path_for(&record.id)?;
        std::fs::create_dir_all(&self.directory).map_err(|_| SessionStoreError::Io)?;
        let body = serde_json::to_string_pretty(&to_dto(record))
            .map_err(|_| SessionStoreError::Corrupt)?;
        std::fs::write(path, body).map_err(|_| SessionStoreError::Io)
    }

    fn load(&self, session_id: &str) -> Result<Option<SessionRecord>, SessionStoreError> {
        let path = self.path_for(session_id)?;
        let body = match std::fs::read_to_string(&path) {
            Ok(body) => body,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Some platforms report "not found" when the history location
                // is itself a regular file. That is a broken store, not a
                // session that does not exist, so tell the two apart. A
                // genuinely absent directory stays healthy and is not created.
                return match std::fs::metadata(&self.directory) {
                    Ok(m) if !m.is_dir() => Err(SessionStoreError::Io),
                    _ => Ok(None),
                };
            }
            Err(_) => return Err(SessionStoreError::Io),
        };
        let dto: SessionDto =
            serde_json::from_str(&body).map_err(|_| SessionStoreError::Corrupt)?;
        from_dto(&dto).map(Some)
    }

    fn list(&self) -> Result<Vec<String>, SessionStoreError> {
        let entries = match std::fs::read_dir(&self.directory) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(SessionStoreError::Io),
        };
        let mut ids = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                ids.push(stem.to_owned());
            }
        }
        ids.sort();
        Ok(ids)
    }
}

// ---------------------------------------------------------------------------
// M0.15.1 — read side
//
// The Round Table's public read contract, implemented over stored records. A
// viewer depends on `SessionQuery` and never on this type, this crate, or the
// JSON on disk. That is the whole point of the boundary: the storage format is
// private, and changing it must not reach a product surface.
// ---------------------------------------------------------------------------

/// Read-only query over persisted sessions.
pub struct StoredSessionQuery {
    store: FileSessionStore,
}

impl StoredSessionQuery {
    pub fn new(directory: impl AsRef<Path>) -> Self {
        Self {
            store: FileSessionStore::new(directory),
        }
    }
}

fn query_error(e: SessionStoreError) -> SessionQueryError {
    match e {
        SessionStoreError::Io => SessionQueryError::Unavailable,
        SessionStoreError::Corrupt => SessionQueryError::Corrupt,
        SessionStoreError::InvalidId => SessionQueryError::InvalidId,
    }
}

impl SessionQuery for StoredSessionQuery {
    fn list_sessions(&self) -> Result<Vec<SessionSummary>, SessionQueryError> {
        let ids = self.store.list().map_err(query_error)?;
        let mut summaries = Vec::with_capacity(ids.len());
        for id in &ids {
            // A single corrupt record must not blank the whole history: it is
            // reported when asked for directly, but listing stays useful. A
            // viewer that shows nothing because one file is damaged is worse
            // than one that shows the rest.
            if let Ok(Some(record)) = self.store.load(id) {
                summaries.push(record.to_view().summary);
            }
        }
        // Records exist but none could be read: that is not an empty history,
        // and returning an empty list would say "no sessions" about a store
        // that plainly has some.
        if summaries.is_empty() && !ids.is_empty() {
            return Err(SessionQueryError::Corrupt);
        }
        Ok(summaries)
    }

    fn get_session(&self, session_id: &str) -> Result<Option<SessionView>, SessionQueryError> {
        Ok(self
            .store
            .load(session_id)
            .map_err(query_error)?
            .map(|record| record.to_view()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_ids_cannot_escape_the_store_directory() {
        assert_eq!(
            FileSessionStore::key("../../etc/passwd").unwrap(),
            "______etc_passwd"
        );
        assert_eq!(FileSessionStore::key("session-1").unwrap(), "session-1");
        assert_eq!(
            FileSessionStore::key("  ").err(),
            Some(SessionStoreError::InvalidId)
        );
    }

    #[test]
    fn an_unknown_schema_version_is_corrupt_not_guessed_at() {
        let dto = SessionDto {
            schema_version: SCHEMA_VERSION + 1,
            id: "s".into(),
            assurance: "A3".into(),
            decision: DecisionDto {
                id: "d".into(),
                subject: "s".into(),
                prompt: "p".into(),
                evidence: vec![],
            },
            outcomes: vec![],
            leader: None,
            disagreements: vec![],
            adjudication: None,
            failure: None,
        };
        assert_eq!(from_dto(&dto).err(), Some(SessionStoreError::Corrupt));
    }
}
