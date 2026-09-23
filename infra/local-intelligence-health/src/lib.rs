//! Local Intelligence Observability & Health History (M0.15.10).
//!
//! Turns the machine-readable signals the Local Intelligence Runtime
//! capability (`infra/local-model`, M0.15.9) already produces per call into
//! a bounded, persistent, queryable history. This crate is strictly
//! observational:
//!
//! - It has no dependency capable of starting, stopping, restarting,
//!   replacing or reconfiguring the local runtime (see
//!   `tests/capability_mesh.rs`).
//! - It never persists prompts, model responses, evidence, mail, document
//!   content or credentials — its recording API structurally cannot accept
//!   them, since no such parameter exists on any public function here.
//! - A failure to record never becomes a failure of the inference call it
//!   describes: every `record_*` method returns its own `Result`, which the
//!   caller is expected to discard (`let _ = ...`) after already having
//!   committed to the real outcome. This crate never panics on a storage
//!   failure and never blocks an inference result on one.
//!
//! # Why a new, separate store rather than reusing `infra/sqlite`
//!
//! `infra/sqlite`'s `SqliteStore` is a workspace-scoped, hash-chained,
//! single-writer-locked authoritative audit ledger
//! (`core/store::audit_event`) built for material, governance-grade actions
//! — mail sends, approvals, persisted briefing results. Reusing its
//! `try_lock_exclusive`-based authority lock for frequent, best-effort
//! operational telemetry (a status probe every few seconds) would (a) abuse
//! a governance mechanism for non-authoritative sampling data, and (b) risk
//! reintroducing the exact concurrency hazard already documented as an
//! unconfirmed-but-real risk in
//! `docs/development/LOCAL_INTELLIGENCE_CAPABILITY_RECORD.md`'s "Run 1
//! qualification failure" section, where a fresh `SqliteStore` opened per
//! call could race another thread for the same exclusive lock. This crate
//! therefore owns one small, explicitly-versioned SQLite database of its
//! own, with its own file, no shared lock, and no workspace/authority
//! semantics — genuinely necessary, per the migration directive, and kept
//! small on purpose.
#![forbid(unsafe_code)]

use maia_briefing::LocalProviderFailure;
use maia_local_model::{LocalIntelligenceFailure, LocalIntelligenceStatus};
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Explicit, versioned schema. A future shape change bumps this and adds an
/// explicit migration step; there is no implicit/inferred migration. A store
/// opened against a file recorded under a different version fails closed
/// (`ObserverError::SchemaVersionMismatch`) rather than silently operating
/// against a schema it does not actually match.
const SCHEMA_VERSION: i64 = 1;

/// Defense against a malformed or hostile record: a model identity string is
/// never expected to be long (see `infra/local-model`'s own model strings),
/// so an implausibly long one is refused rather than stored.
const MAX_MODEL_LEN: usize = 256;

/// A single bounded scan limit used by queries that walk from most-recent
/// backward until a stopping condition, so no query can ever become an
/// unbounded table scan regardless of how much history exists.
const MAX_BOUNDED_SCAN: i64 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObserverError {
    /// The store could not be opened, or a write/transaction failed at the
    /// storage layer (disk full, file locked by something else, etc.).
    StorageUnavailable,
    /// An existing on-disk file's recorded schema version does not match
    /// this crate's `SCHEMA_VERSION`.
    SchemaVersionMismatch,
    /// A record was refused before being written (e.g. an empty or
    /// implausibly long model identity).
    InvalidRecord,
    /// A read query failed at the storage layer.
    QueryFailed,
}

/// Which capability-level call produced an inference outcome. A closed,
/// small set — not an open string — so a query result is always one of a
/// known set of call shapes, matching the capability's own public methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationClass {
    Complete,
    CompleteDetailed,
    Consult,
}
impl OperationClass {
    fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::CompleteDetailed => "complete_detailed",
            Self::Consult => "consult",
        }
    }
}

/// Bounds on how much history this store will ever hold. Enforced after
/// every write, not by a separate background job: retention is a property
/// of every insert, not something that can lapse if a scheduler misses a
/// run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// Hard cap on rows kept per table. The newest rows (by recorded time,
    /// row id as tiebreaker) are kept.
    pub max_rows_per_table: usize,
    /// Rows older than this, relative to the time of the enforcing write,
    /// are deleted even if under the row cap.
    pub max_age: Duration,
}
impl Default for RetentionPolicy {
    /// CPU-first / office-PC-first default: generous enough for weeks of
    /// bounded-interval polling and per-call recording, small enough that
    /// this store never becomes the largest resource consumer of the
    /// capability it observes — see `retention_keeps_row_count_bounded`,
    /// which measures the actual on-disk size at this cap.
    fn default() -> Self {
        Self {
            max_rows_per_table: 10_000,
            max_age: Duration::from_secs(30 * 24 * 60 * 60),
        }
    }
}

fn millis_since_epoch(at: SystemTime) -> i64 {
    at.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
fn duration_from_millis(ms: i64) -> Duration {
    Duration::from_millis(ms.max(0) as u64)
}
fn time_from_millis(ms: i64) -> SystemTime {
    UNIX_EPOCH + duration_from_millis(ms)
}

fn validate_model(model: &str) -> Result<(), ObserverError> {
    if model.is_empty() || model.len() > MAX_MODEL_LEN {
        Err(ObserverError::InvalidRecord)
    } else {
        Ok(())
    }
}

/// Maps the rich, capability-owned taxonomy to a stable, explicit storage
/// label. Exhaustive on purpose (no `_` arm): a variant added to
/// `LocalIntelligenceFailure` in the future is a compile error here until
/// this mapping says what its stored label is, the same discipline
/// `infra/local-model::to_provider_failure` already uses.
fn rich_failure_label(failure: LocalIntelligenceFailure) -> &'static str {
    match failure {
        LocalIntelligenceFailure::RuntimeUnavailable => "runtime_unavailable",
        LocalIntelligenceFailure::Timeout => "timeout",
        LocalIntelligenceFailure::WriteFailure => "write_failure",
        LocalIntelligenceFailure::ResponseTooLarge => "response_too_large",
        LocalIntelligenceFailure::ModelRejected => "model_rejected",
        LocalIntelligenceFailure::MalformedResponse => "malformed_response",
        LocalIntelligenceFailure::Other => "other",
    }
}

