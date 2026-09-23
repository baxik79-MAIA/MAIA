//! Local Intelligence Hypothesis Evidence Ledger (M0.15.12).
//!
//! Turns individual `DiagnosticSnapshot` reads (M0.15.11) into persistent
//! longitudinal evidence a future hypothesis/evolution system can query
//! without knowing SQLite health-history schema, `infra/local-model`
//! implementation details, provider wire protocols, or Round Table
//! implementation.
//!
//! This ledger stores diagnostic observations and caller-produced proposed
//! hypotheses. It does not generate or evaluate hypotheses, supervise a
//! lifecycle, plan repairs, execute actions, schedule work, or call a model.
//! It has **no process-control authority** (see
//! `tests/capability_mesh.rs`).
//!
//! # Discovery (before implementation)
//!
//! A repository-wide search for a hypothesis ledger, evidence ledger,
//! diagnostic observation store, longitudinal diagnostic record, or
//! generic immutable evidence store found nothing to reuse. The only
//! "ledger" concept anywhere in the repository is Round Table's own
//! coordination ledger (`reports/roundtable/ledger/*.json`) — Python
//! report-generation tooling, Round-Table-specific, and off-limits as a
//! dependency here regardless (no Round Table dependency is allowed). A
//! new, small, typed contract was genuinely necessary — see the M0.15.12
//! report.
//!
//! # Why a new, separate store (again)
//!
//! Same reasoning as `infra/local-intelligence-health`'s own module docs:
//! `infra/sqlite`'s authoritative, workspace-scoped, hash-chained audit
//! ledger is the wrong tool for this — this data is not a material,
//! governance-grade action, and reusing that store's single-writer
//! exclusive-lock discipline for evidence bookkeeping would misapply a
//! governance mechanism outside its purpose. This crate owns one small,
//! explicitly-versioned SQLite database of its own.
//!
//! # Persisting an interpretation, not raw history again
//!
//! One row here is one already-computed `DiagnosticSnapshot`, stored
//! whole (as JSON) alongside a caller-supplied identity and timestamp —
//! never a second copy of the raw `status_samples`/`inference_outcomes`
//! rows M0.15.10's health store already owns.
#![forbid(unsafe_code)]

use maia_local_intelligence_diagnostics::{DiagnosticPattern, DiagnosticSnapshot, EvidenceLevel};
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Unchanged by M0.15.15's addition of the `hypotheses` table: a version
/// bump is for when an EXISTING table's shape changes in a way that could
/// break an older reader (`observations`' own shape is untouched here).
/// Adding a new, independent table via `CREATE TABLE IF NOT EXISTS` is
/// purely additive — an older binary opening a file with `hypotheses`
/// already present simply never queries it; a newer binary opening an
/// older file without it creates it fresh. Neither direction can conflict
/// or corrupt data, so no migration and no version bump is needed.
const SCHEMA_VERSION: i64 = 1;
const MAX_OBSERVATION_ID_LEN: usize = 256;
/// Sanity ceiling on any single bounded query or scan this crate performs,
/// mirroring `infra/local-intelligence-health`'s own `MAX_BOUNDED_SCAN`.
pub const MAX_BOUNDED_SCAN: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerError {
    /// The ledger could not be opened, or a write/query failed at the
    /// storage layer.
    StorageUnavailable,
    /// An existing on-disk file's recorded schema version does not match
    /// this crate's `SCHEMA_VERSION`.
    SchemaVersionMismatch,
    /// A caller-supplied ID was empty or implausibly long, a payload was
    /// invalid, or a query window was invalid (`until` not after `since`).
    InvalidRecord,
    /// A read query failed at the storage layer, or a stored payload could
    /// not be deserialized back into its typed record (a corrupt or
    /// foreign row) — either way, a typed error, never a panic and never a
    /// fabricated result.
    QueryFailed,
}

/// A caller-supplied, stable identity for one explicit diagnostic
/// recording operation. The *caller* derives this (e.g. from the
/// requested window plus a scheduling tick, or a UUID it generates once
/// and retries with) — this crate only enforces uniqueness; it never
/// invents identity on the caller's behalf, which would defeat the
/// crash/retry idempotence this type exists for.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct ObservationId(String);
impl ObservationId {
    pub fn new(value: impl Into<String>) -> Result<Self, LedgerError> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_OBSERVATION_ID_LEN {
            Err(LedgerError::InvalidRecord)
        } else {
            Ok(Self(value))
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One persisted diagnostic observation: a whole `DiagnosticSnapshot`,
/// with the identity and wall-clock time of the explicit recording
/// operation that produced it. Preserves the M0.15.11 scope distinction
/// unchanged — `snapshot.current_availability`/`current_model` are
/// store-wide latest state; everything else on `snapshot` describes the
/// requested window — by simply not re-deriving or re-scoping any of it.
#[derive(Debug, Clone, PartialEq)]
pub struct DiagnosticObservation {
    pub observation_id: ObservationId,
    pub observed_at: SystemTime,
    pub snapshot: DiagnosticSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordOutcome {
    /// A new row was created.
    Recorded,
    /// `observation_id` already existed; no new row was created and the
    /// existing one was left untouched. Callers that need the existing
    /// value can fetch it with `Ledger::get`.
    AlreadyExists,
}

/// A caller-supplied, stable identity for one submitted hypothesis — same
/// validation and idempotence shape as `ObservationId` (M0.15.15). The
/// caller derives this (e.g. deterministically from its input and evidence
/// window, so a retried submission
/// does not create a duplicate hypothesis record); this type only enforces
/// uniqueness, never invents identity itself.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct HypothesisId(String);
impl HypothesisId {
    pub fn new(value: impl Into<String>) -> Result<Self, LedgerError> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_OBSERVATION_ID_LEN {
            Err(LedgerError::InvalidRecord)
        } else {
            Ok(Self(value))
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A qualitative confidence value asserted by the caller. The ledger does
/// not compute, validate, or endorse this assessment. No certainty or
/// confirmation variant is offered by this persistence contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HypothesisConfidence {
    Low,
    Moderate,
    High,
}

/// Storage status supplied with the record. `Proposed` is the only status
/// available in this milestone; the ledger cannot promote or adjudicate it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HypothesisStatus {
    Proposed,
}

/// One caller-submitted diagnostic hypothesis. Every assessment and claimed
/// evidence relationship here is caller-supplied, never ledger-computed.
///
/// `explanation` is an interpretation, not evidence. The two evidence lists
/// are supplied ID references only: this ledger does not check that those
/// observations exist, are relevant, are independent, support or contradict
/// the explanation, or establish causality. `subject` reuses
/// `maia_local_intelligence_diagnostics::DiagnosticPattern` directly
/// rather than introducing a parallel "observed condition" concept.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DiagnosticHypothesis {
    pub hypothesis_id: HypothesisId,
    pub subject: DiagnosticPattern,
    pub explanation: String,
    /// Caller-supplied references, not verified supporting evidence.
    pub supporting_evidence: Vec<ObservationId>,
    /// Caller-supplied references, not verified contradicting evidence.
    pub contradicting_evidence: Vec<ObservationId>,
    /// Caller assertion; no confidence calculation occurs in this crate.
    pub confidence: HypothesisConfidence,
    /// Caller-supplied account of what remains uncertain. Older records
    /// pre-dating this field deserialize with an empty value.
    #[serde(default)]
    pub uncertainty: String,
    /// Caller assertion; the ledger does not determine sufficiency.
    pub evidence_sufficient: bool,
    /// Caller-supplied timestamp of generation/submission.
    pub generated_at: SystemTime,
    /// Caller-supplied generator label. The ledger cannot verify its identity.
    pub generator: String,
    /// Caller-supplied status, restricted to `Proposed` in this milestone.
    pub status: HypothesisStatus,
}

/// Bounds on how much evidence this ledger will ever hold — the ledger
/// must also be bounded, per the M0.15.12 directive, same discipline as
/// `infra/local-intelligence-health`'s `RetentionPolicy`: enforced on
/// every successful write, not a separate scheduled job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub max_rows: usize,
    pub max_age: Duration,
}
impl Default for RetentionPolicy {
    /// Matches `infra/local-intelligence-health`'s own default numbers for
    /// consistency. Unlike the health store's samples (recorded on a fixed
    /// 5-second cadence, giving a precisely computable retention horizon),
    /// ledger observations are recorded only on explicit calls — no
    /// scheduler exists yet (M0.15.12 §8) — so the effective retained
    /// duration depends entirely on how often a future caller invokes
    /// `record`, and is not claimed here.
    fn default() -> Self {
        Self {
            max_rows: 10_000,
            max_age: Duration::from_secs(30 * 24 * 60 * 60),
        }
    }
}

/// How much ledger history is actually retained right now — see
/// `Ledger::coverage`'s docs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerCoverage {
    pub earliest_retained: Option<SystemTime>,
    pub latest_retained: Option<SystemTime>,
    pub observation_count: u64,
}

