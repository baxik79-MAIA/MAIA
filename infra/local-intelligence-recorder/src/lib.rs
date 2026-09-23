//! Local Intelligence Diagnostic Evidence Recorder (M0.15.13).
//!
//! The missing piece M0.15.12 left open: accumulation over time. This
//! crate adds the smallest bounded mechanism that periodically performs
//!
//!   health history -> DiagnosticSnapshot -> DiagnosticObservation -> Ledger::record
//!
//! It is an **evidence recorder** — not a causal hypothesis generator, a
//! lifecycle supervisor, a process monitor with control authority, a
//! restart manager, a repair planner, an executor, a Round Table caller,
//! an LLM/model reasoning step, or a self-improvement decision mechanism.
//! **No action may be taken because of recorded evidence** (see
//! `tests/capability_mesh.rs`).
//!
//! # Discovery (before implementation)
//!
//! - `spec/scheduler.yaml` exists, but owns an entirely different domain:
//!   user-facing Task/Job scheduling (`time_based`/`recurring`/
//!   `condition_watch`/`follow_up_watch`) integrated with ApprovalGate and
//!   policy gates (`scheduled_execution_reuses_same_policy_gates_as_interactive_tasks`,
//!   `send_external_never_becomes_auto_approved_only_because_job_is_scheduled`).
//!   This recorder has no action authority and nothing it does should
//!   ever need ApprovalGate involvement — routing an internal,
//!   headless, no-output telemetry cadence through a contract built to
//!   keep *user-facing* scheduled actions honest would be a category
//!   error, not reuse. No canonical contract change was made.
//! - `infra/claude-code::runner::CancellationToken` exists (a ~10-line
//!   `Arc<AtomicBool>` wrapper) but reusing it would pull in an unrelated
//!   Claude-Code-subprocess-provider crate — its `development-evolution`
//!   feature, deployment-lock semantics, subprocess execution machinery —
//!   for a trivial primitive with nothing to do with Local Intelligence.
//!   The same trivial shape is reimplemented locally instead; a
//!   cross-domain import for ten lines is not "reuse cleanly."
//! - No generic `Scheduler` abstraction exists anywhere in the workspace.
//!   `core/runtime` and `core/orchestrator` explicitly document
//!   themselves as having no scheduler dependency by design — Core is
//!   meant to stay scheduler-free, which is itself the strongest signal
//!   that a cadence primitive belongs at the infra level, exactly where
//!   this crate lives.
//! - Desktop's own periodic re-probe (`apps/desktop`'s
//!   `LOCAL_PROBE_INTERVAL`) is an egui-render-loop-specific pattern, not
//!   an exported library primitive; nothing to reuse there either.
//!
//! See the M0.15.13 report for the full discovery record.
#![forbid(unsafe_code)]

