//! Argument parsing and the `plan` command.
//!
//! Exit codes: 0 ok, 2 usage error, 3 refused (the request cannot be planned),
//! 4 an input file could not be read.

use crate::host::HostConfig;
use crate::plan::{MAX_PROMPT_BYTES, build_plan, render_plan};
use crate::registry::{MAX_REGISTRY_BYTES, load_registry, parse_level};
use maia_domain::ReasoningAssuranceLevel;
use std::io::Read;

pub const EXIT_OK: i32 = 0;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_REFUSED: i32 = 3;
pub const EXIT_INPUT_UNREADABLE: i32 = 4;

pub const USAGE: &str = "\
usage: maia-roundtable-invoke plan      --registry <file> --level A3 --prompt-file <file> --subject <text>
       maia-roundtable-invoke preflight <plan options> <host options> --history <dir>
       maia-roundtable-invoke run       <plan options> <host options> --history <dir>

host options (all required, no defaults, no search, no environment fallback):
       --claude-exe <absolute path>       the Claude Code executable
       --claude-audit-dir <absolute dir>  where the provider writes its audit records
       --working-dir <absolute dir>       the child's working directory
       --local-endpoint <loopback ip:port> the local model, e.g. 127.0.0.1:11434

plan      Dry run. Shows who would be asked, how many calls, what it would cost and
          what authority it has. Constructs no provider, calls nothing, writes nothing.
preflight Checks that `run` could be set up, without calling any model: registry,
          plan, executable, `--version`, history and audit directories, loopback
          endpoint, recursion markers, participant identities. It may run
          `<claude-exe> --version` (bounded) and create/probe the history and audit
          directories; it never invokes a model and saves no session, so it asks for
          no confirmation. Refuses inside a Claude Code session. Needs a build with
          the `development-evolution` feature.
run       Asks the participants, after a typed confirmation, and saves the session
          under --history. Needs a build with the `development-evolution` feature,
          a real terminal, and no ambient Claude Code session.