fn millis_since_epoch(at: SystemTime) -> i64 {
    at.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
fn time_from_millis(ms: i64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(ms.max(0) as u64)
}

/// The tri-state read the M0.15.12 directive requires: never collapse
/// missing/insufficient/partial evidence into false counter-evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceState {
    /// The pattern was present in an observation whose evidence was
    /// sufficient and whose requested window was fully covered by
    /// retained history.
    Supported,
    /// The pattern was absent from an observation whose evidence was
    /// sufficient and whose requested window was fully covered by
    /// retained history — a genuine counter-observation.
    NotSupported,
    /// The observation's evidence was `NoEvidence`/`InsufficientEvidence`,
    /// or its requested window was only partially covered by retained
    /// history. Neither support nor counter-evidence: the conservative
    /// classification the directive explicitly recommends over turning
    /// weak evidence into a false negative.
    Indeterminate,
}

/// The tri-state classification of one observation with respect to one
/// pattern — a pure function of already-recorded data, exposed directly so
/// its rule is independently testable and documented in one place. An
/// observation is `Indeterminate` whenever its evidence is
/// `NoEvidence`/`InsufficientEvidence` OR its requested window was only
/// partially covered by retained history (the M0.15.12 directive's
/// explicit "partial retained-history coverage must remain visible and
/// must not silently be treated as complete evidence" — silently letting
/// partial coverage count toward `NotSupported` would be exactly that).
pub fn pattern_state(
    observation: &DiagnosticObservation,
    pattern: DiagnosticPattern,
) -> EvidenceState {
    let insufficient = matches!(
        observation.snapshot.evidence_level,
        EvidenceLevel::NoEvidence | EvidenceLevel::InsufficientEvidence
    );
    let partial_coverage = !observation
        .snapshot
        .coverage
        .status_history_covers_requested_window;
    if insufficient || partial_coverage {
        EvidenceState::Indeterminate
    } else if observation.snapshot.detected_patterns.contains(&pattern) {
        EvidenceState::Supported
    } else {
        EvidenceState::NotSupported
    }
}

/// Why `consecutive_supported_run`'s walk stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStopReason {
    /// The next-older observation was a genuine counter-observation.
    NotSupported,
    /// The next-older observation was `Indeterminate` — evidence gaps
    /// break a run exactly like a counter-observation does (M0.15.12 §4's
    /// explicit "repeated does not mean independent": an indeterminate
    /// observation in the middle of a sequence must not be silently
    /// skipped over, or five snapshots with one weak link would be
    /// misreported as an unbroken run of five).
    Indeterminate,
    /// No older observations exist in the ledger at all.
    NoMoreObservations,
    /// The scan reached its bounded limit before finding a stopping
    /// condition — the true run length may be longer, but this call never
    /// performed an unbounded scan to find out.
    ReachedScanLimit,
}

/// The strict, adjacent-only consecutive-support read: walks from the most
/// recent observation backward, counting while (and only while) each
/// observation in turn is `Supported`, stopping at the first one that is
/// not (see `RunStopReason`). Deliberately conservative — see
/// `Ledger::consecutive_supported_run`'s docs for the companion,
/// eligibility-skipping query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsecutiveRunResult {
    pub pattern: DiagnosticPattern,
    pub run_length: u64,
    pub stopped_reason: RunStopReason,
    /// `observed_at` of the newest observation in the run (the most
    /// recent observation overall, if `run_length > 0`).
    pub newest_in_run_observed_at: Option<SystemTime>,
    /// `observed_at` of the oldest observation in the run.
    pub oldest_in_run_observed_at: Option<SystemTime>,
}

/// The eligibility-skipping companion read: walks from the most recent
/// observation backward, SKIPPING `Indeterminate` ones (they neither
/// support nor break the count) until `requested_n` eligible
/// (`Supported`/`NotSupported`) observations have been found or the scan
/// is exhausted. `all_supported` is `true` only when exactly
/// `requested_n` eligible observations were found and every one of them
/// was `Supported` — never `true` merely because no counter-observation
/// happened to be found yet. `oldest_examined_at`/`newest_examined_at`
/// span every observation actually looked at (including skipped
/// indeterminate ones), so a caller can judge how much wall-clock time —
/// and therefore how much window overlap — the read actually spans, per
/// the M0.15.12 directive's explicit "consecutive does not mean
/// independent" requirement. This type deliberately carries no confidence
/// score.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EligibleSupportResult {
    pub pattern: DiagnosticPattern,
    pub requested_n: usize,
    pub eligible_found: usize,
    pub all_supported: bool,
    pub indeterminate_skipped: usize,
    pub observations_scanned: usize,
    pub oldest_examined_at: Option<SystemTime>,
    pub newest_examined_at: Option<SystemTime>,
}

/// A small, dedicated, append-oriented SQLite ledger of diagnostic
/// observations. One `Ledger` is meant to be constructed once per process
/// and shared (e.g. behind an `Arc`) — same single-connection-behind-a-
/// `Mutex` discipline as `infra/local-intelligence-health::HealthStore`,
/// for the same reason (see that crate's module docs): opening a fresh
/// connection per call risks exactly the concurrency hazard flagged in
/// the capability record's Run 1 postmortem.
pub struct Ledger {
    conn: Mutex<Connection>,
    retention: RetentionPolicy,
}

