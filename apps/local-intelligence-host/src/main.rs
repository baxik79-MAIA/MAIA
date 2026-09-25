//! Local Intelligence Host (M0.15.14 recorder ownership; M0.15.17
//! application-facing advisory consumption).
//!
//! The smallest application-level composition root that keeps one
//! `maia_local_intelligence_recorder::Recorder` running continuously so
//! Local Intelligence diagnostic evidence actually accumulates over time,
//! while the process is running — turning M0.15.13's "the mechanism
//! exists and can be started" into "evidence is genuinely flowing".
//!
//! This is **not** a Local Intelligence Lifecycle Supervisor, Ollama
//! lifecycle ownership, process restart logic, repair/remediation, causal
//! causal confirmation, model reasoning, Round Table invocation,
//! self-improvement logic, Windows-service installation, startup-task
//! registration, or deployment/autostart packaging. It is only the
//! smallest composition/lifecycle integration required to keep one
//! `Recorder` instance running continuously in a real host process (see
//! `tests/capability_mesh.rs`).
//!
//! # Discovery (before implementation)
//!
//! Inspected `apps/*`, `ops/*`, `composition/*` for an existing long-running
//! host suitable for owning the Recorder:
//! - `apps/desktop` is a long-running process (an egui event loop), but the
//!   directive explicitly warns not to assume it is the right owner, and it
//!   is not neutral: it is a GUI product surface whose lifetime is tied to
//!   whether a human has a window open, which has nothing to do with
//!   whether Local Intelligence evidence should be accumulating. Coupling
//!   continuous recording to "is the Desktop window open" would be an
//!   architecturally wrong dependency, not a convenience.
//! - `apps/local-briefing-host` is a **one-shot** CLI (build one packet,
//!   persist it, exit via `ExitCode`) — wrong shape entirely; it never
//!   stays running.
//! - `apps/roundtable-viewer` and `ops/roundtable-invoke` are Round-Table
//!   member crates; depending on either — or being owned by either — would
//!   create a Round Table dependency, explicitly forbidden.
//! - No existing generic "background host" abstraction exists anywhere in
//!   `composition/*` either (both existing composition crates are
//!   Round-Table-specific).
//!
//! No existing host cleanly fits. Per the directive's own selection rule,
//! this crate is the smallest new, neutral, dedicated application-level
//! composition root, at the directive's own preferred conceptual location.
//! Naming follows the existing `apps/local-briefing-host` /
//! `maia-local-briefing-host` convention (`apps/local-intelligence-host` /
//! `maia-local-intelligence-host`); like `apps/desktop`, it is a plain
//! application package. M0.15.17 adds a small library target in this same
//! package as the explicit application-facing read-only advisory boundary;
//! no second application or infrastructure owner is created.
#![forbid(unsafe_code)]

use maia_local_intelligence_health::{
    HealthStore, ObserverError, RetentionPolicy as HealthRetentionPolicy,
};
use maia_local_intelligence_host::{
    ApplicationDiagnosticRequest, ApplicationDiagnosticResult, ApplicationEvaluationFailure,
    ApplicationEvaluationStage, evaluate_local_intelligence,
};
use maia_local_intelligence_hypothesis_ledger::{
    Ledger, LedgerError, RetentionPolicy as LedgerRetentionPolicy,
};
use maia_local_intelligence_recorder::{CycleOutcome, Recorder, RecorderConfig, StopOutcome};
use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
    time::{Duration, SystemTime},
};

const DEFAULT_HEALTH_DB: &str = r"C:\MAIA\data\maia-local-intelligence-health.sqlite3";
const DEFAULT_LEDGER_DB: &str = r"C:\MAIA\data\maia-local-intelligence-ledger.sqlite3";
/// Deterministic, stable identity for this host's `Recorder` — never
/// derived from a PID, a timestamp, or any other per-launch value (M0.15.14
/// §7: "Do not derive a random recorder_id at each launch"). An ordinary
/// host restart reuses this exact string, which is what lets the slot
/// identity M0.15.13 already establishes (`recorder:{id}:slot:{index}`)
/// remain stable across restarts and hit the ledger's existing idempotent
/// `ON CONFLICT DO NOTHING` guarantee rather than a fresh, unrelated one.
const DEFAULT_RECORDER_ID: &str = "local-intelligence-host";

