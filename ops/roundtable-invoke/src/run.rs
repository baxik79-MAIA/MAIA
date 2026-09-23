//! The `run` pipeline (M0.15.7c), against whatever participants the caller builds.
//!
//! ```text
//! guards -> plan -> history checks -> typed confirmation -> history writable
//!        -> build participants -> realize_plan -> save -> read back -> render
//! ```
//!
//! ## The trusted construction boundary
//!
//! Participants are built by a closure that receives the [`Authorized`] token.
//! `execute_run` calls that closure only after the guards are clear, the plan is
//! printed, the history location and session id have been checked, the person has
//! typed the session id, and the history location has been shown writable. That
//! order is what is guaranteed, and it is pinned by an event-trace test.
//!
//! It is not a type guarantee. Rust would let a caller build a provider first and
//! capture it in the closure, so the closure is the *trusted construction
//! boundary*: whatever builds providers must do it inside the closure, and the
//! live builder (M0.15.7d) is reviewed as exactly that boundary. This module
//! never names a provider: tests pass fakes.
//!
//! ## History
//!
//! This is the only writer of Round Table history. A session that ends without a
//! decision is saved like any other, so a failed run is visible in the viewer. A
//! run that is refused or declined creates no history directory.
//!
//! After confirmation and before any participant exists, the history directory
//! is established and shown writable with a `create_new` probe file that is then
//! removed. The directory is *kept*: an empty history directory left by a later
//! failure is acceptable, whereas removing it would let the destination vanish
//! or change while model calls are running. If the probe file cannot be removed
//! the run fails closed, before any participant is built. Pre-existing
//! directories and files are never deleted, truncated or replaced.
//!
//! A session id that is already present is never overwritten: it is refused
//! before confirmation, and checked again immediately before the write. The
//! public `SessionStore` port has no create-only save, so those two checks are
//! not atomic against another writer using the same id in the gap; a create-only
//! save would need a store-contract change and is recorded as such, not invented
//! here.
//!
//! What is printed is rendered from the record read back through the same read
//! contract the viewer uses, with the same pure renderer, so the run's summary
//! and `maia-roundtable-viewer show` cannot say different things. If the read
//! back fails the output is rendered from the in-memory record instead and says
//! so; that is not a claim of correspondence with the stored file.

use crate::cli::{EXIT_OK, EXIT_REFUSED, Output};
use crate::gate::{Authorized, Env, Refusal, Terminal, authorize, check_guards};
use crate::plan::{MAX_OUTPUT_TOKENS, build_plan};
use crate::registry::LoadedRegistry;
use maia_domain::ReasoningAssuranceLevel;
use maia_roundtable::{
    Adjudicator, Clock, DecisionRequest, ParticipantAdjudicator, ParticipantResolver,
    ResolutionFailure, SessionRecord, SessionStore, SessionStoreError, resolve_verified,
    select_leader,
};
use maia_roundtable_composition::{leader_candidates, realize_plan};
use maia_roundtable_observability::render::render_panel;
use maia_roundtable_observability::{SessionPanel, load_panel, panel_from};
use maia_roundtable_store::{FileSessionStore, StoredSessionQuery};
use std::path::{Path, PathBuf};

/// The session ran and was saved, but it did not reach a decision.
pub const EXIT_SESSION_FAILED: i32 = 8;
/// The session ran but could not be saved. The result is still printed.
pub const EXIT_NOT_SAVED: i32 = 9;
/// Nothing ran: participants could not be constructed. Nothing was saved.
pub const EXIT_NOT_CONSTRUCTED: i32 = 10;
/// The history location cannot hold a session, or cannot be shown to be able to.
pub const EXIT_HISTORY_UNUSABLE: i32 = 11;
/// The session id is already in history. Nothing was constructed or overwritten.
pub const EXIT_SESSION_ID_IN_USE: i32 = 12;
/// The session was written, but reading it back to verify it failed.
pub const EXIT_READBACK_UNVERIFIED: i32 = 13;
/// A preflight check failed. No model was called and nothing was saved.
pub const EXIT_PREFLIGHT_FAILED: i32 = 14;

/// Live participants: a resolver and the adjudicator built on the leader.
pub struct Participants {
    pub resolver: Box<dyn ParticipantResolver>,
    pub adjudicator: Box<dyn Adjudicator>,
}

/// Why participants could not be built. Nothing has been called at this point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    /// This build has no live participant wiring (M0.15.7d is a separate boundary).
    NotWired,
    NoLeader,
    LeaderUnresolvable(ResolutionFailure),
    /// The host configuration cannot be used (executable, working directory,
    /// audit directory, version query). Found before any model call.
    HostUnusable(String),
}

impl BuildError {
    pub fn render(&self) -> String {
        match self {
            Self::NotWired => "not run: live participants are not wired in this build (it was \
                 built without the `development-evolution` feature). Nothing was constructed \
                 or called, and no session was saved.\n"
                .to_owned(),
            Self::HostUnusable(why) => format!(
                "not run: the live participants cannot be set up ({why}). No model was called \
                 and no session was saved.\n"
            ),
            Self::NoLeader => "not run: no leader could be chosen. Nothing was called or \
                 saved.\n"
                .to_owned(),
            Self::LeaderUnresolvable(why) => format!(
                "not run: the leader could not be constructed ({why:?}). Nothing was called or \
                 saved.\n"
            ),
        }
    }
}

/// Wrap a resolver with an adjudicator that asks the deterministically chosen
/// leader. The leader is resolved through `resolve_verified`, so a resolver that
/// substitutes another participant is a failure, not a success.
pub fn participants_with_leader(
    resolver: Box<dyn ParticipantResolver>,
    loaded: &LoadedRegistry,
    level: ReasoningAssuranceLevel,
) -> Result<Participants, BuildError> {
    let candidates = leader_candidates(&loaded.registry).map_err(|_| BuildError::NoLeader)?;
    let leader = select_leader(level, &candidates).map_err(|_| BuildError::NoLeader)?;
    let participant = resolve_verified(resolver.as_ref(), &leader.leader)
        .map_err(BuildError::LeaderUnresolvable)?;
    Ok(Participants {
        adjudicator: Box::new(ParticipantAdjudicator::new(participant, MAX_OUTPUT_TOKENS)),
        resolver,
    })
}