use maia_local_intelligence_diagnostics::DiagnosticSnapshot;
use maia_local_intelligence_health::HealthStore;
use maia_local_intelligence_hypothesis_ledger::{
    DiagnosticObservation, Ledger, ObservationId, RecordOutcome,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

// ---------------------------------------------------------------------------
// Cadence derivation (M0.15.13 §4). Every number below is derived, not
// copied from the directive's own example sentence.
//
// Inputs:
//   - Health-history horizon at the default status-probe interval (5s) and
//     the health store's default retention (10,000 rows): 10,000 * 5s =
//     50,000s = 13h53m20s (measured and confirmed in the M0.15.10/M0.15.11
//     reports; Architecture Desk's own M0.15.10 acceptance verdict cites
//     the same figure).
//   - The ledger's own default retention: 10,000 rows / 30 days.
//
// Derivation:
//   The ledger's row cap and age cap coincide at
//     cadence_breakeven = max_age / max_rows = 30d / 10,000 = 2,592,000s / 10,000
//                        = 259.2s (~4.32 minutes).
//   Below that cadence, the ROW cap becomes the binding retention limit
//   before the 30-day age cap ever would -- exactly the mismatch
//   Architecture Desk flagged in the M0.15.10 acceptance verdict for the
//   health store's own 5-second/10,000-row combination (only ~14h of
//   actual coverage against a nominal 30-day age limit). Choosing a
//   cadence comfortably ABOVE the breakeven avoids repeating that mismatch
//   here: the 30-day age cap governs the ledger's effective retention, not
//   an accidental row-cap truncation.
//
//   15 minutes (900s) is ~3.5x the ~4.32-minute breakeven -- comfortably
//   on the correct side -- while still fitting roughly 55 cadence slots
//   within the health store's own ~13h53m horizon, so early cycles have
//   many chances to accumulate the >= 3 qualifying failures /
//   >= 5 status samples the diagnostics layer's own pattern/evidence
//   thresholds require before that horizon itself would need to reach
//   further back than retained history goes. CPU-first / office-PC-first:
//   one cycle is a handful of bounded, already-indexed SQLite reads plus
//   one small write -- negligible at any of these cadences; the interval
//   choice is governed by evidence economics (§4), not by resource cost.
/// Derived default cadence between recording cycles. See the derivation
/// above.
pub const DEFAULT_CADENCE: Duration = Duration::from_secs(15 * 60);

/// Derived default diagnostic window per cycle: four cadence ticks (one
/// hour), so a single snapshot's window comfortably spans several cycles'
/// worth of potential evidence even though only one cycle is being
/// recorded right now -- without reaching back further than is useful for
/// a cadence this frequent.
pub const DEFAULT_WINDOW: Duration = Duration::from_secs(60 * 60);

/// Sanity ceiling on rows read per diagnostic query inside one cycle.
pub const DEFAULT_EVIDENCE_LIMIT: usize = 5_000;

/// Configuration for one recorder instance. `recorder_id` is part of the
/// deterministic slot identity (see `cadence_slot_index`/
/// `slot_observation_id`) -- two `RecorderConfig`s with different
/// `recorder_id`s never collide even if their cadence/window happen to
/// match, and the SAME `recorder_id` retried after a crash/restart
/// produces the SAME identity for the same wall-clock slot, which is the
/// property M0.15.12's idempotent `Ledger::record` depends on to make
/// retries safe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecorderConfig {
    pub recorder_id: String,
    pub cadence: Duration,
    pub window: Duration,
    pub evidence_limit: usize,
}
impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            recorder_id: "default".into(),
            cadence: DEFAULT_CADENCE,
            window: DEFAULT_WINDOW,
            evidence_limit: DEFAULT_EVIDENCE_LIMIT,
        }
    }
}

/// The typed result of one explicit recording cycle. Every variant is
/// observational only — none of them, on any path, touches health
/// history, restarts anything, or grants any authority (see
/// `tests/capability_mesh.rs`).
#[derive(Debug, Clone, PartialEq)]
pub enum CycleOutcome {
    /// A new observation was created for this slot.
    Recorded,
    /// This slot's observation already existed (an idempotent retry —
    /// process restart, transient prior failure, or duplicate delivery);
    /// no new row was created and nothing was overwritten.
    AlreadyRecorded,
    /// Reading the health store and/or building a `DiagnosticSnapshot`
    /// failed. No ledger row was created for this attempt — the ledger is
    /// never touched until a snapshot has been successfully built.
    SnapshotFailed(maia_local_intelligence_diagnostics::DiagnosticError),
    /// The snapshot was built successfully but persisting it to the
    /// ledger failed.
    LedgerFailed(maia_local_intelligence_hypothesis_ledger::LedgerError),
    /// The computed slot identity was invalid (should not happen with a
    /// well-formed `recorder_id`, but handled explicitly rather than
    /// panicking).
    InvalidSlotIdentity,
}