/// Maps Briefing's own, coarser, already-mapped-down failure shape to a
/// storage label that is prefixed and disjoint from `rich_failure_label`'s
/// output on purpose: a query result must never let the two taxonomies'
/// labels collide and look like the same fidelity of information. A record
/// stored under one of these labels means "this call site only had
/// Briefing's `LocalProviderFailure` available, not the richer
/// `LocalIntelligenceFailure`" — see the module docs on `consult()` /
/// `consult_and_validate` for why. Exhaustive on purpose, same discipline as
/// `rich_failure_label`.
fn briefing_failure_label(failure: LocalProviderFailure) -> &'static str {
    match failure {
        LocalProviderFailure::Unavailable => "briefing_unavailable",
        LocalProviderFailure::ModelUnavailable => "briefing_model_unavailable",
        LocalProviderFailure::Timeout => "briefing_timeout",
        LocalProviderFailure::MalformedResponse => "briefing_malformed_response",
        LocalProviderFailure::ProviderError => "briefing_provider_error",
    }
}

/// A recorded status probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusSampleRecord {
    pub recorded_at: SystemTime,
    pub model: String,
    pub available: bool,
    pub probe_duration: Option<Duration>,
}

/// A recorded inference outcome (success or failure), whichever taxonomy
/// produced it. `outcome` is `"success"` or one of `rich_failure_label`'s /
/// `briefing_failure_label`'s stable strings — never free text, never a
/// prompt or response fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceOutcomeRecord {
    pub recorded_at: SystemTime,
    pub model: String,
    pub operation: OperationClass,
    pub outcome: String,
    pub duration: Option<Duration>,
}

/// How much history is actually retained right now, independent of any
/// requested query window. A diagnostic consumer needs this to tell "the
/// last 24 hours are healthy" apart from "only 14 hours of history exist at
/// all, and all of it looks healthy" — retention (`RetentionPolicy`) bounds
/// how much survives, and at a short polling interval that bound can be
/// reached well under the nominal `max_age`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryCoverage {
    /// (earliest, latest) `recorded_at` across every retained status
    /// sample, or `None` if the table is empty.
    pub status_samples_span: Option<(SystemTime, SystemTime)>,
    /// (earliest, latest) `recorded_at` across every retained inference
    /// outcome, or `None` if the table is empty.
    pub inference_outcomes_span: Option<(SystemTime, SystemTime)>,
    pub status_sample_count: u64,
    pub inference_outcome_count: u64,
}

/// A small, dedicated, append-oriented SQLite store for Local Intelligence
/// operational health history. See the module docs for why this is a
/// separate store from `infra/sqlite`'s authoritative workspace ledger.
///
/// One `HealthStore` is meant to be constructed once per process and shared
/// (e.g. behind an `Arc`) across every thread that records into it — a
/// single `Connection` guarded by a `Mutex` serializes writes safely and
/// cheaply, without the OS-level exclusive-file-lock arbitration
/// `infra/sqlite` needs for cross-process authority. Opening a fresh
/// `HealthStore` per call would reintroduce the same anti-pattern flagged
/// for `SqliteStore` in the capability record's Run 1 postmortem; callers
/// should not do that.
pub struct HealthStore {
    conn: Mutex<Connection>,
    retention: RetentionPolicy,
}

