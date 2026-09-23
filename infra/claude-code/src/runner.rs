//! Process invocation port.
//!
//! The port itself is always compiled so deterministic tests can inject a fake.
//! The real subprocess implementation is compiled only under the
//! `development-evolution` feature, so a DEPLOYMENT_LOCKED build contains no
//! process-spawning code for this capability at all.
//!
//! # Boundedness (M0.15.7e.1, corrected by M0.15.7e.2)
//!
//! The first live run hung for over fifteen minutes although the invocation had a
//! 180 second timeout. The runner drained the child's stdout and stderr through
//! pipes on reader threads and joined those threads after the child exited or
//! was killed. A descendant that inherited the pipe handles keeps them open, so
//! the join waited for the descendant, not for the deadline; and the capture was
//! unbounded. [`StdProcessRunner`] no longer has that shape:
//!
//! * **No output pipes, no reader threads.** stdout and stderr go to two scratch
//!   files. Nothing waits for end-of-file, so a descendant holding the handles
//!   cannot delay the call.
//! * **Bounded capture.** The files are polled while the child runs and the child
//!   is killed as soon as either exceeds its cap; at most `cap + 1` bytes are ever
//!   read back. Overflow is [`ProcessError::OutputTooLarge`], never a truncation
//!   presented as a result and never unbounded memory.
//! * Stdin is a pipe written by a detached thread that is never joined, so a child
//!   (or descendant) that does not read it cannot hold the caller either.
//!
//! ## One absolute invocation deadline (M0.15.7e.2)
//!
//! The deadline is fixed ONCE, at the outer caller boundary, before the worker
//! thread exists: `deadline = now + Invocation::timeout`. That same absolute
//! instant is handed to the worker; the worker never starts a clock of its own.
//! `Invocation::timeout` is the maximum time in which a SUCCESS may be produced.
//!
//! * A [`ProcessOutcome`] is a success only if it was finalized strictly before the
//!   deadline. That covers child completion, the stdout and stderr read-back and
//!   the UTF-8 decoding. If any of it finishes at or after the deadline the result
//!   is a timeout, even when the child exited with code 0 and produced valid
//!   output. The caller applies the same rule once more to whatever the worker
//!   sends, so a success that reaches it after the deadline is still a timeout.
//! * `HARD_GRACE` extends only how long the caller waits for a timeout, failure or
//!   cleanup to settle. It never converts a late success into a success.
//! * When the caller stops waiting (deadline + grace) it raises a cancellation
//!   latch private to that invocation (no process-global state) and returns a
//!   timeout. A worker that has not spawned yet then never spawns; a worker that
//!   has just spawned observes the latch before it sends any stdin, stops the child
//!   best effort and exits. A late worker result can never replace the timeout that
//!   was already returned. There is no retry.
//! * The caller itself never kills or waits for the child: cleanup belongs to the
//!   worker, so it cannot hold the orchestrator.
//!
//! ## Stdin delivery gate (M0.15.7e.3, linearized by M0.15.7e.4)
//!
//! The prompt is written by a detached thread that is never joined. That thread
//! shares the SAME absolute deadline and the SAME per-invocation state as the worker
//! and the caller (no process-global state, no clock of its own). The worker also
//! re-checks right before handing the handle over.
//!
//! Delivery permission is a protocol over a four-state, lock-free gate:
//! `PENDING -> CLAIMED -> STARTED`, with `PENDING | CLAIMED -> ABANDONED` on terminal
//! closure. CLAIMED means only "trying". The writer checks eligibility, claims,
//! re-validates the same deadline and abandonment state (so an expiry between the
//! check and the acquisition is caught), wins STARTED by compare-and-swap, and
//! re-validates once more before its first byte. A refused writer drops the handle and
//! writes ZERO prompt bytes. ABANDONED is never left; STARTED is reached at most once.
//!
//! Terminal closure happens on EVERY way an invocation ends: the worker closes it as
//! soon as it has any terminal result (timeout, success, output overflow, I/O failure,
//! invalid UTF-8, ...), and `run_bounded` closes it on every way out (worker result,
//! `settle` conversion, outer timeout, error). A writer paused anywhere before STARTED
//! therefore cannot obtain permission afterwards. Closure never touches a delivery that
//! already STARTED and never blocks, joins or waits.
//!
//! The guarantee, precisely: a writer that has not reached a valid STARTED before the
//! invocation's terminal closure never begins prompt delivery afterwards. The
//! linearization point of permission is the successful `CLAIMED -> STARTED` swap,
//! confirmed by the eligibility check that follows it. Safe Rust cannot make a clock
//! read and an OS write syscall atomic: a writer that legitimately won STARTED while
//! the invocation was eligible may still be writing when the caller returns (a tiny
//! window after the last check is accepted), and stopping the child is then best
//! effort. The caller never joins the writer and nothing is held across the write, so
//! a writer blocked on a child that does not read cannot hold the caller past
//! deadline + grace.
//!
//! Likewise an atomic flag cannot prove that no OS process is ever created in the few
//! instructions between the last pre-spawn check and `Command::spawn`. A child
//! discovered after cancellation receives no stdin and is stopped best effort.
//!
//! Strictly UTF-8: stdout is required textual output and is validated as UTF-8 from
//! the original bytes ([`ProcessError::InvalidUtf8`]); it is never repaired with
//! replacement characters. Standard error is diagnostic only and is decoded
//! lossily for the audit record.
//!
//! No shell is involved and no element is ever concatenated into a command string.
//!
//! BOUNDED: caller waiting (deadline + grace), captured memory (`cap + 1` bytes per
//! stream), and success eligibility (nothing after the deadline is a success).
//!
//! NOT YET FULLY BOUNDED, stated plainly: descendant lifetime and, with it, the
//! total scratch disk a surviving descendant could still write. Killing the
//! immediate child does not kill its descendants (that needs job objects or process
//! groups, which need `unsafe`, forbidden in this crate). A leftover descendant
//! cannot delay the caller or grow its memory; it can outlive the call. This is not
//! process-tree containment.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

/// Default cap on captured standard output. The Claude Code JSON envelope is far
/// smaller; anything larger is treated as a failure, not stored.
pub const DEFAULT_MAX_STDOUT_BYTES: usize = 1024 * 1024;
/// Default cap on captured standard error.
pub const DEFAULT_MAX_STDERR_BYTES: usize = 256 * 1024;

/// A fully-resolved child process invocation. No shell is involved and no
/// element is ever concatenated into a command string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub working_directory: PathBuf,
    /// Variables removed from the inherited environment (credential scrub).
    pub env_remove: Vec<String>,
    /// Variables explicitly set on the child (recursion marker).
    pub env_set: BTreeMap<String, String>,
    pub stdin_payload: String,
    /// The absolute success deadline, measured from the moment `run` is called:
    /// spawn, exit, capture and decoding must all finish inside it.
    pub timeout: Duration,
    /// Directory that holds the two short-lived capture files. Created if missing.
    pub scratch_directory: PathBuf,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutcome {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
    pub timed_out: bool,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessError {
    ExecutableNotFound,
    SpawnFailed,
    IoFailure,
    /// The child wrote more than its cap to this stream and was killed.
    OutputTooLarge(OutputStream),
    /// The scratch directory or capture files could not be created or read, or a
    /// capture file name was already taken (the existing file is never touched).
    ScratchFailed,
    /// The child exited in time but this stream is not valid UTF-8. The original
    /// bytes are never repaired with replacement characters and called valid.
    InvalidUtf8(OutputStream),
}

/// Cooperative cancellation handle shared with a caller.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    flag: Arc<std::sync::atomic::AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.flag.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(std::sync::atomic::Ordering::SeqCst)
    }
}

pub trait ProcessRunner: Send + Sync {
    fn run(
        &self,
        invocation: &Invocation,
        cancel: &CancellationToken,
    ) -> Result<ProcessOutcome, ProcessError>;
}

#[cfg(feature = "development-evolution")]
pub use real::StdProcessRunner;

#[cfg(feature = "development-evolution")]
mod real {
    use super::*;
    use std::{
        io::{Read, Write},
        path::Path,
        process::{Child, Command, Stdio},
        sync::{
            atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
            mpsc,
        },
        time::Instant,
    };

