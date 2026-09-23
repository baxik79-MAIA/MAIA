//! Everything that must be true before a provider may be constructed (M0.15.7b).
//!
//! Order, fixed by the design (proposal sections 4 and 6):
//!
//! ```text
//! guards -> plan output -> typed interactive confirmation -> provider construction -> calls
//! ```
//!
//! A refusal at any step means nothing was constructed. That is a property of
//! the order [`crate::run::execute_run`] follows, pinned by tests, not a type
//! guarantee: [`Authorized`] has no public constructor and [`authorize`] is the
//! only thing that makes one, and the builder closure must be handed one, but
//! Rust does not stop a caller from building a provider elsewhere and capturing
//! it in that closure. The trusted construction boundary is therefore the
//! builder passed to `execute_run` (the live one, when it exists, is reviewed as
//! that boundary), and the guarantee is that `execute_run` calls it only after
//! the guards, the plan, the history checks and the typed confirmation.
//!
//! Every check fails closed. An environment marker counts as present when it is
//! set at all, including to the empty string.

use crate::plan::{PlanReport, render_plan};
use std::io::{BufRead, IsTerminal, Write};

/// Set by an ambient Claude Code session (interactive or autonomous). The
/// continuation driver tests the same variable. Unlike the driver, the invoker
/// has no `--allow-nested`.
pub const CLAUDE_CODE_SESSION_MARKER: &str = "CLAUDECODE";

/// The provider sets this on its own child. Repeated here, rather than imported,
/// because the provider dependency is optional: the guard must exist in a build
/// without it. A feature-gated test asserts the two strings are equal.
pub const PROVIDER_MARKER: &str = "MAIA_CLAUDE_CODE_PROVIDER_ACTIVE";

/// Refused if already set, so the invoker cannot be started by a process that
/// something else launched on its behalf.
pub const OWN_MARKER: &str = "MAIA_ROUNDTABLE_INVOKE_ACTIVE";

/// Exit code: a guard refused (recursion / ambient session).
pub const EXIT_GUARD_REFUSED: i32 = 5;
/// Exit code: no terminal, so consent cannot be obtained. Nothing was constructed.
pub const EXIT_CONFIRMATION_UNAVAILABLE: i32 = 6;
/// Exit code: the user did not type the session id. Nothing was constructed.
pub const EXIT_CONFIRMATION_DECLINED: i32 = 7;

/// Read-only view of the environment, injected so tests never touch the real one.
pub trait Env {
    fn is_set(&self, key: &str) -> bool;
}

pub struct ProcessEnv;

impl Env for ProcessEnv {
    fn is_set(&self, key: &str) -> bool {
        std::env::var_os(key).is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardRefusal {
    AmbientClaudeCodeSession,
    ProviderMarker,
    OwnMarker,
}

impl GuardRefusal {
    pub fn marker(self) -> &'static str {
        match self {
            Self::AmbientClaudeCodeSession => CLAUDE_CODE_SESSION_MARKER,
            Self::ProviderMarker => PROVIDER_MARKER,
            Self::OwnMarker => OWN_MARKER,
        }
    }
}

/// Every guard, in the order they are checked. All of them are always evaluated
/// by [`check_guards`] so the refusal names every reason, not just the first.
const GUARDS: [GuardRefusal; 3] = [
    GuardRefusal::AmbientClaudeCodeSession,
    GuardRefusal::ProviderMarker,
    GuardRefusal::OwnMarker,
];

/// Refuse to run inside a Claude Code session or a provider child. Returns every
/// guard that tripped; empty means clear.
pub fn check_guards(env: &dyn Env) -> Vec<GuardRefusal> {
    GUARDS
        .iter()
        .copied()
        .filter(|g| env.is_set(g.marker()))
        .collect()
}

pub fn render_guard_refusal(tripped: &[GuardRefusal]) -> String {
    let mut out = String::from(
        "refused: this tool must be started by a person from a terminal that is not inside a \
         Claude Code session, and never from a participant's process.\n",
    );
    for g in tripped {
        let why = match g {
            GuardRefusal::AmbientClaudeCodeSession => "an ambient Claude Code session is present",
            GuardRefusal::ProviderMarker => "a Round Table provider child process is present",
            GuardRefusal::OwnMarker => "this tool is already active in the process tree",
        };
        out.push_str(&format!("  - {} is set: {why}\n", g.marker()));
    }
    out.push_str("There is no override. Nothing was constructed, called, or written.\n");
    out
}

/// The terminal the confirmation is read from. Injected so tests can drive it.
pub trait Terminal {
    /// True only if both stdin and stdout are terminals.
    fn is_interactive(&self) -> bool;
    /// Show `text`, then read one line. `None` on end of input or a read error.
    fn ask(&mut self, text: &str) -> Option<String>;
}

pub struct StdTerminal;

impl Terminal for StdTerminal {
    fn is_interactive(&self) -> bool {
        std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
    }

