//! Local Intelligence Diagnostic Evidence & Snapshot (M0.15.11).
//!
//! Turns M0.15.10's raw health history into a typed, bounded, deterministic
//! diagnostic snapshot a future consumer (a Hypothesis Ledger, a human
//! operator, a CLI) can read without knowing SQLite schema or
//! `infra/local-model` implementation details. This crate is the bridge:
//!
//!   raw health history -> bounded diagnostic evidence -> future Hypothesis Ledger
//!
//! It is **not** the Hypothesis Ledger itself, **not** a Lifecycle
//! Supervisor, and has **no action authority whatsoever** — it never
//! starts, stops, restarts or reconfigures anything, and never mutates the
//! health history it reads (see `tests/capability_mesh.rs`).
//!
//! # Discovery (before implementation)
//!
//! No existing reusable representation for diagnostic evidence, health
//! snapshots, hypothesis inputs, time-window statistics or
//! self-development evidence was found anywhere in `core/`, `infra/` or
//! `apps/` (a repository-wide search for `Snapshot`/`Diagnostic`/
//! `Hypothesis` found only UI-label strings and unrelated comments — see
//! the M0.15.11 report). A new, small, typed contract was therefore
//! genuinely necessary, not a duplicate of something that already exists.
//!
//! # Evidence-gated, not predictive
//!
//! Every classification this crate produces is a direct, deterministic
//! function of already-recorded counts and timestamps — no anomaly
//! detection, no trend extrapolation beyond a simple two-half-window mean
//! comparison, and no assertion of root cause. A "pattern" here means "this
//! many qualifying records exist in this window," never "this is why."
#![forbid(unsafe_code)]

use maia_local_intelligence_health::{HealthStore, ObserverError};
use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime},
};

/// Status samples below this count in the requested window are not enough
/// to compute a trustworthy availability ratio or rule out a degradation
/// pattern — deliberately small and arbitrary, chosen only so that a
/// single probe (or two) can never produce a confident "healthy" or
/// "degrading" read. See `EvidenceLevel::InsufficientEvidence`.
const MIN_SAMPLES_FOR_CONFIDENT_READ: usize = 5;

/// A failure count at or above this, within one class, in the requested
/// window, is reported as a "repeated"/"cluster" pattern. Deliberately
/// small (a handful, not dozens) because this crate observes bounded local
/// history, not a high-volume service — see the M0.15.11 report for the
/// reasoning.
const REPEATED_FAILURE_THRESHOLD: u64 = 3;

/// Minimum samples required in EACH half of a two-half-window latency
/// comparison before `LatencyDegradation` may be considered at all — a
/// baseline built from one or two samples must never produce a
/// degradation claim.
const MIN_LATENCY_SAMPLES_PER_HALF: usize = 3;

/// The recent half's mean must exceed the earlier half's mean by at least
/// this factor before `LatencyDegradation` fires. Deliberately
/// conservative (50% slower, not any measurable difference) so ordinary
/// run-to-run variance does not read as degradation.
const LATENCY_DEGRADATION_FACTOR: f64 = 1.5;

/// Bound on how many rows any single internal query pulls from the health
/// store. Every `DiagnosticSnapshot::build` call site chooses its own
/// `evidence_limit`; this is only the crate's own sanity ceiling so a
/// misconfigured caller cannot force an unbounded read.
pub const MAX_EVIDENCE_LIMIT: usize = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticError {
    /// The health store could not be read (open failure, storage error,
    /// schema mismatch, or a query itself failed). Wraps
    /// `maia_local_intelligence_health::ObserverError` without
    /// distinguishing its variants further — at the diagnostic layer, any
    /// of them equally means "no trustworthy snapshot can be built right
    /// now," which is itself the correct thing to report (see
    /// `DiagnosticError` is never treated as "unavailable" at the
    /// inference layer — section 10 of the M0.15.11 report).
    StoreUnavailable,
    /// `until` was not after `since`.
    InvalidWindow,
}
impl From<ObserverError> for DiagnosticError {
    fn from(_: ObserverError) -> Self {
        Self::StoreUnavailable
    }
}