    /// How long after the deadline the caller still waits for the worker to settle a
    /// timeout, failure or cleanup. It never makes a late success a success.
    const HARD_GRACE: Duration = Duration::from_millis(1500);
    /// How often the worker checks the child, the capture files and cancellation.
    const POLL: Duration = Duration::from_millis(10);
    /// After a kill, how long to wait for the child to be reaped.
    const REAP_BOUND: Duration = Duration::from_secs(1);
    /// Longest timeout honoured; keeps instant arithmetic far from overflow.
    const MAX_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);

    /// Real subprocess runner. Present only in a DEVELOPMENT_EVOLUTION build.
    pub struct StdProcessRunner;

    impl ProcessRunner for StdProcessRunner {
        fn run(
            &self,
            invocation: &Invocation,
            cancel: &CancellationToken,
        ) -> Result<ProcessOutcome, ProcessError> {
            run_bounded(invocation, cancel, HARD_GRACE, &Seams::default())
        }
    }

    /// The one absolute invocation deadline, fixed by the outer caller before the
    /// worker exists and shared with it. Nothing else starts a timeout clock.
    #[derive(Debug, Clone, Copy)]
    struct Deadline {
        start: Instant,
        at: Instant,
    }

    impl Deadline {
        fn starting_now(timeout: Duration) -> Self {
            let start = Instant::now();
            Self {
                start,
                at: start + timeout.min(MAX_TIMEOUT),
            }
        }
        fn expired(&self) -> bool {
            Instant::now() >= self.at
        }
    }

    /// No writer has tried to acquire delivery permission yet.
    const DELIVERY_PENDING: u8 = 0;
    /// A writer is ATTEMPTING to acquire permission. It has not won it and may not
    /// write: it must still re-validate eligibility, then move to STARTED.
    const DELIVERY_CLAIMED: u8 = 1;
    /// Delivery permission was won while the invocation was still eligible.
    const DELIVERY_STARTED: u8 = 2;
    /// Terminal: the invocation is over (or was found ineligible) before permission
    /// was won. Nothing ever leaves this state.
    const DELIVERY_ABANDONED: u8 = 3;

    #[derive(Debug, Default)]
    struct AbandonedState {
        raised: AtomicBool,
        /// The stdin delivery gate:
        /// `PENDING -> CLAIMED -> STARTED`, and `PENDING | CLAIMED -> ABANDONED`
        /// (plus the writer's own `STARTED -> ABANDONED` when its last check fails).
        /// ABANDONED is never left; STARTED is reached at most once.
        delivery: AtomicU8,
    }

    /// State private to one invocation (no process-global state), shared by the
    /// caller, the worker and the detached stdin writer. It carries the terminal
    /// closure of the invocation and the stdin delivery gate.
    #[derive(Debug, Clone, Default)]
    struct Abandoned(Arc<AbandonedState>);

    impl Abandoned {
        /// Terminal closure: the invocation is over (timeout, failure or success) or
        /// the caller stopped waiting. Idempotent, lock-free, never blocks.
        /// `PENDING | CLAIMED -> ABANDONED`, so a writer paused anywhere before
        /// STARTED can never obtain permission afterwards. A delivery that already
        /// STARTED is left alone (it is in flight; the worker stops the child).
        fn close(&self) {
            self.0.raised.store(true, Ordering::SeqCst);
            let _ = self
                .0
                .delivery
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |state| match state {
                    DELIVERY_PENDING | DELIVERY_CLAIMED => Some(DELIVERY_ABANDONED),
                    _ => None,
                });
        }
        fn is_raised(&self) -> bool {
            self.0.raised.load(Ordering::SeqCst)
        }
        fn transition(&self, from: u8, to: u8) -> bool {
            self.0
                .delivery
                .compare_exchange(from, to, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
        }
        /// `PENDING -> CLAIMED`. Grants nothing: only the right to try.
        fn claim(&self) -> bool {
            self.transition(DELIVERY_PENDING, DELIVERY_CLAIMED)
        }
        /// `CLAIMED -> STARTED`. Fails if the invocation was closed in the meantime
        /// (state ABANDONED) and can succeed at most once.
        fn start(&self) -> bool {
            self.transition(DELIVERY_CLAIMED, DELIVERY_STARTED)
        }
        /// The writer's own exit when its last eligibility check failed.
        fn abandon_started(&self) {
            let _ = self.transition(DELIVERY_STARTED, DELIVERY_ABANDONED);
        }
    }

    /// Closes the invocation on EVERY way out of a scope, so no return path has to
    /// remember to.
    struct CloseOnExit(Abandoned);

    impl Drop for CloseOnExit {
        fn drop(&mut self) {
            self.0.close();
        }
    }

    fn ineligible(abandoned: &Abandoned, deadline: &Deadline) -> bool {
        abandoned.is_raised() || deadline.expired()
    }

    /// The detached stdin writer's whole job. Delivery permission is a protocol, not
    /// a single check:
    ///
    /// 1. cheap eligibility check (same absolute deadline, same abandonment state);
    /// 2. `PENDING -> CLAIMED`: the right to try, not to write;
    /// 3. re-validate, so an expiry or closure between 1 and 2 is caught;
    /// 4. `CLAIMED -> STARTED`: permission is won. Terminal closure can no longer
    ///    turn it back; it can only have prevented it;
    /// 5. re-validate once more before the first byte.
    ///
    /// A writer refused at any step drops the handle having written ZERO bytes.
    /// `sink` is dropped on return, which closes the pipe and signals end of prompt.
    fn deliver_stdin<W: Write>(
        mut sink: W,
        payload: &[u8],
        abandoned: &Abandoned,
        deadline: &Deadline,
        seams: &Seams,
    ) {
        seams.reach(Phase::WriterEntry);
        if ineligible(abandoned, deadline) {
            abandoned.close();
            return;
        }
        seams.reach(Phase::BeforeClaim);
        if !abandoned.claim() {
            return;
        }
        seams.reach(Phase::Claimed);
        if ineligible(abandoned, deadline) {
            abandoned.close();
            return;
        }
        if !abandoned.start() {
            return;
        }
        seams.reach(Phase::Started);
        if ineligible(abandoned, deadline) {
            abandoned.abandon_started();
            return;
        }
        seams.reach(Phase::Delivering);
        // Permission was won while the invocation was eligible, so this delivery is
        // legitimate. It may block (a child that does not read); only this detached
        // thread waits, never the caller. Nothing is held across the write.
        let _ = sink.write_all(payload);
    }

    /// Points in the worker where deterministic tests may pause it. Production code
    /// installs nothing, so these compile to nothing outside tests.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Phase {
        /// Scratch files created, child not yet spawned.
        BeforeSpawn,
        /// Child just spawned.
        AfterSpawn,
        /// The stdin handle is taken; the worker is about to re-check and hand it to
        /// the writer. Not reached if cancellation was seen right after the spawn.
        BeforeStdin,
        /// The detached stdin writer has started running, before its first check.
        WriterEntry,
        /// The writer passed its first eligibility check; the `PENDING -> CLAIMED`
        /// acquisition is next (the exact window of the M0.15.7e.4 finding).
        BeforeClaim,
        /// The writer holds CLAIMED: permission is not won yet.
        Claimed,
        /// The writer won STARTED; its last eligibility check is next.
        Started,
        /// The last check passed; the prompt is about to be written.
        Delivering,
        /// Top of every poll iteration.
        PollTick,
        /// Output read back and decoded; the final deadline check is next.
        Decoded,
        /// The worker is about to send its result to the caller.
        BeforeSend,
    }

    #[cfg(test)]
    type PauseHook = Arc<dyn Fn(Phase) + Send + Sync>;
    #[cfg(test)]
    type NameHook = Arc<dyn Fn(&str) -> String + Send + Sync>;
    #[cfg(test)]
    type StateSlot = Arc<std::sync::Mutex<Option<Abandoned>>>;

    #[derive(Clone, Default)]
    struct Seams {
        #[cfg(test)]
        pause: Option<PauseHook>,
        #[cfg(test)]
        scratch_name: Option<NameHook>,
        /// Receives a handle on the invocation's own state, so a test can look at
        /// the delivery gate after `run_bounded` has returned.
        #[cfg(test)]
        capture: Option<StateSlot>,
    }

    impl Seams {
        fn reach(&self, _phase: Phase) {
            #[cfg(test)]
            if let Some(pause) = &self.pause {
                pause(_phase);
            }
        }
        fn observe(&self, _abandoned: &Abandoned) {
            #[cfg(test)]
            if let Some(slot) = &self.capture {
                *slot.lock().unwrap() = Some(_abandoned.clone());
            }
        }
        fn scratch_name(&self, kind: &str) -> String {
            #[cfg(test)]
            if let Some(name) = &self.scratch_name {
                return name(kind);
            }
            scratch_name(kind)
        }
    }

    fn timed_out_outcome(deadline: &Deadline, stdout: String, stderr: String) -> ProcessOutcome {
        ProcessOutcome {
            exit_code: None,
            stdout,
            stderr,
            duration: deadline.start.elapsed(),
            timed_out: true,
            cancelled: false,
        }
    }

    /// The outer caller boundary. Fixes the deadline, runs the worker, waits at most
    /// deadline + `grace`, and never lets a success through after the deadline.
    fn run_bounded(
        invocation: &Invocation,
        cancel: &CancellationToken,
        grace: Duration,
        seams: &Seams,
    ) -> Result<ProcessOutcome, ProcessError> {
        let deadline = Deadline::starting_now(invocation.timeout);
        let abandoned = Abandoned::default();
        seams.observe(&abandoned);
        // Whatever this function returns (a worker result, a `settle` conversion, the
        // outer timeout or an error), the invocation is closed first: no writer that
        // is still pending or claimed can obtain delivery permission afterwards.
        let _close = CloseOnExit(abandoned.clone());
        let (tx, rx) = mpsc::channel();
        let (worker_invocation, worker_cancel, worker_abandoned, worker_seams) = (
            invocation.clone(),
            cancel.clone(),
            abandoned.clone(),
            seams.clone(),
        );
        std::thread::Builder::new()
            .name("maia-process-worker".into())
            .spawn(move || {
                let result = run_inner(
                    &worker_invocation,
                    &worker_cancel,
                    deadline,
                    &worker_abandoned,
                    &worker_seams,
                );
                // The worker closes the invocation itself the moment it has a terminal
                // result, before the caller has even received it.
                worker_abandoned.close();
                worker_seams.reach(Phase::BeforeSend);
                let _ = tx.send(result);
            })
            .map_err(|_| ProcessError::IoFailure)?;

        let wait = deadline.at.saturating_duration_since(Instant::now()) + grace;
        match rx.recv_timeout(wait) {
            Ok(result) => settle(result, &deadline),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Close and return (`_close` also closes on the way out). The worker,
                // not this thread, stops any child, so cleanup cannot hold the caller.
                abandoned.close();
                Ok(timed_out_outcome(&deadline, String::new(), String::new()))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(ProcessError::IoFailure),
        }
    }

    /// A success that reaches the caller at or after the deadline is a timeout.
    fn settle(
        result: Result<ProcessOutcome, ProcessError>,
        deadline: &Deadline,
    ) -> Result<ProcessOutcome, ProcessError> {
        match result {
            Ok(outcome) if !outcome.timed_out && !outcome.cancelled && deadline.expired() => {
                Ok(timed_out_outcome(deadline, outcome.stdout, outcome.stderr))
            }
            other => other,
        }
    }

    /// The capture files this invocation created, removed when this is dropped. A
    /// path is recorded only AFTER `create_new` succeeded, so a name that was
    /// already taken is never deleted or modified by this invocation.
    #[derive(Default)]
    struct Scratch {
        owned: Vec<PathBuf>,
    }

    impl Scratch {
        fn create(
            &mut self,
            directory: &Path,
            kind: &str,
            seams: &Seams,
        ) -> Result<(PathBuf, std::fs::File), ProcessError> {
            let path = directory.join(seams.scratch_name(kind));
            let file = create_new(&path)?;
            self.owned.push(path.clone());
            Ok((path, file))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            // Best effort: a descendant may still hold a handle open.
            for path in &self.owned {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    /// The spawned child. Dropping it stops the child best effort, so no early
    /// return can leave one running.
    struct Running(Child);

    impl Running {
        fn kill_and_reap(&mut self) {
            let started = Instant::now();
            loop {
                let _ = self.0.kill();
                match self.0.try_wait() {
                    Ok(Some(_)) | Err(_) => return,
                    Ok(None) => {}
                }
                if started.elapsed() >= REAP_BOUND {
                    return;
                }
                std::thread::sleep(POLL);
            }
        }
    }

    impl Drop for Running {
        fn drop(&mut self) {
            let _ = self.0.kill();
        }
    }

    fn scratch_name(kind: &str) -> String {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        // A leading dot and no `.json`: never mistaken for an audit record or a
        // Round Table session.
        format!(
            ".maia-capture-{}-{nanos}-{}.{kind}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst)
        )
    }

    /// Fails, touching nothing, if the path already exists.
    fn create_new(path: &Path) -> Result<std::fs::File, ProcessError> {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_| ProcessError::ScratchFailed)
    }

    fn size_of(path: &Path) -> u64 {
        std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
    }

    /// At most `cap + 1` bytes, so an overflow is detected and never buffered.
    fn read_capped(path: &Path, cap: usize) -> Result<Vec<u8>, ProcessError> {
        let file = std::fs::File::open(path).map_err(|_| ProcessError::ScratchFailed)?;
        let mut buf = Vec::new();
        file.take(cap as u64 + 1)
            .read_to_end(&mut buf)
            .map_err(|_| ProcessError::ScratchFailed)?;
        Ok(buf)
    }

    /// What was captured so far, for the audit record of a stopped child. Bounded,
    /// best effort, never an error: a timeout is reported as a timeout. Lossy on
    /// purpose: this is a record of a failure, never a validated result.
    fn partial(path: &Path, cap: usize) -> String {
        let mut bytes = read_capped(path, cap).unwrap_or_default();
        bytes.truncate(cap);
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn run_inner(
        invocation: &Invocation,
        cancel: &CancellationToken,
        deadline: Deadline,
        abandoned: &Abandoned,
        seams: &Seams,
    ) -> Result<ProcessOutcome, ProcessError> {
        let too_late = || abandoned.is_raised() || deadline.expired();
        let nothing = || timed_out_outcome(&deadline, String::new(), String::new());

        if too_late() {
            return Ok(nothing());
        }
        std::fs::create_dir_all(&invocation.scratch_directory)
            .map_err(|_| ProcessError::ScratchFailed)?;
        let mut scratch = Scratch::default();
        let (out_path, out_file) = scratch.create(&invocation.scratch_directory, "out", seams)?;
        let (err_path, err_file) = scratch.create(&invocation.scratch_directory, "err", seams)?;

        let mut command = Command::new(&invocation.program);
        command
            .args(&invocation.args)
            .current_dir(&invocation.working_directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::from(out_file))
            .stderr(Stdio::from(err_file));
        for key in &invocation.env_remove {
            command.env_remove(key);
        }
        for (key, value) in &invocation.env_set {
            command.env(key, value);
        }

        seams.reach(Phase::BeforeSpawn);
        // The last check, as close to `spawn` as practical: the caller may have given
        // up while this worker was delayed, and then no consultation may begin. An
        // atomic flag cannot prove that no OS process is ever created in the few
        // instructions between this check and the spawn; what is guaranteed is that a
        // child discovered afterwards receives no prompt and is stopped (below).
        if too_late() {
            return Ok(nothing());
        }
        let child = command.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ProcessError::ExecutableNotFound,
            _ => ProcessError::SpawnFailed,
        })?;
        // Release our copies of the capture handles now that the child has its own.
        drop(command);
        let mut running = Running(child);

        seams.reach(Phase::AfterSpawn);
        // Cancellation is observed BEFORE anything is sent: an abandoned worker
        // that just spawned stops the child (on drop) and sends no prompt.
        if too_late() {
            running.kill_and_reap();
            return Ok(nothing());
        }

        // Deliver the prompt on a detached thread that is never joined: a child
        // that does not read it, or a descendant holding the read end, cannot hold
        // the caller. The writer shares the invocation's deadline and abandonment
        // state and must obtain delivery permission itself, so a writer that only
        // gets to run after the caller returned a timeout writes nothing.
        let stdin = running.0.stdin.take().ok_or(ProcessError::IoFailure)?;
        seams.reach(Phase::BeforeStdin);
        if too_late() {
            running.kill_and_reap();
            return Ok(nothing());
        }
        let payload = invocation.stdin_payload.clone().into_bytes();
        let (writer_abandoned, writer_seams) = (abandoned.clone(), seams.clone());
        std::thread::Builder::new()
            .name("maia-stdin-writer".into())
            .spawn(move || {
                deliver_stdin(stdin, &payload, &writer_abandoned, &deadline, &writer_seams)
            })
            .map_err(|_| ProcessError::IoFailure)?;

        let stopped = |running: &mut Running, timed_out: bool, cancelled: bool| {
            running.kill_and_reap();
            Ok(ProcessOutcome {
                exit_code: None,
                stdout: partial(&out_path, invocation.max_stdout_bytes),
                stderr: partial(&err_path, invocation.max_stderr_bytes),
                duration: deadline.start.elapsed(),
                timed_out,
                cancelled,
            })
        };

        let exit_code = loop {
            seams.reach(Phase::PollTick);
            match running.0.try_wait() {
                Ok(Some(status)) => break status.code(),
                Ok(None) => {}
                Err(_) => return Err(ProcessError::IoFailure),
            }
            if size_of(&out_path) > invocation.max_stdout_bytes as u64 {
                running.kill_and_reap();
                return Err(ProcessError::OutputTooLarge(OutputStream::Stdout));
            }
            if size_of(&err_path) > invocation.max_stderr_bytes as u64 {
                running.kill_and_reap();
                return Err(ProcessError::OutputTooLarge(OutputStream::Stderr));
            }
            if abandoned.is_raised() || deadline.expired() {
                return stopped(&mut running, true, false);
            }
            if cancel.is_cancelled() {
                return stopped(&mut running, false, true);
            }
            std::thread::sleep(POLL);
        };

        // The child exited. If that was observed after the deadline it is a
        // timeout, whatever the exit code and whatever it wrote.
        if too_late() {
            return stopped(&mut running, true, false);
        }

        // Read back at most cap + 1 bytes; a descendant that still holds the
        // handles cannot delay this, because nothing waits for end-of-file.
        let stdout = read_capped(&out_path, invocation.max_stdout_bytes)?;
        if stdout.len() > invocation.max_stdout_bytes {
            return Err(ProcessError::OutputTooLarge(OutputStream::Stdout));
        }
        let stderr = read_capped(&err_path, invocation.max_stderr_bytes)?;
        if stderr.len() > invocation.max_stderr_bytes {
            return Err(ProcessError::OutputTooLarge(OutputStream::Stderr));
        }
        // Strict for the required textual output; never repaired and called valid.
        let stdout = String::from_utf8(stdout)
            .map_err(|_| ProcessError::InvalidUtf8(OutputStream::Stdout))?;
        // Diagnostic only: decoded lossily for the audit record, never parsed.
        let stderr = String::from_utf8_lossy(&stderr).into_owned();

        seams.reach(Phase::Decoded);
        // Every success-critical step is done; it only counts if it was done in time.
        if too_late() {
            return Ok(timed_out_outcome(&deadline, stdout, stderr));
        }
        Ok(ProcessOutcome {
            exit_code,
            stdout,
            stderr,
            duration: deadline.start.elapsed(),
            timed_out: false,
            cancelled: false,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::Mutex;

        /// The lib test binary is re-run as a harmless helper child: a real
        /// subprocess that never touches Claude, a model, the network or a shell.
        /// Without the variable this is an ordinary, instantly passing test.
        #[test]
        fn helper_child() {
            let Ok(mode) = std::env::var("MAIA_RUNNER_UNIT_MODE") else {
                return;
            };
            match mode.as_str() {
                "ok" => println!("unit-ok"),
                "utf8" => println!("zażółć gęślą jaźń"),
                "bad_utf8" => {
                    let _ = std::io::stdout().write_all(&[b'v', 0xff, 0xfe, b'\n']);
                }
                "bad_utf8_stderr" => {
                    println!("unit-ok");
                    let _ = std::io::stderr().write_all(&[b'w', 0xff, b'\n']);
                }
                "hang" => std::thread::sleep(Duration::from_secs(8)),
                // Appends to the marker file for every stdin byte it receives, so the
                // file exists only if some prompt byte reached the child.
                "stdin_stream" => {
                    let mut buf = [0u8; 4096];
                    let mut stdin = std::io::stdin();
                    while let Ok(n) = stdin.read(&mut buf) {
                        if n == 0 {
                            break;
                        }
                        if let Ok(path) = std::env::var("MAIA_RUNNER_UNIT_MARKER") {
                            if let Ok(mut f) = std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(path)
                            {
                                let _ = writeln!(f, "got {n}");
                            }
                        }
                    }
                }
                // Reports the length and an FNV-1a hash of everything on stdin.
                "stdin_sum" => {
                    let mut input = Vec::new();
                    let _ = std::io::stdin().read_to_end(&mut input);
                    println!("stdin_len={} fnv={}", input.len(), fnv1a(&input));
                }
                // Writes the marker file at once, proving it ran.
                "marker" => write_marker(),
                // Writes the marker only if it is still alive after two seconds.
                "marker_late" => {
                    std::thread::sleep(Duration::from_secs(2));
                    write_marker();
                }
                _ => {}
            }
        }

        fn fnv1a(bytes: &[u8]) -> u64 {
            bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
                (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
            })
        }

        fn write_marker() {
            if let Ok(path) = std::env::var("MAIA_RUNNER_UNIT_MARKER") {
                let _ = std::fs::write(path, b"ran");
            }
        }

        fn scratch(name: &str) -> PathBuf {
            let dir = std::env::temp_dir()
                .join(format!("maia-runner-unit-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            dir
        }

        fn invocation(mode: &str, dir: &Path, timeout: Duration) -> Invocation {
            let mut env_set = BTreeMap::new();
            env_set.insert("MAIA_RUNNER_UNIT_MODE".to_owned(), mode.to_owned());
            env_set.insert(
                "MAIA_RUNNER_UNIT_MARKER".to_owned(),
                dir.join("marker").display().to_string(),
            );
            Invocation {
                program: std::env::current_exe().unwrap(),
                args: [
                    "--exact",
                    "runner::real::tests::helper_child",
                    "--nocapture",
                    "--test-threads=1",
                ]
                .map(str::to_owned)
                .to_vec(),
                working_directory: std::env::temp_dir(),
                env_remove: vec![],
                env_set,
                stdin_payload: String::new(),
                timeout,
                scratch_directory: dir.join("scratch"),
                max_stdout_bytes: 4096,
                max_stderr_bytes: 4096,
            }
        }

        fn listing(dir: &Path) -> Vec<String> {
            std::fs::read_dir(dir)
                .map(|it| {
                    it.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                        .collect()
                })
                .unwrap_or_default()
        }

        /// Records every phase reached and optionally sleeps at one of them, the
        /// first time only.
        fn seams(log: &Arc<Mutex<Vec<Phase>>>, pause_at: Option<(Phase, Duration)>) -> Seams {
            let log = Arc::clone(log);
            let fired = Arc::new(AtomicBool::new(false));
            Seams {
                pause: Some(Arc::new(move |phase| {
                    log.lock().unwrap().push(phase);
                    if let Some((at, how_long)) = pause_at {
                        if phase == at && !fired.swap(true, Ordering::SeqCst) {
                            std::thread::sleep(how_long);
                        }
                    }
                })),
                scratch_name: None,
                capture: None,
            }
        }

        /// The same seams, additionally handing the test the invocation's own state.
        fn capturing(mut s: Seams, slot: &StateSlot) -> Seams {
            s.capture = Some(Arc::clone(slot));
            s
        }

        fn gate(slot: &StateSlot) -> u8 {
            slot.lock()
                .unwrap()
                .as_ref()
                .expect("run_bounded captured its state")
                .0
                .delivery
                .load(Ordering::SeqCst)
        }

        fn phases(log: &Arc<Mutex<Vec<Phase>>>) -> Vec<Phase> {
            log.lock().unwrap().clone()
        }

        const TIMEOUT: Duration = Duration::from_secs(2);
        /// Wide enough that the caller does not give up in the "late worker" cases.
        const WIDE_GRACE: Duration = Duration::from_secs(4);

        fn assert_timeout_not_success(out: &ProcessOutcome, took: Duration) {
            assert!(out.timed_out, "must be a timeout: {out:?}");
            assert!(!out.cancelled);
            assert_eq!(out.exit_code, None, "a late exit code 0 is not a success");
            assert!(took >= TIMEOUT, "returned before the deadline: {took:?}");
        }

        /// A. The child exits (code 0, valid output) but is observed only AFTER the
        /// deadline and BEFORE the grace expires. Timeout, never Success.
        #[test]
        fn a_child_observed_exiting_after_the_deadline_is_a_timeout_not_a_success() {
            let dir = scratch("a");
            let log = Arc::new(Mutex::new(Vec::new()));
            // The first poll waits past the deadline; by then the child has exited.
            let s = seams(
                &log,
                Some((Phase::PollTick, TIMEOUT + Duration::from_millis(400))),
            );
            let started = Instant::now();
            let out = run_bounded(
                &invocation("ok", &dir, TIMEOUT),
                &CancellationToken::new(),
                WIDE_GRACE,
                &s,
            )
            .expect("a timeout is an outcome");
            assert_timeout_not_success(&out, started.elapsed());
            assert!(
                out.stdout.contains("unit-ok"),
                "the valid output existed and was still not a success: {out:?}"
            );
            assert!(started.elapsed() < TIMEOUT + WIDE_GRACE);
            assert!(
                !phases(&log).contains(&Phase::Decoded),
                "no success processing"
            );
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// B. The output is available in time, but the final success processing
        /// finishes after the deadline. Timeout.
        #[test]
        fn success_processing_that_finishes_after_the_deadline_is_a_timeout() {
            let dir = scratch("b");
            let log = Arc::new(Mutex::new(Vec::new()));
            let s = seams(
                &log,
                Some((Phase::Decoded, TIMEOUT + Duration::from_millis(300))),
            );
            let started = Instant::now();
            let out = run_bounded(
                &invocation("ok", &dir, TIMEOUT),
                &CancellationToken::new(),
                WIDE_GRACE,
                &s,
            )
            .expect("a timeout is an outcome");
            assert!(
                phases(&log).contains(&Phase::Decoded),
                "the child was too slow on this machine; the case was not exercised"
            );
            assert_timeout_not_success(&out, started.elapsed());
            assert!(out.stdout.contains("unit-ok"), "provenance kept: {out:?}");
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// C. The worker finalizes in time but its result reaches the caller during
        /// the grace, after the deadline. It stays a timeout.
        #[test]
        fn a_worker_result_arriving_during_the_grace_stays_a_timeout() {
            let dir = scratch("c");
            let log = Arc::new(Mutex::new(Vec::new()));
            // Pause AFTER the worker's own check passed: only the caller can catch it.
            // The result is decided at BeforeSend, so sleep there past the deadline.
            let s = seams(&log, Some((Phase::BeforeSend, TIMEOUT)));
            let started = Instant::now();
            let out = run_bounded(
                &invocation("ok", &dir, Duration::from_millis(1500)),
                &CancellationToken::new(),
                WIDE_GRACE,
                &s,
            )
            .expect("a timeout is an outcome");
            let took = started.elapsed();
            assert!(phases(&log).contains(&Phase::BeforeSend));
            assert!(out.timed_out && out.exit_code.is_none(), "{out:?}");
            assert!(took >= Duration::from_millis(1500));
            assert!(took < Duration::from_millis(1500) + WIDE_GRACE, "{took:?}");
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// The grace itself does not make a success later than the deadline: a
        /// success before the deadline is still a success.
        #[test]
        fn a_success_inside_the_deadline_is_still_a_success() {
            let dir = scratch("in-time");
            let log = Arc::new(Mutex::new(Vec::new()));
            let out = run_bounded(
                &invocation("ok", &dir, Duration::from_secs(20)),
                &CancellationToken::new(),
                WIDE_GRACE,
                &seams(&log, None),
            )
            .unwrap();
            assert!(!out.timed_out && !out.cancelled);
            assert_eq!(out.exit_code, Some(0));
            assert!(out.stdout.contains("unit-ok"));
            assert_eq!(listing(&dir.join("scratch")), Vec::<String>::new());
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// D. The caller times out while the worker is delayed before spawning. The
        /// worker must then never start the consultation.
        #[test]
        fn a_delayed_worker_that_finds_the_caller_gone_never_spawns() {
            let dir = scratch("d");
            let log = Arc::new(Mutex::new(Vec::new()));
            let timeout = Duration::from_millis(200);
            let grace = Duration::from_millis(300);
            // Wakes well after the caller has returned.
            let s = seams(
                &log,
                Some((
                    Phase::BeforeSpawn,
                    timeout + grace + Duration::from_millis(1000),
                )),
            );
            let started = Instant::now();
            let out = run_bounded(
                &invocation("marker", &dir, timeout),
                &CancellationToken::new(),
                grace,
                &s,
            )
            .expect("a timeout is an outcome");
            assert!(out.timed_out && out.exit_code.is_none());
            assert!(started.elapsed() < timeout + grace + Duration::from_millis(700));

            // Let the delayed worker wake up and finish.
            std::thread::sleep(Duration::from_millis(2500));
            assert_eq!(
                phases(&log),
                vec![Phase::BeforeSpawn, Phase::BeforeSend],
                "it must not spawn: nothing between BeforeSpawn and the (discarded) send"
            );
            assert!(
                !dir.join("marker").exists(),
                "the child must never have run"
            );
            assert_eq!(
                listing(&dir.join("scratch")),
                Vec::<String>::new(),
                "the worker cleaned its capture files"
            );
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// D2. The child has just been spawned when the caller gives up. The worker
        /// sees the latch before sending stdin, and stops the child.
        #[test]
        fn a_worker_that_spawned_as_the_caller_left_stops_before_stdin() {
            let dir = scratch("d2");
            let log = Arc::new(Mutex::new(Vec::new()));
            let timeout = Duration::from_millis(200);
            let grace = Duration::from_millis(300);
            let s = seams(
                &log,
                Some((
                    Phase::AfterSpawn,
                    timeout + grace + Duration::from_millis(400),
                )),
            );
            let mut inv = invocation("marker_late", &dir, timeout);
            inv.stdin_payload = "must-not-be-sent".into();
            let out = run_bounded(&inv, &CancellationToken::new(), grace, &s)
                .expect("a timeout is an outcome");
            assert!(out.timed_out && out.exit_code.is_none());

            std::thread::sleep(Duration::from_millis(3200));
            let seen = phases(&log);
            assert!(seen.contains(&Phase::AfterSpawn));
            assert!(
                !seen.contains(&Phase::BeforeStdin),
                "stdin must not be sent: {seen:?}"
            );
            assert!(
                !dir.join("marker").exists(),
                "the child was stopped, not left to finish"
            );
            let _ = std::fs::remove_dir_all(&dir);
        }

        fn colliding(kind_to_collide: &'static str, name: &'static str) -> Seams {
            Seams {
                pause: None,
                scratch_name: Some(Arc::new(move |kind| {
                    if kind == kind_to_collide {
                        name.to_owned()
                    } else {
                        scratch_name(kind)
                    }
                })),
                capture: None,
            }
        }

        /// E. A capture name that already exists is never deleted or modified.
        #[test]
        fn a_scratch_name_collision_leaves_the_existing_file_byte_for_byte() {
            for (collide, other) in [("out", "err"), ("err", "out")] {
                let dir = scratch(&format!("e-{collide}"));
                let scratch_dir = dir.join("scratch");
                std::fs::create_dir_all(&scratch_dir).unwrap();
                let name = if collide == "out" {
                    ".taken.out"
                } else {
                    ".taken.err"
                };
                let existing = scratch_dir.join(name);
                let bytes: Vec<u8> = vec![0, 1, 2, 0xff, b'x', b'\n', 0x80];
                std::fs::write(&existing, &bytes).unwrap();

                let got = run_bounded(
                    &invocation("marker", &dir, Duration::from_secs(5)),
                    &CancellationToken::new(),
                    WIDE_GRACE,
                    &colliding(collide, name),
                );
                assert_eq!(got, Err(ProcessError::ScratchFailed), "{collide}");
                assert_eq!(
                    std::fs::read(&existing).unwrap(),
                    bytes,
                    "{collide}: untouched"
                );
                assert!(!dir.join("marker").exists(), "no child was started");
                // Whatever this invocation did create ({other}) is removed again.
                assert_eq!(
                    listing(&scratch_dir),
                    vec![name.to_owned()],
                    "{collide}/{other}"
                );
                let _ = std::fs::remove_dir_all(&dir);
            }
        }

        /// F. Invalid UTF-8 on stdout is rejected, not repaired into a success.
        #[test]
        fn invalid_utf8_on_stdout_is_rejected_not_lossily_accepted() {
            let dir = scratch("f");
            let got = StdProcessRunner.run(
                &invocation("bad_utf8", &dir, Duration::from_secs(20)),
                &CancellationToken::new(),
            );
            assert_eq!(got, Err(ProcessError::InvalidUtf8(OutputStream::Stdout)));
            assert_eq!(listing(&dir.join("scratch")), Vec::<String>::new());
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn valid_multibyte_utf8_is_accepted_and_stderr_junk_is_diagnostic_only() {
            let dir = scratch("f2");
            let out = StdProcessRunner
                .run(
                    &invocation("utf8", &dir, Duration::from_secs(20)),
                    &CancellationToken::new(),
                )
                .unwrap();
            assert!(out.stdout.contains("zażółć gęślą jaźń"), "{out:?}");

            let out = StdProcessRunner
                .run(
                    &invocation("bad_utf8_stderr", &dir, Duration::from_secs(20)),
                    &CancellationToken::new(),
                )
                .unwrap();
            assert!(out.stdout.contains("unit-ok"));
            assert!(out.stderr.contains('\u{FFFD}'), "stderr is lossy by design");
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn a_huge_timeout_does_not_overflow_and_a_zero_timeout_never_spawns_a_success() {
            let dir = scratch("edge");
            let mut inv = invocation("ok", &dir, Duration::MAX);
            let out = StdProcessRunner
                .run(&inv, &CancellationToken::new())
                .unwrap();
            assert!(!out.timed_out && out.exit_code == Some(0));
            inv.timeout = Duration::ZERO;
            let out = StdProcessRunner
                .run(&inv, &CancellationToken::new())
                .unwrap();
            assert!(out.timed_out && out.exit_code.is_none());
            let _ = std::fs::remove_dir_all(&dir);
        }

        // ------------------------------------------- stdin delivery (M0.15.7e.3)

        fn count(log: &Arc<Mutex<Vec<Phase>>>, phase: Phase) -> usize {
            phases(log).iter().filter(|p| **p == phase).count()
        }

        /// A sink that records every byte it is given.
        #[derive(Clone, Default)]
        struct RecordingSink(Arc<Mutex<Vec<u8>>>);

        impl Write for RecordingSink {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        const PAYLOAD: &[u8] = b"consultation prompt";

        fn live() -> Deadline {
            Deadline::starting_now(Duration::from_secs(60))
        }

        fn state_of(a: &Abandoned) -> u8 {
            a.0.delivery.load(Ordering::SeqCst)
        }

        /// A `Seams` whose pause hook runs `act` the first time `at` is reached.
        fn acting_at(
            log: &Arc<Mutex<Vec<Phase>>>,
            at: Phase,
            act: impl Fn() + Send + Sync + 'static,
        ) -> Seams {
            let log = Arc::clone(log);
            let fired = Arc::new(AtomicBool::new(false));
            Seams {
                pause: Some(Arc::new(move |phase| {
                    log.lock().unwrap().push(phase);
                    if phase == at && !fired.swap(true, Ordering::SeqCst) {
                        act();
                    }
                })),
                scratch_name: None,
                capture: None,
            }
        }

        /// The state machine, transition by transition.
        #[test]
        fn the_gate_transitions_are_exactly_the_documented_ones() {
            // PENDING -> CLAIMED -> STARTED, once.
            let g = Abandoned::default();
            assert_eq!(state_of(&g), DELIVERY_PENDING);
            assert!(!g.start(), "STARTED cannot be won without claiming first");
            assert!(g.claim());
            assert_eq!(state_of(&g), DELIVERY_CLAIMED);
            assert!(!g.claim(), "no second claim");
            assert!(g.start());
            assert_eq!(state_of(&g), DELIVERY_STARTED);
            assert!(!g.start() && !g.claim(), "no second STARTED, no re-claim");
            g.close();
            assert_eq!(
                state_of(&g),
                DELIVERY_STARTED,
                "closure leaves a started delivery"
            );

            // PENDING -> ABANDONED, and it is final.
            let g = Abandoned::default();
            g.close();
            assert_eq!(state_of(&g), DELIVERY_ABANDONED);
            assert!(!g.claim() && !g.start());
            assert_eq!(state_of(&g), DELIVERY_ABANDONED);

            // CLAIMED -> ABANDONED: a claimed writer cannot then win STARTED.
            let g = Abandoned::default();
            assert!(g.claim());
            g.close();
            assert_eq!(state_of(&g), DELIVERY_ABANDONED);
            assert!(!g.start(), "CLAIMED -> STARTED must fail after closure");
            assert!(!g.claim());
            assert_eq!(state_of(&g), DELIVERY_ABANDONED);
        }

        /// F. Exactly one delivery: from any number of racing writers, exactly one can
        /// obtain STARTED.
        #[test]
        fn exactly_one_writer_can_ever_obtain_started() {
            for _ in 0..300 {
                let g = Abandoned::default();
                let wins = Arc::new(AtomicU64::new(0));
                let handles: Vec<_> = (0..8)
                    .map(|_| {
                        let (g, wins) = (g.clone(), Arc::clone(&wins));
                        std::thread::spawn(move || {
                            if g.claim() && g.start() {
                                wins.fetch_add(1, Ordering::SeqCst);
                            }
                        })
                    })
                    .collect();
                for h in handles {
                    h.join().unwrap();
                }
                assert_eq!(wins.load(Ordering::SeqCst), 1);
                assert_eq!(state_of(&g), DELIVERY_STARTED);
            }
        }

        /// E + F. In time, the writer delivers the exact payload; a second writer on
        /// the same gate writes nothing.
        #[test]
        fn a_permitted_writer_delivers_the_exact_payload_and_a_second_one_writes_nothing() {
            let g = Abandoned::default();
            let sink = RecordingSink::default();
            deliver_stdin(sink.clone(), PAYLOAD, &g, &live(), &Seams::default());
            assert_eq!(*sink.0.lock().unwrap(), PAYLOAD.to_vec());
            assert_eq!(state_of(&g), DELIVERY_STARTED);

            let again = RecordingSink::default();
            deliver_stdin(again.clone(), PAYLOAD, &g, &live(), &Seams::default());
            assert!(again.0.lock().unwrap().is_empty(), "no second delivery");
        }

        /// A writer that runs after terminal closure, or after the deadline, writes
        /// zero bytes.
        #[test]
        fn a_writer_that_runs_after_closure_or_after_the_deadline_writes_zero_bytes() {
            let g = Abandoned::default();
            g.close();
            let sink = RecordingSink::default();
            deliver_stdin(sink.clone(), PAYLOAD, &g, &live(), &Seams::default());
            assert!(sink.0.lock().unwrap().is_empty());

            let g = Abandoned::default();
            let sink = RecordingSink::default();
            let expired = Deadline::starting_now(Duration::ZERO);
            deliver_stdin(sink.clone(), PAYLOAD, &g, &expired, &Seams::default());
            assert!(sink.0.lock().unwrap().is_empty());
            assert_eq!(state_of(&g), DELIVERY_ABANDONED, "and it closed the gate");
        }

        /// A. THE M0.15.7e.4 RACE. The writer passes its first eligibility check and
        /// is suspended BEFORE the PENDING -> CLAIMED acquisition. The invocation
        /// then ends (terminal closure). Resumed, the writer must write ZERO bytes
        /// and never reach CLAIMED, STARTED or Delivering.
        #[test]
        fn a_writer_suspended_between_its_check_and_the_acquisition_writes_zero_bytes() {
            let g = Abandoned::default();
            let log = Arc::new(Mutex::new(Vec::new()));
            let closer = g.clone();
            // "Terminal Timeout while the writer is suspended": the caller closes.
            let seams = acting_at(&log, Phase::BeforeClaim, move || closer.close());
            let sink = RecordingSink::default();
            deliver_stdin(sink.clone(), PAYLOAD, &g, &live(), &seams);

            assert!(sink.0.lock().unwrap().is_empty(), "zero prompt bytes");
            assert_eq!(
                count(&log, Phase::BeforeClaim),
                1,
                "it did reach the window"
            );
            for never in [Phase::Claimed, Phase::Started, Phase::Delivering] {
                assert_eq!(count(&log, never), 0, "{never:?} must not be reached");
            }
            assert_eq!(state_of(&g), DELIVERY_ABANDONED);
        }

        /// A2. The same window, but nothing closes the gate: only the DEADLINE passes
        /// while the writer is suspended. The re-validation after the claim must catch
        /// it, so STARTED is never won.
        #[test]
        fn a_writer_whose_deadline_expires_between_check_and_claim_never_reaches_started() {
            let g = Abandoned::default();
            let log = Arc::new(Mutex::new(Vec::new()));
            let deadline = Deadline::starting_now(Duration::from_millis(150));
            let seams = acting_at(&log, Phase::BeforeClaim, || {
                std::thread::sleep(Duration::from_millis(300))
            });
            let sink = RecordingSink::default();
            deliver_stdin(sink.clone(), PAYLOAD, &g, &deadline, &seams);

            assert!(sink.0.lock().unwrap().is_empty(), "zero prompt bytes");
            assert_eq!(count(&log, Phase::Claimed), 1, "it did win the claim");
            assert_eq!(count(&log, Phase::Started), 0, "but never STARTED");
            assert_eq!(count(&log, Phase::Delivering), 0);
            assert_eq!(state_of(&g), DELIVERY_ABANDONED);
        }

        /// B. The writer holds CLAIMED when the terminal event happens. Closure turns
        /// CLAIMED into ABANDONED, so the resumed writer cannot obtain STARTED.
        #[test]
        fn a_writer_suspended_while_claimed_cannot_obtain_started_after_closure() {
            let g = Abandoned::default();
            let log = Arc::new(Mutex::new(Vec::new()));
            let closer = g.clone();
            let seen_in_window = Arc::new(AtomicU64::new(0));
            let probe = (g.clone(), Arc::clone(&seen_in_window));
            let seams = acting_at(&log, Phase::Claimed, move || {
                // Suspended holding CLAIMED; the invocation ends now.
                assert_eq!(state_of(&probe.0), DELIVERY_CLAIMED);
                closer.close();
                probe
                    .1
                    .store(u64::from(state_of(&probe.0)), Ordering::SeqCst);
            });
            let sink = RecordingSink::default();
            deliver_stdin(sink.clone(), PAYLOAD, &g, &live(), &seams);

            assert_eq!(
                seen_in_window.load(Ordering::SeqCst),
                u64::from(DELIVERY_ABANDONED),
                "closure changed CLAIMED -> ABANDONED"
            );
            assert!(sink.0.lock().unwrap().is_empty(), "zero prompt bytes");
            assert_eq!(count(&log, Phase::Started), 0);
            assert_eq!(count(&log, Phase::Delivering), 0);
            assert_eq!(state_of(&g), DELIVERY_ABANDONED, "and it never comes back");
        }

        /// The last check: closure that lands after STARTED but before the first byte
        /// still stops the write (a tiny window after it is the accepted residual).
        #[test]
        fn closure_between_started_and_the_first_byte_still_writes_nothing() {
            let g = Abandoned::default();
            let log = Arc::new(Mutex::new(Vec::new()));
            let closer = g.clone();
            let seams = acting_at(&log, Phase::Started, move || closer.close());
            let sink = RecordingSink::default();
            deliver_stdin(sink.clone(), PAYLOAD, &g, &live(), &seams);
            assert!(sink.0.lock().unwrap().is_empty());
            assert_eq!(count(&log, Phase::Delivering), 0);
        }

        /// C. A Timeout that arrives NORMALLY through `Ok(result)` (the worker's own
        /// timeout outcome, well inside the grace) closes a still-pending writer's
        /// gate before the caller returns. The writer is held before its first check
        /// for the whole test, so nothing but terminal closure can change the gate.
        #[test]
        fn a_worker_produced_timeout_closes_the_pending_delivery_before_the_caller_returns() {
            let dir = scratch("gate-c");
            let log = Arc::new(Mutex::new(Vec::new()));
            let slot: StateSlot = Arc::new(Mutex::new(None));
            let timeout = Duration::from_millis(300);
            let s = capturing(
                seams(&log, Some((Phase::WriterEntry, Duration::from_secs(2)))),
                &slot,
            );
            let mut inv = invocation("stdin_stream", &dir, timeout);
            inv.stdin_payload = "consultation prompt".repeat(50);
            let started = Instant::now();
            let out = run_bounded(&inv, &CancellationToken::new(), WIDE_GRACE, &s)
                .expect("a timeout is an outcome");
            assert!(out.timed_out && out.exit_code.is_none(), "{out:?}");
            assert!(
                started.elapsed() < WIDE_GRACE,
                "it came through Ok(result), not the outer timeout"
            );
            assert_eq!(count(&log, Phase::WriterEntry), 1);
            assert_eq!(
                gate(&slot),
                DELIVERY_ABANDONED,
                "closed at return, while the writer is still suspended"
            );
            std::thread::sleep(Duration::from_millis(2200));
            assert_eq!(count(&log, Phase::Claimed), 0);
            assert_eq!(count(&log, Phase::Delivering), 0);
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// D. A late success that `settle` converts to a Timeout also leaves the gate
        /// closed. (The worker finished in time and paused before sending; the
        /// deadline passes; the caller receives it inside the grace.)
        #[test]
        fn a_settle_produced_timeout_closes_the_pending_delivery_too() {
            let dir = scratch("gate-d");
            let log = Arc::new(Mutex::new(Vec::new()));
            let slot: StateSlot = Arc::new(Mutex::new(None));
            let timeout = Duration::from_millis(1500);
            // The writer is held before its first check for the whole test.
            let held = acting_at(&log, Phase::WriterEntry, || {
                std::thread::sleep(Duration::from_secs(4))
            });
            // Pause the worker after its terminal result, past the deadline.
            let mut s = capturing(held, &slot);
            let inner = s.pause.take().unwrap();
            s.pause = Some(Arc::new(move |phase| {
                inner(phase);
                if phase == Phase::BeforeSend {
                    std::thread::sleep(Duration::from_millis(1600));
                }
            }));
            let mut inv = invocation("ok", &dir, timeout);
            inv.stdin_payload = "consultation prompt".into();
            let out = run_bounded(&inv, &CancellationToken::new(), WIDE_GRACE, &s)
                .expect("a timeout is an outcome");
            assert!(out.timed_out && out.exit_code.is_none(), "{out:?}");
            assert_eq!(gate(&slot), DELIVERY_ABANDONED);
            assert_eq!(count(&log, Phase::Claimed), 0);
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// The stale-writer case on NON-timeout terminal results: an output overflow
        /// ends the invocation while the writer is still pending. It must be closed.
        #[test]
        fn a_non_timeout_terminal_failure_closes_the_pending_delivery_as_well() {
            let dir = scratch("gate-fail");
            let log = Arc::new(Mutex::new(Vec::new()));
            let slot: StateSlot = Arc::new(Mutex::new(None));
            let s = capturing(
                seams(&log, Some((Phase::WriterEntry, Duration::from_secs(3)))),
                &slot,
            );
            // Prints more than a 4-byte stdout cap allows.
            let mut inv = invocation("ok", &dir, Duration::from_secs(20));
            inv.max_stdout_bytes = 4;
            let got = run_bounded(&inv, &CancellationToken::new(), WIDE_GRACE, &s);
            assert_eq!(got, Err(ProcessError::OutputTooLarge(OutputStream::Stdout)));
            assert_eq!(gate(&slot), DELIVERY_ABANDONED);

            // And invalid UTF-8.
            let slot2: StateSlot = Arc::new(Mutex::new(None));
            let s = capturing(
                seams(&log, Some((Phase::WriterEntry, Duration::from_secs(3)))),
                &slot2,
            );
            let inv = invocation("bad_utf8", &dir, Duration::from_secs(20));
            let got = run_bounded(&inv, &CancellationToken::new(), WIDE_GRACE, &s);
            assert_eq!(got, Err(ProcessError::InvalidUtf8(OutputStream::Stdout)));
            assert_eq!(gate(&slot2), DELIVERY_ABANDONED);
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// The outer timeout (a stuck worker) closes it as well: the caller alone,
        /// with no help from the worker, leaves the gate closed.
        #[test]
        fn the_outer_timeout_closes_the_pending_delivery() {
            let dir = scratch("gate-outer");
            let log = Arc::new(Mutex::new(Vec::new()));
            let slot: StateSlot = Arc::new(Mutex::new(None));
            let timeout = Duration::from_millis(200);
            let grace = Duration::from_millis(300);
            // The worker is stuck before it can even spawn.
            let s = capturing(
                seams(&log, Some((Phase::BeforeSpawn, Duration::from_secs(2)))),
                &slot,
            );
            let out = run_bounded(
                &invocation("ok", &dir, timeout),
                &CancellationToken::new(),
                grace,
                &s,
            )
            .expect("a timeout is an outcome");
            assert!(out.timed_out);
            assert_eq!(gate(&slot), DELIVERY_ABANDONED);
            std::thread::sleep(Duration::from_millis(2200));
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// A. The worker is delayed immediately before the stdin handoff. The caller
        /// reaches Timeout and returns. The resumed worker must hand nothing over:
        /// the child receives ZERO consultation bytes.
        #[test]
        fn a_worker_delayed_before_the_stdin_handoff_delivers_nothing_after_the_timeout() {
            let dir = scratch("stdin-a");
            let log = Arc::new(Mutex::new(Vec::new()));
            let timeout = Duration::from_millis(300);
            let grace = Duration::from_millis(300);
            let s = seams(
                &log,
                Some((
                    Phase::BeforeStdin,
                    timeout + grace + Duration::from_millis(900),
                )),
            );
            let mut inv = invocation("stdin_stream", &dir, timeout);
            inv.stdin_payload = "consultation prompt".repeat(100);
            let started = Instant::now();
            let out = run_bounded(&inv, &CancellationToken::new(), grace, &s)
                .expect("a timeout is an outcome");
            assert!(out.timed_out && out.exit_code.is_none(), "{out:?}");
            assert!(started.elapsed() < timeout + grace + Duration::from_millis(600));

            std::thread::sleep(Duration::from_millis(2000));
            assert!(phases(&log).contains(&Phase::BeforeStdin));
            assert_eq!(count(&log, Phase::WriterEntry), 0, "no writer was created");
            assert_eq!(count(&log, Phase::Delivering), 0);
            assert!(!dir.join("marker").exists(), "the child got zero bytes");
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// B. The exact Codex failure: the writer exists but is held before its
        /// permission check. The caller returns Timeout and abandons. Released, the
        /// writer must write ZERO prompt bytes.
        #[test]
        fn a_stdin_writer_delayed_until_after_the_caller_timed_out_writes_zero_bytes() {
            let dir = scratch("stdin-b");
            let log = Arc::new(Mutex::new(Vec::new()));
            let timeout = Duration::from_millis(300);
            let grace = Duration::from_millis(300);
            let s = seams(
                &log,
                Some((
                    Phase::WriterEntry,
                    timeout + grace + Duration::from_millis(900),
                )),
            );
            let mut inv = invocation("stdin_stream", &dir, timeout);
            inv.stdin_payload = "consultation prompt".repeat(100);
            let started = Instant::now();
            let out = run_bounded(&inv, &CancellationToken::new(), grace, &s)
                .expect("a timeout is an outcome");
            assert!(out.timed_out && out.exit_code.is_none(), "{out:?}");
            assert!(started.elapsed() < timeout + grace + Duration::from_millis(600));

            std::thread::sleep(Duration::from_millis(2000));
            assert_eq!(count(&log, Phase::WriterEntry), 1, "the writer did run");
            assert_eq!(
                count(&log, Phase::Delivering),
                0,
                "but it was refused permission and wrote nothing"
            );
            assert!(!dir.join("marker").exists(), "the child got zero bytes");
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// B2. The writer is delayed past the DEADLINE while the caller is still
        /// inside its grace. The deadline alone refuses permission.
        #[test]
        fn a_stdin_writer_that_starts_after_the_deadline_is_refused_even_inside_the_grace() {
            let dir = scratch("stdin-b2");
            let log = Arc::new(Mutex::new(Vec::new()));
            let timeout = Duration::from_millis(400);
            let s = seams(
                &log,
                Some((Phase::WriterEntry, timeout + Duration::from_millis(300))),
            );
            let mut inv = invocation("stdin_stream", &dir, timeout);
            inv.stdin_payload = "consultation prompt".into();
            let out = run_bounded(&inv, &CancellationToken::new(), WIDE_GRACE, &s)
                .expect("a timeout is an outcome");
            assert!(out.timed_out && out.exit_code.is_none(), "{out:?}");
            std::thread::sleep(Duration::from_millis(500));
            assert_eq!(count(&log, Phase::WriterEntry), 1);
            assert_eq!(count(&log, Phase::Delivering), 0);
            assert!(!dir.join("marker").exists());
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// C. A permitted delivery still arrives byte for byte, exactly once.
        /// E. One writer, one delivery: no second attempt.
        #[test]
        fn a_permitted_delivery_arrives_byte_for_byte_and_exactly_once() {
            let dir = scratch("stdin-c");
            let log = Arc::new(Mutex::new(Vec::new()));
            let mut payload = String::from("zażółć gęślą jaźń \u{1F600}\r\n\t");
            payload.push_str(&"prompt ".repeat(20_000));
            let mut inv = invocation("stdin_sum", &dir, Duration::from_secs(20));
            inv.stdin_payload = payload.clone();
            let out = run_bounded(
                &inv,
                &CancellationToken::new(),
                WIDE_GRACE,
                &seams(&log, None),
            )
            .unwrap();
            let expected = format!(
                "stdin_len={} fnv={}",
                payload.len(),
                fnv1a(payload.as_bytes())
            );
            assert!(!out.timed_out && out.exit_code == Some(0), "{out:?}");
            assert!(out.stdout.contains(&expected), "{}", out.stdout);
            assert_eq!(count(&log, Phase::WriterEntry), 1);
            assert_eq!(count(&log, Phase::Delivering), 1);
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// D. Delivery legitimately started before the timeout; the child never
        /// reads, so the write blocks (payload far beyond the pipe buffer). The
        /// caller must still return inside deadline + grace.
        #[test]
        fn a_started_delivery_that_blocks_cannot_hold_the_caller_past_the_bound() {
            let dir = scratch("stdin-d");
            let log = Arc::new(Mutex::new(Vec::new()));
            let timeout = Duration::from_millis(600);
            let grace = Duration::from_millis(900);
            let mut inv = invocation("hang", &dir, timeout);
            inv.stdin_payload = "p".repeat(4 * 1024 * 1024);
            let started = Instant::now();
            let out = run_bounded(&inv, &CancellationToken::new(), grace, &seams(&log, None))
                .expect("a timeout is an outcome");
            let took = started.elapsed();
            assert!(out.timed_out && out.exit_code.is_none(), "{out:?}");
            assert_eq!(count(&log, Phase::Delivering), 1, "delivery did start");
            assert!(
                took < timeout + grace + Duration::from_millis(500),
                "took {took:?}"
            );
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// D2. The same, with the writer held INSIDE its permitted delivery by the
        /// test (a write that never returns): the caller is not held.
        #[test]
        fn a_writer_stuck_inside_a_permitted_delivery_does_not_hold_the_caller() {
            let dir = scratch("stdin-d2");
            let log = Arc::new(Mutex::new(Vec::new()));
            let timeout = Duration::from_millis(500);
            let grace = Duration::from_millis(800);
            let s = seams(&log, Some((Phase::Delivering, Duration::from_secs(5))));
            let mut inv = invocation("hang", &dir, timeout);
            inv.stdin_payload = "prompt".into();
            let started = Instant::now();
            let out = run_bounded(&inv, &CancellationToken::new(), grace, &s)
                .expect("a timeout is an outcome");
            assert!(out.timed_out && out.exit_code.is_none(), "{out:?}");
            assert!(started.elapsed() < timeout + grace + Duration::from_millis(500));
            assert_eq!(count(&log, Phase::Delivering), 1);
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
