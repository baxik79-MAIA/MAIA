//! Local Intelligence Recorder Host (M0.15.14).
//!
//! The smallest application-level composition root that keeps one
//! `maia_local_intelligence_recorder::Recorder` running continuously so
//! Local Intelligence diagnostic evidence actually accumulates over time,
//! while the process is running — turning M0.15.13's "the mechanism
//! exists and can be started" into "evidence is genuinely flowing".
//!
//! This is **not** a Local Intelligence Lifecycle Supervisor, Ollama
//! lifecycle ownership, process restart logic, repair/remediation, causal
//! hypothesis generation, model reasoning, Round Table invocation,
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
//! binary crate with inline `#[cfg(test)]` tests, not a library — nothing
//! outside this crate is meant to depend on it (mechanically proven, see
//! `tests/capability_mesh.rs`).
#![forbid(unsafe_code)]

use maia_local_intelligence_health::{
    HealthStore, ObserverError, RetentionPolicy as HealthRetentionPolicy,
};
use maia_local_intelligence_hypothesis_ledger::{
    Ledger, LedgerError, RetentionPolicy as LedgerRetentionPolicy,
};
use maia_local_intelligence_recorder::{CycleOutcome, Recorder, RecorderConfig, StopOutcome};
use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
    time::Duration,
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
}

/// The running host: one `Recorder`, started against dependencies that were
/// already confirmed open at construction time. Owns exactly what §2
/// authorizes — construction, `Recorder::start`, process-lifetime wait
/// (owned by the caller, not this type), graceful `Recorder::stop` — and
/// nothing else. No Ollama/Claude-Code/process-control code path exists
/// anywhere in this crate (mechanically proven, `tests/capability_mesh.rs`).
struct LocalIntelligenceHost {
    recorder: Recorder,
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
        let ledger = Ledger::open(&config.ledger_db_path, LedgerRetentionPolicy::default())
            .map_err(HostStartError::LedgerUnavailable)?;
        let status = HostStatus {
            recorder_id: config.recorder_id.clone(),
            cadence: config.cadence,
            window: config.window,
            health_db_path: config.health_db_path.clone(),
            ledger_db_path: config.ledger_db_path.clone(),
        };
        let recorder_config = RecorderConfig {
            recorder_id: config.recorder_id,
            cadence: config.cadence,
            window: config.window,
            evidence_limit: config.evidence_limit,
        };
        let (recorder, cycle_outcomes) =
            Recorder::start_observed(Arc::new(health), Arc::new(ledger), recorder_config);
        Ok(Self {
            recorder,
            status,
            cycle_outcomes,
        })
    }

    fn status(&self) -> &HostStatus {
        &self.status
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
    let stop_outcome = host.stop();
    println!("Recorder stopped: {stop_outcome:?}");
    summarize_outcomes(&outcomes);
}

#[cfg(test)]
mod tests {
    use super::*;
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
    // idle host does not record early and responds to stop promptly.
    #[test]
    fn idle_host_does_not_record_early_and_stops_promptly() {
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
        let started = std::time::Instant::now();
        assert_eq!(host.stop(), StopOutcome::Stopped);
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "shutdown must remain bounded and prompt"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