impl HealthStore {
    /// Opens (creating if necessary) a health store at `path`. Creates the
    /// parent directory if it does not exist; a failure to do so is not
    /// itself fatal here (the subsequent `Connection::open` will fail with
    /// its own clear `StorageUnavailable` if the path is truly unusable).
    pub fn open(path: &Path, retention: RetentionPolicy) -> Result<Self, ObserverError> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path).map_err(|_| ObserverError::StorageUnavailable)?;
        Self::from_connection(conn, retention)
    }

    /// An in-memory store: identical schema and behavior, no file I/O.
    /// Intended for tests and for any future call site that wants scoped,
    /// disposable history without touching disk.
    pub fn open_in_memory(retention: RetentionPolicy) -> Result<Self, ObserverError> {
        let conn = Connection::open_in_memory().map_err(|_| ObserverError::StorageUnavailable)?;
        Self::from_connection(conn, retention)
    }

    fn from_connection(
        conn: Connection,
        retention: RetentionPolicy,
    ) -> Result<Self, ObserverError> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS schema_meta (
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS status_samples (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 recorded_at_ms INTEGER NOT NULL,
                 model TEXT NOT NULL,
                 available INTEGER NOT NULL,
                 probe_duration_ms INTEGER
             );
             CREATE INDEX IF NOT EXISTS idx_status_samples_time
                 ON status_samples(recorded_at_ms);
             CREATE TABLE IF NOT EXISTS inference_outcomes (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 recorded_at_ms INTEGER NOT NULL,
                 model TEXT NOT NULL,
                 operation TEXT NOT NULL,
                 outcome TEXT NOT NULL,
                 duration_ms INTEGER
             );
             CREATE INDEX IF NOT EXISTS idx_inference_outcomes_time
                 ON inference_outcomes(recorded_at_ms);",
        )
        .map_err(|_| ObserverError::StorageUnavailable)?;
        conn.execute(
            "INSERT OR IGNORE INTO schema_meta(key, value) VALUES ('version', ?1)",
            params![SCHEMA_VERSION.to_string()],
        )
        .map_err(|_| ObserverError::StorageUnavailable)?;
        let stored: String = conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = 'version'",
                [],
                |row| row.get(0),
            )
            .map_err(|_| ObserverError::StorageUnavailable)?;
        if stored != SCHEMA_VERSION.to_string() {
            return Err(ObserverError::SchemaVersionMismatch);
        }
        Ok(Self {
            conn: Mutex::new(conn),
            retention,
        })
    }

    /// Records one bounded status probe (`LoopbackLocalProvider::status()`'s
    /// result). `probe_duration` is the caller-measured wall time of the
    /// probe call itself, if cheaply measured; `recorded_at` is caller-
    /// supplied (not `SystemTime::now()` internally) so tests can use
    /// deterministic fake time instead of real sleeps.
    pub fn record_status_sample(
        &self,
        status: &LocalIntelligenceStatus,
        probe_duration: Option<Duration>,
        recorded_at: SystemTime,
    ) -> Result<(), ObserverError> {
        validate_model(&status.model)?;
        let conn = self
            .conn
            .lock()
            .map_err(|_| ObserverError::StorageUnavailable)?;
        conn.execute(
            "INSERT INTO status_samples
                 (recorded_at_ms, model, available, probe_duration_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                millis_since_epoch(recorded_at),
                status.model,
                status.available as i64,
                probe_duration.map(|d| d.as_millis() as i64),
            ],
        )
        .map_err(|_| ObserverError::StorageUnavailable)?;
        enforce_retention(&conn, "status_samples", self.retention, recorded_at)
    }

    /// Records a successful inference call. `model`/`operation` identify
    /// which capability call produced it; `duration` is the caller-measured
    /// call time (e.g. `CompletionOutcome.duration`).
    pub fn record_inference_success(
        &self,
        model: &str,
        operation: OperationClass,
        duration: Duration,
        recorded_at: SystemTime,
    ) -> Result<(), ObserverError> {
        self.insert_outcome(model, operation, "success", Some(duration), recorded_at)
    }

    /// Records a failed inference call using the rich, capability-owned
    /// taxonomy — the full-fidelity path, available to any caller that used
    /// `complete_detailed()` (or a future rich consult-equivalent).
    pub fn record_inference_failure(
        &self,
        model: &str,
        operation: OperationClass,
        failure: LocalIntelligenceFailure,
        duration: Option<Duration>,
        recorded_at: SystemTime,
    ) -> Result<(), ObserverError> {
        self.insert_outcome(
            model,
            operation,
            rich_failure_label(failure),
            duration,
            recorded_at,
        )
    }

    /// Records a failed inference call using Briefing's coarser,
    /// already-mapped-down failure shape — the path available to any
    /// caller going through `consult_and_validate`/`LocalModelProvider::consult`,
    /// which have already discarded the rich taxonomy by the time a caller
    /// can observe the failure (see the module docs and
    /// `docs/development/LOCAL_INTELLIGENCE_CAPABILITY_RECORD.md`). Stored
    /// under a label visibly distinct from the rich taxonomy's own labels
    /// (`briefing_*` prefix), never silently merged with it.
    pub fn record_inference_failure_from_briefing(
        &self,
        model: &str,
        operation: OperationClass,
        failure: LocalProviderFailure,
        duration: Option<Duration>,
        recorded_at: SystemTime,
    ) -> Result<(), ObserverError> {
        self.insert_outcome(
            model,
            operation,
            briefing_failure_label(failure),
            duration,
            recorded_at,
        )
    }

    fn insert_outcome(
        &self,
        model: &str,
        operation: OperationClass,
        outcome: &str,
        duration: Option<Duration>,
        recorded_at: SystemTime,
    ) -> Result<(), ObserverError> {
        validate_model(model)?;
        let conn = self
            .conn
            .lock()
            .map_err(|_| ObserverError::StorageUnavailable)?;
        conn.execute(
            "INSERT INTO inference_outcomes
                 (recorded_at_ms, model, operation, outcome, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                millis_since_epoch(recorded_at),
                model,
                operation.as_str(),
                outcome,
                duration.map(|d| d.as_millis() as i64),
            ],
        )
        .map_err(|_| ObserverError::StorageUnavailable)?;
        enforce_retention(&conn, "inference_outcomes", self.retention, recorded_at)
    }

    /// The most recently recorded status sample, if any.
    pub fn latest_status(&self) -> Result<Option<StatusSampleRecord>, ObserverError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| ObserverError::StorageUnavailable)?;
        conn.query_row(
            "SELECT recorded_at_ms, model, available, probe_duration_ms
             FROM status_samples
             ORDER BY recorded_at_ms DESC, id DESC
             LIMIT 1",
            [],
            |row| {
                let ms: i64 = row.get(0)?;
                let model: String = row.get(1)?;
                let available: i64 = row.get(2)?;
                let probe_ms: Option<i64> = row.get(3)?;
                Ok(StatusSampleRecord {
                    recorded_at: time_from_millis(ms),
                    model,
                    available: available != 0,
                    probe_duration: probe_ms.map(duration_from_millis),
                })
            },
        )
        .optional()
        .map_err(|_| ObserverError::QueryFailed)
    }

    /// Status samples recorded in `[since, until]`, most recent first,
    /// bounded to at most `limit` rows. Distinct from `latest_status()`
    /// (which returns only the single most recent sample): this is the
    /// availability *history* a self-diagnosis consumer needs to answer
    /// "how has availability looked over the last hour", not just "right
    /// now".
    pub fn status_samples_in_window(
        &self,
        since: SystemTime,
        until: SystemTime,
        limit: usize,
    ) -> Result<Vec<StatusSampleRecord>, ObserverError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| ObserverError::StorageUnavailable)?;
        let mut stmt = conn
            .prepare(
                "SELECT recorded_at_ms, model, available, probe_duration_ms
                 FROM status_samples
                 WHERE recorded_at_ms >= ?1 AND recorded_at_ms <= ?2
                 ORDER BY recorded_at_ms DESC, id DESC
                 LIMIT ?3",
            )
            .map_err(|_| ObserverError::QueryFailed)?;
        let rows = stmt
            .query_map(
                params![
                    millis_since_epoch(since),
                    millis_since_epoch(until),
                    limit as i64
                ],
                |row| {
                    let ms: i64 = row.get(0)?;
                    let model: String = row.get(1)?;
                    let available: i64 = row.get(2)?;
                    let probe_ms: Option<i64> = row.get(3)?;
                    Ok(StatusSampleRecord {
                        recorded_at: time_from_millis(ms),
                        model,
                        available: available != 0,
                        probe_duration: probe_ms.map(duration_from_millis),
                    })
                },
            )
            .map_err(|_| ObserverError::QueryFailed)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| ObserverError::QueryFailed)
    }

    /// How much history is actually retained right now, across both
    /// tables, independent of any query window. A single bounded
    /// `MIN`/`MAX`/`COUNT` aggregate per table — cheap regardless of row
    /// count, since both are indexed on `recorded_at_ms`. See
    /// `HistoryCoverage`'s docs for why a diagnostic consumer needs this.
    pub fn history_coverage(&self) -> Result<HistoryCoverage, ObserverError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| ObserverError::StorageUnavailable)?;
        let (status_span, status_count) = span_and_count(&conn, "status_samples")?;
        let (outcome_span, outcome_count) = span_and_count(&conn, "inference_outcomes")?;
        Ok(HistoryCoverage {
            status_samples_span: status_span,
            inference_outcomes_span: outcome_span,
            status_sample_count: status_count,
            inference_outcome_count: outcome_count,
        })
    }

    /// Failed inference outcomes in `[since, until]`, most recent first,
    /// bounded to at most `limit` rows — never an unbounded scan regardless
    /// of how much history exists in the window.
    pub fn failures_in_window(
        &self,
        since: SystemTime,
        until: SystemTime,
        limit: usize,
    ) -> Result<Vec<InferenceOutcomeRecord>, ObserverError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| ObserverError::StorageUnavailable)?;
        let mut stmt = conn
            .prepare(
                "SELECT recorded_at_ms, model, operation, outcome, duration_ms
                 FROM inference_outcomes
                 WHERE recorded_at_ms >= ?1 AND recorded_at_ms <= ?2 AND outcome != 'success'
                 ORDER BY recorded_at_ms DESC, id DESC
                 LIMIT ?3",
            )
            .map_err(|_| ObserverError::QueryFailed)?;
        let rows = stmt
            .query_map(
                params![
                    millis_since_epoch(since),
                    millis_since_epoch(until),
                    limit as i64
                ],
                row_to_outcome,
            )
            .map_err(|_| ObserverError::QueryFailed)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| ObserverError::QueryFailed)
    }

    /// Failure counts grouped by stored outcome label within `[since,
    /// until]`. Rich and Briefing-sourced labels are never merged (see
    /// `rich_failure_label`/`briefing_failure_label`).
    pub fn failure_counts_by_class(
        &self,
        since: SystemTime,
        until: SystemTime,
    ) -> Result<BTreeMap<String, u64>, ObserverError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| ObserverError::StorageUnavailable)?;
        let mut stmt = conn
            .prepare(
                "SELECT outcome, COUNT(*)
                 FROM inference_outcomes
                 WHERE recorded_at_ms >= ?1 AND recorded_at_ms <= ?2 AND outcome != 'success'
                 GROUP BY outcome",
            )
            .map_err(|_| ObserverError::QueryFailed)?;
        let rows = stmt
            .query_map(
                params![millis_since_epoch(since), millis_since_epoch(until)],
                |row| {
                    let outcome: String = row.get(0)?;
                    let count: i64 = row.get(1)?;
                    Ok((outcome, count.max(0) as u64))
                },
            )
            .map_err(|_| ObserverError::QueryFailed)?;
        let mut result = BTreeMap::new();
        for row in rows {
            let (outcome, count) = row.map_err(|_| ObserverError::QueryFailed)?;
            result.insert(outcome, count);
        }
        Ok(result)
    }

    /// Durations of recorded outcomes (success or failure, whichever
    /// carried a duration) in `[since, until]`, most recent first, bounded
    /// to `limit` rows.
    pub fn recent_latencies(
        &self,
        since: SystemTime,
        until: SystemTime,
        limit: usize,
    ) -> Result<Vec<Duration>, ObserverError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| ObserverError::StorageUnavailable)?;
        let mut stmt = conn
            .prepare(
                "SELECT duration_ms
                 FROM inference_outcomes
                 WHERE recorded_at_ms >= ?1 AND recorded_at_ms <= ?2 AND duration_ms IS NOT NULL
                 ORDER BY recorded_at_ms DESC, id DESC
                 LIMIT ?3",
            )
            .map_err(|_| ObserverError::QueryFailed)?;
        let rows = stmt
            .query_map(
                params![
                    millis_since_epoch(since),
                    millis_since_epoch(until),
                    limit as i64
                ],
                |row| {
                    let ms: i64 = row.get(0)?;
                    Ok(duration_from_millis(ms))
                },
            )
            .map_err(|_| ObserverError::QueryFailed)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| ObserverError::QueryFailed)
    }

    /// How many of the most-recent inference outcomes, walking backward from
    /// now, are consecutively `RuntimeUnavailable` or `Timeout` (rich
    /// taxonomy) or their Briefing-sourced equivalents
    /// (`briefing_unavailable`/`briefing_timeout`) — stopping at the first
    /// outcome that is neither, including any success. Answers "how long has
    /// the runtime been unreachable or slow, right now", not a windowed
    /// total. Bounded to at most `MAX_BOUNDED_SCAN` rows even if every one
    /// of them qualifies.
    pub fn consecutive_unavailable_or_timeout_count(&self) -> Result<u64, ObserverError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| ObserverError::StorageUnavailable)?;
        let mut stmt = conn
            .prepare(
                "SELECT outcome FROM inference_outcomes
                 ORDER BY recorded_at_ms DESC, id DESC
                 LIMIT ?1",
            )
            .map_err(|_| ObserverError::QueryFailed)?;
        let rows = stmt
            .query_map(params![MAX_BOUNDED_SCAN], |row| row.get::<_, String>(0))
            .map_err(|_| ObserverError::QueryFailed)?;
        let mut count = 0u64;
        for row in rows {
            let outcome = row.map_err(|_| ObserverError::QueryFailed)?;
            let qualifies = matches!(
                outcome.as_str(),
                "runtime_unavailable" | "timeout" | "briefing_unavailable" | "briefing_timeout"
            );
            if qualifies {
                count += 1;
            } else {
                break;
            }
        }
        Ok(count)
    }
}