/// Configuration for one host instance. Cadence/window default to
/// `RecorderConfig::default()`'s own values — read from that type, never
/// duplicated as separate constants here (M0.15.14 §7) — overridable only
/// through explicit environment variables (a documented configuration/test
/// seam, used by this crate's own integration/smoke tests; never silently
/// different from the real default in ordinary operation).
#[derive(Debug, Clone)]
struct HostConfig {
    recorder_id: String,
    health_db_path: PathBuf,
    ledger_db_path: PathBuf,
    cadence: Duration,
    window: Duration,
    evidence_limit: usize,
}
impl Default for HostConfig {
    fn default() -> Self {
        let recorder_defaults = RecorderConfig::default();
        let cadence = std::env::var("MAIA_LOCAL_INTELLIGENCE_HOST_CADENCE_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(Duration::from_millis)
            .unwrap_or(recorder_defaults.cadence);
        let window = std::env::var("MAIA_LOCAL_INTELLIGENCE_HOST_WINDOW_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(Duration::from_millis)
            .unwrap_or(recorder_defaults.window);
        Self {
            recorder_id: std::env::var("MAIA_LOCAL_INTELLIGENCE_HOST_RECORDER_ID")
                .unwrap_or_else(|_| DEFAULT_RECORDER_ID.into()),
            health_db_path: PathBuf::from(
                std::env::var("MAIA_LOCAL_INTELLIGENCE_HEALTH_DB")
                    .unwrap_or_else(|_| DEFAULT_HEALTH_DB.into()),
            ),
            ledger_db_path: PathBuf::from(
                std::env::var("MAIA_LOCAL_INTELLIGENCE_LEDGER_DB")
                    .unwrap_or_else(|_| DEFAULT_LEDGER_DB.into()),
            ),
            cadence,
            window,
            evidence_limit: recorder_defaults.evidence_limit,
        }
    }
}

/// Why `LocalIntelligenceHost::start` failed. Startup fails closed on
/// either variant: no `Recorder` is ever constructed with a half-opened
/// dependency (M0.15.14 §4).
#[derive(Debug)]
enum HostStartError {
    // The payload is read only through the derived `Debug` impl (printed
    // as operator-visible startup-failure diagnostics, M0.15.14 §4/§8),
    // which rustc's dead-code analysis does not count as a "read" --
    // `#[allow(dead_code)]` records that this is intentional, not an
    // oversight.
    #[allow(dead_code)]
    HealthStoreUnavailable(ObserverError),
    #[allow(dead_code)]
    LedgerUnavailable(LedgerError),
}

/// Bounded, operator-visible status — everything M0.15.14 §8 requires be
/// answerable, gathered from configuration already known at start time (no
/// control plane, no repair actions — purely descriptive).
#[derive(Debug, Clone)]
struct HostStatus {
    recorder_id: String,
    cadence: Duration,
    window: Duration,
    health_db_path: PathBuf,
    ledger_db_path: PathBuf,
    evidence_limit: usize,
}

/// The running host: one `Recorder`, started against dependencies that were
/// already confirmed open at construction time. Owns exactly what §2
/// authorizes — construction, `Recorder::start`, process-lifetime wait
/// (owned by the caller, not this type), graceful `Recorder::stop` — and
/// nothing else. No Ollama/Claude-Code/process-control code path exists
/// anywhere in this crate (mechanically proven, `tests/capability_mesh.rs`).
struct LocalIntelligenceHost {
    recorder: Recorder,
    ledger: Arc<Ledger>,
    status: HostStatus,
    cycle_outcomes: mpsc::Receiver<CycleOutcome>,
}

impl LocalIntelligenceHost {
    /// Opens the health store and ledger (in that order), failing closed on
    /// the first error — `Recorder::start` is only ever called once both
    /// dependencies are confirmed ready, never against a half-configured
    /// pair. Uses `Recorder::start_observed` so cycle failures remain
    /// answerable (§8) without granting this crate any new authority: the
    /// outcome is reported strictly after a cycle has already completed.
    fn start(config: HostConfig) -> Result<Self, HostStartError> {
        let health = HealthStore::open(&config.health_db_path, HealthRetentionPolicy::default())
            .map_err(HostStartError::HealthStoreUnavailable)?;
        let ledger = Arc::new(
            Ledger::open(&config.ledger_db_path, LedgerRetentionPolicy::default())
                .map_err(HostStartError::LedgerUnavailable)?,
        );
        let status = HostStatus {
            recorder_id: config.recorder_id.clone(),
            cadence: config.cadence,
            window: config.window,
            health_db_path: config.health_db_path.clone(),
            ledger_db_path: config.ledger_db_path.clone(),
            evidence_limit: config.evidence_limit,
        };
        let recorder_config = RecorderConfig {
            recorder_id: config.recorder_id,
            cadence: config.cadence,
            window: config.window,
            evidence_limit: config.evidence_limit,
        };
        let (recorder, cycle_outcomes) =
            Recorder::start_observed(Arc::new(health), Arc::clone(&ledger), recorder_config);
        Ok(Self {
            recorder,
            ledger,
            status,
            cycle_outcomes,
        })
    }

    fn status(&self) -> &HostStatus {
        &self.status
    }

    /// Shares the already-open public ledger capability with the read-only
    /// application adapter. It neither opens SQLite directly nor grants the
    /// adapter access to Recorder lifecycle control.
    fn advisory_ledger(&self) -> Arc<Ledger> {
        Arc::clone(&self.ledger)
    }

    /// Drains every cycle outcome reported so far without blocking —
    /// purely observational, never influences anything.
    fn drain_cycle_outcomes(&self) -> Vec<CycleOutcome> {
        self.cycle_outcomes.try_iter().collect()
    }

    /// Requests a graceful stop, respecting `Recorder::stop`'s own bounded
    /// shutdown contract exactly — no second shutdown mechanism is created
    /// here (M0.15.14 §5).
    fn stop(self) -> StopOutcome {
        self.recorder.stop()
    }
}

fn print_status(status: &HostStatus) {
    println!("  recorder_id: {}", status.recorder_id);
    println!("  cadence: {:?}", status.cadence);
    println!("  window: {:?}", status.window);
    println!("  health_db: {}", status.health_db_path.display());
    println!("  ledger_db: {}", status.ledger_db_path.display());
    println!("  evidence_limit: {}", status.evidence_limit);
}

fn summarize_outcomes(outcomes: &[CycleOutcome]) {
    if outcomes.is_empty() {
        println!("  (no completed cycles observed)");
        return;
    }
    let recorded = outcomes
        .iter()
        .filter(|o| **o == CycleOutcome::Recorded)
        .count();
    let already = outcomes
        .iter()
        .filter(|o| **o == CycleOutcome::AlreadyRecorded)
        .count();
    let failed = outcomes.len() - recorded - already;
    println!(
        "  cycles observed: {} (recorded={recorded}, already_recorded={already}, failed={failed})",
        outcomes.len()
    );
    if failed > 0 {
        for outcome in outcomes
            .iter()
            .filter(|o| !matches!(o, CycleOutcome::Recorded | CycleOutcome::AlreadyRecorded))
        {
            println!("    cycle failure: {outcome:?}");
        }
    }
}

fn print_advisory(result: &ApplicationDiagnosticResult) {
    println!("Local Intelligence advisory (read-only):");
    match result {
        ApplicationDiagnosticResult::Evaluated { assessments } => {
            for assessment in assessments {
                println!(
                    "  {} hypothesis={} subject={:?} confidence={:?} qualification={:?} reason={:?}",
                    assessment.state.code(),
                    assessment.hypothesis_id().as_str(),
                    assessment.subject(),
                    assessment.confidence(),
                    assessment.qualification.state,
                    assessment.qualification.reason,
                );
                println!(
                    "    uncertainty: {}",
                    assessment.qualification.hypothesis.uncertainty
                );
                println!(
                    "    qualified_at={:?} source_window={:?} stale={} rule={} gaps={:?}",
                    assessment.qualification.qualified_at,
                    assessment.qualification.source_window,
                    assessment.qualification.evidence_is_stale,
                    assessment.qualification.rule,
                    assessment.qualification.evidence_gaps,
                );
                for evidence in &assessment.evidence {
                    println!(
                        "    evidence={} role={:?} available={} observed_at={:?} requested={:?} actual={:?}",
                        evidence.observation_id.as_str(),
                        evidence.role,
                        evidence.available,
                        evidence.observed_at,
                        evidence.requested_window,
                        evidence.actual_status_span,
                    );
                }
            }
        }
        ApplicationDiagnosticResult::InsufficientEvidence {
            observations_scanned,
            indeterminate_observations,
        } => println!(
            "  INSUFFICIENT_EVIDENCE observations_scanned={observations_scanned} indeterminate_observations={indeterminate_observations}"
        ),
        ApplicationDiagnosticResult::Unavailable { stage, reason } => {
            println!("  UNAVAILABLE stage={stage:?} reason={reason:?}")
        }
        ApplicationDiagnosticResult::EvaluationFailed { stage, reason } => {
            println!("  EVALUATION_FAILED stage={stage:?} reason={reason:?}")
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bounded_seconds: Option<u64> = args
        .windows(2)
        .find(|pair| pair[0] == "--seconds")
        .and_then(|pair| pair[1].parse().ok());

    let config = HostConfig::default();
    println!("Local Intelligence Host starting...");

    let host = match LocalIntelligenceHost::start(config) {
        Ok(host) => host,
        Err(error) => {
            eprintln!("host failed to start: {error:?}");
            std::process::exit(1);
        }
    };
    println!("Recorder started.");
    print_status(host.status());

    if let Some(seconds) = bounded_seconds {
        // Bounded demonstration run (matches
        // `local-intelligence-recorder run --seconds N`'s existing
        // convention) -- used by this crate's own smoke test and by an
        // operator who wants a finite trial run without Ctrl+C.
        std::thread::sleep(Duration::from_secs(seconds));
        println!("Bounded run of {seconds}s elapsed.");
    } else {
        let (tx, rx) = mpsc::channel();
        if let Err(error) = ctrlc::set_handler(move || {
            let _ = tx.send(());
        }) {
            eprintln!("failed to install Ctrl+C handler: {error}");
            let outcome = host.stop();
            println!("Recorder stopped: {outcome:?}");
            std::process::exit(1);
        }
        let _ = rx.recv();
        println!("Shutdown requested (Ctrl+C).");
    }

    let outcomes = host.drain_cycle_outcomes();
    let status = host.status().clone();
    let advisory_ledger = host.advisory_ledger();
    let stop_outcome = host.stop();
    println!("Recorder stopped: {stop_outcome:?}");
    summarize_outcomes(&outcomes);
    let advisory = evaluate_advisory(&advisory_ledger, &status, SystemTime::now());
    print_advisory(&advisory);
}

/// How far back the advisory scans `observed_at`, derived from the
/// recorder's own cadence/window rather than a separate constant.
///
/// Each observation covers the inclusive span `[observed_at - window,
/// observed_at]`, so two observations can supply disjoint support only when
/// they are more than `window` apart. The recorder emits at most one
/// observation per cadence slot, so the smallest such separation is the
/// least cadence multiple strictly greater than `window`. One further
/// cadence admits the newest observation trailing the evaluation instant by
/// up to one slot. Scanning only `window` (the pre-F1 behaviour) makes
/// disjoint support, and thus `QUALIFIED`, unreachable. Qualification itself
/// is unchanged: freshness, coverage and counter-evidence still apply to
/// every scanned observation. `None` for a zero cadence or on overflow.
fn advisory_lookback(cadence: Duration, window: Duration) -> Option<Duration> {
    let cadence_ns = cadence.as_nanos();
    if cadence_ns == 0 {
        return None;
    }
    let separation_ns = (window.as_nanos() / cadence_ns + 1).checked_mul(cadence_ns)?;
    let lookback_ns = separation_ns.checked_add(cadence_ns)?;
    u64::try_from(lookback_ns).ok().map(Duration::from_nanos)
}

/// The host's read-only advisory evaluation at `evaluated_at`.
fn evaluate_advisory(
    ledger: &Ledger,
    status: &HostStatus,
    evaluated_at: SystemTime,
) -> ApplicationDiagnosticResult {
    let since = advisory_lookback(status.cadence, status.window)
        .and_then(|lookback| evaluated_at.checked_sub(lookback));
    match since {
        Some(since) => evaluate_local_intelligence(
            ledger,
            ApplicationDiagnosticRequest {
                since,
                until: evaluated_at,
                scan_limit: status.evidence_limit,
                evaluated_at,
                qualification_policy: Default::default(),
            },
        ),
        None => ApplicationDiagnosticResult::EvaluationFailed {
            stage: ApplicationEvaluationStage::Generation,
            reason: ApplicationEvaluationFailure::InvalidRequest,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maia_local_intelligence_host::ApplicationDiagnosticState;
    use maia_local_intelligence_recorder::record_cycle;
    use std::{
        collections::BTreeSet,
        sync::atomic::{AtomicU64, Ordering},
    };

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_test_dir(label: &str) -> PathBuf {
        let n = TEST_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "maia-li-host-test-{label}-{}-{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create temp test dir");
        dir
    }

    fn open_ledger(path: &std::path::Path) -> Ledger {
        Ledger::open(path, LedgerRetentionPolicy::default()).expect("open ledger for assertions")
    }

    fn unique_observation_ids(
        observations: &[maia_local_intelligence_hypothesis_ledger::DiagnosticObservation],
    ) -> bool {
        let ids: BTreeSet<&str> = observations
            .iter()
            .map(|o| o.observation_id.as_str())
            .collect();
        ids.len() == observations.len()
    }

    // A. invalid configuration -> host fails closed.
    #[test]
    fn invalid_health_db_configuration_fails_closed() {
        let dir = temp_test_dir("a");
        let config = HostConfig {
            health_db_path: PathBuf::from("Z:\\definitely-not-a-real-drive\\health.sqlite3"),
            ledger_db_path: dir.join("ledger.sqlite3"),
            ..HostConfig::default()
        };
        let result = LocalIntelligenceHost::start(config);
        assert!(matches!(
            result,
            Err(HostStartError::HealthStoreUnavailable(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // B. unavailable/unopenable ledger -> no fabricated observation (the
    // host never even reaches Recorder::start in this case).
    #[test]
    fn unopenable_ledger_fails_closed_before_recorder_starts() {
        let dir = temp_test_dir("b");
        let config = HostConfig {
            health_db_path: dir.join("health.sqlite3"),
            ledger_db_path: PathBuf::from("Z:\\definitely-not-a-real-drive\\ledger.sqlite3"),
            ..HostConfig::default()
        };
        let result = LocalIntelligenceHost::start(config);
        assert!(matches!(result, Err(HostStartError::LedgerUnavailable(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // C. snapshot failure -> no synthetic row.
    #[test]
    fn a_persistent_snapshot_failure_creates_no_synthetic_rows() {
        let dir = temp_test_dir("c");
        let ledger_path = dir.join("ledger.sqlite3");
        let config = HostConfig {
            health_db_path: dir.join("health.sqlite3"),
            ledger_db_path: ledger_path.clone(),
            cadence: Duration::from_millis(30),
            window: Duration::ZERO, // forces InvalidWindow every cycle
            ..HostConfig::default()
        };
        let host = LocalIntelligenceHost::start(config).expect("start");
        std::thread::sleep(Duration::from_millis(140));
        let outcomes = host.drain_cycle_outcomes();
        let stop_outcome = host.stop();
        assert_eq!(stop_outcome, StopOutcome::Stopped);
        assert!(
            !outcomes.is_empty()
                && outcomes
                    .iter()
                    .all(|o| matches!(o, CycleOutcome::SnapshotFailed(_))),
            "every observed cycle must have failed at the snapshot stage: {outcomes:?}"
        );
        let ledger = open_ledger(&ledger_path);
        assert_eq!(
            ledger.latest_n(100).unwrap().len(),
            0,
            "no row may ever be created when every cycle's snapshot build failed"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // §13 (1-8) + §6 (duplicate host) + D/F: the central real-integration
    // proof. Uses isolated temporary file-backed stores and a shortened
    // TEST-ONLY cadence via HostConfig's own fields -- RecorderConfig's
    // real DEFAULT_CADENCE/DEFAULT_WINDOW constants are never touched.
    #[test]
    fn continuous_accumulation_restart_idempotence_and_duplicate_hosts_never_inflate_counts() {
        let dir = temp_test_dir("integration");
        let health_path = dir.join("health.sqlite3");
        let ledger_path = dir.join("ledger.sqlite3");
        let base_config = HostConfig {
            recorder_id: "integration-test-host".into(),
            health_db_path: health_path,
            ledger_db_path: ledger_path.clone(),
            cadence: Duration::from_millis(40),
            window: Duration::from_secs(5),
            evidence_limit: RecorderConfig::default().evidence_limit,
        };

        // 1-4: start, allow several cycles, verify automatic accumulation
        // with no explicit record-once call anywhere in this test.
        let host1 = LocalIntelligenceHost::start(base_config.clone()).expect("start 1");
        std::thread::sleep(Duration::from_millis(220));
        // 5: stop.
        assert_eq!(host1.stop(), StopOutcome::Stopped);

        let after_first_run = open_ledger(&ledger_path).latest_n(1000).unwrap();
        assert!(
            !after_first_run.is_empty(),
            "observations must accumulate automatically while the host runs, with no explicit record-once call"
        );
        assert!(
            unique_observation_ids(&after_first_run),
            "slot IDs must remain deterministic and unique"
        );

        // 8 (partial): no cycles occur after clean shutdown.
        let count_after_stop = open_ledger(&ledger_path).latest_n(1000).unwrap().len();
        std::thread::sleep(Duration::from_millis(120));
        let count_after_wait = open_ledger(&ledger_path).latest_n(1000).unwrap().len();
        assert_eq!(
            count_after_stop, count_after_wait,
            "no further cycles may run after stop() has returned"
        );

        // 6-7: restart using the SAME recorder identity/store.
        let host2 = LocalIntelligenceHost::start(base_config.clone()).expect("start 2 (restart)");
        std::thread::sleep(Duration::from_millis(220));
        assert_eq!(host2.stop(), StopOutcome::Stopped);
        let after_restart = open_ledger(&ledger_path).latest_n(1000).unwrap();
        assert!(
            unique_observation_ids(&after_restart),
            "restart must not duplicate an already-recorded logical slot"
        );
        assert!(
            after_restart.len() >= after_first_run.len(),
            "restart must still be able to record NEW slots as real time moves forward, not be stuck"
        );

        // §6: two LIVE host instances against the SAME store at the same
        // time must never inflate the count -- M0.15.12's ledger-level
        // atomic uniqueness must hold across independent Recorder
        // instances (simulating independent processes), not merely within
        // one.
        let host_a = LocalIntelligenceHost::start(base_config.clone()).expect("start a");
        let host_b = LocalIntelligenceHost::start(base_config).expect("start b");
        std::thread::sleep(Duration::from_millis(220));
        assert_eq!(host_a.stop(), StopOutcome::Stopped);
        assert_eq!(host_b.stop(), StopOutcome::Stopped);
        let after_duplicate = open_ledger(&ledger_path).latest_n(1000).unwrap();
        assert!(
            unique_observation_ids(&after_duplicate),
            "two concurrent host instances racing on the same slots must never produce \
             duplicate rows"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // §14 (the actual compiled host binary, run as a real subprocess) lives
    // in tests/host_binary_smoke.rs instead of here: `CARGO_BIN_EXE_*` is
    // only set by Cargo for files under `tests/`, not for a binary
    // crate's own inline `#[cfg(test)]` module.

    // L (resource governance sanity, mirroring M0.15.13's own idle test):
    // idle host does not record before its first cadence boundary and stops
    // through the Recorder's bounded shutdown contract.
    #[test]
    fn idle_host_does_not_record_before_first_cadence_and_stops() {
        let dir = temp_test_dir("idle");
        let config = HostConfig {
            health_db_path: dir.join("health.sqlite3"),
            ledger_db_path: dir.join("ledger.sqlite3"),
            cadence: Duration::from_secs(10),
            ..HostConfig::default()
        };
        let ledger_path = config.ledger_db_path.clone();
        let host = LocalIntelligenceHost::start(config).expect("start");
        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(
            open_ledger(&ledger_path).latest_n(10).unwrap().len(),
            0,
            "must not record before its first cadence boundary"
        );
        assert_eq!(host.stop(), StopOutcome::Stopped);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // F1 (pre-publication): the lookback is derived from cadence/window and
    // exceeds the window, which is what makes disjoint support scannable.
    #[test]
    fn advisory_lookback_is_derived_from_cadence_and_window() {
        let defaults = RecorderConfig::default();
        let lookback = advisory_lookback(defaults.cadence, defaults.window).unwrap();
        assert!(lookback > defaults.window + defaults.cadence);
        assert_eq!(
            advisory_lookback(Duration::from_secs(15 * 60), Duration::from_secs(60 * 60)),
            Some(Duration::from_secs(90 * 60))
        );
        assert_eq!(
            advisory_lookback(Duration::from_secs(20), Duration::from_secs(50)),
            Some(Duration::from_secs(80))
        );
        assert_eq!(
            advisory_lookback(Duration::ZERO, Duration::from_secs(1)),
            None
        );
        assert_eq!(advisory_lookback(Duration::MAX, Duration::MAX), None);
    }

    fn default_status() -> HostStatus {
        let defaults = RecorderConfig::default();
        HostStatus {
            recorder_id: defaults.recorder_id,
            cadence: defaults.cadence,
            window: defaults.window,
            health_db_path: PathBuf::new(),
            ledger_db_path: PathBuf::new(),
            evidence_limit: defaults.evidence_limit,
        }
    }

    /// Cadence-aligned slot boundary `slot`, far from the epoch.
    fn boundary(config: &RecorderConfig, slot: u32) -> SystemTime {
        SystemTime::UNIX_EPOCH + config.cadence * (2_000_000 + slot)
    }

    /// Real health evidence (per-minute status samples plus the given
    /// timeouts) recorded through the recorder's own `record_cycle` at the
    /// default cadence/window for slots `0..=slots`.
    fn recorded_ledger(config: &RecorderConfig, slots: u32, timeouts: &[SystemTime]) -> Ledger {
        use maia_local_intelligence_health::OperationClass;
        use maia_local_model::{LocalIntelligenceFailure, LocalIntelligenceStatus};

        let health = HealthStore::open_in_memory(HealthRetentionPolicy::default()).unwrap();
        let ledger = Ledger::open_in_memory(LedgerRetentionPolicy::default()).unwrap();
        let status = LocalIntelligenceStatus {
            available: true,
            model: "synthetic".into(),
        };
        let mut sample = boundary(config, 0).checked_sub(config.window).unwrap();
        while sample <= boundary(config, slots) {
            health.record_status_sample(&status, None, sample).unwrap();
            sample += Duration::from_secs(60);
        }
        for &failed_at in timeouts {
            health
                .record_inference_failure(
                    "synthetic",
                    OperationClass::CompleteDetailed,
                    LocalIntelligenceFailure::Timeout,
                    None,
                    failed_at,
                )
                .unwrap();
        }
        for slot in 0..=slots {
            let outcome = record_cycle(&health, &ledger, config, boundary(config, slot));
            assert_eq!(outcome, CycleOutcome::Recorded);
        }
        ledger
    }

    /// Three timeouts shortly before `at`.
    fn timeout_burst(at: SystemTime) -> [SystemTime; 3] {
        [2u64, 5, 8].map(|minutes| at - Duration::from_secs(minutes * 60))
    }

    fn assessed_states(result: &ApplicationDiagnosticResult) -> Vec<ApplicationDiagnosticState> {
        let ApplicationDiagnosticResult::Evaluated { assessments } = result else {
            panic!("expected evaluated result, got {result:?}")
        };
        assessments.iter().map(|a| a.state).collect()
    }

    #[test]
    fn real_host_path_reaches_qualified_with_recorded_disjoint_evidence() {
        let status = default_status();
        let config = RecorderConfig::default();
        let slots = 8;
        let timeouts: Vec<_> = (0..=slots)
            .flat_map(|slot| timeout_burst(boundary(&config, slot)))
            .collect();
        let ledger = recorded_ledger(&config, slots, &timeouts);
        // Evaluate part-way into the following slot, as a real shutdown would.
        let evaluated_at = boundary(&config, slots) + Duration::from_secs(10 * 60);

        let result = evaluate_advisory(&ledger, &status, evaluated_at);
        let states = assessed_states(&result);
        assert!(
            states.contains(&ApplicationDiagnosticState::Qualified),
            "QUALIFIED must be reachable through the host path: {states:?}"
        );
        let ApplicationDiagnosticResult::Evaluated { assessments } = &result else {
            unreachable!()
        };
        for assessment in assessments {
            assert!(!assessment.evidence.is_empty());
            for item in &assessment.evidence {
                assert!(item.available && item.observed_at.is_some());
                assert!(item.requested_window.is_some());
            }
        }

        // Root cause: the same evidence scanned over only `window` cannot
        // yield disjoint support, so it never qualifies.
        let pre_f1 = evaluate_local_intelligence(
            &ledger,
            ApplicationDiagnosticRequest {
                since: evaluated_at - status.window,
                until: evaluated_at,
                scan_limit: status.evidence_limit,
                evaluated_at,
                qualification_policy: Default::default(),
            },
        );
        assert!(!assessed_states(&pre_f1).contains(&ApplicationDiagnosticState::Qualified));
    }

    #[test]
    fn real_host_path_keeps_counter_evidence_precedence() {
        let status = default_status();
        let config = RecorderConfig::default();
        let slots = 12;
        // Evaluating at slot 12 scans slots 6..=12. Slots 7 and 12 have
        // disjoint supporting windows, but healthy slots 8..=11 contradict.
        let mut timeouts =
            timeout_burst(boundary(&config, 3) + Duration::from_secs(10 * 60)).to_vec();
        timeouts.extend(timeout_burst(boundary(&config, 12)));
        let ledger = recorded_ledger(&config, slots, &timeouts);
        let states = assessed_states(&evaluate_advisory(
            &ledger,
            &status,
            boundary(&config, slots),
        ));
        assert!(
            !states.contains(&ApplicationDiagnosticState::Qualified),
            "counter-evidence must prevent QUALIFIED: {states:?}"
        );
        assert!(
            states.contains(&ApplicationDiagnosticState::CounterEvidencePresent),
            "counter-evidence must stay visible: {states:?}"
        );
    }

    #[test]
    fn real_host_path_does_not_assess_evidence_outside_the_lookback() {
        let status = default_status();
        let config = RecorderConfig::default();
        let slots = 8;
        let timeouts: Vec<_> = (0..=slots)
            .flat_map(|slot| timeout_burst(boundary(&config, slot)))
            .collect();
        let ledger = recorded_ledger(&config, slots, &timeouts);
        let evaluated_at = boundary(&config, slots) + Duration::from_secs(2 * 24 * 60 * 60);
        assert!(matches!(
            evaluate_advisory(&ledger, &status, evaluated_at),
            ApplicationDiagnosticResult::InsufficientEvidence { .. }
        ));
    }
}