/// How much of the requested window is actually backed by retained
/// evidence. Retention (`RetentionPolicy` in `maia-local-intelligence-health`)
/// bounds how much survives; at a short polling interval that bound can be
/// reached well under the nominal `max_age` (Architecture Desk's own
/// M0.15.10 acceptance measured ~13h53m of continuous status history at
/// the default 5-second interval and 10,000-row cap). A diagnostic
/// consumer must never silently behave as if a requested window ("last 24
/// hours") were fully present when retention has already evicted part of
/// it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WindowCoverage {
    pub requested_since: SystemTime,
    pub requested_until: SystemTime,
    /// The full retained span of status samples, across ALL history (not
    /// window-bounded) — from `HealthStore::history_coverage()`. `None` if
    /// no status sample has ever been recorded (or none survive
    /// retention).
    pub retained_status_span: Option<(SystemTime, SystemTime)>,
    /// The full retained span of inference outcomes, across all history.
    pub retained_outcome_span: Option<(SystemTime, SystemTime)>,
    /// Earliest/latest timestamps of the actual bounded status samples used
    /// in this snapshot's requested window. Unlike retained_status_span,
    /// this excludes samples outside the window and samples omitted by the
    /// query limit. It describes sample boundaries, not continuous sampling.
    /// Older serialized snapshots have no such proof and deserialize as None.
    #[serde(default)]
    pub status_evidence_span_in_window: Option<(SystemTime, SystemTime)>,
    /// `true` only if retained status history reaches back to at least
    /// `requested_since` — i.e. nothing that would have answered the
    /// requested window has already been evicted or simply never existed.
    /// `false` whenever `retained_status_span` is `None` or starts later
    /// than `requested_since`; this is the "partial coverage" signal the
    /// M0.15.11 directive requires every time-reasoning result to expose.
    pub status_history_covers_requested_window: bool,
}

/// Simple, deterministic latency statistics for a bounded set of samples —
/// no trend/anomaly inference here; see `LatencyComparison` for the one
/// explicit two-half comparison this crate performs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LatencySummary {
    pub sample_count: usize,
    pub min: Option<Duration>,
    pub max: Option<Duration>,
    pub mean: Option<Duration>,
    pub median: Option<Duration>,
}
impl LatencySummary {
    fn from_samples(mut samples: Vec<Duration>) -> Self {
        if samples.is_empty() {
            return Self {
                sample_count: 0,
                min: None,
                max: None,
                mean: None,
                median: None,
            };
        }
        samples.sort();
        let sample_count = samples.len();
        let min = samples.first().copied();
        let max = samples.last().copied();
        let total_nanos: u128 = samples.iter().map(|d| d.as_nanos()).sum();
        let mean = Some(Duration::from_nanos(
            (total_nanos / sample_count as u128) as u64,
        ));
        let median = Some(if sample_count % 2 == 1 {
            samples[sample_count / 2]
        } else {
            let a = samples[sample_count / 2 - 1];
            let b = samples[sample_count / 2];
            (a + b) / 2
        });
        Self {
            sample_count,
            min,
            max,
            mean,
            median,
        }
    }
}

/// An explicit two-half-window latency baseline comparison. Always
/// present in a snapshot (not only when a pattern fires), so a consumer
/// can see the exact sample counts and means behind any degradation
/// claim, per the M0.15.11 directive's explicit requirement that "any
/// baseline comparison must expose the sample count and evidence window
/// behind it."
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LatencyComparison {
    pub earlier_half: LatencySummary,
    pub recent_half: LatencySummary,
    /// `true` only if both halves meet `MIN_LATENCY_SAMPLES_PER_HALF` and
    /// the recent half's mean exceeds the earlier half's by at least
    /// `LATENCY_DEGRADATION_FACTOR`.
    pub degradation_supported_by_evidence: bool,
}

/// Which failure-classification fidelity is actually present in the
/// window's recorded failures. See `infra/local-intelligence-health`'s own
/// module docs (and the M0.15.10 report, section 7) for why Desktop's
/// actual product path (`consult()`) only ever yields the coarser
/// `briefing_`-prefixed labels, never the rich taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TaxonomyFidelity {
    /// No failures recorded in the window at all.
    NoFailures,
    /// Every recorded failure used the rich, capability-owned taxonomy.
    Rich,
    /// Every recorded failure used Briefing's coarser, already-mapped-down
    /// taxonomy.
    BriefingCoarse,
    /// Both were present in the same window.
    Mixed,
}

/// An evidence-backed observation about the pattern of failures or
/// availability in the window — never an asserted root cause. Naming
/// mirrors the M0.15.11 directive's own suggested pattern names exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DiagnosticPattern {
    /// Exactly one failure in the window; not evidence of an ongoing
    /// problem by itself.
    IsolatedFailure,
    /// `>= REPEATED_FAILURE_THRESHOLD` unavailable-classified failures
    /// (rich or Briefing-sourced) in the window.
    RepeatedUnavailability,
    /// `>= REPEATED_FAILURE_THRESHOLD` timeout-classified failures (rich
    /// or Briefing-sourced) in the window.
    RepeatedTimeout,
    /// The most recent status samples in the window show availability
    /// toggling (not monotonically stable), with enough samples to be
    /// meaningful.
    IntermittentAvailability,
    /// `>= REPEATED_FAILURE_THRESHOLD` malformed-response-classified
    /// failures in the window.
    MalformedResponseCluster,
    /// `>= REPEATED_FAILURE_THRESHOLD` failures in the window that are
    /// neither unavailable, timeout nor malformed-response (write
    /// failure, response-too-large, model-rejected, other, or Briefing's
    /// undifferentiated provider-error bucket).
    ProviderErrorCluster,
    /// The recent half of the window's latency samples average at least
    /// `LATENCY_DEGRADATION_FACTOR` times the earlier half's, with enough
    /// samples in both halves to support the comparison
    /// (`LatencyComparison.degradation_supported_by_evidence`).
    LatencyDegradation,
}