fn row_to_outcome(row: &rusqlite::Row) -> rusqlite::Result<InferenceOutcomeRecord> {
    let ms: i64 = row.get(0)?;
    let model: String = row.get(1)?;
    let operation_str: String = row.get(2)?;
    let outcome: String = row.get(3)?;
    let duration_ms: Option<i64> = row.get(4)?;
    let operation = match operation_str.as_str() {
        "complete" => OperationClass::Complete,
        "complete_detailed" => OperationClass::CompleteDetailed,
        _ => OperationClass::Consult,
    };
    Ok(InferenceOutcomeRecord {
        recorded_at: time_from_millis(ms),
        model,
        operation,
        outcome,
        duration: duration_ms.map(duration_from_millis),
    })
}

/// The `(earliest, latest, count)` of `recorded_at_ms` in `table` (always
/// one of this crate's own two hardcoded table names, never external
/// input). `None` span when the table is empty; count is always `Some`
/// (zero when empty).
fn span_and_count(
    conn: &Connection,
    table: &'static str,
) -> Result<(Option<(SystemTime, SystemTime)>, u64), ObserverError> {
    let (min, max, count): (Option<i64>, Option<i64>, i64) = conn
        .query_row(
            &format!("SELECT MIN(recorded_at_ms), MAX(recorded_at_ms), COUNT(*) FROM {table}"),
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| ObserverError::QueryFailed)?;
    let span = match (min, max) {
        (Some(min), Some(max)) => Some((time_from_millis(min), time_from_millis(max))),
        _ => None,
    };
    Ok((span, count.max(0) as u64))
}