    fn ask(&mut self, text: &str) -> Option<String> {
        let mut stdout = std::io::stdout();
        stdout.write_all(text.as_bytes()).ok()?;
        stdout.flush().ok()?;
        let mut line = String::new();
        match std::io::stdin().lock().read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line),
        }
    }
}

/// Proof that the guards were clear and the person typed the session id, for
/// this invocation only. It cannot be built outside this module, cloned, stored
/// in a file, or carried into a later run.
#[derive(Debug)]
pub struct Authorized {
    session_id: String,
    _private: (),
}

impl Authorized {
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    Guards(Vec<GuardRefusal>),
    /// stdin or stdout is not a terminal, so no one can be asked.
    ConfirmationUnavailable,
    /// Input ended, or what was typed was not the session id.
    ConfirmationDeclined,
}

impl Refusal {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Guards(_) => EXIT_GUARD_REFUSED,
            Self::ConfirmationUnavailable => EXIT_CONFIRMATION_UNAVAILABLE,
            Self::ConfirmationDeclined => EXIT_CONFIRMATION_DECLINED,
        }
    }

    pub fn render(&self) -> String {
        match self {
            Self::Guards(tripped) => render_guard_refusal(tripped),
            Self::ConfirmationUnavailable => "refused: confirmation is unavailable because stdin \
                and stdout are not both a terminal. There is no --yes and no environment or file \
                override. Nothing was constructed, called, or written.\n"
                .to_owned(),
            Self::ConfirmationDeclined => "not confirmed: the session id was not typed. Nothing \
                was constructed, called, or written.\n"
                .to_owned(),
        }
    }
}

/// A session id built from the full clock reading, the process id and a random
/// word. Shown to the person and typed back, so it is deliberately not something
/// a stray Enter or `y` can satisfy. Safe as a store key (`[0-9a-f-]` only).
///
/// Nothing is masked or folded: the fields are written in full and separated, so
/// two different `(now_nanos, pid, entropy)` triples always give different ids.
/// (The first version xor-ed the pid above bit 48 and then kept 48 bits, which
/// discarded the pid entirely.)
pub fn new_session_id(now_nanos: u128, pid: u32, entropy: u32) -> String {
    format!("rt-{now_nanos:x}-{pid:x}-{entropy:08x}")
}

/// An id for this invocation. The entropy word comes from the standard library's
/// randomly seeded hasher, so no dependency is added for it; uniqueness against
/// what is already in history is still checked by the caller, not assumed.
pub fn fresh_session_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(now);
    // Truncating the hash to 32 bits is intended: it is one field of three.
    new_session_id(now, std::process::id(), hasher.finish() as u32)
}

/// The invoker's own recursion marker as the provider's typed child marker, so
/// the provider sets it on the child it spawns. The invoker's process
/// environment is never modified.
#[cfg(feature = "development-evolution")]
pub fn own_child_marker() -> maia_claude_code::ChildMarker {
    maia_claude_code::ChildMarker::new(OWN_MARKER)
        .expect("OWN_MARKER has the MAIA_<...>_ACTIVE shape the provider accepts")
}