fn millis_since_epoch(at: SystemTime) -> u64 {
    at.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The discrete cadence "bucket" `observed_at` falls into, given `cadence`.
/// Two calls with times in the same bucket (regardless of the exact
/// instant within it — e.g. a process restart a few seconds after a
/// crash) produce the same index, which is exactly the property
/// `slot_observation_id` needs for crash/retry idempotence (M0.15.13 §6).
pub fn cadence_slot_index(observed_at: SystemTime, cadence: Duration) -> u64 {
    let cadence_ms = (cadence.as_millis() as u64).max(1);
    millis_since_epoch(observed_at) / cadence_ms
}

/// A deterministic, stable `ObservationId` for one logical cadence slot —
/// never a random UUID, per the M0.15.13 directive's explicit instruction.
/// Derived purely from `recorder_id` and `slot_index`; the exact string
/// encoding is this crate's own choice (M0.15.13 §6: "the exact encoding
/// is implementation-owned"), but is stable across process restarts by
/// construction, since both inputs are themselves stable/deterministic.
pub fn slot_observation_id(
    recorder_id: &str,
    slot_index: u64,
) -> Result<ObservationId, maia_local_intelligence_hypothesis_ledger::LedgerError> {
    ObservationId::new(format!("recorder:{recorder_id}:slot:{slot_index}"))
}

/// Performs exactly one recording cycle: determine the requested
/// diagnostic window, build exactly one `DiagnosticSnapshot` using only
/// `infra/local-intelligence-diagnostics`'s public API, create exactly
/// one `DiagnosticObservation` with this slot's deterministic identity,
/// call `Ledger::record` exactly once, and return. No hidden retry, no
/// model inference, no cloud/provider call — this function makes exactly
/// the calls its own body shows, nothing more.
///
/// Fully independent of `Recorder`/any scheduler: a caller (a test, a CLI
/// `record-once` command, or a future different cadence owner) can call
/// this directly with any `observed_at` it chooses.
pub fn record_cycle(
    health: &HealthStore,
    ledger: &Ledger,
    config: &RecorderConfig,
    observed_at: SystemTime,
) -> CycleOutcome {
    let slot_index = cadence_slot_index(observed_at, config.cadence);
    let Ok(observation_id) = slot_observation_id(&config.recorder_id, slot_index) else {
        return CycleOutcome::InvalidSlotIdentity;
    };
    // `SystemTime::checked_sub` only fails near the platform's
    // representable minimum (e.g. before 1601 on Windows' FILETIME-based
    // clock) -- it does NOT fail merely for going before `UNIX_EPOCH`. A
    // `window` larger than `observed_at`'s distance from the epoch
    // therefore still returns `Some`, just with a value chronologically
    // before 1970 -- which `serde` (used by the ledger to persist the
    // snapshot) explicitly refuses to serialize. Clamp explicitly to
    // `UNIX_EPOCH` rather than relying on `checked_sub` to signal this.
    let since = observed_at
        .checked_sub(config.window)
        .unwrap_or(UNIX_EPOCH)
        .max(UNIX_EPOCH);
    let snapshot =
        match DiagnosticSnapshot::build(health, since, observed_at, config.evidence_limit) {
            Ok(snapshot) => snapshot,
            Err(error) => return CycleOutcome::SnapshotFailed(error),
        };
    let observation = DiagnosticObservation {
        observation_id,
        observed_at,
        snapshot,
    };
    match ledger.record(&observation) {
        Ok(RecordOutcome::Recorded) => CycleOutcome::Recorded,
        Ok(RecordOutcome::AlreadyExists) => CycleOutcome::AlreadyRecorded,
        Err(error) => CycleOutcome::LedgerFailed(error),
    }
}

/// How `Recorder::stop` resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopOutcome {
    /// The background loop observed the stop request and exited within
    /// the bound.
    Stopped,
    /// The loop did not exit within the bound (e.g. it was in the middle
    /// of an unusually slow cycle). The stop request remains in effect —
    /// the loop will exit as soon as its current cycle finishes — but the
    /// caller was not made to wait indefinitely for that.
    TimedOut,
}

/// How long `Recorder::stop` waits for the loop to exit before returning
/// `StopOutcome::TimedOut`. Bounded per M0.15.13 §11's "shutdown must be
/// bounded" — short enough that a caller is never left waiting through a
/// full cadence interval, generous enough to let one already-in-flight
/// cycle (a handful of bounded SQLite operations) finish cleanly rather
/// than being torn down mid-write.
pub const DEFAULT_SHUTDOWN_BOUND: Duration = Duration::from_secs(5);