/// Deletes rows older than `retention.max_age` (relative to `now`), then
/// deletes any rows beyond `retention.max_rows_per_table`, keeping the
/// newest. Both bounds are enforced on every write, not by a separate
/// scheduled job, so retention cannot lapse if nothing ever polls for it.
/// `table` is always one of this crate's own two hardcoded table names,
/// never external input.
fn enforce_retention(
    conn: &Connection,
    table: &'static str,
    retention: RetentionPolicy,
    now: SystemTime,
) -> Result<(), ObserverError> {
    let cutoff = millis_since_epoch(now) - retention.max_age.as_millis() as i64;
    conn.execute(
        &format!("DELETE FROM {table} WHERE recorded_at_ms < ?1"),
        params![cutoff],
    )
    .map_err(|_| ObserverError::StorageUnavailable)?;
    conn.execute(
        &format!(
            "DELETE FROM {table} WHERE id NOT IN \
             (SELECT id FROM {table} ORDER BY recorded_at_ms DESC, id DESC LIMIT ?1)"
        ),
        params![retention.max_rows_per_table as i64],
    )
    .map_err(|_| ObserverError::StorageUnavailable)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> HealthStore {
        HealthStore::open_in_memory(RetentionPolicy::default()).expect("open in-memory store")
    }

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn a_runtime_available_sample_round_trips() {
        let store = store();
        let status = LocalIntelligenceStatus {
            available: true,
            model: "qwen3:4b".into(),
        };
        store
            .record_status_sample(&status, Some(Duration::from_millis(12)), at(1000))
            .expect("record");
        let latest = store.latest_status().expect("query").expect("some row");
        assert_eq!(latest.model, "qwen3:4b");
        assert!(latest.available);
        assert_eq!(latest.probe_duration, Some(Duration::from_millis(12)));
        assert_eq!(latest.recorded_at, at(1000));
    }

    #[test]
    fn a_runtime_unavailable_sample_round_trips() {
        let store = store();
        let status = LocalIntelligenceStatus {
            available: false,
            model: "qwen3:4b".into(),
        };
        store
            .record_status_sample(&status, None, at(2000))
            .expect("record");
        let latest = store.latest_status().expect("query").expect("some row");
        assert!(!latest.available);
        assert_eq!(latest.probe_duration, None);
    }

    #[test]
    fn latest_status_reflects_the_most_recent_sample_not_insertion_order() {
        let store = store();
        let status = |available| LocalIntelligenceStatus {
            available,
            model: "qwen3:4b".into(),
        };
        store
            .record_status_sample(&status(true), None, at(100))
            .unwrap();
        store
            .record_status_sample(&status(false), None, at(50))
            .unwrap();
        store
            .record_status_sample(&status(true), None, at(75))
            .unwrap();
        let latest = store.latest_status().unwrap().unwrap();
        assert_eq!(latest.recorded_at, at(100));
        assert!(latest.available);
    }

    #[test]
    fn a_success_outcome_persists_with_its_duration() {
        let store = store();
        store
            .record_inference_success(
                "qwen3:4b",
                OperationClass::CompleteDetailed,
                Duration::from_millis(4200),
                at(10),
            )
            .expect("record");
        let latencies = store.recent_latencies(at(0), at(20), 10).unwrap();
        assert_eq!(latencies, vec![Duration::from_millis(4200)]);
        let failures = store.failures_in_window(at(0), at(20), 10).unwrap();
        assert!(
            failures.is_empty(),
            "a success must never appear in failures_in_window"
        );
    }

    #[test]
    fn every_rich_failure_class_is_recorded_under_its_own_distinct_label() {
        let store = store();
        let all = [
            LocalIntelligenceFailure::RuntimeUnavailable,
            LocalIntelligenceFailure::Timeout,
            LocalIntelligenceFailure::WriteFailure,
            LocalIntelligenceFailure::ResponseTooLarge,
            LocalIntelligenceFailure::ModelRejected,
            LocalIntelligenceFailure::MalformedResponse,
            LocalIntelligenceFailure::Other,
        ];
        for (index, failure) in all.iter().enumerate() {
            store
                .record_inference_failure(
                    "qwen3:4b",
                    OperationClass::CompleteDetailed,
                    *failure,
                    None,
                    at(1000 + index as u64),
                )
                .expect("record");
        }
        let counts = store.failure_counts_by_class(at(0), at(2000)).unwrap();
        assert_eq!(counts.len(), all.len(), "every class must be distinct");
        for expected in [
            "runtime_unavailable",
            "timeout",
            "write_failure",
            "response_too_large",
            "model_rejected",
            "malformed_response",
            "other",
        ] {
            assert_eq!(counts.get(expected), Some(&1), "missing label {expected}");
        }
    }

    #[test]
    fn every_briefing_failure_class_is_recorded_under_a_distinct_prefixed_label() {
        let store = store();
        let all = [
            LocalProviderFailure::Unavailable,
            LocalProviderFailure::ModelUnavailable,
            LocalProviderFailure::Timeout,
            LocalProviderFailure::MalformedResponse,
            LocalProviderFailure::ProviderError,
        ];
        for (index, failure) in all.into_iter().enumerate() {
            store
                .record_inference_failure_from_briefing(
                    "qwen3:4b",
                    OperationClass::Consult,
                    failure,
                    None,
                    at(3000 + index as u64),
                )
                .expect("record");
        }
        let counts = store.failure_counts_by_class(at(0), at(4000)).unwrap();
        for expected in [
            "briefing_unavailable",
            "briefing_model_unavailable",
            "briefing_timeout",
            "briefing_malformed_response",
            "briefing_provider_error",
        ] {
            assert_eq!(counts.get(expected), Some(&1), "missing label {expected}");
            assert!(
                !counts.contains_key(expected.trim_start_matches("briefing_")),
                "briefing-sourced label must never collide with a rich-taxonomy label"
            );
        }
    }

    #[test]
    fn status_samples_in_window_returns_availability_history_not_just_the_latest() {
        let store = store();
        let status = |available| LocalIntelligenceStatus {
            available,
            model: "qwen3:4b".into(),
        };
        store
            .record_status_sample(&status(true), None, at(10))
            .unwrap();
        store
            .record_status_sample(&status(false), None, at(20))
            .unwrap();
        store
            .record_status_sample(&status(true), None, at(30))
            .unwrap();
        let history = store.status_samples_in_window(at(0), at(100), 10).unwrap();
        assert_eq!(
            history.len(),
            3,
            "must return every sample in window, not just the latest"
        );
        assert_eq!(
            history.iter().map(|s| s.recorded_at).collect::<Vec<_>>(),
            vec![at(30), at(20), at(10)],
            "must be ordered most-recent first"
        );
        assert_eq!(
            history.iter().map(|s| s.available).collect::<Vec<_>>(),
            vec![true, false, true]
        );
    }

    #[test]
    fn status_samples_in_window_respects_the_time_bound_and_limit() {
        let store = store();
        let status = LocalIntelligenceStatus {
            available: true,
            model: "qwen3:4b".into(),
        };
        for index in 0..10u64 {
            store
                .record_status_sample(&status, None, at(1000 + index))
                .unwrap();
        }
        let in_window = store.status_samples_in_window(at(0), at(1005), 3).unwrap();
        assert_eq!(in_window.len(), 3, "must respect the requested limit");
        for sample in &in_window {
            assert!(sample.recorded_at <= at(1005));
        }
    }

    #[test]
    fn failures_in_window_respects_the_time_bound() {
        let store = store();
        store
            .record_inference_failure(
                "qwen3:4b",
                OperationClass::Complete,
                LocalIntelligenceFailure::Timeout,
                None,
                at(100),
            )
            .unwrap();
        store
            .record_inference_failure(
                "qwen3:4b",
                OperationClass::Complete,
                LocalIntelligenceFailure::Timeout,
                None,
                at(9_999),
            )
            .unwrap();
        let in_window = store.failures_in_window(at(0), at(500), 10).unwrap();
        assert_eq!(in_window.len(), 1);
        assert_eq!(in_window[0].recorded_at, at(100));
    }

    #[test]
    fn failures_in_window_is_bounded_by_the_requested_limit() {
        let store = store();
        for index in 0..50 {
            store
                .record_inference_failure(
                    "qwen3:4b",
                    OperationClass::Complete,
                    LocalIntelligenceFailure::Timeout,
                    None,
                    at(1000 + index),
                )
                .unwrap();
        }
        let limited = store.failures_in_window(at(0), at(5000), 5).unwrap();
        assert_eq!(
            limited.len(),
            5,
            "must never return more than the requested limit"
        );
    }

    #[test]
    fn consecutive_unavailable_or_timeout_count_stops_at_the_first_non_qualifying_outcome() {
        let store = store();
        store
            .record_inference_success(
                "qwen3:4b",
                OperationClass::Complete,
                Duration::from_millis(1),
                at(1),
            )
            .unwrap();
        store
            .record_inference_failure(
                "qwen3:4b",
                OperationClass::Complete,
                LocalIntelligenceFailure::Timeout,
                None,
                at(2),
            )
            .unwrap();
        store
            .record_inference_failure(
                "qwen3:4b",
                OperationClass::Complete,
                LocalIntelligenceFailure::RuntimeUnavailable,
                None,
                at(3),
            )
            .unwrap();
        store
            .record_inference_failure(
                "qwen3:4b",
                OperationClass::Complete,
                LocalIntelligenceFailure::Timeout,
                None,
                at(4),
            )
            .unwrap();
        assert_eq!(store.consecutive_unavailable_or_timeout_count().unwrap(), 3);
    }

    #[test]
    fn consecutive_count_treats_a_success_as_resetting_to_zero() {
        let store = store();
        store
            .record_inference_failure(
                "qwen3:4b",
                OperationClass::Complete,
                LocalIntelligenceFailure::Timeout,
                None,
                at(1),
            )
            .unwrap();
        store
            .record_inference_success(
                "qwen3:4b",
                OperationClass::Complete,
                Duration::from_millis(1),
                at(2),
            )
            .unwrap();
        assert_eq!(store.consecutive_unavailable_or_timeout_count().unwrap(), 0);
    }

    #[test]
    fn consecutive_count_treats_a_non_qualifying_failure_as_resetting_to_zero() {
        let store = store();
        store
            .record_inference_failure(
                "qwen3:4b",
                OperationClass::Complete,
                LocalIntelligenceFailure::Timeout,
                None,
                at(1),
            )
            .unwrap();
        store
            .record_inference_failure(
                "qwen3:4b",
                OperationClass::Complete,
                LocalIntelligenceFailure::MalformedResponse,
                None,
                at(2),
            )
            .unwrap();
        assert_eq!(store.consecutive_unavailable_or_timeout_count().unwrap(), 0);
    }

    #[test]
    fn retention_keeps_row_count_bounded_by_max_rows() {
        let retention = RetentionPolicy {
            max_rows_per_table: 5,
            max_age: Duration::from_secs(1_000_000),
        };
        let store = HealthStore::open_in_memory(retention).expect("open");
        for index in 0..20u64 {
            store
                .record_inference_success(
                    "qwen3:4b",
                    OperationClass::Complete,
                    Duration::from_millis(1),
                    at(index),
                )
                .unwrap();
        }
        let all = store.failures_in_window(at(0), at(1000), 1000).unwrap();
        assert!(all.is_empty(), "all 20 records were successes");
        // Successes do not show up in failures_in_window; assert bounded row
        // count through recent_latencies instead, which returns every
        // recorded duration regardless of outcome.
        let latencies = store.recent_latencies(at(0), at(1000), 1000).unwrap();
        assert_eq!(
            latencies.len(),
            5,
            "retention must cap stored rows at max_rows_per_table"
        );
    }

    #[test]
    fn retention_keeps_the_newest_rows_not_the_oldest() {
        let retention = RetentionPolicy {
            max_rows_per_table: 3,
            max_age: Duration::from_secs(1_000_000),
        };
        let store = HealthStore::open_in_memory(retention).expect("open");
        for index in 0..10u64 {
            store
                .record_inference_failure(
                    "qwen3:4b",
                    OperationClass::Complete,
                    LocalIntelligenceFailure::Timeout,
                    None,
                    at(index),
                )
                .unwrap();
        }
        let remaining = store.failures_in_window(at(0), at(1000), 1000).unwrap();
        assert_eq!(remaining.len(), 3);
        let newest_times: Vec<SystemTime> = remaining.iter().map(|r| r.recorded_at).collect();
        assert_eq!(newest_times, vec![at(9), at(8), at(7)]);
    }

    #[test]
    fn retention_removes_rows_older_than_max_age_even_under_the_row_cap() {
        let retention = RetentionPolicy {
            max_rows_per_table: 1000,
            max_age: Duration::from_secs(100),
        };
        let store = HealthStore::open_in_memory(retention).expect("open");
        store
            .record_inference_failure(
                "qwen3:4b",
                OperationClass::Complete,
                LocalIntelligenceFailure::Timeout,
                None,
                at(0),
            )
            .unwrap();
        // A write far enough in the future that, relative to IT, the first
        // row is now older than max_age — retention is enforced relative to
        // the time of the enforcing write, not wall-clock "now".
        store
            .record_inference_failure(
                "qwen3:4b",
                OperationClass::Complete,
                LocalIntelligenceFailure::Timeout,
                None,
                at(1000),
            )
            .unwrap();
        let remaining = store.failures_in_window(at(0), at(2000), 1000).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].recorded_at, at(1000));
    }

    #[test]
    fn an_empty_model_identity_is_refused_as_invalid() {
        let store = store();
        let status = LocalIntelligenceStatus {
            available: true,
            model: String::new(),
        };
        let result = store.record_status_sample(&status, None, at(1));
        assert_eq!(result, Err(ObserverError::InvalidRecord));
    }

    #[test]
    fn an_implausibly_long_model_identity_is_refused_as_invalid() {
        let store = store();
        let oversized = "x".repeat(MAX_MODEL_LEN + 1);
        let result = store.record_inference_success(
            &oversized,
            OperationClass::Complete,
            Duration::from_millis(1),
            at(1),
        );
        assert_eq!(result, Err(ObserverError::InvalidRecord));
    }

    #[test]
    fn opening_a_store_at_an_unwritable_path_fails_closed_not_panics() {
        // A path whose parent cannot possibly be created (a bare drive root
        // under a name that does not exist) forces Connection::open to fail;
        // this must surface as a typed error, never a panic.
        let result = HealthStore::open(
            Path::new("Z:\\definitely-not-a-real-drive\\health.sqlite3"),
            RetentionPolicy::default(),
        );
        assert_eq!(result.err(), Some(ObserverError::StorageUnavailable));
    }

    #[test]
    fn no_public_recording_method_accepts_a_prompt_or_response_parameter() {
        // Structural, not behavioral: every recording method's signature is
        // exactly (model/status, operation-or-none, outcome-or-duration,
        // duration, timestamp) — there is no parameter anywhere that a
        // prompt or model response string could be passed through. This
        // test exists so a future change that widens a signature to accept
        // free text is forced to also change this comment and think about
        // it, not slip through as a plausible-looking new parameter.
        let store = store();
        let status = LocalIntelligenceStatus {
            available: true,
            model: "qwen3:4b".into(),
        };
        store.record_status_sample(&status, None, at(1)).unwrap();
        store
            .record_inference_success(
                "qwen3:4b",
                OperationClass::Complete,
                Duration::from_millis(1),
                at(2),
            )
            .unwrap();
        store
            .record_inference_failure(
                "qwen3:4b",
                OperationClass::Complete,
                LocalIntelligenceFailure::Timeout,
                None,
                at(3),
            )
            .unwrap();
        store
            .record_inference_failure_from_briefing(
                "qwen3:4b",
                OperationClass::Consult,
                LocalProviderFailure::Timeout,
                None,
                at(4),
            )
            .unwrap();
    }

    #[test]
    fn a_second_open_of_the_same_file_reuses_the_existing_schema() {
        let dir =
            std::env::temp_dir().join(format!("maia-health-store-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("health.sqlite3");
        {
            let store = HealthStore::open(&path, RetentionPolicy::default()).expect("open 1");
            store
                .record_inference_success(
                    "qwen3:4b",
                    OperationClass::Complete,
                    Duration::from_millis(5),
                    at(1),
                )
                .unwrap();
        }
        {
            let store = HealthStore::open(&path, RetentionPolicy::default()).expect("open 2");
            let latencies = store.recent_latencies(at(0), at(10), 10).unwrap();
            assert_eq!(latencies, vec![Duration::from_millis(5)]);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn history_coverage_is_empty_for_a_fresh_store() {
        let store = store();
        let coverage = store.history_coverage().unwrap();
        assert_eq!(coverage.status_samples_span, None);
        assert_eq!(coverage.inference_outcomes_span, None);
        assert_eq!(coverage.status_sample_count, 0);
        assert_eq!(coverage.inference_outcome_count, 0);
    }

    #[test]
    fn history_coverage_reports_the_true_earliest_and_latest_regardless_of_insertion_order() {
        let store = store();
        let status = LocalIntelligenceStatus {
            available: true,
            model: "qwen3:4b".into(),
        };
        store.record_status_sample(&status, None, at(100)).unwrap();
        store.record_status_sample(&status, None, at(10)).unwrap();
        store.record_status_sample(&status, None, at(50)).unwrap();
        store
            .record_inference_success(
                "qwen3:4b",
                OperationClass::Complete,
                Duration::from_millis(1),
                at(200),
            )
            .unwrap();
        let coverage = store.history_coverage().unwrap();
        assert_eq!(coverage.status_samples_span, Some((at(10), at(100))));
        assert_eq!(coverage.status_sample_count, 3);
        assert_eq!(coverage.inference_outcomes_span, Some((at(200), at(200))));
        assert_eq!(coverage.inference_outcome_count, 1);
    }

    #[test]
    fn history_coverage_reflects_retention_eviction_not_the_original_earliest_write() {
        let retention = RetentionPolicy {
            max_rows_per_table: 3,
            max_age: Duration::from_secs(1_000_000),
        };
        let store = HealthStore::open_in_memory(retention).expect("open");
        let status = LocalIntelligenceStatus {
            available: true,
            model: "qwen3:4b".into(),
        };
        for index in 0..10u64 {
            store
                .record_status_sample(&status, None, at(index))
                .unwrap();
        }
        let coverage = store.history_coverage().unwrap();
        // Only the newest 3 (indices 7, 8, 9) survive retention -- coverage
        // must report THAT as the earliest, not the original at(0), so a
        // diagnostic consumer never believes more history exists than
        // retention actually kept.
        assert_eq!(coverage.status_samples_span, Some((at(7), at(9))));
        assert_eq!(coverage.status_sample_count, 3);
    }
}