/// `no evidence` / `insufficient evidence` / `healthy evidence` /
/// `degradation-pattern evidence` — the top-level read the M0.15.11
/// directive requires every snapshot to expose. Absence of records is
/// never interpreted as health: `NoEvidence` and `InsufficientEvidence`
/// are both distinct from, and never conflated with, `HealthyEvidence`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EvidenceLevel {
    /// Zero status samples recorded in the requested window.
    NoEvidence,
    /// Some status samples exist, but fewer than
    /// `MIN_SAMPLES_FOR_CONFIDENT_READ` — too few for a trustworthy
    /// availability ratio or pattern read.
    InsufficientEvidence,
    /// Enough samples exist, and no degradation pattern (other than an
    /// `IsolatedFailure`, which alone is not degradation) was detected.
    HealthyEvidence,
    /// Enough samples exist, and at least one non-isolated degradation
    /// pattern was detected.
    DegradationPatternEvidence,
}

/// The full, typed, bounded diagnostic snapshot. Every field is directly
/// traceable to a `maia-local-intelligence-health` query result — nothing
/// here is inferred beyond the deterministic, documented rules in this
/// crate. Not `Eq` — `availability_ratio_in_window` is an `Option<f64>`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DiagnosticSnapshot {
    pub coverage: WindowCoverage,
    /// The single most recent status sample overall (not window-bounded) —
    /// "current recorded availability", distinct from any ratio computed
    /// over the requested window. `None` if no status sample has ever been
    /// recorded.
    pub current_availability: Option<bool>,
    pub current_model: Option<String>,
    pub status_sample_count_in_window: usize,
    /// Fraction of in-window status samples with `available == true`.
    /// `None` when `status_sample_count_in_window == 0` — never `0.0`,
    /// which would misleadingly look like "confirmed always down."
    pub availability_ratio_in_window: Option<f64>,
    /// From `HealthStore::consecutive_unavailable_or_timeout_count()` —
    /// store-wide "right now", not window-bounded (matches that method's
    /// own established semantics).
    pub consecutive_unavailable_or_timeout: u64,
    pub failure_counts_by_class: BTreeMap<String, u64>,
    pub latency: LatencySummary,
    pub latency_trend: LatencyComparison,
    pub taxonomy_fidelity: TaxonomyFidelity,
    pub detected_patterns: Vec<DiagnosticPattern>,
    pub evidence_level: EvidenceLevel,
}