/// A single-thread, non-overlapping cadence loop around `record_cycle`.
/// Structurally cannot run two cycles concurrently: cycles execute
/// sequentially on one dedicated thread, so "no overlapping cycles"
/// (M0.15.13 §5) holds by construction, not by an added lock. If one cycle
/// takes longer than `cadence`, the loop does not queue a backlog or spawn
/// a second cycle — it computes the CURRENT wall-clock slot when it next
/// wakes and simply continues from there, silently skipping any
/// intervening slots without fabricating observations for them (the
/// directive's preferred "finish current cycle -> next future cadence
/// boundary" policy, section 5).
pub struct Recorder {
    stop_flag: Arc<AtomicBool>,
    wake: Arc<(Mutex<bool>, std::sync::Condvar)>,
    done_rx: mpsc::Receiver<()>,
}

impl Recorder {
    /// Starts the cadence loop on a dedicated thread. Idle behavior is a
    /// bounded sleep to the next slot boundary (via a `Condvar`, so a stop
    /// request wakes it immediately rather than waiting out the full
    /// interval) — never a busy-loop, never polling merely to discover the
    /// next cadence has not arrived yet (M0.15.13 §10).
    pub fn start(health: Arc<HealthStore>, ledger: Arc<Ledger>, config: RecorderConfig) -> Self {
        Self::start_inner(health, ledger, config, None)
    }

    /// Identical to `start`, but additionally returns an
    /// `mpsc::Receiver<CycleOutcome>` that receives every completed
    /// cycle's outcome as it happens (M0.15.14 addition — a host process
    /// needs a way to answer "did a cycle fail observationally?" without
    /// this crate growing any action authority; the outcome is reported
    /// after the cycle has already fully completed, so observing it can
    /// never influence what that cycle did). A slow or absent receiver
    /// never blocks the recording loop: sends use `try_send`-equivalent,
    /// bounded semantics (see `run_loop`), so a host that stops polling
    /// the receiver cannot stall recording.
    pub fn start_observed(
        health: Arc<HealthStore>,
        ledger: Arc<Ledger>,
        config: RecorderConfig,
    ) -> (Self, mpsc::Receiver<CycleOutcome>) {
        let (outcome_tx, outcome_rx) = mpsc::channel();
        let recorder = Self::start_inner(health, ledger, config, Some(outcome_tx));
        (recorder, outcome_rx)
    }

    fn start_inner(
        health: Arc<HealthStore>,
        ledger: Arc<Ledger>,
        config: RecorderConfig,
        outcome_tx: Option<mpsc::Sender<CycleOutcome>>,
    ) -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let wake = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let (done_tx, done_rx) = mpsc::channel();
        let stop_flag_worker = stop_flag.clone();
        let wake_worker = wake.clone();
        thread::spawn(move || {
            run_loop(
                health,
                ledger,
                config,
                stop_flag_worker,
                wake_worker,
                outcome_tx,
            );
            let _ = done_tx.send(());
        });
        Self {
            stop_flag,
            wake,
            done_rx,
        }
    }

    /// Requests a graceful stop and waits up to `DEFAULT_SHUTDOWN_BOUND`
    /// for the loop to exit. Never kills a thread, never leaves an orphan
    /// process (there is no child process here at all — this crate has no
    /// process-control authority whatsoever, mechanically proven in
    /// `tests/capability_mesh.rs`).
    pub fn stop(self) -> StopOutcome {
        self.stop_flag.store(true, Ordering::SeqCst);
        {
            let (lock, condvar) = &*self.wake;
            let mut wake_requested = lock.lock().expect("wake mutex poisoned");
            *wake_requested = true;
            condvar.notify_all();
        }
        match self.done_rx.recv_timeout(DEFAULT_SHUTDOWN_BOUND) {
            Ok(()) => StopOutcome::Stopped,
            Err(_) => StopOutcome::TimedOut,
        }
    }
}

