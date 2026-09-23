//! M0.15.7 - the first explicit, human-initiated Round Table invocation path.
//!
//! ```text
//! maia-roundtable-invoke plan --registry <file> --level A3 --prompt-file <file> --subject <text>
//! ```
//!
//! `plan` is the dry run: it resolves who would be asked and prints what the user
//! would be told before any live call. It constructs no provider, makes no call,
//! and writes nothing, so it needs no confirmation.
//!
//! Every input is explicit. There is no default registry, no default history
//! directory and no environment lookup for either, because a tool that guessed
//! could ask the wrong participants without saying so.
//!
//! `run` (see [`run`]) is the pipeline that asks the participants and saves the
//! session. In a `development-evolution` build the binary wires it to the live
//! builder in `live` (Claude Code plus a loopback local model, host configuration
//! from explicit options, see [`host`]); `preflight` checks that wiring without
//! calling any model. Without the feature both refuse up front and the crate
//! contains no provider code at all. The pipeline itself is exercised with fakes.
//!
//! This crate grants no execution authority. It can trigger reasoning and show
//! the result; nothing here acts on a synthesis (ADR-0046, `execution_authority`
//! is false on every path).
//!
//! Live execution must come from a process that is not itself a Claude Code
//! session: the provider refuses to run recursively, and a nested autonomous turn
//! must never attempt it.
#![forbid(unsafe_code)]

pub mod cli;
pub mod gate;
pub mod host;
#[cfg(feature = "development-evolution")]
pub mod inspect;
#[cfg(feature = "development-evolution")]
pub mod live;
pub mod plan;
pub mod registry;
pub mod run;