impl DiagnosticSnapshot {
    /// Builds a diagnostic snapshot for `[since, until]`, reading only the
    /// public query contract of `maia-local-intelligence-health`.
    /// `evidence_limit` bounds every internal query (clamped to
    /// `MAX_EVIDENCE_LIMIT`) — this call can never load an unbounded
    /// amount of history into memory regardless of how much exists.
    ///
    /// A failure here is strictly observational: it never touches the
    /// health store's data, never implies the runtime is unavailable, and
    /// never has any effect beyond returning `Err`.
    pub fn build(
        store: &HealthStore,
        since: SystemTime,
        until: SystemTime,
        evidence_limit: usize,
    ) -> Result<Self, DiagnosticError> {
        if until <= since {
            return Err(DiagnosticError::InvalidWindow);
        }
        let limit = evidence_limit.clamp(1, MAX_EVIDENCE_LIMIT);

        let history = store.history_coverage()?;
        let status_samples = store.status_samples_in_window(since, until, limit)?;
        let status_evidence_span_in_window = status_samples
            .iter()
            .map(|sample| sample.recorded_at)
            .min()
            .zip(status_samples.iter().map(|sample| sample.recorded_at).max());
        let coverage = WindowCoverage {
            requested_since: since,
            requested_until: until,
            retained_status_span: history.status_samples_span,
            retained_outcome_span: history.inference_outcomes_span,
            status_evidence_span_in_window,
            status_history_covers_requested_window: history
                .status_samples_span
                .is_some_and(|(earliest, _)| earliest <= since),
        };

        let latest = store.latest_status()?;
        let current_availability = latest.as_ref().map(|s| s.available);
        let current_model = latest.map(|s| s.model);

        let status_sample_count_in_window = status_samples.len();
        let availability_ratio_in_window = if status_sample_count_in_window == 0 {
            None
        } else {
            let available_count = status_samples.iter().filter(|s| s.available).count();
            Some(available_count as f64 / status_sample_count_in_window as f64)
        };

        let consecutive_unavailable_or_timeout =
            store.consecutive_unavailable_or_timeout_count()?;
        let failure_counts_by_class = store.failure_counts_by_class(since, until)?;

        let latency_samples = store.recent_latencies(since, until, limit)?;
        let latency = LatencySummary::from_samples(latency_samples);

        let mid = midpoint(since, until);
        // Both `recent_latencies` bounds are inclusive, so the two halves
        // must not share `mid` itself or a record recorded exactly at the
        // midpoint would be double-counted in both halves.
        let earlier_until = mid.checked_sub(Duration::from_millis(1)).unwrap_or(since);
        let earlier_latencies = store.recent_latencies(since, earlier_until, limit)?;
        let recent_latencies = store.recent_latencies(mid, until, limit)?;
        let earlier_half = LatencySummary::from_samples(earlier_latencies);
        let recent_half = LatencySummary::from_samples(recent_latencies);
        let degradation_supported_by_evidence = earlier_half.sample_count
            >= MIN_LATENCY_SAMPLES_PER_HALF
            && recent_half.sample_count >= MIN_LATENCY_SAMPLES_PER_HALF
            && match (earlier_half.mean, recent_half.mean) {
                (Some(earlier), Some(recent)) => {
                    recent.as_secs_f64() >= earlier.as_secs_f64() * LATENCY_DEGRADATION_FACTOR
                }
                _ => false,
            };
        let latency_trend = LatencyComparison {
            earlier_half,
            recent_half,
            degradation_supported_by_evidence,
        };

        let taxonomy_fidelity = classify_taxonomy_fidelity(&failure_counts_by_class);
        let mut detected_patterns =
            detect_failure_patterns(&failure_counts_by_class, REPEATED_FAILURE_THRESHOLD);
        if detect_intermittent_availability(&status_samples) {
            detected_patterns.push(DiagnosticPattern::IntermittentAvailability);
        }
        if latency_trend.degradation_supported_by_evidence {
            detected_patterns.push(DiagnosticPattern::LatencyDegradation);
        }

        let evidence_level = if status_sample_count_in_window == 0 {
            EvidenceLevel::NoEvidence
        } else if status_sample_count_in_window < MIN_SAMPLES_FOR_CONFIDENT_READ {
            EvidenceLevel::InsufficientEvidence
        } else if detected_patterns
            .iter()
            .any(|p| *p != DiagnosticPattern::IsolatedFailure)
        {
            EvidenceLevel::DegradationPatternEvidence
        } else {
            EvidenceLevel::HealthyEvidence
        };

        Ok(Self {
            coverage,
            current_availability,
            current_model,
            status_sample_count_in_window,
            availability_ratio_in_window,
            consecutive_unavailable_or_timeout,
            failure_counts_by_class,
            latency,
            latency_trend,
            taxonomy_fidelity,
            detected_patterns,
            evidence_level,
        })
    }
}

fn midpoint(since: SystemTime, until: SystemTime) -> SystemTime {
    let span = until.duration_since(since).unwrap_or(Duration::ZERO);
    since + span / 2
}

/// Rich labels never start with `briefing_` (see
/// `infra/local-intelligence-health`'s `rich_failure_label`/
/// `briefing_failure_label`, which are deliberately disjoint on this exact
/// property).
fn is_briefing_sourced(label: &str) -> bool {
    label.starts_with("briefing_")
}

fn classify_taxonomy_fidelity(counts: &BTreeMap<String, u64>) -> TaxonomyFidelity {
    if counts.is_empty() {
        return TaxonomyFidelity::NoFailures;
    }
    let (mut any_rich, mut any_coarse) = (false, false);
    for label in counts.keys() {
        if is_briefing_sourced(label) {
            any_coarse = true;
        } else {
            any_rich = true;
        }
    }
    match (any_rich, any_coarse) {
        (true, true) => TaxonomyFidelity::Mixed,
        (true, false) => TaxonomyFidelity::Rich,
        (false, true) => TaxonomyFidelity::BriefingCoarse,
        (false, false) => TaxonomyFidelity::NoFailures,
    }
}