impl Ledger {
    pub fn open(path: &std::path::Path, retention: RetentionPolicy) -> Result<Self, LedgerError> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path).map_err(|_| LedgerError::StorageUnavailable)?;
        Self::from_connection(conn, retention)
    }

    pub fn open_in_memory(retention: RetentionPolicy) -> Result<Self, LedgerError> {
        let conn = Connection::open_in_memory().map_err(|_| LedgerError::StorageUnavailable)?;
        Self::from_connection(conn, retention)
    }

    fn from_connection(conn: Connection, retention: RetentionPolicy) -> Result<Self, LedgerError> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS schema_meta (
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS observations (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 observation_id TEXT NOT NULL UNIQUE,
                 observed_at_ms INTEGER NOT NULL,
                 payload_json TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_observations_time
                 ON observations(observed_at_ms);
             CREATE TABLE IF NOT EXISTS hypotheses (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 hypothesis_id TEXT NOT NULL UNIQUE,
                 generated_at_ms INTEGER NOT NULL,
                 payload_json TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_hypotheses_time
                 ON hypotheses(generated_at_ms);",
        )
        .map_err(|_| LedgerError::StorageUnavailable)?;
        conn.execute(
            "INSERT OR IGNORE INTO schema_meta(key, value) VALUES ('version', ?1)",
            params![SCHEMA_VERSION.to_string()],
        )
        .map_err(|_| LedgerError::StorageUnavailable)?;
        let stored: String = conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = 'version'",
                [],
                |row| row.get(0),
            )
            .map_err(|_| LedgerError::StorageUnavailable)?;
        if stored != SCHEMA_VERSION.to_string() {
            return Err(LedgerError::SchemaVersionMismatch);
        }
        Ok(Self {
            conn: Mutex::new(conn),
            retention,
        })
    }

    /// Records one `DiagnosticObservation`. Atomic create-only semantics:
    /// a single `INSERT ... ON CONFLICT(observation_id) DO NOTHING`
    /// executed under the same connection lock as every other operation —
    /// never a separate check-then-insert, so a crash/retry of the exact
    /// same call can never create a second row for the same
    /// `observation_id`, and therefore can never inflate a consecutive
    /// pattern count.
    pub fn record(
        &self,
        observation: &DiagnosticObservation,
    ) -> Result<RecordOutcome, LedgerError> {
        let payload =
            serde_json::to_string(&observation.snapshot).map_err(|_| LedgerError::InvalidRecord)?;
        let conn = self
            .conn
            .lock()
            .map_err(|_| LedgerError::StorageUnavailable)?;
        let changed = conn
            .execute(
                "INSERT INTO observations (observation_id, observed_at_ms, payload_json)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(observation_id) DO NOTHING",
                params![
                    observation.observation_id.as_str(),
                    millis_since_epoch(observation.observed_at),
                    payload,
                ],
            )
            .map_err(|_| LedgerError::StorageUnavailable)?;
        let outcome = if changed == 1 {
            RecordOutcome::Recorded
        } else {
            RecordOutcome::AlreadyExists
        };
        if outcome == RecordOutcome::Recorded {
            enforce_retention(
                &conn,
                "observations",
                "observed_at_ms",
                self.retention,
                observation.observed_at,
            )?;
        }
        Ok(outcome)
    }

    /// Fetches one observation by its identity, or `None` if it does not
    /// exist (never distinguished from "not yet recorded" vs. "evicted by
    /// retention" — both are simply absent).
    pub fn get(
        &self,
        observation_id: &ObservationId,
    ) -> Result<Option<DiagnosticObservation>, LedgerError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| LedgerError::StorageUnavailable)?;
        conn.query_row(
            "SELECT observation_id, observed_at_ms, payload_json
             FROM observations WHERE observation_id = ?1",
            params![observation_id.as_str()],
            row_to_observation,
        )
        .optional()
        .map_err(|_| LedgerError::StorageUnavailable)?
        .transpose()
    }

    /// The most recently recorded observation, if any.
    pub fn latest(&self) -> Result<Option<DiagnosticObservation>, LedgerError> {
        Ok(self.latest_n(1)?.into_iter().next())
    }

    /// The `limit` most recent observations, most-recent-first. Bounded to
    /// `MAX_BOUNDED_SCAN` regardless of the requested limit.
    pub fn latest_n(&self, limit: usize) -> Result<Vec<DiagnosticObservation>, LedgerError> {
        let bounded = limit.clamp(1, MAX_BOUNDED_SCAN);
        let conn = self
            .conn
            .lock()
            .map_err(|_| LedgerError::StorageUnavailable)?;
        let mut stmt = conn
            .prepare(
                "SELECT observation_id, observed_at_ms, payload_json
                 FROM observations
                 ORDER BY observed_at_ms DESC, id DESC
                 LIMIT ?1",
            )
            .map_err(|_| LedgerError::QueryFailed)?;
        let rows = stmt
            .query_map(params![bounded as i64], row_to_observation)
            .map_err(|_| LedgerError::QueryFailed)?;
        collect_rows(rows)
    }

    /// Observations recorded in `[since, until]`, most-recent-first,
    /// bounded to at most `limit` rows.
    pub fn observations_in_window(
        &self,
        since: SystemTime,
        until: SystemTime,
        limit: usize,
    ) -> Result<Vec<DiagnosticObservation>, LedgerError> {
        if until <= since {
            return Err(LedgerError::InvalidRecord);
        }
        let bounded = limit.clamp(1, MAX_BOUNDED_SCAN);
        let conn = self
            .conn
            .lock()
            .map_err(|_| LedgerError::StorageUnavailable)?;
        let mut stmt = conn
            .prepare(
                "SELECT observation_id, observed_at_ms, payload_json
                 FROM observations
                 WHERE observed_at_ms >= ?1 AND observed_at_ms <= ?2
                 ORDER BY observed_at_ms DESC, id DESC
                 LIMIT ?3",
            )
            .map_err(|_| LedgerError::QueryFailed)?;
        let rows = stmt
            .query_map(
                params![
                    millis_since_epoch(since),
                    millis_since_epoch(until),
                    bounded as i64
                ],
                row_to_observation,
            )
            .map_err(|_| LedgerError::QueryFailed)?;
        collect_rows(rows)
    }

    /// How much ledger history is actually retained right now, independent
    /// of any query window — the ledger's own analogue of
    /// `HealthStore::history_coverage()`. Lets a caller report retention
    /// truncation explicitly (M0.15.12 §7: "if requested ledger history
    /// predates retained observations, expose partial coverage /
    /// truncated-history information rather than pretending the entire
    /// period was inspected") instead of a query silently returning fewer
    /// rows than expected with no explanation.
    pub fn coverage(&self) -> Result<LedgerCoverage, LedgerError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| LedgerError::StorageUnavailable)?;
        let (min, max, count): (Option<i64>, Option<i64>, i64) = conn
            .query_row(
                "SELECT MIN(observed_at_ms), MAX(observed_at_ms), COUNT(*) FROM observations",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|_| LedgerError::QueryFailed)?;
        Ok(LedgerCoverage {
            earliest_retained: min.map(time_from_millis),
            latest_retained: max.map(time_from_millis),
            observation_count: count.max(0) as u64,
        })
    }

    /// Records one `DiagnosticHypothesis` (M0.15.15). Same atomic
    /// create-only semantics as `record`: a single
    /// `INSERT ... ON CONFLICT(hypothesis_id) DO NOTHING`, so a retried
    /// submission attempt with the same `hypothesis_id` can never create a
    /// duplicate record or overwrite the original.
    /// All assessment fields and evidence references are stored as supplied;
    /// this operation performs no evidence lookup or evaluation.
    pub fn record_hypothesis(
        &self,
        hypothesis: &DiagnosticHypothesis,
    ) -> Result<RecordOutcome, LedgerError> {
        let payload = serde_json::to_string(hypothesis).map_err(|_| LedgerError::InvalidRecord)?;
        let conn = self
            .conn
            .lock()
            .map_err(|_| LedgerError::StorageUnavailable)?;
        let changed = conn
            .execute(
                "INSERT INTO hypotheses (hypothesis_id, generated_at_ms, payload_json)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(hypothesis_id) DO NOTHING",
                params![
                    hypothesis.hypothesis_id.as_str(),
                    millis_since_epoch(hypothesis.generated_at),
                    payload,
                ],
            )
            .map_err(|_| LedgerError::StorageUnavailable)?;
        let outcome = if changed == 1 {
            RecordOutcome::Recorded
        } else {
            RecordOutcome::AlreadyExists
        };
        if outcome == RecordOutcome::Recorded {
            enforce_retention(
                &conn,
                "hypotheses",
                "generated_at_ms",
                self.retention,
                hypothesis.generated_at,
            )?;
        }
        Ok(outcome)
    }

    /// Fetches one hypothesis by its identity, or `None` if it does not
    /// exist.
    pub fn get_hypothesis(
        &self,
        hypothesis_id: &HypothesisId,
    ) -> Result<Option<DiagnosticHypothesis>, LedgerError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| LedgerError::StorageUnavailable)?;
        conn.query_row(
            "SELECT hypothesis_id, generated_at_ms, payload_json
             FROM hypotheses WHERE hypothesis_id = ?1",
            params![hypothesis_id.as_str()],
            row_to_hypothesis,
        )
        .optional()
        .map_err(|_| LedgerError::StorageUnavailable)?
        .transpose()
    }

    /// The `limit` most recently generated hypotheses, most-recent-first.
    /// Bounded to `MAX_BOUNDED_SCAN` regardless of the requested limit.
    pub fn latest_hypotheses(
        &self,
        limit: usize,
    ) -> Result<Vec<DiagnosticHypothesis>, LedgerError> {
        let bounded = limit.clamp(1, MAX_BOUNDED_SCAN);
        let conn = self
            .conn
            .lock()
            .map_err(|_| LedgerError::StorageUnavailable)?;
        let mut stmt = conn
            .prepare(
                "SELECT hypothesis_id, generated_at_ms, payload_json
                 FROM hypotheses
                 ORDER BY generated_at_ms DESC, id DESC
                 LIMIT ?1",
            )
            .map_err(|_| LedgerError::QueryFailed)?;
        let rows = stmt
            .query_map(params![bounded as i64], row_to_hypothesis)
            .map_err(|_| LedgerError::QueryFailed)?;
        collect_hypothesis_rows(rows)
    }

    /// Hypotheses generated in `[since, until]`, most-recent-first, bounded
    /// to at most `limit` rows.
    pub fn hypotheses_in_window(
        &self,
        since: SystemTime,
        until: SystemTime,
        limit: usize,
    ) -> Result<Vec<DiagnosticHypothesis>, LedgerError> {
        if until <= since {
            return Err(LedgerError::InvalidRecord);
        }
        let bounded = limit.clamp(1, MAX_BOUNDED_SCAN);
        let conn = self
            .conn
            .lock()
            .map_err(|_| LedgerError::StorageUnavailable)?;
        let mut stmt = conn
            .prepare(
                "SELECT hypothesis_id, generated_at_ms, payload_json
                 FROM hypotheses
                 WHERE generated_at_ms >= ?1 AND generated_at_ms <= ?2
                 ORDER BY generated_at_ms DESC, id DESC
                 LIMIT ?3",
            )
            .map_err(|_| LedgerError::QueryFailed)?;
        let rows = stmt
            .query_map(
                params![
                    millis_since_epoch(since),
                    millis_since_epoch(until),
                    bounded as i64
                ],
                row_to_hypothesis,
            )
            .map_err(|_| LedgerError::QueryFailed)?;
        collect_hypothesis_rows(rows)
    }

    /// The strict, adjacent-only consecutive-support read — see
    /// `ConsecutiveRunResult`'s docs. `scan_limit` bounds how far back
    /// this call will ever look.
    pub fn consecutive_supported_run(
        &self,
        pattern: DiagnosticPattern,
        scan_limit: usize,
    ) -> Result<ConsecutiveRunResult, LedgerError> {
        let observations = self.latest_n(scan_limit)?;
        let mut run_length = 0u64;
        let mut newest_in_run_observed_at = None;
        let mut oldest_in_run_observed_at = None;
        let mut stopped_reason = RunStopReason::NoMoreObservations;
        for observation in &observations {
            match pattern_state(observation, pattern) {
                EvidenceState::Supported => {
                    run_length += 1;
                    if newest_in_run_observed_at.is_none() {
                        newest_in_run_observed_at = Some(observation.observed_at);
                    }
                    oldest_in_run_observed_at = Some(observation.observed_at);
                }
                EvidenceState::NotSupported => {
                    stopped_reason = RunStopReason::NotSupported;
                    return Ok(ConsecutiveRunResult {
                        pattern,
                        run_length,
                        stopped_reason,
                        newest_in_run_observed_at,
                        oldest_in_run_observed_at,
                    });
                }
                EvidenceState::Indeterminate => {
                    stopped_reason = RunStopReason::Indeterminate;
                    return Ok(ConsecutiveRunResult {
                        pattern,
                        run_length,
                        stopped_reason,
                        newest_in_run_observed_at,
                        oldest_in_run_observed_at,
                    });
                }
            }
        }
        // Walked every observation returned without hitting a stop
        // condition: either the ledger truly has no more history, or the
        // bounded scan itself was the limiting factor.
        if run_length as usize == scan_limit.clamp(1, MAX_BOUNDED_SCAN) {
            stopped_reason = RunStopReason::ReachedScanLimit;
        }
        Ok(ConsecutiveRunResult {
            pattern,
            run_length,
            stopped_reason,
            newest_in_run_observed_at,
            oldest_in_run_observed_at,
        })
    }

    /// The eligibility-skipping companion read — see
    /// `EligibleSupportResult`'s docs. `scan_limit` bounds how far back
    /// this call will ever look while searching for `n` eligible
    /// observations.
    pub fn eligible_support_over_last_n(
        &self,
        pattern: DiagnosticPattern,
        n: usize,
        scan_limit: usize,
    ) -> Result<EligibleSupportResult, LedgerError> {
        let observations = self.latest_n(scan_limit)?;
        let mut eligible_found = 0usize;
        let mut indeterminate_skipped = 0usize;
        let mut observations_scanned = 0usize;
        let mut all_supported = true;
        let mut oldest_examined_at = None;
        let mut newest_examined_at = None;
        for observation in &observations {
            if eligible_found >= n {
                break;
            }
            observations_scanned += 1;
            if newest_examined_at.is_none() {
                newest_examined_at = Some(observation.observed_at);
            }
            oldest_examined_at = Some(observation.observed_at);
            match pattern_state(observation, pattern) {
                EvidenceState::Indeterminate => indeterminate_skipped += 1,
                EvidenceState::Supported => eligible_found += 1,
                EvidenceState::NotSupported => {
                    eligible_found += 1;
                    all_supported = false;
                }
            }
        }
        let all_supported = all_supported && eligible_found == n;
        Ok(EligibleSupportResult {
            pattern,
            requested_n: n,
            eligible_found,
            all_supported,
            indeterminate_skipped,
            observations_scanned,
            oldest_examined_at,
            newest_examined_at,
        })
    }
}