/// Run the guards, print the plan, and obtain typed confirmation. The plan is
/// printed only after the guards are clear and a terminal is known to exist, so
/// a refused run shows nothing that reads like an offer.
///
/// `history` is the directory the session would be saved under; it appears in
/// the plan's side-effects line.
pub fn authorize(
    env: &dyn Env,
    terminal: &mut dyn Terminal,
    report: &PlanReport,
    history: &str,
    session_id: &str,
) -> Result<Authorized, Refusal> {
    authorize_described(
        env,
        terminal,
        &render_plan(report, Some(history)),
        session_id,
        "session id",
    )
}

/// The same guards, terminal check and typed confirmation as [`authorize`], for
/// a caller that shows its own description of what is about to happen (the
/// preflight, which calls no model and so must not show the invocation plan).
/// `phrase` is what the person must type back exactly, called `label` in the
/// prompt.
pub fn authorize_described(
    env: &dyn Env,
    terminal: &mut dyn Terminal,
    block: &str,
    phrase: &str,
    label: &str,
) -> Result<Authorized, Refusal> {
    let tripped = check_guards(env);
    if !tripped.is_empty() {
        return Err(Refusal::Guards(tripped));
    }
    if !terminal.is_interactive() {
        return Err(Refusal::ConfirmationUnavailable);
    }
    let question = format!(
        "{block}\nTo proceed, type this {label} exactly and press Enter: {phrase}\n\
         Anything else cancels.\n> "
    );
    match terminal.ask(&question) {
        Some(typed) if typed.trim_end_matches(['\r', '\n']) == phrase => Ok(Authorized {
            session_id: phrase.to_owned(),
            _private: (),
        }),
        _ => Err(Refusal::ConfirmationDeclined),
    }
}