/// Every rich/Briefing-sourced label pair that means "the same underlying
/// concept", grouped for pattern detection. `provider_error_cluster` is
/// deliberately a catch-all for everything that is neither unavailable,
/// timeout nor malformed-response, matching the directive's own suggested
/// "provider-error cluster" name.
fn detect_failure_patterns(
    counts: &BTreeMap<String, u64>,
    threshold: u64,
) -> Vec<DiagnosticPattern> {
    let total: u64 = counts.values().sum();
    let mut patterns = Vec::new();
    if total == 1 {
        patterns.push(DiagnosticPattern::IsolatedFailure);
    }
    let sum_of = |labels: &[&str]| -> u64 {
        labels
            .iter()
            .map(|label| counts.get(*label).copied().unwrap_or(0))
            .sum()
    };
    let unavailable = sum_of(&["runtime_unavailable", "briefing_unavailable"]);
    let timeout = sum_of(&["timeout", "briefing_timeout"]);
    let malformed = sum_of(&["malformed_response", "briefing_malformed_response"]);
    let provider_error = sum_of(&[
        "write_failure",
        "response_too_large",
        "model_rejected",
        "other",
        "briefing_provider_error",
        "briefing_model_unavailable",
    ]);
    if unavailable >= threshold {
        patterns.push(DiagnosticPattern::RepeatedUnavailability);
    }
    if timeout >= threshold {
        patterns.push(DiagnosticPattern::RepeatedTimeout);
    }
    if malformed >= threshold {
        patterns.push(DiagnosticPattern::MalformedResponseCluster);
    }
    if provider_error >= threshold {
        patterns.push(DiagnosticPattern::ProviderErrorCluster);
    }
    patterns
}

/// `true` if, among the in-window status samples (most-recent-first, as
/// returned by `status_samples_in_window`), availability is not uniform —
/// i.e. at least one `true` and at least one `false` are both present, and
/// there are enough samples for that to be meaningful rather than a single
/// startup transition.
fn detect_intermittent_availability(
    samples: &[maia_local_intelligence_health::StatusSampleRecord],
) -> bool {
    if samples.len() < MIN_SAMPLES_FOR_CONFIDENT_READ {
        return false;
    }
    let any_available = samples.iter().any(|s| s.available);
    let any_unavailable = samples.iter().any(|s| !s.available);
    any_available && any_unavailable
}

#[cfg(test)]
mod tests {
    use super::*;
    use maia_briefing::LocalProviderFailure;
    use maia_local_intelligence_health::{OperationClass, RetentionPolicy};
    use maia_local_model::{LocalIntelligenceFailure, LocalIntelligenceStatus};
    use std::time::UNIX_EPOCH;

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn store() -> HealthStore {
        HealthStore::open_in_memory(RetentionPolicy::default()).expect("open in-memory store")
    }

    fn status(available: bool) -> LocalIntelligenceStatus {
        LocalIntelligenceStatus {
            available,
            model: "qwen3:4b".into(),
        }
    }

