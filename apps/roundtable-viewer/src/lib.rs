//! M0.15.4 — read-only command-line viewer for Round Table session history.
//!
//! ```text
//! maia-roundtable-viewer --history <dir> list
//! maia-roundtable-viewer --history <dir> show <session-id>
//! ```
//!
//! Read-only by construction: it reads through `SessionQuery` and renders with
//! the pure functions in `maia-roundtable-observability`. It never saves, never
//! creates the history directory, and cannot reach a provider.
//!
//! The history location is always explicit. There is no default path, no
//! environment lookup and no search, because a viewer that guessed where history
//! lives could show the wrong history, or none, without saying so.
//!
//! Exit codes: 0 shown, 1 no such session, 2 usage error, 3 history could not
//! be read. An empty history is 0: it is healthy, not a fault.
#![forbid(unsafe_code)]

use maia_roundtable::SessionQuery;
use maia_roundtable_observability::render::{
    render_missing, render_panel, render_state, render_unavailable,
};
use maia_roundtable_observability::{load_panel, load_state};
use maia_roundtable_store::StoredSessionQuery;

pub const EXIT_OK: i32 = 0;
pub const EXIT_NOT_FOUND: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_HISTORY_UNAVAILABLE: i32 = 3;

pub const USAGE: &str = "\
usage: maia-roundtable-viewer --history <dir> list
       maia-roundtable-viewer --history <dir> show <session-id>

Read-only. Shows Round Table session history; it cannot run a session, change
one, or act on a conclusion. --history is required: there is no default.
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    List,
    Show(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub history: String,
    pub command: Command,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

impl Output {
    fn ok(stdout: String) -> Self {
        Self {
            stdout,
            stderr: String::new(),
            code: EXIT_OK,
        }
    }
    fn usage(problem: &str) -> Self {
        Self {
            stdout: String::new(),
            stderr: format!("{problem}\n\n{USAGE}"),
            code: EXIT_USAGE,
        }
    }
}

/// Parse arguments (without the program name). Anything not recognised is a
/// usage error rather than being ignored, so a mistyped flag cannot silently
/// change what is read.
pub fn parse(args: &[String]) -> Result<Invocation, String> {
    let mut history: Option<String> = None;
    let mut rest: Vec<&str> = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--history" => match it.next() {
                Some(v) if !v.trim().is_empty() => history = Some(v.clone()),
                _ => return Err("--history needs a directory".into()),
            },
            flag if flag.starts_with("--") => return Err(format!("unknown option `{flag}`")),
            other => rest.push(other),
        }
    }
    let history = history.ok_or("--history is required; there is no default location")?;
    let command = match rest.as_slice() {
        ["list"] => Command::List,
        ["show", id] if !id.trim().is_empty() => Command::Show((*id).to_owned()),
        ["show"] => return Err("`show` needs a session id".into()),
        [] => return Err("no command given".into()),
        [other, ..] => return Err(format!("unknown command `{other}`")),
    };
    Ok(Invocation { history, command })
}

/// Run a command against any read contract.
pub fn execute(query: &dyn SessionQuery, command: &Command) -> Output {
    match command {
        Command::List => {
            let state = load_state(query);
            let code = if state.availability.is_available() {
                EXIT_OK
            } else {
                EXIT_HISTORY_UNAVAILABLE
            };
            Output {
                stdout: render_state(&state),
                stderr: String::new(),
                code,
            }
        }
        Command::Show(id) => match load_panel(query, id) {
            Ok(Some(panel)) => Output::ok(render_panel(&panel)),
            Ok(None) => Output {
                stdout: String::new(),
                stderr: render_missing(id),
                code: EXIT_NOT_FOUND,
            },
            Err(availability) => Output {
                stdout: String::new(),
                stderr: render_unavailable(&availability),
                code: EXIT_HISTORY_UNAVAILABLE,
            },
        },
    }
}

/// Entry point: parse, open the history read-only, execute.
pub fn run(args: &[String]) -> Output {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Output::ok(USAGE.to_owned());
    }
    match parse(args) {
        Err(problem) => Output::usage(&problem),
        Ok(inv) => execute(&StoredSessionQuery::new(&inv.history), &inv.command),
    }
}