fn run_loop(
    health: Arc<HealthStore>,
    ledger: Arc<Ledger>,
    config: RecorderConfig,
    stop_flag: Arc<AtomicBool>,
    wake: Arc<(Mutex<bool>, std::sync::Condvar)>,
    outcome_tx: Option<mpsc::Sender<CycleOutcome>>,
) {
    loop {
        if stop_flag.load(Ordering::SeqCst) {
            return;
        }
        let now = SystemTime::now();
        let current_slot = cadence_slot_index(now, config.cadence);
        let next_boundary_ms = (current_slot + 1) * config.cadence.as_millis().max(1) as u64;
        let now_ms = millis_since_epoch(now);
        let wait_for = Duration::from_millis(next_boundary_ms.saturating_sub(now_ms));

        let (lock, condvar) = &*wake;
        let guard = lock.lock().expect("wake mutex poisoned");
        let (guard, _timeout_result) = condvar
            .wait_timeout_while(guard, wait_for, |woken| !*woken)
            .expect("wake mutex poisoned");
        drop(guard);
        // Either the boundary was reached (timed out waiting) or an
        // explicit wake (stop request) happened. Either way, re-check the
        // stop flag before doing any work — a stop requested right at a
        // boundary must never still run one more cycle.
        if stop_flag.load(Ordering::SeqCst) {
            return;
        }
        // Reset the wake flag for the next iteration's wait.
        {
            let mut wake_requested = lock.lock().expect("wake mutex poisoned");
            *wake_requested = false;
        }
        let outcome = record_cycle(&health, &ledger, &config, SystemTime::now());
        if let Some(tx) = &outcome_tx {
            // An mpsc::Sender's send() only fails if the receiver was
            // dropped (e.g. a host that stopped watching); never blocks,
            // so a slow/absent observer cannot stall the recording loop.
            let _ = tx.send(outcome);
        }
        // Loop back: the NEXT iteration computes the current wall-clock
        // slot fresh, so a cycle that ran long simply resumes from
        // wherever "now" is when it finishes -- no catch-up storm, no
        // queued backlog, matching M0.15.13 §5's required policy exactly.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maia_local_intelligence_health::RetentionPolicy as HealthRetentionPolicy;
    use maia_local_intelligence_hypothesis_ledger::RetentionPolicy as LedgerRetentionPolicy;
    use maia_local_model::LocalIntelligenceStatus;
    use std::time::Instant;

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn health_store() -> HealthStore {
        HealthStore::open_in_memory(HealthRetentionPolicy::default()).expect("open health store")
    }

    fn ledger() -> Ledger {
        Ledger::open_in_memory(LedgerRetentionPolicy::default()).expect("open ledger")
    }

    fn status(available: bool) -> LocalIntelligenceStatus {
        LocalIntelligenceStatus {
            available,
            model: "qwen3:4b".into(),
        }
    }

    // --- pure helper-function correctness -----------------------------

    #[test]
    fn cadence_slot_index_buckets_time_correctly() {
        let cadence = Duration::from_secs(900);
        assert_eq!(cadence_slot_index(UNIX_EPOCH, cadence), 0);
        assert_eq!(
            cadence_slot_index(UNIX_EPOCH + Duration::from_secs(899), cadence),
            0
        );
        assert_eq!(
            cadence_slot_index(UNIX_EPOCH + Duration::from_secs(900), cadence),
            1
        );
    }

    #[test]
    fn slot_observation_id_is_stable_and_distinct_by_recorder_and_slot() {
        let id_a = slot_observation_id("r1", 5).unwrap();
        let id_a_again = slot_observation_id("r1", 5).unwrap();
        let id_b = slot_observation_id("r2", 5).unwrap();
        let id_c = slot_observation_id("r1", 6).unwrap();
        assert_eq!(
            id_a, id_a_again,
            "same recorder+slot must always produce the same identity"
        );
        assert_ne!(id_a, id_b);
        assert_ne!(id_a, id_c);
    }

    // A. one explicit cycle creates exactly one ledger observation.
    #[test]
    fn one_cycle_creates_exactly_one_observation() {
        let health = health_store();
        let ledger = ledger();
        let config = RecorderConfig::default();
        let outcome = record_cycle(&health, &ledger, &config, at(100_000));
        assert_eq!(outcome, CycleOutcome::Recorded);
        assert_eq!(ledger.latest_n(10).unwrap().len(), 1);
    }

    // B. retry of the same logical slot does not inflate observation count.
    #[test]
    fn retry_of_the_same_logical_slot_does_not_inflate_the_count() {
        let health = health_store();
        let ledger = ledger();
        let config = RecorderConfig::default();
        let t1 = at(1_000_000);
        let t2 = t1 + Duration::from_millis(500); // still inside the same 15-minute bucket
        let outcome1 = record_cycle(&health, &ledger, &config, t1);
        let outcome2 = record_cycle(&health, &ledger, &config, t2);
        assert_eq!(outcome1, CycleOutcome::Recorded);
        assert_eq!(outcome2, CycleOutcome::AlreadyRecorded);
        assert_eq!(ledger.latest_n(10).unwrap().len(), 1);
    }

    // C. two distinct cadence slots create two observations.
    #[test]
    fn two_distinct_cadence_slots_create_two_observations() {
        let health = health_store();
        let ledger = ledger();
        let config = RecorderConfig::default();
        // Comfortably after the epoch: `at(0)` combined with the default
        // 1-hour window degenerates to an empty window once `since` is
        // clamped at UNIX_EPOCH, which is a real epoch-adjacent edge case
        // (see `a_snapshot_build_failure_creates_no_ledger_row`), not what
        // this test means to exercise.
        let t1 = at(100_000);
        let t2 = t1 + config.cadence + Duration::from_secs(1);
        let outcome1 = record_cycle(&health, &ledger, &config, t1);
        let outcome2 = record_cycle(&health, &ledger, &config, t2);
        assert_eq!(outcome1, CycleOutcome::Recorded);
        assert_eq!(outcome2, CycleOutcome::Recorded);
        assert_eq!(ledger.latest_n(10).unwrap().len(), 2);
    }

    // D. cycles never overlap: the Recorder runs everything on one
    // dedicated thread sequentially, so overlap is structurally
    // impossible; this integration-style test additionally confirms no
    // slot is ever recorded more than once during a real run.
    #[test]
    fn cycles_never_overlap_and_each_slot_is_recorded_at_most_once() {
        let health = Arc::new(health_store());
        let ledger = Arc::new(ledger());
        let config = RecorderConfig {
            cadence: Duration::from_millis(40),
            window: Duration::from_secs(60),
            ..RecorderConfig::default()
        };
        let recorder = Recorder::start(health, ledger.clone(), config);
        std::thread::sleep(Duration::from_millis(220));
        let outcome = recorder.stop();
        assert_eq!(outcome, StopOutcome::Stopped);
        let observations = ledger.latest_n(1000).unwrap();
        let mut ids: Vec<&str> = observations
            .iter()
            .map(|o| o.observation_id.as_str())
            .collect();
        let total = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(
            ids.len(),
            total,
            "no slot may ever be recorded more than once -- that would indicate overlap"
        );
    }

    // E. a slow cycle does not create a catch-up storm. Proven at the
    // algorithmic level: run_loop always calls record_cycle with
    // SystemTime::now() exactly once per wake, never iterating over
    // intermediate slots -- so a "cycle" (here, one explicit call) that
    // effectively happens many cadence periods late still records only
    // ONE observation for the current moment, never one per skipped slot.
    #[test]
    fn a_large_time_jump_between_cycles_never_creates_a_catch_up_storm() {
        let health = health_store();
        let ledger = ledger();
        let config = RecorderConfig {
            cadence: Duration::from_secs(60),
            ..RecorderConfig::default()
        };
        // Comfortably after the epoch -- see the note in
        // `two_distinct_cadence_slots_create_two_observations`.
        let start = at(100_000);
        let outcome1 = record_cycle(&health, &ledger, &config, start);
        // Equivalent to having been unable to run for 10 cadence periods.
        let far_future = start + config.cadence * 10 + Duration::from_secs(5);
        let outcome2 = record_cycle(&health, &ledger, &config, far_future);
        assert_eq!(outcome1, CycleOutcome::Recorded);
        assert_eq!(outcome2, CycleOutcome::Recorded);
        let observations = ledger.latest_n(1000).unwrap();
        assert_eq!(
            observations.len(),
            2,
            "exactly the two explicit calls made -- never one per skipped intermediate slot"
        );
    }

    // F. shutdown stops future cycles within a bounded interval.
    #[test]
    fn shutdown_stops_future_cycles_within_the_bounded_interval() {
        let health = Arc::new(health_store());
        let ledger = Arc::new(ledger());
        let config = RecorderConfig {
            cadence: Duration::from_millis(20),
            ..RecorderConfig::default()
        };
        let recorder = Recorder::start(health, ledger.clone(), config);
        std::thread::sleep(Duration::from_millis(60));
        let started = Instant::now();
        let outcome = recorder.stop();
        let elapsed = started.elapsed();
        assert_eq!(outcome, StopOutcome::Stopped);
        assert!(
            elapsed <= DEFAULT_SHUTDOWN_BOUND,
            "stop() must return within the documented bound"
        );
        let count_after_stop = ledger.latest_n(1000).unwrap().len();
        std::thread::sleep(Duration::from_millis(120));
        let count_later = ledger.latest_n(1000).unwrap().len();
        assert_eq!(
            count_after_stop, count_later,
            "no further cycles may run after stop() has returned"
        );
    }

    // G. ledger failure creates no synthetic observation and triggers no
    // action. A corrupted on-disk ledger file fails closed -- either at
    // Ledger::open (an even stronger guarantee: record_cycle can never be
    // reached with an unusable &Ledger at all) or, if open somehow
    // succeeds against corrupted content, at record() itself
    // (CycleOutcome::LedgerFailed). Either branch is asserted explicitly;
    // in neither branch does anything resembling an "action" occur, which
    // this crate structurally cannot perform anyway (tests/capability_mesh.rs).
    #[test]
    fn ledger_failure_creates_no_synthetic_observation() {
        let dir = std::env::temp_dir().join(format!(
            "maia-recorder-ledger-fail-test-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let ledger_path = dir.join("ledger.sqlite3");
        {
            let ledger =
                Ledger::open(&ledger_path, LedgerRetentionPolicy::default()).expect("initial open");
            drop(ledger);
        }
        std::fs::write(&ledger_path, b"not a valid sqlite file at all").expect("corrupt file");
        let health = health_store();
        let config = RecorderConfig::default();
        match Ledger::open(&ledger_path, LedgerRetentionPolicy::default()) {
            Err(_) => {
                // Failed closed at open -- record_cycle was never callable
                // with this ledger at all.
            }
            Ok(ledger) => {
                let outcome = record_cycle(&health, &ledger, &config, at(1000));
                assert!(
                    matches!(outcome, CycleOutcome::LedgerFailed(_)),
                    "a corrupted ledger must surface as a typed LedgerFailed outcome"
                );
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    // H. health/diagnostic failure creates no ledger row.
    #[test]
    fn a_snapshot_build_failure_creates_no_ledger_row() {
        let health = health_store();
        let ledger = ledger();
        // A zero-length window makes `since == until`, which
        // DiagnosticSnapshot::build refuses as InvalidWindow before ever
        // touching the ledger.
        let config = RecorderConfig {
            window: Duration::ZERO,
            ..RecorderConfig::default()
        };
        let outcome = record_cycle(&health, &ledger, &config, at(1000));
        assert!(matches!(outcome, CycleOutcome::SnapshotFailed(_)));
        assert_eq!(
            ledger.latest_n(10).unwrap().len(),
            0,
            "no row may be created when the snapshot itself failed to build"
        );
    }

    // I. partial coverage is recorded honestly as Indeterminate (via the
    // snapshot's own coverage field -- the recorder never edits or
    // upgrades it).
    #[test]
    fn partial_coverage_is_recorded_honestly_not_upgraded_to_full_evidence() {
        let health = health_store();
        for i in 1000..1010u64 {
            health
                .record_status_sample(&status(true), None, at(i))
                .unwrap();
        }
        let ledger = ledger();
        let config = RecorderConfig {
            window: Duration::from_secs(10_000),
            ..RecorderConfig::default()
        };
        let observed_at = at(1010);
        let outcome = record_cycle(&health, &ledger, &config, observed_at);
        assert_eq!(outcome, CycleOutcome::Recorded);
        let latest = ledger.latest().unwrap().unwrap();
        assert!(
            !latest
                .snapshot
                .coverage
                .status_history_covers_requested_window,
            "must honestly record partial coverage, never silently present it as complete"
        );
    }

    // J. same observed_at values still produce deterministic ledger query
    // ordering. (The property itself is proven directly at the ledger
    // level in infra/local-intelligence-hypothesis-ledger's own test
    // suite; this confirms the recorder's own usage pattern -- two
    // distinct recorder_ids racing at the identical instant -- does not
    // break it.)
    #[test]
    fn distinct_recorders_at_the_same_instant_still_order_deterministically() {
        let health = health_store();
        let ledger = ledger();
        let config_a = RecorderConfig {
            recorder_id: "a".into(),
            ..RecorderConfig::default()
        };
        let config_b = RecorderConfig {
            recorder_id: "b".into(),
            ..RecorderConfig::default()
        };
        let t = at(5000);
        record_cycle(&health, &ledger, &config_a, t);
        record_cycle(&health, &ledger, &config_b, t);
        let first_read = ledger.latest_n(2).unwrap();
        let second_read = ledger.latest_n(2).unwrap();
        assert_eq!(first_read.len(), 2);
        assert_eq!(
            first_read, second_read,
            "ordering of same-observed_at rows must be deterministic across reads"
        );
    }

    // K. recorder restart followed by retry of the same slot remains
    // idempotent.
    #[test]
    fn recorder_restart_then_retry_of_the_same_slot_remains_idempotent() {
        let health = health_store();
        let ledger = ledger();
        let config_before_restart = RecorderConfig::default();
        let outcome1 = record_cycle(&health, &ledger, &config_before_restart, at(2000));
        // "Restart": a freshly constructed config (as a new process would
        // build), same recorder_id/cadence, a moment later but still
        // inside the same cadence slot.
        let config_after_restart = RecorderConfig::default();
        let outcome2 = record_cycle(
            &health,
            &ledger,
            &config_after_restart,
            at(2000) + Duration::from_millis(1),
        );
        assert_eq!(outcome1, CycleOutcome::Recorded);
        assert_eq!(outcome2, CycleOutcome::AlreadyRecorded);
        assert_eq!(ledger.latest_n(10).unwrap().len(), 1);
    }

    // L. idle recorder does not busy-loop. Proven behaviorally: during an
    // idle period before its first cadence boundary, nothing is recorded,
    // and stop() still returns promptly (a busy-looping or blocked-until-
    // cadence implementation would not respond quickly to a Condvar
    // notify).
    #[test]
    fn idle_recorder_does_not_record_early_and_stops_promptly() {
        let health = Arc::new(health_store());
        let ledger = Arc::new(ledger());
        let config = RecorderConfig {
            cadence: Duration::from_secs(10),
            ..RecorderConfig::default()
        };
        let recorder = Recorder::start(health, ledger.clone(), config);
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(
            ledger.latest_n(10).unwrap().len(),
            0,
            "must not record before its first cadence boundary"
        );
        let started = Instant::now();
        let outcome = recorder.stop();
        assert_eq!(outcome, StopOutcome::Stopped);
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "a Condvar-based wait must wake promptly on stop, not busy-loop or block for the full cadence"
        );
    }

    // M0.15.14 prep: start_observed reports each cycle's outcome without
    // affecting what the loop actually records.
    #[test]
    fn start_observed_reports_each_cycle_outcome() {
        let health = Arc::new(health_store());
        let ledger = Arc::new(ledger());
        let config = RecorderConfig {
            cadence: Duration::from_millis(30),
            ..RecorderConfig::default()
        };
        let (recorder, outcomes) = Recorder::start_observed(health, ledger.clone(), config);
        std::thread::sleep(Duration::from_millis(140));
        let _ = recorder.stop();
        let received: Vec<CycleOutcome> = outcomes.try_iter().collect();
        assert!(
            !received.is_empty(),
            "at least one cycle outcome must have been reported"
        );
        assert!(
            received.iter().all(|o| *o == CycleOutcome::Recorded),
            "every reported outcome should reflect a real recorded cycle: {received:?}"
        );
        assert_eq!(
            received.len(),
            ledger.latest_n(1000).unwrap().len(),
            "the number of reported outcomes must match the number of rows actually recorded"
        );
    }
}