Every option is required: there is no default registry, no default history and no
environment lookup. The participant model is the registry `model_ref`. A participant
not named in the registry file is unavailable. Only A3 is supported; A4 is refused,
not downgraded. This tool gives advice only: execution_authority is always false.
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Plan(PlanArgs),
    Run(RunArgs),
    /// Same arguments as `run`: it checks exactly what `run` would use.
    Preflight(RunArgs),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunArgs {
    pub plan: PlanArgs,
    pub history: String,
    pub host: HostConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanArgs {
    pub registry: String,
    pub level: ReasoningAssuranceLevel,
    pub prompt_file: String,
    pub subject: String,
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
    pub(crate) fn fail(code: i32, stderr: String) -> Self {
        Self {
            stdout: String::new(),
            stderr,
            code,
        }
    }
    fn usage(problem: &str) -> Self {
        Self::fail(EXIT_USAGE, format!("{problem}\n\n{USAGE}"))
    }
}

/// Parse arguments (without the program name). Anything unrecognised, repeated
/// or missing is a usage error rather than being ignored or defaulted.
pub fn parse(args: &[String]) -> Result<Command, String> {
    let (command, rest) = args.split_first().ok_or("no command given")?;
    let (is_run, is_preflight) = match command.as_str() {
        "plan" => (false, false),
        "run" => (true, false),
        "preflight" => (false, true),
        other => return Err(format!("unknown command `{other}`")),
    };
    // Only `run` and `preflight` write or start anything, so only they take a
    // history location and the host configuration.
    let takes_host = is_run || is_preflight;
    let mut registry = None;
    let mut level = None;
    let mut prompt_file = None;
    let mut subject = None;
    let mut history = None;
    let mut claude_exe = None;
    let mut claude_audit_dir = None;
    let mut working_dir = None;
    let mut local_endpoint = None;
    let mut it = rest.iter();
    while let Some(flag) = it.next() {
        let slot: &mut Option<String> = match flag.as_str() {
            "--registry" => &mut registry,
            "--level" => &mut level,
            "--prompt-file" => &mut prompt_file,
            "--subject" => &mut subject,
            "--history" if takes_host => &mut history,
            "--claude-exe" if takes_host => &mut claude_exe,
            "--claude-audit-dir" if takes_host => &mut claude_audit_dir,
            "--working-dir" if takes_host => &mut working_dir,
            "--local-endpoint" if takes_host => &mut local_endpoint,
            f if f.starts_with("--") => return Err(format!("unknown option `{f}`")),
            other => return Err(format!("unexpected argument `{other}`")),
        };
        if slot.is_some() {
            return Err(format!("`{flag}` was given more than once"));
        }
        match it.next() {
            Some(v) if !v.trim().is_empty() => *slot = Some(v.clone()),
            _ => return Err(format!("`{flag}` needs a value")),
        }
    }
    let need = |v: Option<String>, name: &str| {
        v.ok_or_else(|| format!("{name} is required; there is no default"))
    };
    let level = need(level, "--level")?;
    let level = parse_level(&level)
        .ok_or_else(|| format!("unknown assurance level `{level}` (known: A0-A4)"))?;
    let plan = PlanArgs {
        registry: need(registry, "--registry")?,
        level,
        prompt_file: need(prompt_file, "--prompt-file")?,
        subject: need(subject, "--subject")?,
    };
    if takes_host {
        let history = need(history, "--history")?;
        let host = HostConfig::parse(
            &need(claude_exe, "--claude-exe")?,
            &need(claude_audit_dir, "--claude-audit-dir")?,
            &need(working_dir, "--working-dir")?,
            &need(local_endpoint, "--local-endpoint")?,
        )
        .map_err(|e| e.to_string())?;
        let args = RunArgs {
            plan,
            history,
            host,
        };
        Ok(if is_run {
            Command::Run(args)
        } else {
            Command::Preflight(args)
        })
    } else {
        Ok(Command::Plan(plan))
    }
}

/// Read a file no larger than `cap` bytes. One byte more than the cap is read so
/// an oversized file is detected rather than silently truncated.
pub(crate) fn read_capped(path: &str, cap: usize) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("cannot read `{path}`: {e}"))?;
    let mut buf = Vec::new();
    file.take(cap as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|e| format!("cannot read `{path}`: {e}"))?;
    if buf.len() > cap {
        return Err(format!("`{path}` exceeds {cap} bytes"));
    }
    String::from_utf8(buf).map_err(|_| format!("`{path}` is not valid UTF-8"))
}

/// Plan from the text of the inputs. Pure: the seam the tests use.
pub fn plan_from_text(
    registry_text: &str,
    prompt: &str,
    level: ReasoningAssuranceLevel,
    subject: &str,
) -> Output {
    let loaded = match load_registry(registry_text) {
        Ok(l) => l,
        Err(e) => return Output::fail(EXIT_REFUSED, format!("registry refused: {e}\n")),
    };
    match build_plan(&loaded, level, subject, prompt) {
        Ok(report) => Output::ok(render_plan(&report, None)),
        Err(e) => Output::fail(EXIT_REFUSED, format!("plan refused: {e}\n")),
    }
}