    #[test]
    fn a_full_snapshot_round_trips_through_json_exactly() {
        // Foundational for M0.15.12's ledger, which persists a whole
        // DiagnosticSnapshot as its evidence record: every field, including
        // nested Option<(SystemTime, SystemTime)> tuples and a
        // BTreeMap<String, u64>, must survive a serde_json round trip
        // byte-for-byte equal (via PartialEq), not merely "compiles".
        let store = store();
        for i in 0..10u64 {
            store
                .record_status_sample(&status(true), None, at(i))
                .unwrap();
        }
        store
            .record_inference_failure(
                "qwen3:4b",
                OperationClass::CompleteDetailed,
                LocalIntelligenceFailure::Timeout,
                Some(Duration::from_millis(42)),
                at(5),
            )
            .unwrap();
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(20), 100).unwrap();
        let json = serde_json::to_string(&snapshot).expect("serialize");
        let round_tripped: DiagnosticSnapshot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(snapshot, round_tripped);
    }

    #[test]
    fn no_history_produces_no_evidence_not_healthy() {
        let store = store();
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(1000), 100).unwrap();
        assert_eq!(snapshot.evidence_level, EvidenceLevel::NoEvidence);
        assert_eq!(snapshot.status_sample_count_in_window, 0);
        assert_eq!(snapshot.availability_ratio_in_window, None);
        assert_eq!(snapshot.current_availability, None);
        assert!(
            snapshot.detected_patterns.is_empty(),
            "no evidence must never itself be reported as a pattern"
        );
    }

    #[test]
    fn retention_start_flag_does_not_prove_window_end_coverage() {
        let store = store();
        for i in 0..10u64 {
            store
                .record_status_sample(&status(true), None, at(i))
                .unwrap();
        }
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(20), 100).unwrap();
        assert!(snapshot.coverage.status_history_covers_requested_window);
        assert_eq!(snapshot.coverage.retained_status_span, Some((at(0), at(9))));
        assert_eq!(
            snapshot.coverage.status_evidence_span_in_window,
            Some((at(0), at(9)))
        );
        assert!(
            snapshot.coverage.status_evidence_span_in_window.unwrap().1
                < snapshot.coverage.requested_until
        );
    }

    #[test]
    fn actual_evidence_boundaries_exclude_out_of_window_and_query_limited_samples() {
        let store = store();
        for second in 0..=30 {
            store
                .record_status_sample(&status(true), None, at(second))
                .unwrap();
        }
        let complete = DiagnosticSnapshot::build(&store, at(10), at(20), 100).unwrap();
        assert_eq!(
            complete.coverage.retained_status_span,
            Some((at(0), at(30)))
        );
        assert_eq!(
            complete.coverage.status_evidence_span_in_window,
            Some((at(10), at(20)))
        );
        let limited = DiagnosticSnapshot::build(&store, at(10), at(20), 5).unwrap();
        assert!(limited.coverage.status_history_covers_requested_window);
        assert_eq!(
            limited.coverage.status_evidence_span_in_window,
            Some((at(16), at(20)))
        );
        let empty = DiagnosticSnapshot::build(&store, at(40), at(50), 100).unwrap();
        assert_eq!(empty.coverage.status_evidence_span_in_window, None);
    }

    #[test]
    fn legacy_snapshot_decodes_without_fabricating_evidence_boundaries() {
        let store = store();
        for second in 0..=20 {
            store
                .record_status_sample(&status(true), None, at(second))
                .unwrap();
        }
        let mut snapshot = DiagnosticSnapshot::build(&store, at(0), at(20), 100).unwrap();
        assert_eq!(
            snapshot.coverage.status_evidence_span_in_window,
            Some((at(0), at(20)))
        );
        let mut legacy = serde_json::to_value(&snapshot).unwrap();
        legacy["coverage"]
            .as_object_mut()
            .unwrap()
            .remove("status_evidence_span_in_window");
        let decoded: DiagnosticSnapshot = serde_json::from_value(legacy).unwrap();
        assert_eq!(decoded.coverage.status_evidence_span_in_window, None);
        snapshot.coverage.status_evidence_span_in_window = None;
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn a_window_reaching_before_retained_history_reports_partial_coverage() {
        let store = store();
        for i in 100..110u64 {
            store
                .record_status_sample(&status(true), None, at(i))
                .unwrap();
        }
        // Requesting from before any history exists must be honestly
        // reported as partial, never silently treated as fully covered.
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(200), 100).unwrap();
        assert!(!snapshot.coverage.status_history_covers_requested_window);
    }

    #[test]
    fn row_cap_truncated_history_is_reported_as_partial_coverage() {
        let retention = RetentionPolicy {
            max_rows_per_table: 5,
            max_age: Duration::from_secs(1_000_000),
        };
        let store = HealthStore::open_in_memory(retention).expect("open");
        for i in 0..50u64 {
            store
                .record_status_sample(&status(true), None, at(i))
                .unwrap();
        }
        // Only the newest 5 survive; a window asking back to at(0) must be
        // reported as partial, reflecting what retention actually evicted.
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(100), 100).unwrap();
        assert!(!snapshot.coverage.status_history_covers_requested_window);
        assert_eq!(snapshot.status_sample_count_in_window, 5);
    }

    #[test]
    fn few_samples_are_insufficient_evidence_not_healthy() {
        let store = store();
        for i in 0..3u64 {
            store
                .record_status_sample(&status(true), None, at(i))
                .unwrap();
        }
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(10), 100).unwrap();
        assert_eq!(snapshot.evidence_level, EvidenceLevel::InsufficientEvidence);
    }

    #[test]
    fn enough_healthy_samples_with_no_failures_is_healthy_evidence() {
        let store = store();
        for i in 0..10u64 {
            store
                .record_status_sample(&status(true), None, at(i))
                .unwrap();
        }
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(20), 100).unwrap();
        assert_eq!(snapshot.evidence_level, EvidenceLevel::HealthyEvidence);
        assert_eq!(snapshot.availability_ratio_in_window, Some(1.0));
    }

    #[test]
    fn a_single_transient_failure_is_isolated_and_does_not_escalate_evidence_level() {
        let store = store();
        for i in 0..10u64 {
            store
                .record_status_sample(&status(true), None, at(i))
                .unwrap();
        }
        store
            .record_inference_failure(
                "qwen3:4b",
                OperationClass::CompleteDetailed,
                LocalIntelligenceFailure::Timeout,
                None,
                at(5),
            )
            .unwrap();
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(20), 100).unwrap();
        assert_eq!(
            snapshot.detected_patterns,
            vec![DiagnosticPattern::IsolatedFailure]
        );
        assert_eq!(
            snapshot.evidence_level,
            EvidenceLevel::HealthyEvidence,
            "a single isolated failure must not read as a degradation pattern"
        );
    }

    #[test]
    fn repeated_unavailable_samples_are_flagged_as_a_degradation_pattern() {
        let store = store();
        for i in 0..10u64 {
            store
                .record_status_sample(&status(true), None, at(i))
                .unwrap();
        }
        for i in 0..3u64 {
            store
                .record_inference_failure(
                    "qwen3:4b",
                    OperationClass::CompleteDetailed,
                    LocalIntelligenceFailure::RuntimeUnavailable,
                    None,
                    at(100 + i),
                )
                .unwrap();
        }
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(200), 100).unwrap();
        assert!(
            snapshot
                .detected_patterns
                .contains(&DiagnosticPattern::RepeatedUnavailability)
        );
        assert_eq!(
            snapshot.evidence_level,
            EvidenceLevel::DegradationPatternEvidence
        );
    }

    #[test]
    fn repeated_timeouts_are_flagged_as_a_degradation_pattern() {
        let store = store();
        for i in 0..10u64 {
            store
                .record_status_sample(&status(true), None, at(i))
                .unwrap();
        }
        for i in 0..3u64 {
            store
                .record_inference_failure_from_briefing(
                    "qwen3:4b",
                    OperationClass::Consult,
                    LocalProviderFailure::Timeout,
                    None,
                    at(100 + i),
                )
                .unwrap();
        }
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(200), 100).unwrap();
        assert!(
            snapshot
                .detected_patterns
                .contains(&DiagnosticPattern::RepeatedTimeout)
        );
    }

    #[test]
    fn a_recovery_success_resets_the_consecutive_sequence() {
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
                LocalIntelligenceFailure::Timeout,
                None,
                at(2),
            )
            .unwrap();
        store
            .record_inference_success(
                "qwen3:4b",
                OperationClass::Complete,
                Duration::from_millis(10),
                at(3),
            )
            .unwrap();
        for i in 0..5u64 {
            store
                .record_status_sample(&status(true), None, at(10 + i))
                .unwrap();
        }
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(20), 100).unwrap();
        assert_eq!(snapshot.consecutive_unavailable_or_timeout, 0);
    }

    #[test]
    fn mixed_failure_classes_are_each_counted_and_reported_separately() {
        let store = store();
        for i in 0..10u64 {
            store
                .record_status_sample(&status(true), None, at(i))
                .unwrap();
        }
        store
            .record_inference_failure(
                "qwen3:4b",
                OperationClass::CompleteDetailed,
                LocalIntelligenceFailure::Timeout,
                None,
                at(11),
            )
            .unwrap();
        store
            .record_inference_failure(
                "qwen3:4b",
                OperationClass::CompleteDetailed,
                LocalIntelligenceFailure::MalformedResponse,
                None,
                at(12),
            )
            .unwrap();
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(20), 100).unwrap();
        assert_eq!(snapshot.failure_counts_by_class.len(), 2);
        assert_eq!(snapshot.failure_counts_by_class.get("timeout"), Some(&1));
        assert_eq!(
            snapshot.failure_counts_by_class.get("malformed_response"),
            Some(&1)
        );
    }

    #[test]
    fn rich_taxonomy_failures_are_indicated_as_rich() {
        let store = store();
        store
            .record_inference_failure(
                "qwen3:4b",
                OperationClass::CompleteDetailed,
                LocalIntelligenceFailure::Timeout,
                None,
                at(1),
            )
            .unwrap();
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(10), 100).unwrap();
        assert_eq!(snapshot.taxonomy_fidelity, TaxonomyFidelity::Rich);
    }

    #[test]
    fn briefing_coarse_failures_are_indicated_as_briefing_coarse() {
        let store = store();
        store
            .record_inference_failure_from_briefing(
                "qwen3:4b",
                OperationClass::Consult,
                LocalProviderFailure::Timeout,
                None,
                at(1),
            )
            .unwrap();
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(10), 100).unwrap();
        assert_eq!(snapshot.taxonomy_fidelity, TaxonomyFidelity::BriefingCoarse);
    }

    #[test]
    fn mixed_taxonomies_in_the_same_window_are_indicated_as_mixed() {
        let store = store();
        store
            .record_inference_failure(
                "qwen3:4b",
                OperationClass::CompleteDetailed,
                LocalIntelligenceFailure::Timeout,
                None,
                at(1),
            )
            .unwrap();
        store
            .record_inference_failure_from_briefing(
                "qwen3:4b",
                OperationClass::Consult,
                LocalProviderFailure::Timeout,
                None,
                at(2),
            )
            .unwrap();
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(10), 100).unwrap();
        assert_eq!(snapshot.taxonomy_fidelity, TaxonomyFidelity::Mixed);
    }

    #[test]
    fn latency_summary_computes_min_max_mean_median_correctly() {
        let store = store();
        for (i, ms) in [10u64, 20, 30, 40].into_iter().enumerate() {
            store
                .record_inference_success(
                    "qwen3:4b",
                    OperationClass::Complete,
                    Duration::from_millis(ms),
                    at(i as u64),
                )
                .unwrap();
        }
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(10), 100).unwrap();
        assert_eq!(snapshot.latency.sample_count, 4);
        assert_eq!(snapshot.latency.min, Some(Duration::from_millis(10)));
        assert_eq!(snapshot.latency.max, Some(Duration::from_millis(40)));
        assert_eq!(snapshot.latency.mean, Some(Duration::from_millis(25)));
        assert_eq!(snapshot.latency.median, Some(Duration::from_millis(25)));
    }

    #[test]
    fn a_clear_latency_increase_with_enough_samples_is_flagged_as_degradation() {
        let store = store();
        // Earlier half: 3 samples around 100ms. Recent half: 3 samples
        // around 300ms (3x, well over the 1.5x threshold).
        for i in 0..3u64 {
            store
                .record_inference_success(
                    "qwen3:4b",
                    OperationClass::Complete,
                    Duration::from_millis(100),
                    at(i),
                )
                .unwrap();
        }
        for i in 0..3u64 {
            store
                .record_inference_success(
                    "qwen3:4b",
                    OperationClass::Complete,
                    Duration::from_millis(300),
                    at(100 + i),
                )
                .unwrap();
        }
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(200), 100).unwrap();
        assert!(snapshot.latency_trend.degradation_supported_by_evidence);
        assert!(
            snapshot
                .detected_patterns
                .contains(&DiagnosticPattern::LatencyDegradation)
        );
        assert_eq!(snapshot.latency_trend.earlier_half.sample_count, 3);
        assert_eq!(snapshot.latency_trend.recent_half.sample_count, 3);
    }

    #[test]
    fn a_small_latency_sample_never_produces_a_degradation_claim() {
        let store = store();
        // Only 1 sample per half -- must never fire, regardless of ratio.
        store
            .record_inference_success(
                "qwen3:4b",
                OperationClass::Complete,
                Duration::from_millis(10),
                at(1),
            )
            .unwrap();
        store
            .record_inference_success(
                "qwen3:4b",
                OperationClass::Complete,
                Duration::from_millis(1000),
                at(150),
            )
            .unwrap();
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(200), 100).unwrap();
        assert!(!snapshot.latency_trend.degradation_supported_by_evidence);
        assert!(
            !snapshot
                .detected_patterns
                .contains(&DiagnosticPattern::LatencyDegradation)
        );
    }

    #[test]
    fn intermittent_availability_is_flagged_when_samples_toggle() {
        let store = store();
        for i in 0..10u64 {
            let available = i % 2 == 0;
            store
                .record_status_sample(&status(available), None, at(i))
                .unwrap();
        }
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(20), 100).unwrap();
        assert!(
            snapshot
                .detected_patterns
                .contains(&DiagnosticPattern::IntermittentAvailability)
        );
    }

    #[test]
    fn bounded_query_behavior_respects_the_evidence_limit() {
        let store = store();
        for i in 0..100u64 {
            store
                .record_status_sample(&status(true), None, at(i))
                .unwrap();
        }
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(200), 10).unwrap();
        assert_eq!(
            snapshot.status_sample_count_in_window, 10,
            "must never return more than the requested evidence_limit"
        );
    }

    #[test]
    fn an_evidence_limit_above_the_crate_ceiling_is_clamped_not_rejected() {
        let store = store();
        store
            .record_status_sample(&status(true), None, at(1))
            .unwrap();
        let snapshot =
            DiagnosticSnapshot::build(&store, at(0), at(10), usize::MAX).expect("must not error");
        assert_eq!(snapshot.status_sample_count_in_window, 1);
    }

    #[test]
    fn an_invalid_window_is_a_typed_error_not_a_panic() {
        let store = store();
        let result = DiagnosticSnapshot::build(&store, at(100), at(0), 10);
        assert_eq!(result.err(), Some(DiagnosticError::InvalidWindow));
    }

    #[test]
    fn current_availability_reflects_the_latest_sample_independent_of_the_window() {
        let store = store();
        store
            .record_status_sample(&status(false), None, at(1))
            .unwrap();
        store
            .record_status_sample(&status(true), None, at(500))
            .unwrap();
        // Requested window excludes the latest sample; current_availability
        // must still reflect it, since it is defined as store-wide latest,
        // not window-scoped.
        let snapshot = DiagnosticSnapshot::build(&store, at(0), at(10), 100).unwrap();
        assert_eq!(snapshot.current_availability, Some(true));
    }
}
