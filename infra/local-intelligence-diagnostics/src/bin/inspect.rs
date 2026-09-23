//! `local-intelligence-inspect` — the minimal human-readable inspection
//! surface the M0.15.11 directive requires, "so the stored history can be
//! inspected without opening SQLite manually." Displays the exact same
//! typed `DiagnosticSnapshot` a machine consumer would read — never a
//! second interpretation path.
//!
//! Usage:
//!   local-intelligence-inspect [--hours N] [--json]
//!
//! Reads the same health store Desktop writes to (default
//! `C:\MAIA\data\maia-local-intelligence-health.sqlite3`, overridable via
//! `MAIA_LOCAL_INTELLIGENCE_HEALTH_DB`, matching `apps/desktop`'s own
//! convention). Read-only in effect: this binary never calls any
//! `record_*` method.
use maia_local_intelligence_diagnostics::DiagnosticSnapshot;
use maia_local_intelligence_health::{HealthStore, RetentionPolicy};
use std::{path::Path, time::SystemTime};

const DEFAULT_HEALTH_DB: &str = r"C:\MAIA\data\maia-local-intelligence-health.sqlite3";
const DEFAULT_WINDOW_HOURS: u64 = 24;
const DEFAULT_EVIDENCE_LIMIT: usize = 5_000;

fn main() {
    let mut hours = DEFAULT_WINDOW_HOURS;
    let mut json = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--hours" => {
                if let Some(value) = args.next() {
                    if let Ok(parsed) = value.parse::<u64>() {
                        hours = parsed;
                    }
                }
            }
            "--json" => json = true,
            _ => {}
        }
    }

    let path = std::env::var("MAIA_LOCAL_INTELLIGENCE_HEALTH_DB")
        .unwrap_or_else(|_| DEFAULT_HEALTH_DB.into());
    let store = match HealthStore::open(Path::new(&path), RetentionPolicy::default()) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("could not open health store at {path}: {error:?}");
            std::process::exit(1);
        }
    };

    let until = SystemTime::now();
    let since = until - std::time::Duration::from_secs(hours * 3600);
    let snapshot = match DiagnosticSnapshot::build(&store, since, until, DEFAULT_EVIDENCE_LIMIT) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("could not build a diagnostic snapshot: {error:?}");
            std::process::exit(1);
        }
    };

    if json {
        println!("{}", render_json(&snapshot));
    } else {
        println!("{}", render_text(&snapshot, hours));
    }
}

fn millis(t: SystemTime) -> u128 {
    t.duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn render_json(snapshot: &DiagnosticSnapshot) -> String {
    let patterns: Vec<String> = snapshot
        .detected_patterns
        .iter()
        .map(|p| format!("{p:?}"))
        .collect();
    let value = serde_json::json!({
        "evidence_level": format!("{:?}", snapshot.evidence_level),
        "taxonomy_fidelity": format!("{:?}", snapshot.taxonomy_fidelity),
        "current_availability": snapshot.current_availability,
        "current_model": snapshot.current_model,
        "status_sample_count_in_window": snapshot.status_sample_count_in_window,
        "availability_ratio_in_window": snapshot.availability_ratio_in_window,
        "consecutive_unavailable_or_timeout": snapshot.consecutive_unavailable_or_timeout,
        "failure_counts_by_class": snapshot.failure_counts_by_class,
        "latency_ms": {
            "sample_count": snapshot.latency.sample_count,
            "min": snapshot.latency.min.map(|d| d.as_millis()),
            "max": snapshot.latency.max.map(|d| d.as_millis()),
            "mean": snapshot.latency.mean.map(|d| d.as_millis()),
            "median": snapshot.latency.median.map(|d| d.as_millis()),
        },
        "latency_trend": {
            "earlier_half_sample_count": snapshot.latency_trend.earlier_half.sample_count,
            "earlier_half_mean_ms": snapshot.latency_trend.earlier_half.mean.map(|d| d.as_millis()),
            "recent_half_sample_count": snapshot.latency_trend.recent_half.sample_count,
            "recent_half_mean_ms": snapshot.latency_trend.recent_half.mean.map(|d| d.as_millis()),
            "degradation_supported_by_evidence": snapshot.latency_trend.degradation_supported_by_evidence,
        },
        "detected_patterns": patterns,
        "coverage": {
            "requested_since_ms": millis(snapshot.coverage.requested_since),
            "requested_until_ms": millis(snapshot.coverage.requested_until),
            "retained_status_span_ms": snapshot.coverage.retained_status_span.map(|(a, b)| [millis(a), millis(b)]),
            "retained_outcome_span_ms": snapshot.coverage.retained_outcome_span.map(|(a, b)| [millis(a), millis(b)]),
            "status_history_covers_requested_window": snapshot.coverage.status_history_covers_requested_window,
        },
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into())
}

fn render_text(snapshot: &DiagnosticSnapshot, hours: u64) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Local Intelligence diagnostic snapshot — last {hours}h\n"
    ));
    out.push_str(&format!(
        "  evidence level:        {:?}\n",
        snapshot.evidence_level
    ));
    out.push_str(&format!(
        "  window coverage:       {}\n",
        if snapshot.coverage.status_history_covers_requested_window {
            "complete"
        } else {
            "PARTIAL — retained history does not reach the full requested window"
        }
    ));
    out.push_str(&format!(
        "  current availability:  {:?} (model: {:?})\n",
        snapshot.current_availability, snapshot.current_model
    ));
    out.push_str(&format!(
        "  status samples in window: {}\n",
        snapshot.status_sample_count_in_window
    ));
    out.push_str(&format!(
        "  availability ratio:    {:?}\n",
        snapshot.availability_ratio_in_window
    ));
    out.push_str(&format!(
        "  consecutive unavailable/timeout (right now): {}\n",
        snapshot.consecutive_unavailable_or_timeout
    ));
    out.push_str(&format!(
        "  failure taxonomy:      {:?}\n",
        snapshot.taxonomy_fidelity
    ));
    out.push_str(&format!(
        "  failure counts:        {:?}\n",
        snapshot.failure_counts_by_class
    ));
    out.push_str(&format!(
        "  latency (ms): count={} min={:?} max={:?} mean={:?} median={:?}\n",
        snapshot.latency.sample_count,
        snapshot.latency.min.map(|d| d.as_millis()),
        snapshot.latency.max.map(|d| d.as_millis()),
        snapshot.latency.mean.map(|d| d.as_millis()),
        snapshot.latency.median.map(|d| d.as_millis()),
    ));
    out.push_str(&format!(
        "  latency trend:         earlier(n={}, mean={:?}ms) -> recent(n={}, mean={:?}ms), degradation_supported={}\n",
        snapshot.latency_trend.earlier_half.sample_count,
        snapshot.latency_trend.earlier_half.mean.map(|d| d.as_millis()),
        snapshot.latency_trend.recent_half.sample_count,
        snapshot.latency_trend.recent_half.mean.map(|d| d.as_millis()),
        snapshot.latency_trend.degradation_supported_by_evidence,
    ));
    out.push_str(&format!(
        "  detected patterns:     {:?}\n",
        snapshot.detected_patterns
    ));
    out
}