fn row_to_observation(
    row: &rusqlite::Row,
) -> rusqlite::Result<Result<DiagnosticObservation, LedgerError>> {
    let observation_id: String = row.get(0)?;
    let ms: i64 = row.get(1)?;
    let payload: String = row.get(2)?;
    let observation = match (
        ObservationId::new(observation_id),
        serde_json::from_str::<DiagnosticSnapshot>(&payload),
    ) {
        (Ok(observation_id), Ok(snapshot)) => Ok(DiagnosticObservation {
            observation_id,
            observed_at: time_from_millis(ms),
            snapshot,
        }),
        _ => Err(LedgerError::QueryFailed),
    };
    Ok(observation)
}

fn collect_rows(
    rows: rusqlite::MappedRows<
        '_,
        impl FnMut(&rusqlite::Row) -> rusqlite::Result<Result<DiagnosticObservation, LedgerError>>,
    >,
) -> Result<Vec<DiagnosticObservation>, LedgerError> {
    let mut out = Vec::new();
    for row in rows {
        let observation = row.map_err(|_| LedgerError::QueryFailed)??;
        out.push(observation);
    }
    Ok(out)
}

fn row_to_hypothesis(
    row: &rusqlite::Row,
) -> rusqlite::Result<Result<DiagnosticHypothesis, LedgerError>> {
    let stored_id: String = row.get(0)?;
    let stored_ms: i64 = row.get(1)?;
    let payload: String = row.get(2)?;
    let hypothesis = serde_json::from_str::<DiagnosticHypothesis>(&payload)
        .map_err(|_| LedgerError::QueryFailed)
        .and_then(|hypothesis| {
            if hypothesis.hypothesis_id.as_str() == stored_id
                && millis_since_epoch(hypothesis.generated_at) == stored_ms
            {
                Ok(hypothesis)
            } else {
                Err(LedgerError::QueryFailed)
            }
        });
    Ok(hypothesis)
}