pub struct RunRequest<'a> {
    pub loaded: &'a LoadedRegistry,
    pub level: ReasoningAssuranceLevel,
    pub subject: &'a str,
    pub prompt: &'a str,
    pub history: &'a str,
    pub session_id: &'a str,
}

/// Whether a session id can be written without touching something that exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IdState {
    /// Nothing is stored under that id.
    Free,
    /// Something is stored under it (readable or not). Never overwritten.
    InUse,
    /// The location could not be read well enough to tell. Treated as unsafe.
    Unknown,
}

/// The parts of session history this pipeline touches, as a seam so the order
/// of calls and each failure branch can be driven deterministically.
pub(crate) trait History {
    /// A location that exists but cannot hold sessions. Checked before
    /// confirmation. A location that does not exist yet is fine.
    fn location_problem(&self) -> Option<String>;
    fn id_state(&self, session_id: &str) -> IdState;
    /// After confirmation, before any participant exists: establish the location
    /// and show it can be written. The directory is kept; no probe file remains.
    fn verify_writable(&self) -> Result<(), String>;
    fn save(&self, record: &SessionRecord) -> Result<(), SessionStoreError>;
    fn read_back(&self, session_id: &str) -> Option<SessionPanel>;
}

/// History in a directory, through the same store and query the viewer uses.
pub(crate) struct FileHistory {
    dir: PathBuf,
    /// How the probe file is removed. A field only so a test can make removal
    /// fail; production always uses `std::fs::remove_file`.
    remove_probe: fn(&Path) -> std::io::Result<()>,
}

impl FileHistory {
    pub(crate) fn new(dir: &str) -> Self {
        Self {
            dir: PathBuf::from(dir),
            remove_probe: remove_probe_file,
        }
    }
}

/// The production way to remove the probe file.
pub(crate) fn remove_probe_file(path: &Path) -> std::io::Result<()> {
    std::fs::remove_file(path)
}

/// Establish `dir` and show it writable: create it if missing, create a probe
/// file with `create_new` (so nothing that exists can be truncated or replaced),
/// write to it, and remove it. The directory is kept. Any failure, including a
/// probe that cannot be removed, is an error, and the caller must then build
/// nothing. A probe left behind by a failed write is removed on a best-effort
/// basis; it has no `.json` extension so it can never be listed as a session.
pub(crate) fn probe_dir_writable(
    dir: &Path,
    remove: fn(&Path) -> std::io::Result<()>,
) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create it: {}", e.kind()))?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let probe = dir.join(format!(".maia-write-probe-{}-{nanos}", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|e| format!("cannot write to it: {}", e.kind()))?;
    let written = std::io::Write::write_all(&mut file, b"probe");
    drop(file);
    if let Err(e) = written {
        let _ = remove(&probe);
        return Err(format!("cannot write to it: {}", e.kind()));
    }
    remove(&probe).map_err(|e| {
        format!(
            "the write probe `{}` could not be removed: {}",
            probe.display(),
            e.kind()
        )
    })
}

impl History for FileHistory {
    fn location_problem(&self) -> Option<String> {
        match std::fs::metadata(&self.dir) {
            Ok(m) if !m.is_dir() => Some(format!(
                "history location `{}` is not a directory",
                self.dir.display()
            )),
            _ => None,
        }
    }

    fn id_state(&self, session_id: &str) -> IdState {
        match FileSessionStore::new(&self.dir).load(session_id) {
            Ok(None) => IdState::Free,
            // Present, whether or not this build can read it.
            Ok(Some(_)) | Err(SessionStoreError::Corrupt) => IdState::InUse,
            Err(SessionStoreError::Io) | Err(SessionStoreError::InvalidId) => IdState::Unknown,
        }
    }

    fn verify_writable(&self) -> Result<(), String> {
        probe_dir_writable(&self.dir, self.remove_probe)
    }

    fn save(&self, record: &SessionRecord) -> Result<(), SessionStoreError> {
        FileSessionStore::new(&self.dir).save(record)
    }

    fn read_back(&self, session_id: &str) -> Option<SessionPanel> {
        match load_panel(&StoredSessionQuery::new(&self.dir), session_id) {
            Ok(Some(panel)) => Some(panel),
            _ => None,
        }
    }
}

/// What became of the session record. The three failure-adjacent cases are kept
/// apart because they mean different things to the person reading the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stored {
    /// Written and read back; the output is rendered from what was stored.
    Verified,
    /// Not written. The result exists only in this output.
    NotSaved(NotSaved),
    /// Written, but the check that reads it back failed.
    WrittenUnverified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NotSaved {
    WriteFailed,
    /// The id appeared (or became unreadable) after confirmation. Left untouched.
    IdTaken,
}

fn persist(history: &dyn History, record: &SessionRecord) -> (SessionPanel, Stored) {
    let in_memory = || panel_from(&record.to_view());
    // Checked again here, after the participants were asked, so a session that
    // appeared in the meantime is not overwritten. Not atomic: see the module doc.
    if history.id_state(&record.id) != IdState::Free {
        return (in_memory(), Stored::NotSaved(NotSaved::IdTaken));
    }
    if history.save(record).is_err() {
        return (in_memory(), Stored::NotSaved(NotSaved::WriteFailed));
    }
    match history.read_back(&record.id) {
        Some(panel) => (panel, Stored::Verified),
        None => (in_memory(), Stored::WrittenUnverified),
    }
}

/// Whether the session's total cost is known.
///
/// Always `false` in v1, deliberately. Knowing it would require proof that every
/// attempted call, including the adjudication call, has known cost, and the
/// persisted contract cannot give that: `ParticipantAdjudicator` returns only the
/// synthesis text, so the adjudication call's usage is recorded nowhere; a failed
/// attempt carries no usage at all; and first-round usage is optional per
/// participant. Summing what happens to be present would infer zero for the rest,
/// which is the one thing this must not do. Per-participant usage in the record
/// is shown as recorded; this line is about the session as a whole.
fn session_cost_known() -> bool {
    false
}

fn refused(r: &Refusal) -> Output {
    Output::fail(r.exit_code(), r.render())
}

/// Run the pipeline against the history directory `req.history`.
///
/// `build` is the trusted construction boundary (module doc): it is called at
/// most once, and only after guards, plan, history checks, typed confirmation
/// and the writability probe have all passed.
pub fn execute_run<B>(
    req: &RunRequest<'_>,
    env: &dyn Env,
    terminal: &mut dyn Terminal,
    clock: &dyn Clock,
    build: B,
) -> Output
where
    B: FnOnce(&Authorized) -> Result<Participants, BuildError>,
{
    execute_run_with(
        &FileHistory::new(req.history),
        req,
        env,
        terminal,
        clock,
        build,
    )
}