pub fn run(args: &[String]) -> Output {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Output::ok(USAGE.to_owned());
    }
    match parse(args) {
        Err(problem) => Output::usage(&problem),
        // `run` and `preflight` need the live builder, which exists only in a
        // `development-evolution` build. Without it they refuse before any file
        // is read or anyone is asked.
        #[cfg(feature = "development-evolution")]
        Ok(Command::Run(a)) => crate::live::run_command(&a),
        #[cfg(feature = "development-evolution")]
        Ok(Command::Preflight(a)) => crate::live::preflight_command(&a),
        #[cfg(not(feature = "development-evolution"))]
        Ok(Command::Run(_) | Command::Preflight(_)) => Output::fail(
            crate::run::EXIT_NOT_CONSTRUCTED,
            crate::run::BuildError::NotWired.render(),
        ),
        Ok(Command::Plan(a)) => {
            let registry = match read_capped(&a.registry, MAX_REGISTRY_BYTES) {
                Ok(t) => t,
                Err(e) => return Output::fail(EXIT_INPUT_UNREADABLE, format!("{e}\n")),
            };
            let prompt = match read_capped(&a.prompt_file, MAX_PROMPT_BYTES) {
                Ok(t) => t,
                Err(e) => return Output::fail(EXIT_INPUT_UNREADABLE, format!("{e}\n")),
            };
            plan_from_text(&registry, &prompt, a.level, &a.subject)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    fn full() -> Vec<&'static str> {
        vec![
            "plan",
            "--registry",
            "r.json",
            "--level",
            "A3",
            "--prompt-file",
            "p.txt",
            "--subject",
            "s",
        ]
    }

    const PANEL: &str = r#"{"participants":[
        {"id":"a","adapter":"claude-code","provider":"pa","model_ref":"ma",
         "roles":["round_table_member","adjudicator"],"assurance_levels":["A3"],"enabled":true},
        {"id":"b","adapter":"local-model","provider":"pb","model_ref":"mb",
         "roles":["round_table_member","adjudicator"],"assurance_levels":["A3"],"enabled":true}]}"#;

    #[test]
    fn parses_a_complete_plan_command() {
        let Command::Plan(a) = parse(&args(&full())).unwrap() else {
            panic!("expected plan");
        };
        assert_eq!(a.level, ReasoningAssuranceLevel::A3);
        assert_eq!(a.registry, "r.json");
        assert_eq!(a.prompt_file, "p.txt");
        assert_eq!(a.subject, "s");
    }

    #[test]
    fn every_option_is_required() {
        for skip in ["--registry", "--level", "--prompt-file", "--subject"] {
            let mut v = full();
            let i = v.iter().position(|x| *x == skip).unwrap();
            v.drain(i..=i + 1);
            let err = parse(&args(&v)).unwrap_err();
            assert!(err.contains("required"), "{skip}: {err}");
        }
    }

    #[test]
    fn unknown_repeated_and_valueless_options_are_usage_errors() {
        let mut v = full();
        v.extend(["--yes", "x"]);
        assert!(parse(&args(&v)).unwrap_err().contains("unknown option"));
        let mut v = full();
        v.extend(["--level", "A3"]);
        assert!(parse(&args(&v)).unwrap_err().contains("more than once"));
        let mut v = full();
        v.truncate(v.len() - 1);
        assert!(parse(&args(&v)).unwrap_err().contains("needs a value"));
        assert!(
            parse(&args(&["plan", "stray"]))
                .unwrap_err()
                .contains("unexpected")
        );
        assert!(parse(&args(&[])).unwrap_err().contains("no command"));
    }

    #[test]
    fn there_is_no_way_to_skip_confirmation_or_pick_a_default() {
        // No --yes, no --history for plan, no environment override: all unknown.
        for flag in ["--yes", "-y", "--history", "--confirm", "--force"] {
            let mut v = full();
            v.extend([flag, "x"]);
            assert!(parse(&args(&v)).is_err(), "{flag} must not be accepted");
        }
    }

    #[test]
    fn unknown_command_and_level_are_usage_errors() {
        assert!(
            parse(&args(&["frobnicate"]))
                .unwrap_err()
                .contains("unknown command")
        );
        let mut v = full();
        let i = v.iter().position(|x| *x == "A3").unwrap();
        v[i] = "A9";
        assert!(
            parse(&args(&v))
                .unwrap_err()
                .contains("unknown assurance level")
        );
    }

    #[cfg(windows)]
    const ABS: &str = "C:\\maia\\x";
    #[cfg(not(windows))]
    const ABS: &str = "/maia/x";

    fn host_options() -> Vec<&'static str> {
        vec![
            "--claude-exe",
            ABS,
            "--claude-audit-dir",
            ABS,
            "--working-dir",
            ABS,
            "--local-endpoint",
            "127.0.0.1:11434",
        ]
    }

    fn run_args() -> Vec<&'static str> {
        let mut v = full();
        v[0] = "run";
        v.extend(host_options());
        v.extend(["--history", "h"]);
        v
    }

    #[test]
    fn run_requires_an_explicit_history_and_only_run_takes_one() {
        let Command::Run(a) = parse(&args(&run_args())).unwrap() else {
            panic!("expected run");
        };
        assert_eq!(a.history, "h");
        assert_eq!(a.plan.level, ReasoningAssuranceLevel::A3);

        let mut v = run_args();
        v.truncate(v.len() - 2);
        assert!(
            parse(&args(&v))
                .unwrap_err()
                .contains("--history is required")
        );

        let mut v = run_args();
        v.extend(["--history", "again"]);
        assert!(parse(&args(&v)).unwrap_err().contains("more than once"));

        let mut plan = full();
        plan.extend(["--history", "h"]);
        assert!(parse(&args(&plan)).unwrap_err().contains("unknown option"));
    }

    #[test]
    fn preflight_takes_exactly_the_arguments_run_takes() {
        let mut v = run_args();
        v[0] = "preflight";
        let Command::Preflight(p) = parse(&args(&v)).unwrap() else {
            panic!("expected preflight");
        };
        let Command::Run(r) = parse(&args(&run_args())).unwrap() else {
            panic!("expected run");
        };
        assert_eq!(p, r, "preflight checks what run would use");
    }

    #[test]
    fn every_host_option_is_required_and_only_live_commands_take_them() {
        for skip in [
            "--claude-exe",
            "--claude-audit-dir",
            "--working-dir",
            "--local-endpoint",
        ] {
            let mut v = run_args();
            let i = v.iter().position(|x| *x == skip).unwrap();
            v.drain(i..=i + 1);
            let err = parse(&args(&v)).unwrap_err();
            assert!(
                err.contains(&format!("{skip} is required")),
                "{skip}: {err}"
            );
        }
        for flag in [
            "--claude-exe",
            "--claude-audit-dir",
            "--working-dir",
            "--local-endpoint",
        ] {
            let mut v = full();
            v.extend([flag, ABS]);
            assert!(
                parse(&args(&v)).unwrap_err().contains("unknown option"),
                "plan must not accept {flag}"
            );
        }
    }

    #[test]
    fn a_non_loopback_local_endpoint_is_a_usage_error_at_parse_time() {
        for bad in ["0.0.0.0:11434", "192.168.0.5:11434", "localhost:11434"] {
            let mut v = run_args();
            let i = v.iter().position(|x| *x == "--local-endpoint").unwrap();
            v[i + 1] = bad;
            let err = parse(&args(&v)).unwrap_err();
            assert!(err.contains("--local-endpoint"), "{bad}: {err}");
        }
    }

    #[test]
    fn a_relative_claude_executable_is_refused_so_path_is_never_searched() {
        let mut v = run_args();
        let i = v.iter().position(|x| *x == "--claude-exe").unwrap();
        v[i + 1] = "claude";
        assert!(parse(&args(&v)).unwrap_err().contains("absolute"));
    }

    #[test]
    fn run_has_no_way_to_skip_confirmation() {
        for flag in ["--yes", "-y", "--confirm", "--force", "--non-interactive"] {
            let mut v = run_args();
            v.extend([flag, "x"]);
            assert!(parse(&args(&v)).is_err(), "{flag} must not be accepted");
        }
    }

    /// Without `development-evolution` there is no live builder, so both live
    /// commands refuse before reading a file or asking anyone. (With the feature
    /// they are the real commands, and are exercised through their injectable
    /// seams in `live`, never through a real terminal from a test.)
    #[cfg(not(feature = "development-evolution"))]
    #[test]
    fn a_build_without_the_feature_refuses_run_and_preflight_and_touches_nothing() {
        let dir = std::env::temp_dir().join(format!("maia-invoke-notwired-{}", std::process::id()));
        let history = dir.join("history");
        for command in ["run", "preflight"] {
            let mut v = run_args();
            v[0] = command;
            let i = v.iter().position(|x| *x == "--history").unwrap();
            v[i + 1] = history.to_str().unwrap();
            let out = run(&args(&v));
            assert_eq!(out.code, crate::run::EXIT_NOT_CONSTRUCTED);
            assert!(out.stderr.contains("not wired"));
            assert!(out.stdout.is_empty());
            assert!(!dir.exists(), "a refused run creates no history directory");
        }
    }

    #[test]
    fn plan_succeeds_and_prints_the_dry_run() {
        let out = plan_from_text(PANEL, "prompt", ReasoningAssuranceLevel::A3, "subject");
        assert_eq!(out.code, EXIT_OK);
        assert!(out.stderr.is_empty());
        assert!(out.stdout.contains("execution_authority: false"));
        assert!(out.stdout.contains("dry run"));
    }

    #[test]
    fn plan_refusals_use_the_refused_exit_code() {
        let a4 = plan_from_text(PANEL, "p", ReasoningAssuranceLevel::A4, "s");
        assert_eq!(a4.code, EXIT_REFUSED);
        assert!(a4.stderr.contains("not downgraded"));
        assert!(a4.stdout.is_empty());
        let bad = plan_from_text("{}", "p", ReasoningAssuranceLevel::A3, "s");
        assert_eq!(bad.code, EXIT_REFUSED);
        assert!(bad.stderr.contains("registry refused"));
    }

    #[test]
    fn missing_input_files_are_reported_and_nothing_is_created() {
        let dir = std::env::temp_dir().join(format!("maia-invoke-cli-{}", std::process::id()));
        let missing = dir.join("nope.json");
        let out = run(&args(&[
            "plan",
            "--registry",
            missing.to_str().unwrap(),
            "--level",
            "A3",
            "--prompt-file",
            missing.to_str().unwrap(),
            "--subject",
            "s",
        ]));
        assert_eq!(out.code, EXIT_INPUT_UNREADABLE);
        assert!(!dir.exists(), "a read path must not create directories");
    }

    #[test]
    fn plan_end_to_end_through_files_writes_nothing() {
        let dir = std::env::temp_dir().join(format!("maia-invoke-e2e-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let reg = dir.join("registry.json");
        let prompt = dir.join("prompt.txt");
        std::fs::write(&reg, PANEL).unwrap();
        std::fs::write(&prompt, "what should we do").unwrap();
        let before: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();

        let out = run(&args(&[
            "plan",
            "--registry",
            reg.to_str().unwrap(),
            "--level",
            "A3",
            "--prompt-file",
            prompt.to_str().unwrap(),
            "--subject",
            "s",
        ]));
        assert_eq!(out.code, EXIT_OK, "{}", out.stderr);
        let mut after: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        let mut before = before;
        before.sort();
        after.sort();
        assert_eq!(before, after, "plan must not write anything");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_oversized_prompt_file_is_refused_not_truncated() {
        let dir = std::env::temp_dir().join(format!("maia-invoke-big-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let big = dir.join("big.txt");
        std::fs::write(&big, "x".repeat(MAX_PROMPT_BYTES + 1)).unwrap();
        let err = read_capped(big.to_str().unwrap(), MAX_PROMPT_BYTES).unwrap_err();
        assert!(err.contains("exceeds"));
        std::fs::write(&big, "x".repeat(MAX_PROMPT_BYTES)).unwrap();
        assert!(read_capped(big.to_str().unwrap(), MAX_PROMPT_BYTES).is_ok());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