fn collect_hypothesis_rows(
    rows: rusqlite::MappedRows<
        '_,
        impl FnMut(&rusqlite::Row) -> rusqlite::Result<Result<DiagnosticHypothesis, LedgerError>>,
    >,
) -> Result<Vec<DiagnosticHypothesis>, LedgerError> {
    let mut out = Vec::new();
    for row in rows {
        let hypothesis = row.map_err(|_| LedgerError::QueryFailed)??;
        out.push(hypothesis);
    }
    Ok(out)
}

/// Deletes rows older than `retention.max_age` (relative to `now`), then
/// deletes any rows beyond `retention.max_rows`, keeping the newest. Same
/// discipline as `infra/local-intelligence-health::enforce_retention`.
/// `table`/`time_column` are always one of this crate's own hardcoded
/// (table, timestamp-column) pairs, never external input — generalized
/// (M0.15.15) so the same bounded-retention logic covers both
/// `observations` (`observed_at_ms`) and `hypotheses` (`generated_at_ms`)
/// without duplicating it.
fn enforce_retention(
    conn: &Connection,
    table: &'static str,
    time_column: &'static str,
    retention: RetentionPolicy,
    now: SystemTime,
) -> Result<(), LedgerError> {
    let cutoff = millis_since_epoch(now) - retention.max_age.as_millis() as i64;
    conn.execute(
        &format!("DELETE FROM {table} WHERE {time_column} < ?1"),
        params![cutoff],
    )
    .map_err(|_| LedgerError::StorageUnavailable)?;
    conn.execute(
        &format!(
            "DELETE FROM {table} WHERE id NOT IN \
             (SELECT id FROM {table} ORDER BY {time_column} DESC, id DESC LIMIT ?1)"
        ),
        params![retention.max_rows as i64],
    )
    .map_err(|_| LedgerError::StorageUnavailable)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use maia_local_intelligence_health::{
        HealthStore, OperationClass, RetentionPolicy as HealthRetentionPolicy,
    };
    use maia_local_model::{LocalIntelligenceFailure, LocalIntelligenceStatus};

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn ledger() -> Ledger {
        Ledger::open_in_memory(RetentionPolicy::default()).expect("open in-memory ledger")
    }

    fn status(available: bool) -> LocalIntelligenceStatus {
        LocalIntelligenceStatus {
            available,
            model: "qwen3:4b".into(),
        }
    }

    fn observation(
        id: &str,
        observed_at: SystemTime,
        snapshot: DiagnosticSnapshot,
    ) -> DiagnosticObservation {
        DiagnosticObservation {
            observation_id: ObservationId::new(id).expect("valid id"),
            observed_at,
            snapshot,
        }
    }

    /// A snapshot with >= 5 status samples fully covering [0, 200) and 3
    /// timeout failures -- eligible evidence, `RepeatedTimeout` present.
    fn repeated_timeout_snapshot() -> DiagnosticSnapshot {
        let health = HealthStore::open_in_memory(HealthRetentionPolicy::default()).unwrap();
        for i in 0..10u64 {
            health
                .record_status_sample(&status(true), None, at(i))
                .unwrap();
        }
        for i in 0..3u64 {
            health
                .record_inference_failure(
                    "qwen3:4b",
                    OperationClass::CompleteDetailed,
                    LocalIntelligenceFailure::Timeout,
                    None,
                    at(100 + i),
                )
                .unwrap();
        }
        DiagnosticSnapshot::build(&health, at(0), at(200), 100).unwrap()
    }

    /// A snapshot with >= 5 status samples, no failures -- eligible
    /// evidence, no patterns present (`NotSupported` for any pattern).
    fn healthy_snapshot() -> DiagnosticSnapshot {
        let health = HealthStore::open_in_memory(HealthRetentionPolicy::default()).unwrap();
        for i in 0..10u64 {
            health
                .record_status_sample(&status(true), None, at(i))
                .unwrap();
        }
        DiagnosticSnapshot::build(&health, at(0), at(20), 100).unwrap()
    }

    /// Zero status samples in the requested window -- `EvidenceLevel::NoEvidence`.
    fn no_evidence_snapshot() -> DiagnosticSnapshot {
        let health = HealthStore::open_in_memory(HealthRetentionPolicy::default()).unwrap();
        DiagnosticSnapshot::build(&health, at(0), at(20), 100).unwrap()
    }

    /// Two status samples in the requested window -- below
    /// `MIN_SAMPLES_FOR_CONFIDENT_READ` (5) -- `EvidenceLevel::InsufficientEvidence`.
    fn insufficient_evidence_snapshot() -> DiagnosticSnapshot {
        let health = HealthStore::open_in_memory(HealthRetentionPolicy::default()).unwrap();
        for i in 0..2u64 {
            health
                .record_status_sample(&status(true), None, at(i))
                .unwrap();
        }
        DiagnosticSnapshot::build(&health, at(0), at(10), 100).unwrap()
    }

    /// Plenty of samples, but the requested window reaches back before any
    /// retained history exists -- `coverage.status_history_covers_requested_window == false`.
    fn partial_coverage_snapshot() -> DiagnosticSnapshot {
        let health = HealthStore::open_in_memory(HealthRetentionPolicy::default()).unwrap();
        for i in 100..110u64 {
            health
                .record_status_sample(&status(true), None, at(i))
                .unwrap();
        }
        DiagnosticSnapshot::build(&health, at(0), at(200), 100).unwrap()
    }

    // A. one DiagnosticSnapshot persists and round-trips exactly enough evidence.
    #[test]
    fn a_recorded_observation_round_trips_its_evidence_exactly() {
        let ledger = ledger();
        let snapshot = repeated_timeout_snapshot();
        let obs = observation("a1", at(1000), snapshot.clone());
        ledger.record(&obs).expect("record");
        let fetched = ledger
            .get(&ObservationId::new("a1").unwrap())
            .expect("query")
            .expect("some row");
        assert_eq!(fetched.snapshot, snapshot);
        assert_eq!(fetched.observed_at, at(1000));
    }

    // B / L. duplicate observation_id does not create a second record; the
    // create-only INSERT...ON CONFLICT is atomic (single statement, no
    // separate check-then-insert), proven by calling record() twice
    // sequentially with the exact same observation.
    #[test]
    fn duplicate_observation_id_does_not_create_a_second_record() {
        let ledger = ledger();
        let obs = observation("dup", at(1), healthy_snapshot());
        let different_content_same_id = observation("dup", at(2), repeated_timeout_snapshot());
        assert_eq!(ledger.record(&obs).unwrap(), RecordOutcome::Recorded);
        assert_eq!(
            ledger.record(&different_content_same_id).unwrap(),
            RecordOutcome::AlreadyExists
        );
        let all = ledger.latest_n(10).unwrap();
        assert_eq!(
            all.len(),
            1,
            "a duplicate id must never create a second row"
        );
        assert_eq!(
            all[0].snapshot, obs.snapshot,
            "the original recording must be left untouched, not overwritten"
        );
    }

    #[test]
    fn duplicate_retry_of_the_exact_same_observation_is_atomic() {
        let ledger = ledger();
        let obs = observation("l1", at(1), healthy_snapshot());
        assert_eq!(ledger.record(&obs).unwrap(), RecordOutcome::Recorded);
        assert_eq!(ledger.record(&obs).unwrap(), RecordOutcome::AlreadyExists);
        assert_eq!(ledger.latest_n(10).unwrap().len(), 1);
    }

    // C. three consecutive eligible snapshots containing RepeatedTimeout
    // produce a supported run length of three.
    #[test]
    fn three_consecutive_repeated_timeout_snapshots_produce_a_run_of_three() {
        let ledger = ledger();
        for (index, id) in ["c1", "c2", "c3"].iter().enumerate() {
            ledger
                .record(&observation(
                    id,
                    at(100 + index as u64),
                    repeated_timeout_snapshot(),
                ))
                .unwrap();
        }
        let run = ledger
            .consecutive_supported_run(DiagnosticPattern::RepeatedTimeout, 100)
            .unwrap();
        assert_eq!(run.run_length, 3);
    }

    // D. pattern absent from an eligible snapshot breaks a SUPPORTED run.
    #[test]
    fn a_pattern_absent_snapshot_breaks_a_supported_run() {
        let ledger = ledger();
        ledger
            .record(&observation("d1", at(1), repeated_timeout_snapshot()))
            .unwrap();
        ledger
            .record(&observation("d2", at(2), repeated_timeout_snapshot()))
            .unwrap();
        // Most recent: eligible, but the pattern is absent.
        ledger
            .record(&observation("d3", at(3), healthy_snapshot()))
            .unwrap();
        let run = ledger
            .consecutive_supported_run(DiagnosticPattern::RepeatedTimeout, 100)
            .unwrap();
        assert_eq!(run.run_length, 0);
        assert_eq!(run.stopped_reason, RunStopReason::NotSupported);
    }

    // E. NoEvidence does not count as support.
    #[test]
    fn no_evidence_does_not_count_as_support() {
        let ledger = ledger();
        ledger
            .record(&observation("e1", at(1), no_evidence_snapshot()))
            .unwrap();
        let run = ledger
            .consecutive_supported_run(DiagnosticPattern::RepeatedTimeout, 100)
            .unwrap();
        assert_eq!(run.run_length, 0);
        assert_eq!(run.stopped_reason, RunStopReason::Indeterminate);
    }

    // F. InsufficientEvidence does not count as support.
    #[test]
    fn insufficient_evidence_does_not_count_as_support() {
        let ledger = ledger();
        ledger
            .record(&observation("f1", at(1), insufficient_evidence_snapshot()))
            .unwrap();
        let run = ledger
            .consecutive_supported_run(DiagnosticPattern::RepeatedTimeout, 100)
            .unwrap();
        assert_eq!(run.run_length, 0);
        assert_eq!(run.stopped_reason, RunStopReason::Indeterminate);
    }

    // G. partial coverage does not silently count as full evidence.
    #[test]
    fn partial_coverage_is_indeterminate_not_silently_full_evidence() {
        let snapshot = partial_coverage_snapshot();
        assert!(!snapshot.coverage.status_history_covers_requested_window);
        let obs = observation("g1", at(1), snapshot);
        assert_eq!(
            pattern_state(&obs, DiagnosticPattern::RepeatedTimeout),
            EvidenceState::Indeterminate
        );
    }

    // H. overlapping windows can form consecutive observations but are
    // never labeled independent confirmations: the ledger still reports a
    // plain run_length, and its result types carry no confidence/
    // independence concept at all (structural — see also
    // tests/capability_mesh.rs).
    #[test]
    fn overlapping_windows_are_counted_but_never_labeled_independent() {
        let ledger = ledger();
        let snap1 = repeated_timeout_snapshot();
        let snap2 = repeated_timeout_snapshot();
        assert_eq!(
            snap1.coverage.requested_since,
            snap2.coverage.requested_since
        );
        assert_eq!(
            snap1.coverage.requested_until,
            snap2.coverage.requested_until
        );
        ledger.record(&observation("h1", at(1), snap1)).unwrap();
        ledger.record(&observation("h2", at(2), snap2)).unwrap();
        let run = ledger
            .consecutive_supported_run(DiagnosticPattern::RepeatedTimeout, 100)
            .unwrap();
        assert_eq!(
            run.run_length, 2,
            "overlap does not prevent counting toward a run"
        );
        assert_eq!(run.oldest_in_run_observed_at, Some(at(1)));
        assert_eq!(run.newest_in_run_observed_at, Some(at(2)));
    }

    // I. mixed pattern sequence: supported / supported / indeterminate /
    // supported does not incorrectly become a four-observation supported
    // run.
    #[test]
    fn a_mixed_sequence_with_an_indeterminate_gap_does_not_become_a_full_run() {
        let ledger = ledger();
        ledger
            .record(&observation("i1", at(1), repeated_timeout_snapshot()))
            .unwrap();
        ledger
            .record(&observation("i2", at(2), repeated_timeout_snapshot()))
            .unwrap();
        ledger
            .record(&observation("i3", at(3), no_evidence_snapshot()))
            .unwrap();
        ledger
            .record(&observation("i4", at(4), repeated_timeout_snapshot()))
            .unwrap();
        let run = ledger
            .consecutive_supported_run(DiagnosticPattern::RepeatedTimeout, 100)
            .unwrap();
        assert_eq!(
            run.run_length, 1,
            "must not silently skip the indeterminate gap and report a run of 4"
        );
        assert_eq!(run.stopped_reason, RunStopReason::Indeterminate);
    }

    // Ordering corner case (M0.15.13 §7): two observations with the exact
    // same `observed_at` must still produce deterministic query ordering.
    // Proves the existing `ORDER BY observed_at_ms DESC, id DESC` clause
    // (present in every ordered query) already provides a deterministic
    // secondary key -- insertion order, via the monotonically increasing
    // autoincrement `id` -- so M0.15.13 needs no schema change here, only
    // this proof.
    #[test]
    fn same_observed_at_produces_deterministic_ordering_by_insertion() {
        let ledger = ledger();
        ledger
            .record(&observation("tie-first", at(100), healthy_snapshot()))
            .unwrap();
        ledger
            .record(&observation(
                "tie-second",
                at(100),
                repeated_timeout_snapshot(),
            ))
            .unwrap();
        let latest2 = ledger.latest_n(2).unwrap();
        assert_eq!(latest2.len(), 2);
        assert_eq!(
            latest2[0].observation_id.as_str(),
            "tie-second",
            "the more-recently-inserted row must sort first when observed_at ties"
        );
        assert_eq!(latest2[1].observation_id.as_str(), "tie-first");
        // Repeat the read to confirm this is a deterministic property of
        // the query, not an artifact of SQLite's incidental row order on
        // one particular call.
        let latest2_again = ledger.latest_n(2).unwrap();
        assert_eq!(latest2, latest2_again);
    }

    // J. bounded latest-N query.
    #[test]
    fn latest_n_is_bounded_by_the_requested_limit() {
        let ledger = ledger();
        for i in 0..20u64 {
            ledger
                .record(&observation(&format!("j{i}"), at(i), healthy_snapshot()))
                .unwrap();
        }
        let latest5 = ledger.latest_n(5).unwrap();
        assert_eq!(latest5.len(), 5);
        assert_eq!(latest5[0].observed_at, at(19));
    }

    // K. bounded time-range query.
    #[test]
    fn observations_in_window_respects_the_time_bound_and_limit() {
        let ledger = ledger();
        for i in 0..20u64 {
            ledger
                .record(&observation(&format!("k{i}"), at(i), healthy_snapshot()))
                .unwrap();
        }
        let windowed = ledger.observations_in_window(at(5), at(10), 3).unwrap();
        assert_eq!(windowed.len(), 3);
        for obs in &windowed {
            assert!(obs.observed_at >= at(5) && obs.observed_at <= at(10));
        }
    }

    // M. retention truncation is explicitly reported.
    #[test]
    fn retention_truncation_is_explicitly_reported_via_coverage() {
        let retention = RetentionPolicy {
            max_rows: 3,
            max_age: Duration::from_secs(1_000_000),
        };
        let ledger = Ledger::open_in_memory(retention).expect("open");
        for i in 0..10u64 {
            ledger
                .record(&observation(&format!("m{i}"), at(i), healthy_snapshot()))
                .unwrap();
        }
        let coverage = ledger.coverage().unwrap();
        assert_eq!(coverage.observation_count, 3);
        assert_eq!(coverage.earliest_retained, Some(at(7)));
        assert_eq!(coverage.latest_retained, Some(at(9)));
        let windowed = ledger.observations_in_window(at(0), at(9), 100).unwrap();
        assert_eq!(
            windowed.len(),
            3,
            "only what retention actually kept is ever returned"
        );
    }

    // N. store unavailable / invalid query returns typed error, no panic.
    #[test]
    fn opening_at_an_unwritable_path_fails_closed_not_panics() {
        let result = Ledger::open(
            std::path::Path::new("Z:\\definitely-not-a-real-drive\\ledger.sqlite3"),
            RetentionPolicy::default(),
        );
        assert_eq!(result.err(), Some(LedgerError::StorageUnavailable));
    }

    #[test]
    fn an_invalid_window_is_a_typed_error_not_a_panic() {
        let ledger = ledger();
        let result = ledger.observations_in_window(at(100), at(0), 10);
        assert_eq!(result.err(), Some(LedgerError::InvalidRecord));
    }

    #[test]
    fn an_empty_observation_id_is_refused() {
        assert_eq!(
            ObservationId::new("").err(),
            Some(LedgerError::InvalidRecord)
        );
    }

    #[test]
    fn an_implausibly_long_observation_id_is_refused() {
        let oversized = "x".repeat(MAX_OBSERVATION_ID_LEN + 1);
        assert_eq!(
            ObservationId::new(oversized).err(),
            Some(LedgerError::InvalidRecord)
        );
    }

    // Companion coverage for the eligibility-skipping query (§6): not
    // separately lettered in the directive's list, but directly implied by
    // "whether the last N eligible observations support a pattern".
    #[test]
    fn eligible_support_skips_indeterminate_observations_when_counting_toward_n() {
        let ledger = ledger();
        ledger
            .record(&observation("p1", at(1), repeated_timeout_snapshot()))
            .unwrap();
        ledger
            .record(&observation("p2", at(2), no_evidence_snapshot()))
            .unwrap();
        ledger
            .record(&observation("p3", at(3), repeated_timeout_snapshot()))
            .unwrap();
        ledger
            .record(&observation("p4", at(4), repeated_timeout_snapshot()))
            .unwrap();
        let result = ledger
            .eligible_support_over_last_n(DiagnosticPattern::RepeatedTimeout, 3, 100)
            .unwrap();
        assert_eq!(result.eligible_found, 3);
        assert!(result.all_supported);
        assert_eq!(result.indeterminate_skipped, 1);
    }

    #[test]
    fn eligible_support_is_false_when_not_enough_eligible_evidence_exists() {
        let ledger = ledger();
        ledger
            .record(&observation("q1", at(1), repeated_timeout_snapshot()))
            .unwrap();
        let result = ledger
            .eligible_support_over_last_n(DiagnosticPattern::RepeatedTimeout, 5, 100)
            .unwrap();
        assert_eq!(result.eligible_found, 1);
        assert!(
            !result.all_supported,
            "must never claim support when there is not enough eligible evidence to back it"
        );
    }

    #[test]
    fn eligible_support_is_false_on_a_genuine_counter_observation() {
        let ledger = ledger();
        ledger
            .record(&observation("r1", at(1), repeated_timeout_snapshot()))
            .unwrap();
        ledger
            .record(&observation("r2", at(2), healthy_snapshot()))
            .unwrap();
        ledger
            .record(&observation("r3", at(3), repeated_timeout_snapshot()))
            .unwrap();
        let result = ledger
            .eligible_support_over_last_n(DiagnosticPattern::RepeatedTimeout, 3, 100)
            .unwrap();
        assert_eq!(result.eligible_found, 3);
        assert!(!result.all_supported);
    }

    fn hypothesis(id: &str, generated_at: SystemTime) -> DiagnosticHypothesis {
        DiagnosticHypothesis {
            hypothesis_id: HypothesisId::new(id).unwrap(),
            subject: DiagnosticPattern::RepeatedTimeout,
            explanation: "A transport stall may explain the timeouts".into(),
            supporting_evidence: vec![ObservationId::new("support").unwrap()],
            contradicting_evidence: vec![ObservationId::new("counter").unwrap()],
            confidence: HypothesisConfidence::Low,
            uncertainty: "A cause has not been established".into(),
            evidence_sufficient: false,
            generated_at,
            generator: "deterministic-test".into(),
            status: HypothesisStatus::Proposed,
        }
    }

    #[test]
    fn hypothesis_round_trips_as_a_proposal_with_explicit_uncertainty() {
        let ledger = ledger();
        let original = hypothesis("h1", at(100));
        assert_eq!(
            ledger.record_hypothesis(&original),
            Ok(RecordOutcome::Recorded)
        );
        assert_eq!(
            ledger.get_hypothesis(&original.hypothesis_id),
            Ok(Some(original))
        );
    }

    #[test]
    fn duplicate_hypothesis_id_keeps_the_first_record() {
        let ledger = ledger();
        let original = hypothesis("h1", at(100));
        let mut retry = original.clone();
        retry.explanation = "different retry output".into();
        assert_eq!(
            ledger.record_hypothesis(&original),
            Ok(RecordOutcome::Recorded)
        );
        assert_eq!(
            ledger.record_hypothesis(&retry),
            Ok(RecordOutcome::AlreadyExists)
        );
        assert_eq!(
            ledger.get_hypothesis(&original.hypothesis_id),
            Ok(Some(original))
        );
    }

    #[test]
    fn asserted_assessments_and_unvalidated_references_are_stored_verbatim() {
        let ledger = ledger();
        let mut submitted = hypothesis("unverified", at(100));
        submitted.confidence = HypothesisConfidence::High;
        submitted.evidence_sufficient = true;
        // Neither referenced observation has been inserted. The ledger is
        // persistence only: these remain caller assertions and references.
        assert_eq!(ledger.coverage().unwrap().observation_count, 0);
        assert_eq!(
            ledger.record_hypothesis(&submitted),
            Ok(RecordOutcome::Recorded)
        );
        assert_eq!(
            ledger.get_hypothesis(&submitted.hypothesis_id),
            Ok(Some(submitted))
        );
        assert_eq!(ledger.coverage().unwrap().observation_count, 0);
    }

    #[test]
    fn hypothesis_survives_reopen_and_legacy_observation_schema_is_additive() {
        let path = std::env::temp_dir().join(format!(
            "maia-hypothesis-ledger-{}-{}.sqlite3",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        {
            let legacy = Connection::open(&path).unwrap();
            legacy
                .execute_batch(
                    "CREATE TABLE schema_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO schema_meta(key, value) VALUES ('version', '1');
                 CREATE TABLE observations (
                     id INTEGER PRIMARY KEY AUTOINCREMENT,
                     observation_id TEXT NOT NULL UNIQUE,
                     observed_at_ms INTEGER NOT NULL,
                     payload_json TEXT NOT NULL
                 );",
                )
                .unwrap();
        }
        let submitted = hypothesis("persisted", at(100));
        {
            let ledger = Ledger::open(&path, RetentionPolicy::default()).unwrap();
            assert_eq!(
                ledger.record_hypothesis(&submitted),
                Ok(RecordOutcome::Recorded)
            );
        }
        let reopened = Ledger::open(&path, RetentionPolicy::default()).unwrap();
        assert_eq!(
            reopened.get_hypothesis(&submitted.hypothesis_id),
            Ok(Some(submitted))
        );
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn inconsistent_hypothesis_row_fails_closed() {
        let ledger = ledger();
        ledger
            .record_hypothesis(&hypothesis("original", at(100)))
            .unwrap();
        ledger
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE hypotheses SET hypothesis_id = 'altered' WHERE hypothesis_id = 'original'",
                [],
            )
            .unwrap();
        assert_eq!(
            ledger
                .get_hypothesis(&HypothesisId::new("altered").unwrap())
                .err(),
            Some(LedgerError::QueryFailed)
        );
    }

    #[test]
    fn pre_uncertainty_hypothesis_payload_remains_readable() {
        let original = hypothesis("older", at(100));
        let mut value = serde_json::to_value(&original).unwrap();
        value.as_object_mut().unwrap().remove("uncertainty");
        let legacy: DiagnosticHypothesis = serde_json::from_value(value).unwrap();
        assert!(legacy.uncertainty.is_empty());
        assert_eq!(legacy.hypothesis_id, original.hypothesis_id);
        assert_eq!(legacy.supporting_evidence, original.supporting_evidence);
    }

    #[test]
    fn hypothesis_queries_are_bounded_and_retention_is_separate() {
        let ledger = Ledger::open_in_memory(RetentionPolicy {
            max_rows: 3,
            max_age: Duration::from_secs(1_000),
        })
        .unwrap();
        ledger
            .record(&observation("observation", at(1), healthy_snapshot()))
            .unwrap();
        for i in 0..5 {
            ledger
                .record_hypothesis(&hypothesis(&format!("h{i}"), at(i + 1)))
                .unwrap();
        }
        assert_eq!(
            ledger
                .latest_hypotheses(2)
                .unwrap()
                .iter()
                .map(|h| h.hypothesis_id.as_str())
                .collect::<Vec<_>>(),
            vec!["h4", "h3"]
        );
        assert_eq!(
            ledger.hypotheses_in_window(at(2), at(4), 10).unwrap().len(),
            2
        );
        assert_eq!(ledger.latest_hypotheses(10).unwrap().len(), 3);
        assert!(
            ledger
                .get(&ObservationId::new("observation").unwrap())
                .unwrap()
                .is_some()
        );
        assert_eq!(
            ledger.hypotheses_in_window(at(5), at(5), 1).err(),
            Some(LedgerError::InvalidRecord)
        );
    }
}