/// Authorize, and only then construct. `construct` is called at most once and
/// only with a token that a completed confirmation produced; on any refusal it
/// is never called.
pub fn authorize_then<T>(
    env: &dyn Env,
    terminal: &mut dyn Terminal,
    report: &PlanReport,
    history: &str,
    session_id: &str,
    construct: impl FnOnce(&Authorized) -> T,
) -> Result<T, Refusal> {
    let token = authorize(env, terminal, report, history, session_id)?;
    Ok(construct(&token))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::build_plan;
    use crate::registry::load_registry;
    use maia_domain::ReasoningAssuranceLevel;
    use std::cell::Cell;
    use std::collections::HashSet;

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
    fn env(keys: &[&'static str]) -> FakeEnv {
        FakeEnv(keys.iter().copied().collect())
    }

    struct FakeTerminal {
        interactive: bool,
        reply: Option<String>,
        asked: Vec<String>,
    }
    impl FakeTerminal {
        fn typing(reply: &str) -> Self {
            Self {
                interactive: true,
                reply: Some(reply.to_owned()),
                asked: vec![],
            }
        }
    }
    impl Terminal for FakeTerminal {
        fn is_interactive(&self) -> bool {
            self.interactive
        }
        fn ask(&mut self, text: &str) -> Option<String> {
            self.asked.push(text.to_owned());
            self.reply.clone()
        }
    }

    fn report() -> PlanReport {
        let loaded = load_registry(PANEL).unwrap();
        build_plan(&loaded, ReasoningAssuranceLevel::A3, "s", "prompt").unwrap()
    }

    /// The counting resolver: `construct` bumps the counter. Zero means no
    /// provider would have been built.
    fn run_with(
        env: &dyn Env,
        term: &mut FakeTerminal,
        session_id: &str,
    ) -> (Result<(), Refusal>, usize) {
        let built = Cell::new(0usize);
        let r = authorize_then(env, term, &report(), "hist", session_id, |_| {
            built.set(built.get() + 1);
        });
        (r, built.get())
    }

    #[test]
    fn marker_names_match_the_documented_contract() {
        assert_eq!(CLAUDE_CODE_SESSION_MARKER, "CLAUDECODE");
        assert_eq!(PROVIDER_MARKER, "MAIA_CLAUDE_CODE_PROVIDER_ACTIVE");
        assert_eq!(OWN_MARKER, "MAIA_ROUNDTABLE_INVOKE_ACTIVE");
    }

    #[cfg(feature = "development-evolution")]
    #[test]
    fn provider_marker_matches_the_providers_own_constant() {
        assert_eq!(PROVIDER_MARKER, maia_claude_code::RECURSION_MARKER);
    }

    #[test]
    fn a_clear_environment_and_the_typed_id_authorize_and_construct_once() {
        let mut t = FakeTerminal::typing("rt-000000000001\n");
        let (r, built) = run_with(&env(&[]), &mut t, "rt-000000000001");
        assert!(r.is_ok());
        assert_eq!(built, 1);
        // The plan and the id were shown before anything was built.
        assert!(t.asked[0].contains("MAIA Round Table invocation - plan"));
        assert!(t.asked[0].contains("rt-000000000001"));
        assert!(t.asked[0].contains("execution_authority: false"));
        assert!(t.asked[0].contains("saved in full under `hist`"));
    }

    #[test]
    fn each_guard_alone_refuses_before_the_terminal_is_touched() {
        for marker in [CLAUDE_CODE_SESSION_MARKER, PROVIDER_MARKER, OWN_MARKER] {
            let mut t = FakeTerminal::typing("rt-1\n");
            let (r, built) = run_with(&env(&[marker]), &mut t, "rt-1");
            let Err(Refusal::Guards(tripped)) = r else {
                panic!("{marker} must refuse");
            };
            assert_eq!(tripped.len(), 1, "{marker}: only its own guard trips");
            assert_eq!(tripped[0].marker(), marker);
            assert_eq!(built, 0, "{marker}: nothing may be constructed");
            assert!(
                t.asked.is_empty(),
                "{marker}: no plan is shown when refused"
            );
        }
    }

    #[test]
    fn a_guard_refuses_even_when_everything_else_would_pass() {
        // Interactive terminal, correct id, and still refused: the guards are
        // not a hint the terminal check can outvote.
        let mut t = FakeTerminal::typing("rt-1\n");
        let (r, built) = run_with(&env(&[CLAUDE_CODE_SESSION_MARKER]), &mut t, "rt-1");
        assert_eq!(r.unwrap_err().exit_code(), EXIT_GUARD_REFUSED);
        assert_eq!(built, 0);
    }

    #[test]
    fn all_tripped_guards_are_reported_together() {
        let all = [CLAUDE_CODE_SESSION_MARKER, PROVIDER_MARKER, OWN_MARKER];
        let tripped = check_guards(&env(&all));
        assert_eq!(tripped.len(), 3);
        let text = render_guard_refusal(&tripped);
        for m in all {
            assert!(text.contains(m), "{m}");
        }
        assert!(text.contains("There is no override"));
    }

    #[test]
    fn an_unrelated_environment_is_clear() {
        assert!(check_guards(&env(&["PATH", "HOME", "CLAUDE_CODE_OTHER"])).is_empty());
    }

    #[test]
    fn a_non_interactive_process_cannot_confirm_and_constructs_nothing() {
        let mut t = FakeTerminal {
            interactive: false,
            // Even a perfect scripted answer must not be read.
            reply: Some("rt-1\n".to_owned()),
            asked: vec![],
        };
        let (r, built) = run_with(&env(&[]), &mut t, "rt-1");
        assert_eq!(r, Err(Refusal::ConfirmationUnavailable));
        assert_eq!(r.unwrap_err().exit_code(), EXIT_CONFIRMATION_UNAVAILABLE);
        assert_eq!(built, 0);
        assert!(t.asked.is_empty(), "the prompt must not even be shown");
    }

    #[test]
    fn a_keypress_style_answer_does_not_confirm() {
        for reply in [
            "\n",
            "y\n",
            "yes\n",
            "Y",
            "",
            "rt-1 \n x",
            "RT-1\n",
            "rt-2\n",
        ] {
            let mut t = FakeTerminal::typing(reply);
            let (r, built) = run_with(&env(&[]), &mut t, "rt-1");
            assert_eq!(r, Err(Refusal::ConfirmationDeclined), "{reply:?}");
            assert_eq!(built, 0, "{reply:?}");
        }
    }

    #[test]
    fn end_of_input_declines() {
        let mut t = FakeTerminal {
            interactive: true,
            reply: None,
            asked: vec![],
        };
        let (r, built) = run_with(&env(&[]), &mut t, "rt-1");
        assert_eq!(r, Err(Refusal::ConfirmationDeclined));
        assert_eq!(r.unwrap_err().exit_code(), EXIT_CONFIRMATION_DECLINED);
        assert_eq!(built, 0);
    }

    #[test]
    fn crlf_line_endings_are_accepted() {
        let mut t = FakeTerminal::typing("rt-1\r\n");
        let (r, built) = run_with(&env(&[]), &mut t, "rt-1");
        assert!(r.is_ok());
        assert_eq!(built, 1);
    }

    #[test]
    fn consent_is_per_invocation_and_not_carried_over() {
        let mut t = FakeTerminal::typing("rt-1\n");
        let built = Cell::new(0usize);
        for _ in 0..2 {
            let r = authorize_then(&env(&[]), &mut t, &report(), "h", "rt-1", |_| {
                built.set(built.get() + 1);
            });
            assert!(r.is_ok());
        }
        // Each call asked again; nothing was remembered between them.
        assert_eq!(t.asked.len(), 2);
        assert_eq!(built.get(), 2);
        let mut declined = FakeTerminal::typing("no\n");
        let r = authorize_then(&env(&[]), &mut declined, &report(), "h", "rt-1", |_| {
            built.set(built.get() + 1);
        });
        assert!(r.is_err());
        assert_eq!(
            built.get(),
            2,
            "an earlier yes must not authorize a later run"
        );
    }

    #[test]
    fn the_refusal_text_says_nothing_was_done() {
        for r in [
            Refusal::Guards(vec![GuardRefusal::OwnMarker]),
            Refusal::ConfirmationUnavailable,
            Refusal::ConfirmationDeclined,
        ] {
            assert!(r.render().contains("Nothing was constructed"), "{r:?}");
        }
    }

    #[test]
    fn session_ids_are_store_safe_and_differ() {
        let a = new_session_id(1, 2, 3);
        let b = new_session_id(4, 2, 3);
        assert_ne!(a, b);
        for id in [&a, &b, &fresh_session_id()] {
            assert!(id.starts_with("rt-"));
            assert!(id[3..].chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
            assert!(!id.chars().any(|c| c.is_ascii_uppercase()));
        }
    }

    #[test]
    fn the_same_timestamp_with_a_different_pid_does_not_collide() {
        let now = 1_726_000_000_123_456_789u128;
        assert_ne!(new_session_id(now, 100, 7), new_session_id(now, 101, 7));
        // The old fold shifted the pid above bit 48 and masked it away.
        assert_ne!(new_session_id(now, 1, 0), new_session_id(now, 2, 0));
    }

    #[test]
    fn timestamps_differing_by_two_to_the_48th_do_not_collide() {
        let now = 1_726_000_000_123_456_789u128;
        let later = now + (1u128 << 48);
        assert_ne!(new_session_id(now, 9, 5), new_session_id(later, 9, 5));
        // And by any higher bit of the full reading.
        assert_ne!(
            new_session_id(now, 9, 5),
            new_session_id(now + (1u128 << 100), 9, 5)
        );
    }

    #[test]
    fn the_entropy_word_distinguishes_otherwise_identical_readings() {
        assert_ne!(new_session_id(5, 6, 1), new_session_id(5, 6, 2));
    }

    #[test]
    fn fields_cannot_run_into_each_other() {
        // Separated and fixed-width, so different triples cannot spell one id.
        assert_ne!(new_session_id(0x12, 0x3, 0), new_session_id(0x1, 0x23, 0));
    }

    #[test]
    fn fresh_ids_are_distinct_across_calls() {
        let ids: HashSet<String> = (0..64).map(|_| fresh_session_id()).collect();
        assert_eq!(ids.len(), 64);
    }

    #[cfg(feature = "development-evolution")]
    mod child_marker {
        use super::*;
        use maia_claude_code::{
            ClaudeCodeConfig, ClaudeCodeProvider, RECURSION_MARKER,
            audit::{AuditError, AuditRecord, AuditSink},
            runner::{CancellationToken, Invocation, ProcessError, ProcessOutcome, ProcessRunner},
        };
        use maia_roundtable::{DecisionRequest, Participant, ParticipantRequest};
        use std::sync::Mutex;

        /// Records the invocation it was handed and never starts a process.
        #[derive(Default)]
        struct RecordingRunner(Mutex<Vec<Invocation>>);
        impl ProcessRunner for RecordingRunner {
            fn run(
                &self,
                invocation: &Invocation,
                _: &CancellationToken,
            ) -> Result<ProcessOutcome, ProcessError> {
                self.0.lock().unwrap().push(invocation.clone());
                Err(ProcessError::ExecutableNotFound)
            }
        }
        struct NullAudit;
        impl AuditSink for NullAudit {
            fn persist(&self, _: &AuditRecord) -> Result<(), AuditError> {
                Ok(())
            }
        }

        fn child_invocation() -> Invocation {
            let mut config = ClaudeCodeConfig::new("claude", "wd", "audit");
            config.additional_child_markers.push(own_child_marker());
            let provider = ClaudeCodeProvider::new_without_recursion_check(
                "claude-code",
                config,
                "2.1.273".into(),
                RecordingRunner::default(),
                NullAudit,
            )
            .expect("provider constructs");
            let _ = provider.invoke(ParticipantRequest {
                session_id: "rt-1".into(),
                decision: DecisionRequest {
                    id: "rt-1".into(),
                    subject: "s".into(),
                    prompt: "p".into(),
                    evidence: vec![],
                },
                max_output_tokens: 16,
            });
            let seen = provider.runner_for_test().0.lock().unwrap();
            assert_eq!(seen.len(), 1, "exactly one child was described");
            seen[0].clone()
        }

        #[test]
        fn the_child_receives_both_recursion_markers() {
            let inv = child_invocation();
            assert_eq!(
                inv.env_set.get(PROVIDER_MARKER).map(String::as_str),
                Some("1")
            );
            assert_eq!(inv.env_set.get(OWN_MARKER).map(String::as_str), Some("1"));
            assert_eq!(RECURSION_MARKER, PROVIDER_MARKER);
            assert_eq!(inv.env_set.len(), 2, "no other variable is passed through");
        }

        #[test]
        fn marking_the_child_does_not_touch_this_process() {
            let _ = child_invocation();
            assert!(std::env::var_os(OWN_MARKER).is_none());
            assert!(std::env::var_os(PROVIDER_MARKER).is_none());
        }

        #[test]
        fn an_invoker_started_inside_that_child_refuses() {
            struct ChildEnv(Vec<String>);
            impl Env for ChildEnv {
                fn is_set(&self, key: &str) -> bool {
                    self.0.iter().any(|k| k == key)
                }
            }
            let inv = child_invocation();
            let nested = ChildEnv(inv.env_set.keys().cloned().collect());
            let tripped = check_guards(&nested);
            assert!(tripped.contains(&GuardRefusal::OwnMarker));
            assert!(tripped.contains(&GuardRefusal::ProviderMarker));
        }
    }

    #[test]
    fn exit_codes_are_distinct_from_the_plan_codes() {
        let codes = [
            crate::cli::EXIT_OK,
            crate::cli::EXIT_USAGE,
            crate::cli::EXIT_REFUSED,
            crate::cli::EXIT_INPUT_UNREADABLE,
            EXIT_GUARD_REFUSED,
            EXIT_CONFIRMATION_UNAVAILABLE,
            EXIT_CONFIRMATION_DECLINED,
        ];
        let unique: HashSet<_> = codes.iter().collect();
        assert_eq!(unique.len(), codes.len());
    }
}