pub(crate) fn execute_run_with<B>(
    history: &dyn History,
    req: &RunRequest<'_>,
    env: &dyn Env,
    terminal: &mut dyn Terminal,
    clock: &dyn Clock,
    build: B,
) -> Output
where
    B: FnOnce(&Authorized) -> Result<Participants, BuildError>,
{
    let tripped = check_guards(env);
    if !tripped.is_empty() {
        return refused(&Refusal::Guards(tripped));
    }
    let report = match build_plan(req.loaded, req.level, req.subject, req.prompt) {
        Ok(r) => r,
        Err(e) => return Output::fail(EXIT_REFUSED, format!("plan refused: {e}\n")),
    };
    if let Some(problem) = history.location_problem() {
        return Output::fail(
            EXIT_HISTORY_UNUSABLE,
            format!("refused: {problem}. Nothing was constructed, called, or written.\n"),
        );
    }
    match history.id_state(req.session_id) {
        IdState::Free => {}
        IdState::InUse => {
            return Output::fail(
                EXIT_SESSION_ID_IN_USE,
                format!(
                    "refused: session id `{}` already exists under `{}` and is never \
                     overwritten. Nothing was constructed, called, or written.\n",
                    req.session_id, req.history
                ),
            );
        }
        IdState::Unknown => {
            return Output::fail(
                EXIT_HISTORY_UNUSABLE,
                format!(
                    "refused: cannot tell whether session id `{}` is already used under `{}`. \
                     Nothing was constructed, called, or written.\n",
                    req.session_id, req.history
                ),
            );
        }
    }

    let token = match authorize(env, terminal, &report, req.history, req.session_id) {
        Ok(token) => token,
        Err(r) => return refused(&r),
    };
    // Confirmed, but still before any participant exists: a location that cannot
    // be written must not cost usage that could not then be saved.
    if let Err(problem) = history.verify_writable() {
        return Output::fail(
            EXIT_HISTORY_UNUSABLE,
            format!(
                "refused: history location `{}` cannot be established for writing ({problem}). \
                 Confirmed, but nothing was constructed, called, or saved.\n",
                req.history
            ),
        );
    }
    let participants = match build(&token) {
        Ok(p) => p,
        Err(e) => return Output::fail(EXIT_NOT_CONSTRUCTED, e.render()),
    };

    let decision = DecisionRequest {
        id: req.session_id.to_owned(),
        subject: req.subject.to_owned(),
        prompt: req.prompt.to_owned(),
        evidence: Vec::new(),
    };
    let realized = match realize_plan(
        req.session_id,
        &report.plan,
        &decision,
        &req.loaded.registry,
        participants.resolver.as_ref(),
        participants.adjudicator.as_ref(),
        MAX_OUTPUT_TOKENS,
        clock,
    ) {
        Ok(r) => r,
        Err(e) => {
            return Output::fail(
                EXIT_NOT_CONSTRUCTED,
                format!("not run: the plan could not be realized ({e:?}). Nothing was called.\n"),
            );
        }
    };

    // A failed session is a first-class record, not an error to swallow.
    let record = match &realized.outcome {
        Ok(session) => SessionRecord::from_session(session),
        Err(failure) => {
            SessionRecord::from_failure(req.session_id, report.plan.required, &decision, failure)
        }
    };
    let decided = record.decided();
    let (panel, stored) = persist(history, &record);

    let mut stdout = render_panel(&panel);
    stdout.push_str(&format!(
        "\ncost_known: {}\nexecution_authority: {}\n",
        session_cost_known(),
        panel.execution_authority
    ));
    let mut stderr = String::new();
    let view_again = format!(
        "View it again: maia-roundtable-viewer --history \"{dir}\" show {id}\n",
        id = req.session_id,
        dir = req.history
    );
    let code = match stored {
        Stored::Verified => {
            stdout.push_str(&format!(
                "Saved as `{id}` under `{dir}`.\n{view_again}",
                id = req.session_id,
                dir = req.history
            ));
            if decided {
                EXIT_OK
            } else {
                EXIT_SESSION_FAILED
            }
        }
        Stored::NotSaved(why) => {
            stdout.push_str("NOT SAVED: the result above exists only in this output.\n");
            stderr = match why {
                NotSaved::WriteFailed => {
                    format!(
                        "warning: the session could not be saved under `{}`.\n",
                        req.history
                    )
                }
                NotSaved::IdTaken => format!(
                    "warning: session id `{}` appeared under `{}` after confirmation; the \
                     existing session was left untouched and this one was not saved.\n",
                    req.session_id, req.history
                ),
            };
            EXIT_NOT_SAVED
        }
        Stored::WrittenUnverified => {
            stdout.push_str(&format!(
                "SAVED, READ-BACK UNVERIFIED: session `{id}` was written under `{dir}`, but \
                 reading it back to check it failed. The result above is rendered from this \
                 run's own record, not from the stored file.\n{view_again}",
                id = req.session_id,
                dir = req.history
            ));
            stderr = format!(
                "warning: session `{}` was saved under `{}` but could not be read back.\n",
                req.session_id, req.history
            );
            EXIT_READBACK_UNVERIFIED
        }
    };
    Output {
        stdout,
        stderr,
        code,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::{
        EXIT_CONFIRMATION_DECLINED, EXIT_CONFIRMATION_UNAVAILABLE, EXIT_GUARD_REFUSED,
    };
    use crate::registry::load_registry;
    use maia_roundtable::{
        Participant, ParticipantDescriptor, ParticipantFailure, ParticipantFailureKind,
        ParticipantRequest, ParticipantResponse, SessionQuery, Timestamp, UsageCostMetadata,
    };
    use maia_roundtable_observability::load_state;
    use maia_roundtable_observability::render::render_state;
    use std::cell::{Cell, RefCell};
    use std::collections::HashSet;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicU64, Ordering};

    const PANEL: &str = r#"{"participants":[
        {"id":"a","adapter":"claude-code","provider":"pa","model_ref":"ma",
         "roles":["round_table_member","adjudicator"],"assurance_levels":["A3"],"enabled":true},
        {"id":"b","adapter":"local-model","provider":"pb","model_ref":"mb",
         "roles":["round_table_member","adjudicator"],"assurance_levels":["A3"],"enabled":true}]}"#;

    struct FakeEnv(HashSet<&'static str>);
    impl Env for FakeEnv {
        fn is_set(&self, key: &str) -> bool {
            self.0.contains(key)
        }
    }
    fn clear() -> FakeEnv {
        FakeEnv(HashSet::new())
    }

    struct Scripted {
        interactive: bool,
        answer: Option<String>,
        asked: usize,
    }
    impl Scripted {
        fn types(answer: &str) -> Self {
            Self {
                interactive: true,
                answer: Some(answer.to_owned()),
                asked: 0,
            }
        }
    }
    impl Terminal for Scripted {
        fn is_interactive(&self) -> bool {
            self.interactive
        }
        fn ask(&mut self, _: &str) -> Option<String> {
            self.asked += 1;
            self.answer.clone()
        }
    }

    struct TestClock(AtomicU64);
    impl Clock for TestClock {
        fn now(&self) -> Timestamp {
            Timestamp(self.0.fetch_add(1, Ordering::SeqCst))
        }
    }
    fn clock() -> TestClock {
        TestClock(AtomicU64::new(1_000))
    }

    /// Answers with a view, and with usage that says its own cost is known or not.
    struct Fake {
        descriptor: ParticipantDescriptor,
        cost_known: bool,
    }
    impl Participant for Fake {
        fn descriptor(&self) -> ParticipantDescriptor {
            self.descriptor.clone()
        }
        fn invoke(&self, _: ParticipantRequest) -> Result<ParticipantResponse, ParticipantFailure> {
            let id = self.descriptor.id.as_str();
            Ok(ParticipantResponse {
                participant: self.descriptor.clone(),
                response_text: format!("view from {id}"),
                evidence: vec![],
                provider_request_id: Some(format!("req-{id}")),
                usage: Some(UsageCostMetadata {
                    input_tokens: Some(10),
                    output_tokens: Some(20),
                    cost_known: self.cost_known,
                    cost_minor: self.cost_known.then_some(5),
                    currency: self.cost_known.then(|| "USD".to_owned()),
                }),
                model_ref_used: None,
            })
        }
    }
    struct Down(ParticipantDescriptor);
    impl Participant for Down {
        fn descriptor(&self) -> ParticipantDescriptor {
            self.0.clone()
        }
        fn invoke(&self, _: ParticipantRequest) -> Result<ParticipantResponse, ParticipantFailure> {
            Err(ParticipantFailure {
                kind: ParticipantFailureKind::Transient,
                provider_request_id: None,
            })
        }
    }

    /// Answers for every descriptor, except the ids listed as down. Ids listed as
    /// `known` report their cost as known; the rest report it unknown.
    struct Resolver {
        down: Vec<&'static str>,
        known: Vec<&'static str>,
    }
    impl ParticipantResolver for Resolver {
        fn resolve(
            &self,
            d: &ParticipantDescriptor,
        ) -> Result<Box<dyn Participant>, ResolutionFailure> {
            if self.down.contains(&d.id.as_str()) {
                Ok(Box::new(Down(d.clone())))
            } else {
                Ok(Box::new(Fake {
                    descriptor: d.clone(),
                    cost_known: self.known.contains(&d.id.as_str()),
                }))
            }
        }
    }

    fn fakes_with(
        down: Vec<&'static str>,
        known: Vec<&'static str>,
    ) -> impl FnOnce(&Authorized) -> Result<Participants, BuildError> {
        move |_| {
            let loaded = load_registry(PANEL).unwrap();
            participants_with_leader(
                Box::new(Resolver { down, known }),
                &loaded,
                ReasoningAssuranceLevel::A3,
            )
        }
    }

    fn fakes(
        down: Vec<&'static str>,
    ) -> impl FnOnce(&Authorized) -> Result<Participants, BuildError> {
        fakes_with(down, vec![])
    }

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("maia-invoke-run-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    const ID: &str = "rt-0123456789ab";

    fn request<'a>(loaded: &'a LoadedRegistry, history: &'a str, id: &'a str) -> RunRequest<'a> {
        RunRequest {
            loaded,
            level: ReasoningAssuranceLevel::A3,
            subject: "ship it?",
            prompt: "should we ship on friday",
            history,
            session_id: id,
        }
    }

    fn go<B>(history: &Path, env: &dyn Env, term: &mut Scripted, build: B) -> Output
    where
        B: FnOnce(&Authorized) -> Result<Participants, BuildError>,
    {
        go_id(history, ID, env, term, build)
    }

    fn go_id<B>(history: &Path, id: &str, env: &dyn Env, term: &mut Scripted, build: B) -> Output
    where
        B: FnOnce(&Authorized) -> Result<Participants, BuildError>,
    {
        let loaded = load_registry(PANEL).unwrap();
        let history = history.to_str().unwrap();
        execute_run(&request(&loaded, history, id), env, term, &clock(), build)
    }

    #[test]
    fn a_confirmed_run_saves_and_prints_exactly_what_the_viewer_shows() {
        let dir = temp("decided");
        let out = go(&dir, &clear(), &mut Scripted::types(ID), fakes(vec![]));
        assert_eq!(out.code, EXIT_OK, "{}{}", out.stdout, out.stderr);
        assert!(out.stderr.is_empty());

        // The viewer's own read path, run against what the invoker saved.
        let query = StoredSessionQuery::new(&dir);
        let panel = load_panel(&query, ID).unwrap().expect("saved and viewable");
        let viewer_text = render_panel(&panel);
        assert!(
            out.stdout.starts_with(&viewer_text),
            "run output must begin with the viewer rendering:\n{}",
            out.stdout
        );
        assert!(viewer_text.contains("Prompt: should we ship on friday"));
        assert!(viewer_text.contains("Conclusion:"));

        assert!(out.stdout.contains("execution_authority: false"));
        assert!(out.stdout.contains("cost_known: false"));
        assert!(out.stdout.contains(&format!("show {ID}")));
        assert_eq!(query.list_sessions().unwrap().len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_failed_session_is_saved_first_class_and_visible_in_the_viewer() {
        let dir = temp("failed");
        // Both members down: quorum cannot be met, nothing is adjudicated.
        let out = go(
            &dir,
            &clear(),
            &mut Scripted::types(ID),
            fakes(vec!["a", "b"]),
        );
        assert_eq!(out.code, EXIT_SESSION_FAILED, "{}", out.stdout);
        assert!(out.stdout.contains("did not reach a decision"));
        assert!(out.stdout.contains("execution_authority: false"));

        let query = StoredSessionQuery::new(&dir);
        let panel = load_panel(&query, ID)
            .unwrap()
            .expect("failed run is saved");
        assert!(out.stdout.starts_with(&render_panel(&panel)));
        assert_eq!(panel.conclusion, None);
        assert!(panel.failures().count() >= 1);
        let listing = render_state(&load_state(&query));
        assert!(listing.contains(ID), "the viewer lists it: {listing}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn counted<'a>(
        count: &'a Cell<usize>,
    ) -> impl FnOnce(&Authorized) -> Result<Participants, BuildError> + 'a {
        move |_| {
            count.set(count.get() + 1);
            Err(BuildError::NotWired)
        }
    }

    #[test]
    fn a_refused_or_declined_run_constructs_nothing_and_creates_no_history() {
        let mut env_guard = HashSet::new();
        env_guard.insert("CLAUDECODE");
        let cases: Vec<(FakeEnv, Scripted, i32)> = vec![
            (FakeEnv(env_guard), Scripted::types(ID), EXIT_GUARD_REFUSED),
            (
                clear(),
                Scripted {
                    interactive: false,
                    answer: Some(ID.into()),
                    asked: 0,
                },
                EXIT_CONFIRMATION_UNAVAILABLE,
            ),
            (clear(), Scripted::types("y"), EXIT_CONFIRMATION_DECLINED),
            (clear(), Scripted::types(""), EXIT_CONFIRMATION_DECLINED),
            (
                clear(),
                Scripted {
                    interactive: true,
                    answer: None,
                    asked: 0,
                },
                EXIT_CONFIRMATION_DECLINED,
            ),
        ];
        for (i, (env, mut term, code)) in cases.into_iter().enumerate() {
            let dir = temp(&format!("refused{i}"));
            let builds = Cell::new(0);
            let out = go(&dir, &env, &mut term, counted(&builds));
            assert_eq!(out.code, code, "case {i}: {}", out.stderr);
            assert_eq!(builds.get(), 0, "case {i}: nothing may be constructed");
            assert!(
                !dir.exists(),
                "case {i}: no history directory may be created"
            );
            assert!(
                out.stdout.is_empty(),
                "case {i}: nothing reads like a result"
            );
        }
    }

    #[test]
    fn construction_happens_once_and_only_after_the_typed_confirmation() {
        let dir = temp("once");
        let builds = Cell::new(0);
        let mut term = Scripted::types(ID);
        let out = go(&dir, &clear(), &mut term, counted(&builds));
        assert_eq!(builds.get(), 1);
        assert_eq!(term.asked, 1, "asked exactly once, before construction");
        // The builder said it cannot run: that is a clean refusal, nothing saved.
        assert_eq!(out.code, EXIT_NOT_CONSTRUCTED);
        assert!(out.stderr.contains("not wired"));
        assert_empty_dir(
            &dir,
            "the established directory is kept, with no probe left",
        );
    }

    #[test]
    fn guards_are_checked_before_the_plan_so_a_nested_run_learns_nothing_else() {
        let mut set = HashSet::new();
        set.insert("MAIA_CLAUDE_CODE_PROVIDER_ACTIVE");
        let loaded = load_registry(PANEL).unwrap();
        let out = execute_run(
            &RunRequest {
                level: ReasoningAssuranceLevel::A4,
                ..request(&loaded, "unused", ID)
            },
            &FakeEnv(set),
            &mut Scripted::types(ID),
            &clock(),
            fakes(vec![]),
        );
        assert_eq!(out.code, EXIT_GUARD_REFUSED);
    }

    #[test]
    fn an_unplannable_request_is_refused_before_anyone_is_asked() {
        let loaded = load_registry(PANEL).unwrap();
        let mut term = Scripted::types(ID);
        let out = execute_run(
            &RunRequest {
                level: ReasoningAssuranceLevel::A4,
                ..request(&loaded, "unused", ID)
            },
            &clear(),
            &mut term,
            &clock(),
            fakes(vec![]),
        );
        assert_eq!(out.code, EXIT_REFUSED);
        assert!(out.stderr.contains("not downgraded"));
        assert_eq!(term.asked, 0);
    }

    #[test]
    fn a_history_location_that_is_a_file_is_refused_before_confirmation() {
        let file = temp("afile");
        std::fs::write(&file, "x").unwrap();
        let builds = Cell::new(0);
        let mut term = Scripted::types(ID);
        let out = go(&file, &clear(), &mut term, counted(&builds));
        assert_eq!(out.code, EXIT_HISTORY_UNUSABLE);
        assert_eq!(
            term.asked, 0,
            "the person is not asked to spend usage first"
        );
        assert_eq!(builds.get(), 0);
        std::fs::remove_file(&file).unwrap();
    }

    #[test]
    fn an_unresolvable_leader_means_nothing_runs_and_nothing_is_saved() {
        struct Substitutes;
        impl ParticipantResolver for Substitutes {
            fn resolve(
                &self,
                d: &ParticipantDescriptor,
            ) -> Result<Box<dyn Participant>, ResolutionFailure> {
                // Answers with a different participant than the one asked for.
                let mut other = d.clone();
                other.id = maia_roundtable::ParticipantId::new("someone-else").unwrap();
                Ok(Box::new(Fake {
                    descriptor: other,
                    cost_known: false,
                }))
            }
        }
        let dir = temp("leader");
        let out = go(&dir, &clear(), &mut Scripted::types(ID), |_| {
            let loaded = load_registry(PANEL).unwrap();
            participants_with_leader(Box::new(Substitutes), &loaded, ReasoningAssuranceLevel::A3)
        });
        assert_eq!(out.code, EXIT_NOT_CONSTRUCTED);
        assert!(out.stderr.contains("leader could not be constructed"));
        assert_empty_dir(&dir, "nothing was saved");
    }

    // ------------------------------------------------ A: session id collisions

    fn session_file(dir: &Path, id: &str) -> PathBuf {
        dir.join(format!("{id}.json"))
    }

    /// The directory exists and holds nothing: no session and no probe file.
    fn assert_empty_dir(dir: &Path, why: &str) {
        let names: Vec<_> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("{why}: {} must exist: {e}", dir.display()))
            .map(|e| e.unwrap().file_name())
            .collect();
        assert!(names.is_empty(), "{why}: found {names:?}");
    }

    #[test]
    fn a_reused_session_id_is_refused_and_the_original_record_is_left_intact() {
        let dir = temp("reuse");
        let first = go(&dir, &clear(), &mut Scripted::types(ID), fakes(vec![]));
        assert_eq!(first.code, EXIT_OK, "{}{}", first.stdout, first.stderr);
        let path = session_file(&dir, ID);
        let original_bytes = std::fs::read(&path).unwrap();
        let original_record = FileSessionStore::new(&dir).load(ID).unwrap().unwrap();

        // Same id, different question and a different outcome: it must not land.
        let loaded = load_registry(PANEL).unwrap();
        let history = dir.to_str().unwrap();
        let builds = Cell::new(0);
        let mut term = Scripted::types(ID);
        let out = execute_run(
            &RunRequest {
                subject: "a different subject",
                prompt: "a different prompt",
                ..request(&loaded, history, ID)
            },
            &clear(),
            &mut term,
            &clock(),
            counted(&builds),
        );
        assert_eq!(out.code, EXIT_SESSION_ID_IN_USE, "{}", out.stderr);
        assert!(out.stderr.contains("never overwritten"));
        assert_eq!(builds.get(), 0, "no provider may be constructed");
        assert_eq!(term.asked, 0, "the person is not asked to confirm");
        assert!(out.stdout.is_empty());

        assert_eq!(
            std::fs::read(&path).unwrap(),
            original_bytes,
            "byte for byte"
        );
        assert_eq!(
            FileSessionStore::new(&dir).load(ID).unwrap().unwrap(),
            original_record,
            "and semantically"
        );
        assert_eq!(
            StoredSessionQuery::new(&dir).list_sessions().unwrap().len(),
            1
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_unreadable_record_under_the_same_id_also_counts_as_in_use() {
        let dir = temp("corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        let path = session_file(&dir, ID);
        std::fs::write(&path, "{ not a session").unwrap();
        let builds = Cell::new(0);
        let out = go(&dir, &clear(), &mut Scripted::types(ID), counted(&builds));
        assert_eq!(out.code, EXIT_SESSION_ID_IN_USE);
        assert_eq!(builds.get(), 0);
        assert_eq!(std::fs::read(&path).unwrap(), b"{ not a session");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn something_that_is_not_a_file_where_the_session_belongs_is_refused_up_front() {
        let dir = temp("dirslot");
        std::fs::create_dir_all(session_file(&dir, ID)).unwrap();
        let builds = Cell::new(0);
        let mut term = Scripted::types(ID);
        let out = go(&dir, &clear(), &mut term, counted(&builds));
        assert_eq!(out.code, EXIT_HISTORY_UNUSABLE);
        assert_eq!((builds.get(), term.asked), (0, 0));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // ------------------------------------------------ fake history: branches

    type Log = Rc<RefCell<Vec<&'static str>>>;

    struct FakeHistory {
        log: Log,
        /// Successive answers to `id_state`; the last one repeats.
        id_states: RefCell<Vec<IdState>>,
        writable: Result<(), String>,
        save_ok: bool,
        saves: Cell<usize>,
    }
    impl FakeHistory {
        fn new(log: &Log) -> Self {
            Self {
                log: Rc::clone(log),
                id_states: RefCell::new(vec![IdState::Free]),
                writable: Ok(()),
                save_ok: true,
                saves: Cell::new(0),
            }
        }
    }
    impl History for FakeHistory {
        fn location_problem(&self) -> Option<String> {
            self.log.borrow_mut().push("history_location");
            None
        }
        fn id_state(&self, _: &str) -> IdState {
            self.log.borrow_mut().push("history_id");
            let mut states = self.id_states.borrow_mut();
            if states.len() > 1 {
                states.remove(0)
            } else {
                states[0]
            }
        }
        fn verify_writable(&self) -> Result<(), String> {
            self.log.borrow_mut().push("history_writable");
            self.writable.clone()
        }
        fn save(&self, _: &SessionRecord) -> Result<(), SessionStoreError> {
            self.log.borrow_mut().push("save");
            self.saves.set(self.saves.get() + 1);
            if self.save_ok {
                Ok(())
            } else {
                Err(SessionStoreError::Io)
            }
        }
        fn read_back(&self, _: &str) -> Option<SessionPanel> {
            self.log.borrow_mut().push("read_back");
            None
        }
    }

    fn new_log() -> Log {
        Rc::new(RefCell::new(Vec::new()))
    }

    /// The trace with immediate repeats folded, so "the guards ran" is one event
    /// however many markers were looked at.
    fn folded(log: &Log) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for e in log.borrow().iter() {
            if out.last() != Some(e) {
                out.push(e);
            }
        }
        out
    }

    struct TracingEnv(Log);
    impl Env for TracingEnv {
        fn is_set(&self, _: &str) -> bool {
            self.0.borrow_mut().push("guards");
            false
        }
    }

    struct TracingTerminal(Log);
    impl Terminal for TracingTerminal {
        fn is_interactive(&self) -> bool {
            true
        }
        fn ask(&mut self, _: &str) -> Option<String> {
            self.0.borrow_mut().push("confirm");
            Some(ID.to_owned())
        }
    }

    fn run_traced(
        history: &FakeHistory,
        log: &Log,
        level: ReasoningAssuranceLevel,
        build: impl FnOnce(&Authorized) -> Result<Participants, BuildError>,
    ) -> Output {
        let loaded = load_registry(PANEL).unwrap();
        execute_run_with(
            history,
            &RunRequest {
                level,
                ..request(&loaded, "hist", ID)
            },
            &TracingEnv(Rc::clone(log)),
            &mut TracingTerminal(Rc::clone(log)),
            &clock(),
            build,
        )
    }

    // ------------------------------------------------ E: ordered event trace

    #[test]
    fn the_builder_is_called_after_guards_plan_history_checks_and_confirmation() {
        let log = new_log();
        let history = FakeHistory::new(&log);
        let build_log = Rc::clone(&log);
        let out = run_traced(&history, &log, ReasoningAssuranceLevel::A3, move |t| {
            build_log.borrow_mut().push("build");
            fakes(vec![])(t)
        });
        assert_eq!(
            out.code, EXIT_READBACK_UNVERIFIED,
            "{}{}",
            out.stdout, out.stderr
        );
        assert_eq!(
            folded(&log),
            [
                "guards",
                "history_location",
                "history_id",
                // `authorize` re-checks the guards immediately before asking.
                "guards",
                "confirm",
                "history_writable",
                "build",
                // After the participants ran: the id is re-checked, then written.
                "history_id",
                "save",
                "read_back",
            ]
        );
    }

    #[test]
    fn an_unplannable_request_stops_after_the_guards_and_touches_nothing_else() {
        let log = new_log();
        let history = FakeHistory::new(&log);
        let out = run_traced(&history, &log, ReasoningAssuranceLevel::A4, fakes(vec![]));
        assert_eq!(out.code, EXIT_REFUSED);
        assert_eq!(folded(&log), ["guards"]);
    }

    #[test]
    fn a_taken_id_stops_before_confirmation_and_construction() {
        let log = new_log();
        let history = FakeHistory::new(&log);
        *history.id_states.borrow_mut() = vec![IdState::InUse];
        let build_log = Rc::clone(&log);
        let out = run_traced(&history, &log, ReasoningAssuranceLevel::A3, move |t| {
            build_log.borrow_mut().push("build");
            fakes(vec![])(t)
        });
        assert_eq!(out.code, EXIT_SESSION_ID_IN_USE);
        assert_eq!(folded(&log), ["guards", "history_location", "history_id"]);
    }

    #[test]
    fn an_id_that_cannot_be_checked_is_treated_as_unsafe() {
        let log = new_log();
        let history = FakeHistory::new(&log);
        *history.id_states.borrow_mut() = vec![IdState::Unknown];
        let out = run_traced(&history, &log, ReasoningAssuranceLevel::A3, fakes(vec![]));
        assert_eq!(out.code, EXIT_HISTORY_UNUSABLE);
        assert!(!folded(&log).contains(&"confirm"));
    }

    #[test]
    fn a_session_that_appears_while_participants_run_is_not_overwritten() {
        let log = new_log();
        let history = FakeHistory::new(&log);
        // Free before confirmation, taken by the time the result is ready.
        *history.id_states.borrow_mut() = vec![IdState::Free, IdState::InUse];
        let out = run_traced(&history, &log, ReasoningAssuranceLevel::A3, fakes(vec![]));
        assert_eq!(out.code, EXIT_NOT_SAVED);
        assert_eq!(history.saves.get(), 0, "save must never be attempted");
        assert!(out.stdout.contains("Conclusion:"), "the result is not lost");
        assert!(out.stdout.contains("NOT SAVED"));
        assert!(out.stderr.contains("left untouched"));
    }

    // ------------------------------------------------ F: writability first

    #[test]
    fn an_unwritable_history_after_confirmation_constructs_and_calls_nothing() {
        let log = new_log();
        let mut history = FakeHistory::new(&log);
        history.writable = Err("permission denied".into());
        let builds = Cell::new(0);
        let build_log = Rc::clone(&log);
        let out = run_traced(&history, &log, ReasoningAssuranceLevel::A3, |_| {
            build_log.borrow_mut().push("build");
            builds.set(builds.get() + 1);
            Err(BuildError::NotWired)
        });
        assert_eq!(out.code, EXIT_HISTORY_UNUSABLE);
        assert_eq!(builds.get(), 0, "no participant may be constructed");
        let trace = folded(&log);
        assert!(trace.contains(&"confirm"), "it failed after confirmation");
        assert_eq!(trace.last(), Some(&"history_writable"));
        assert_eq!(history.saves.get(), 0);
        assert!(
            out.stderr
                .contains("nothing was constructed, called, or saved")
        );
    }

    #[test]
    fn a_history_that_cannot_be_created_is_refused_before_any_participant_exists() {
        // A regular file where a parent directory would have to be.
        let file = temp("parentfile");
        std::fs::write(&file, "x").unwrap();
        let history = file.join("sub");
        let builds = Cell::new(0);
        let out = go(
            &history,
            &clear(),
            &mut Scripted::types(ID),
            counted(&builds),
        );
        assert_eq!(out.code, EXIT_HISTORY_UNUSABLE, "{}", out.stderr);
        assert_eq!(builds.get(), 0);
        assert_eq!(std::fs::read(&file).unwrap(), b"x", "untouched");
        std::fs::remove_file(&file).unwrap();
    }

    #[test]
    fn the_probe_keeps_the_directory_removes_its_file_and_spares_an_existing_session() {
        // Existing history with a session in it.
        let dir = temp("probe-existing");
        let first = go(&dir, &clear(), &mut Scripted::types(ID), fakes(vec![]));
        assert_eq!(first.code, EXIT_OK);
        let listing = |d: &Path| {
            let mut names: Vec<_> = std::fs::read_dir(d)
                .unwrap()
                .map(|e| e.unwrap().file_name())
                .collect();
            names.sort();
            names
        };
        let before = listing(&dir);
        let bytes = std::fs::read(session_file(&dir, ID)).unwrap();

        let builds = Cell::new(0);
        let out = go_id(
            &dir,
            "rt-another",
            &clear(),
            &mut Scripted::types("rt-another"),
            counted(&builds),
        );
        assert_eq!(out.code, EXIT_NOT_CONSTRUCTED);
        assert_eq!(
            builds.get(),
            1,
            "the probe passed, so construction was reached"
        );
        assert_eq!(listing(&dir), before, "no probe file is left behind");
        assert_eq!(std::fs::read(session_file(&dir, ID)).unwrap(), bytes);
        assert!(dir.is_dir(), "a pre-existing directory is never deleted");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_new_history_directory_stays_established_after_the_preflight_probe() {
        // Several levels deep, none of which exist yet.
        let root = temp("probe-deep");
        let deep = root.join("a").join("b");
        let builds = Cell::new(0);
        let out = go(&deep, &clear(), &mut Scripted::types(ID), counted(&builds));
        assert_eq!(out.code, EXIT_NOT_CONSTRUCTED);
        assert_eq!(builds.get(), 1);
        // Kept for the duration of the run, so it cannot vanish under it.
        assert_empty_dir(&deep, "the directory made for the probe is kept");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_probe_that_cannot_be_removed_fails_closed_with_zero_construction() {
        fn refuse_removal(_: &Path) -> std::io::Result<()> {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
        let dir = temp("probe-stuck");
        let history = FileHistory {
            dir: dir.clone(),
            remove_probe: refuse_removal,
        };
        let loaded = load_registry(PANEL).unwrap();
        let builds = Cell::new(0);
        let mut term = Scripted::types(ID);
        let out = execute_run_with(
            &history,
            &request(&loaded, dir.to_str().unwrap(), ID),
            &clear(),
            &mut term,
            &clock(),
            counted(&builds),
        );
        assert_eq!(out.code, EXIT_HISTORY_UNUSABLE, "{}", out.stderr);
        assert!(out.stderr.contains("could not be removed"));
        assert_eq!(term.asked, 1, "it failed after confirmation");
        assert_eq!(builds.get(), 0, "no participant may be constructed");
        assert!(!session_file(&dir, ID).exists(), "nothing was saved");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn probe_dir_writable_never_truncates_or_replaces_what_exists() {
        let dir = temp("probe-unit");
        std::fs::create_dir_all(&dir).unwrap();
        let precious = dir.join("keep.json");
        std::fs::write(&precious, "precious").unwrap();
        assert_eq!(probe_dir_writable(&dir, remove_probe_file), Ok(()));
        assert_eq!(std::fs::read(&precious).unwrap(), b"precious");
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1, "only the original file remains: {names:?}");
        // A regular file where the directory should be is an error, not replaced.
        assert!(probe_dir_writable(&precious, remove_probe_file).is_err());
        assert_eq!(std::fs::read(&precious).unwrap(), b"precious");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // ------------------------------------------------ D: save vs read-back

    #[test]
    fn a_save_failure_says_not_saved() {
        let log = new_log();
        let mut history = FakeHistory::new(&log);
        history.save_ok = false;
        let out = run_traced(&history, &log, ReasoningAssuranceLevel::A3, fakes(vec![]));
        assert_eq!(out.code, EXIT_NOT_SAVED);
        assert!(out.stdout.contains("Conclusion:"), "the result is not lost");
        assert!(out.stdout.contains("NOT SAVED"));
        assert!(out.stderr.contains("could not be saved"));
        assert!(!out.stdout.contains("READ-BACK"));
        assert!(!out.stdout.contains("View it again"));
        assert!(!folded(&log).contains(&"read_back"), "nothing to read back");
    }

    #[test]
    fn a_read_back_failure_after_a_successful_save_does_not_say_not_saved() {
        let log = new_log();
        // Saves fine; reading it back fails (the fake returns nothing).
        let history = FakeHistory::new(&log);
        let out = run_traced(&history, &log, ReasoningAssuranceLevel::A3, fakes(vec![]));
        assert_eq!(history.saves.get(), 1);
        assert_eq!(out.code, EXIT_READBACK_UNVERIFIED);
        assert!(out.stdout.contains("SAVED, READ-BACK UNVERIFIED"));
        assert!(
            !out.stdout.contains("NOT SAVED"),
            "it was saved: {}",
            out.stdout
        );
        assert!(out.stdout.contains(ID), "the session id is preserved");
        assert!(out.stdout.contains("View it again"));
        assert!(out.stdout.contains("Conclusion:"), "the result is not lost");
        assert!(out.stderr.contains(ID));
        assert!(out.stderr.contains("could not be read back"));
    }

    // ------------------------------------------------ B: session cost

    fn cost_line(out: &Output) -> &str {
        out.stdout
            .lines()
            .find(|l| l.starts_with("cost_known:"))
            .expect("every run states cost_known")
    }

    #[test]
    fn known_first_round_usage_with_an_unaccounted_adjudication_is_not_known() {
        let dir = temp("cost-all-known");
        // Every first-round response reports known cost, and the record says so...
        let out = go(
            &dir,
            &clear(),
            &mut Scripted::types(ID),
            fakes_with(vec![], vec!["a", "b"]),
        );
        assert_eq!(out.code, EXIT_OK, "{}{}", out.stdout, out.stderr);
        let stored = FileSessionStore::new(&dir).load(ID).unwrap().unwrap();
        let first_round: Vec<_> = stored
            .outcomes
            .iter()
            .filter_map(|o| o.response())
            .collect();
        assert_eq!(first_round.len(), 2);
        assert!(
            first_round
                .iter()
                .all(|r| r.usage.as_ref().is_some_and(|u| u.cost_known))
        );
        // ...but the adjudication call is recorded nowhere, so the session's cost
        // is not proven, and must not be reported as known.
        assert!(
            stored
                .outcomes
                .iter()
                .all(|o| o.role == maia_roundtable::ContributionRole::FirstRound),
            "the adjudication call has no persisted outcome"
        );
        assert_eq!(cost_line(&out), "cost_known: false");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn mixed_known_and_unknown_first_round_usage_is_not_known() {
        let dir = temp("cost-mixed");
        let out = go(
            &dir,
            &clear(),
            &mut Scripted::types(ID),
            fakes_with(vec![], vec!["a"]),
        );
        assert_eq!(out.code, EXIT_OK, "{}{}", out.stdout, out.stderr);
        assert_eq!(cost_line(&out), "cost_known: false");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_failed_contribution_is_not_known_even_when_the_rest_is() {
        let dir = temp("cost-failed");
        // One member down, the other reporting known cost.
        let out = go(
            &dir,
            &clear(),
            &mut Scripted::types(ID),
            fakes_with(vec!["a"], vec!["b"]),
        );
        assert_eq!(
            out.code, EXIT_SESSION_FAILED,
            "{}{}",
            out.stdout, out.stderr
        );
        assert_eq!(cost_line(&out), "cost_known: false");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn session_cost_is_never_inferred() {
        assert!(!session_cost_known());
    }

    #[test]
    fn exit_codes_are_distinct() {
        use crate::cli::{EXIT_INPUT_UNREADABLE, EXIT_USAGE};
        let all = [
            EXIT_OK,
            EXIT_USAGE,
            EXIT_REFUSED,
            EXIT_INPUT_UNREADABLE,
            EXIT_GUARD_REFUSED,
            EXIT_CONFIRMATION_UNAVAILABLE,
            EXIT_CONFIRMATION_DECLINED,
            EXIT_SESSION_FAILED,
            EXIT_NOT_SAVED,
            EXIT_NOT_CONSTRUCTED,
            EXIT_HISTORY_UNUSABLE,
            EXIT_SESSION_ID_IN_USE,
            EXIT_READBACK_UNVERIFIED,
            EXIT_PREFLIGHT_FAILED,
        ];
        let unique: HashSet<_> = all.iter().collect();
        assert_eq!(unique.len(), all.len());
    }
}
