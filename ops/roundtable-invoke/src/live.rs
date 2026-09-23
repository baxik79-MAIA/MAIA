//! The live builder and the preflight (M0.15.7d-prep). Development-evolution only.
//!
//! This module is the *trusted construction boundary* for real providers (see
//! `run`): it is the only place in the invoker that names a provider, and it is
//! reached only through the closure `execute_run` calls after the guards, the
//! plan, the history checks and the typed confirmation.
//!
//! Composition, all through existing public boundaries and nothing new in Core:
//!
//! | registry `adapter` | participant |
//! |---|---|
//! | `claude-code` | `ClaudeCodeProvider::new(id, ClaudeCodeConfig, cli_version, runner, FileAuditSink)` |
//! | `local-model` | `LocalModelParticipant::new(descriptor, LoopbackLocalProvider::new(endpoint, model))` |
//!
//! The model of each participant is the registry `model_ref`; the host
//! configuration (`--claude-exe`, `--claude-audit-dir`, `--working-dir`,
//! `--local-endpoint`) is explicit and never defaulted, searched or read from
//! the environment (see [`crate::host`]).
//!
//! The Claude child receives `MAIA_CLAUDE_CODE_PROVIDER_ACTIVE=1` from the
//! provider and `MAIA_ROUNDTABLE_INVOKE_ACTIVE=1` through the provider's typed
//! `ChildMarker` configuration ([`claude_config`]); this process's own
//! environment is never modified.
//!
//! `claude --version` is bounded local executable inspection, not a
//! consultation. It runs through [`crate::inspect`], a dedicated runner with one
//! end-to-end deadline and capped capture, not through the consultation runner.
//!
//! [`execute_preflight`] proves the composition can be set up without calling a
//! model: it constructs every participant and checks its identity against the
//! registry, and never invokes one. It needs the recursion guards to be clear but
//! no typed confirmation, because it makes no model call and spends nothing.
//! `run` keeps its own independent typed session-id confirmation.

use crate::cli::{EXIT_INPUT_UNREADABLE, EXIT_OK, EXIT_REFUSED, Output, RunArgs, read_capped};
use crate::gate::{
    Authorized, Env, OWN_MARKER, PROVIDER_MARKER, ProcessEnv, Refusal, StdTerminal, Terminal,
    check_guards, fresh_session_id, own_child_marker,
};
use crate::host::HostConfig;
use crate::inspect::{InspectSpec, Inspector, Limits, StdInspector};
use crate::plan::{MAX_PROMPT_BYTES, build_plan};
use crate::registry::{Adapter, LoadedRegistry, MAX_REGISTRY_BYTES, load_registry};
use crate::run::{
    BuildError, EXIT_NOT_CONSTRUCTED, EXIT_PREFLIGHT_FAILED, FileHistory, History, Participants,
    RunRequest, execute_run, participants_with_leader, probe_dir_writable, remove_probe_file,
};
use maia_claude_code::{
    ClaudeCodeConfig, ClaudeCodeProvider, SCRUBBED_ENVIRONMENT,
    audit::FileAuditSink,
    runner::{
        CancellationToken, Invocation, ProcessError, ProcessOutcome, ProcessRunner,
        StdProcessRunner,
    },
};
use maia_domain::ReasoningAssuranceLevel;
use maia_local_model::LoopbackLocalProvider;
use maia_local_model_roundtable::LocalModelParticipant;
use maia_roundtable::{
    Clock, Participant, ParticipantDescriptor, ParticipantResolver, ResolutionFailure, SystemClock,
    resolve_verified, select_leader,
};
use maia_roundtable_composition::{leader_candidates, round_table_descriptors};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

/// End-to-end bound for `--version`: start, exit, capture and cleanup.
const VERSION_DEADLINE: Duration = Duration::from_secs(10);
/// Cap on each of the version query's output streams.
const VERSION_MAX_BYTES: usize = 4096;
const MAX_VERSION_CHARS: usize = 200;

/// A process runner that can be cloned into every provider the resolver builds.
/// Production uses the provider's own `StdProcessRunner`; tests use a fake that
/// starts nothing. Only consultations go through it; `--version` does not.
#[derive(Clone)]
pub struct SharedRunner(Arc<dyn ProcessRunner>);

impl SharedRunner {
    pub fn new(runner: impl ProcessRunner + 'static) -> Self {
        Self(Arc::new(runner))
    }

    pub fn standard() -> Self {
        Self::new(StdProcessRunner)
    }
}

impl ProcessRunner for SharedRunner {
    fn run(
        &self,
        invocation: &Invocation,
        cancel: &CancellationToken,
    ) -> Result<ProcessOutcome, ProcessError> {
        self.0.run(invocation, cancel)
    }
}

/// The Claude Code configuration for one participant. `model` is the registry
/// `model_ref`; the working directory, executable and audit directory are the
/// explicit host configuration; the invoker's own recursion marker is set on the
/// child through the provider's typed configuration.
pub fn claude_config(host: &HostConfig, model: &str) -> ClaudeCodeConfig {
    let mut config =
        ClaudeCodeConfig::new(&host.claude_exe, &host.working_dir, &host.claude_audit_dir);
    config.model = Some(model.to_owned());
    config.additional_child_markers.push(own_child_marker());
    config
}

/// Run `<claude-exe> --version` under the bounded inspector and return its first
/// line. Not a model call: no prompt reaches the child (its stdin is the null
/// device), and the executable is exactly the supplied path. The scratch files
/// for the capture live in the explicit audit directory.
pub fn query_cli_version(inspector: &dyn Inspector, host: &HostConfig) -> Result<String, String> {
    let mut env_set = BTreeMap::new();
    env_set.insert(PROVIDER_MARKER.to_owned(), "1".to_owned());
    env_set.insert(OWN_MARKER.to_owned(), "1".to_owned());
    let spec = InspectSpec {
        program: host.claude_exe.clone(),
        args: vec!["--version".to_owned()],
        working_directory: host.working_dir.clone(),
        scratch_dir: host.claude_audit_dir.clone(),
        env_remove: SCRUBBED_ENVIRONMENT
            .iter()
            .map(|s| (*s).to_owned())
            .collect(),
        env_set,
        limits: Limits {
            deadline: VERSION_DEADLINE,
            max_stdout_bytes: VERSION_MAX_BYTES,
            max_stderr_bytes: VERSION_MAX_BYTES,
        },
    };
    let got = inspector
        .inspect(&spec)
        .map_err(|e| format!("`--version` inspection failed: {e}"))?;
    if got.exit_code != Some(0) {
        return Err(format!("`--version` exited with {:?}", got.exit_code));
    }
    let text = String::from_utf8(got.stdout)
        .map_err(|_| "`--version` printed output that is not UTF-8".to_owned())?;
    let line = text.lines().next().unwrap_or("").trim();
    if line.is_empty()
        || line.chars().count() > MAX_VERSION_CHARS
        || line.chars().any(char::is_control)
    {
        return Err("`--version` printed nothing usable".to_owned());
    }
    Ok(line.to_owned())
}

/// Builds each participant from its registry `adapter`. The one place where a
/// provider is named; composition and orchestration never are.
pub struct LiveResolver {
    adapters: Vec<(String, Adapter)>,
    host: HostConfig,
    cli_version: String,
    runner: SharedRunner,
}

impl LiveResolver {
    pub fn new(
        loaded: &LoadedRegistry,
        host: &HostConfig,
        cli_version: String,
        runner: SharedRunner,
    ) -> Self {
        Self {
            adapters: loaded.adapters.clone(),
            host: host.clone(),
            cli_version,
            runner,
        }
    }
}

impl ParticipantResolver for LiveResolver {
    fn resolve(
        &self,
        descriptor: &ParticipantDescriptor,
    ) -> Result<Box<dyn Participant>, ResolutionFailure> {
        let adapter = self
            .adapters
            .iter()
            .find(|(id, _)| id == descriptor.id.as_str())
            .map(|(_, a)| *a)
            .ok_or(ResolutionFailure::UnknownParticipant)?;
        match adapter {
            Adapter::ClaudeCode => {
                let provider = ClaudeCodeProvider::new(
                    descriptor.id.as_str(),
                    claude_config(&self.host, descriptor.model.as_str()),
                    self.cli_version.clone(),
                    self.runner.clone(),
                    FileAuditSink::new(&self.host.claude_audit_dir),
                )
                .map_err(|_| ResolutionFailure::ProviderUnavailable)?;
                Ok(Box::new(provider))
            }
            Adapter::LocalModel => {
                // Refuses a non-loopback endpoint again, whatever built `host`.
                let provider =
                    LoopbackLocalProvider::new(self.host.local_endpoint, descriptor.model.as_str())
                        .map_err(|_| ResolutionFailure::ProviderUnavailable)?;
                Ok(Box::new(LocalModelParticipant::new(
                    descriptor.clone(),
                    provider,
                )))
            }
        }
    }
}

/// One checked participant, for the preflight report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    pub descriptor: ParticipantDescriptor,
    pub adapter: Adapter,
}

fn describe_failure(descriptor: &ParticipantDescriptor, why: ResolutionFailure) -> String {
    let id = descriptor.id.as_str();
    match why {
        ResolutionFailure::DescriptorMismatch => format!(
            "participant `{id}` would not be the registered identity (registry says provider \
             `{}` model `{}`; the adapter builds a different one). Fix the registry entry, \
             nothing is substituted",
            descriptor.provider.as_str(),
            descriptor.model.as_str()
        ),
        other => format!("participant `{id}` cannot be constructed ({other:?})"),
    }
}

/// Construct every participant a session would use (each member and the leader)
/// through `resolve_verified` and check its identity against the registry. Never
/// invokes one: construction only. Used both before a live run, so a bad member
/// is found before the first call is spent, and by the preflight.
pub fn verify_identities(
    resolver: &dyn ParticipantResolver,
    loaded: &LoadedRegistry,
    level: ReasoningAssuranceLevel,
) -> Result<Vec<Checked>, String> {
    let mut descriptors = round_table_descriptors(&loaded.registry, level)
        .map_err(|e| format!("the registry yields no Round Table members ({e:?})"))?;
    let candidates =
        leader_candidates(&loaded.registry).map_err(|e| format!("no leader candidates ({e:?})"))?;
    let leader = select_leader(level, &candidates).map_err(|e| format!("no leader ({e:?})"))?;
    if !descriptors.contains(&leader.leader) {
        descriptors.push(leader.leader);
    }
    let mut checked = Vec::new();
    for descriptor in descriptors {
        resolve_verified(resolver, &descriptor).map_err(|e| describe_failure(&descriptor, e))?;
        let adapter = loaded
            .adapter_of(descriptor.id.as_str())
            .ok_or_else(|| format!("participant `{}` has no adapter", descriptor.id.as_str()))?;
        checked.push(Checked {
            descriptor,
            adapter,
        });
    }
    Ok(checked)
}

/// The live builder `run` hands to `execute_run`. Runs only after confirmation
/// and after the history directory has been established. Everything here fails
/// before any model call: the executable and working directory exist, the audit
/// directory is writable (an audit write failure would otherwise fail a call
/// that had already been made), the bounded `--version` inspection answers, and
/// every participant is constructible and matches the registry.
pub fn build_live(
    loaded: &LoadedRegistry,
    level: ReasoningAssuranceLevel,
    host: &HostConfig,
    runner: &SharedRunner,
    inspector: &dyn Inspector,
) -> Result<Participants, BuildError> {
    if let Some(problem) = host.local_problem() {
        return Err(BuildError::HostUnusable(problem));
    }
    probe_dir_writable(&host.claude_audit_dir, remove_probe_file).map_err(|e| {
        BuildError::HostUnusable(format!(
            "audit directory `{}`: {e}",
            host.claude_audit_dir.display()
        ))
    })?;
    let version = query_cli_version(inspector, host).map_err(BuildError::HostUnusable)?;
    let resolver = LiveResolver::new(loaded, host, version, runner.clone());
    verify_identities(&resolver, loaded, level).map_err(BuildError::HostUnusable)?;
    participants_with_leader(Box::new(resolver), loaded, level)
}

// ------------------------------------------------------------------ commands

struct Inputs {
    loaded: LoadedRegistry,
    prompt: String,
}

fn read_inputs(a: &RunArgs) -> Result<Inputs, Output> {
    let unreadable = |e: String| Output::fail(EXIT_INPUT_UNREADABLE, format!("{e}\n"));
    let registry = read_capped(&a.plan.registry, MAX_REGISTRY_BYTES).map_err(unreadable)?;
    let prompt = read_capped(&a.plan.prompt_file, MAX_PROMPT_BYTES)
        .map_err(|e| Output::fail(EXIT_INPUT_UNREADABLE, format!("{e}\n")))?;
    let loaded = load_registry(&registry)
        .map_err(|e| Output::fail(EXIT_REFUSED, format!("registry refused: {e}\n")))?;
    Ok(Inputs { loaded, prompt })
}

/// Refuse, before anything is read or opened, when a recursion or ambient
/// guard is active. The inner guards in `execute_run` / `execute_preflight`
/// stay as defense in depth.
fn outer_guard(env: &dyn Env) -> Option<Output> {
    let tripped = check_guards(env);
    if tripped.is_empty() {
        return None;
    }
    let r = Refusal::Guards(tripped);
    Some(Output::fail(r.exit_code(), r.render()))
}

/// `run`, against the real process environment, terminal, clock and executable.
/// Not exercised by tests: they use [`run_command_in`], which takes all of it.
pub fn run_command(a: &RunArgs) -> Output {
    run_command_in(
        a,
        &ProcessEnv,
        &mut StdTerminal,
        &SystemClock,
        &SharedRunner::standard(),
        &StdInspector,
        &fresh_session_id(),
    )
}

/// `preflight`, against the real process environment and executable. There is no
/// terminal parameter: it asks for no confirmation.
pub fn preflight_command(a: &RunArgs) -> Output {
    preflight_command_in(a, &ProcessEnv, &SharedRunner::standard(), &StdInspector)
}

/// `run` with every environment-facing part injected. The outer guard comes first
/// (no input file is opened while a guard is active), then the inputs are read
/// and [`run_live`] takes over.
pub fn run_command_in(
    a: &RunArgs,
    env: &dyn Env,
    terminal: &mut dyn Terminal,
    clock: &dyn Clock,
    runner: &SharedRunner,
    inspector: &dyn Inspector,
    session_id: &str,
) -> Output {
    if let Some(refused) = outer_guard(env) {
        return refused;
    }
    let inputs = match read_inputs(a) {
        Ok(i) => i,
        Err(out) => return out,
    };
    run_live(
        a,
        &inputs.loaded,
        &inputs.prompt,
        env,
        terminal,
        clock,
        runner,
        inspector,
        session_id,
    )
}

/// `preflight` with every environment-facing part injected. Same outer guard.
pub fn preflight_command_in(
    a: &RunArgs,
    env: &dyn Env,
    runner: &SharedRunner,
    inspector: &dyn Inspector,
) -> Output {
    if let Some(refused) = outer_guard(env) {
        return refused;
    }
    let inputs = match read_inputs(a) {
        Ok(i) => i,
        Err(out) => return out,
    };
    execute_preflight(
        &PreflightInput {
            loaded: &inputs.loaded,
            level: a.plan.level,
            subject: &a.plan.subject,
            prompt: &inputs.prompt,
            history: &a.history,
            host: &a.host,
        },
        env,
        inspector,
        &FileHistory::new(&a.history),
        &|version| {
            Box::new(LiveResolver::new(
                &inputs.loaded,
                &a.host,
                version.to_owned(),
                runner.clone(),
            ))
        },
    )
}

/// The `run` pipeline with the host's files checked (metadata only) first, then
/// `execute_run`: plan, history checks, typed confirmation, writability, and only
/// then [`build_live`]. Confirmation is required here and only here.
#[allow(clippy::too_many_arguments)]
pub fn run_live(
    a: &RunArgs,
    loaded: &LoadedRegistry,
    prompt: &str,
    env: &dyn Env,
    terminal: &mut dyn Terminal,
    clock: &dyn Clock,
    runner: &SharedRunner,
    inspector: &dyn Inspector,
    session_id: &str,
) -> Output {
    if let Some(refused) = outer_guard(env) {
        return refused;
    }
    if let Some(problem) = a.host.local_problem() {
        return Output::fail(
            EXIT_NOT_CONSTRUCTED,
            BuildError::HostUnusable(problem).render(),
        );
    }
    execute_run(
        &RunRequest {
            loaded,
            level: a.plan.level,
            subject: &a.plan.subject,
            prompt,
            history: &a.history,
            session_id,
        },
        env,
        terminal,
        clock,
        |_token: &Authorized| build_live(loaded, a.plan.level, &a.host, runner, inspector),
    )
}

// ------------------------------------------------------------------ preflight

pub(crate) struct PreflightInput<'a> {
    pub loaded: &'a LoadedRegistry,
    pub level: ReasoningAssuranceLevel,
    pub subject: &'a str,
    pub prompt: &'a str,
    pub history: &'a str,
    pub host: &'a HostConfig,
}

/// What the preflight may do, stated at the top of every report.
fn preflight_notice(input: &PreflightInput<'_>) -> String {
    format!(
        "MAIA Round Table preflight\n\
         ==========================\n\
         This is a local readiness check. It may:\n\
         \x20 - execute `{exe} --version` (bounded local executable inspection);\n\
         \x20 - perform local filesystem readiness checks: create `{history}` and the Claude\n\
         \x20   audit directory `{audit}` if missing, and write then remove a small probe file;\n\
         \x20 - construct configuration objects and participants, without invoking them.\n\
         It will NOT invoke a model: no consultation, no local-model request, no adjudication,\n\
         and no session is saved. It needs no typed confirmation because it spends nothing.\n\n",
        exe = input.host.claude_exe.display(),
        history = input.history,
        audit = input.host.claude_audit_dir.display(),
    )
}

fn preflight_failed(passed: &str, check: &str, why: &str) -> Output {
    Output {
        stdout: passed.to_owned(),
        stderr: format!(
            "preflight FAILED at {check}: {why}. No model was called and no session was saved.\n"
        ),
        code: EXIT_PREFLIGHT_FAILED,
    }
}

/// Prove the live builder can be configured, without calling a model.
///
/// Order: guards, plan, local checks of the host files, history location, then
/// history and audit directories, the bounded `--version` inspection, and
/// construction and identity checks of every participant. No participant is ever
/// invoked, no session is written, no socket is opened, and nothing is asked of
/// the person: there is no terminal parameter, so there is no way to prompt.
pub(crate) fn execute_preflight(
    input: &PreflightInput<'_>,
    env: &dyn Env,
    inspector: &dyn Inspector,
    history: &dyn History,
    resolver_for: &dyn Fn(&str) -> Box<dyn ParticipantResolver>,
) -> Output {
    if let Some(refused) = outer_guard(env) {
        return refused;
    }
    let report = match build_plan(input.loaded, input.level, input.subject, input.prompt) {
        Ok(r) => r,
        Err(e) => return Output::fail(EXIT_REFUSED, format!("plan refused: {e}\n")),
    };
    let mut passed = preflight_notice(input);
    if let Some(problem) = input.host.local_problem() {
        return preflight_failed(&passed, "host configuration", &problem);
    }
    if let Some(problem) = history.location_problem() {
        return preflight_failed(&passed, "history location", &problem);
    }

    let mut line = |name: &str, detail: String| {
        passed.push_str(&format!("  {name:<20}{detail}\n"));
    };
    line(
        "registry",
        format!("parsed: {} participant(s)", input.loaded.adapters.len()),
    );
    line(
        "plan",
        format!(
            "{} Round Table; members: {}; leader: {}; worst case {} call(s), none made",
            report.plan.required,
            report
                .members
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            report.leader.id,
            report.worst_case_calls()
        ),
    );
    line(
        "claude executable",
        format!("`{}` exists", input.host.claude_exe.display()),
    );
    line(
        "working directory",
        format!("`{}` exists", input.host.working_dir.display()),
    );
    line(
        "local endpoint",
        format!("{} (loopback; not contacted)", input.host.local_endpoint),
    );

    if let Err(e) = history.verify_writable() {
        return preflight_failed(&passed, "history directory", &e);
    }
    line(
        "history",
        format!(
            "`{}` established and writable; kept; no session written",
            input.history
        ),
    );
    if let Err(e) = probe_dir_writable(&input.host.claude_audit_dir, remove_probe_file) {
        return preflight_failed(&passed, "claude audit directory", &e);
    }
    line(
        "claude audit dir",
        format!(
            "`{}` established and writable",
            input.host.claude_audit_dir.display()
        ),
    );

    let version = match query_cli_version(inspector, input.host) {
        Ok(v) => v,
        Err(e) => return preflight_failed(&passed, "claude version", &e),
    };
    line("claude version", format!("{version} (from `--version`)"));

    // The configuration the live builder really uses, not a copy of it.
    let config = claude_config(input.host, "preflight");
    let own = own_child_marker();
    if !config.additional_child_markers.contains(&own) || own.name() != OWN_MARKER {
        return preflight_failed(
            &passed,
            "recursion markers",
            "the invoker's own marker is not configured",
        );
    }
    line(
        "child markers",
        format!("{PROVIDER_MARKER}=1 (provider), {OWN_MARKER}=1 (invoker)"),
    );

    let resolver = resolver_for(&version);
    let checked = match verify_identities(resolver.as_ref(), input.loaded, input.level) {
        Ok(c) => c,
        Err(e) => return preflight_failed(&passed, "participant identities", &e),
    };
    line(
        "participants",
        "each constructed and identity-checked against the registry; none invoked".to_owned(),
    );
    for c in &checked {
        passed.push_str(&format!(
            "    - {} ({}) provider={} model={}\n",
            c.descriptor.id.as_str(),
            match c.adapter {
                Adapter::ClaudeCode => "claude-code",
                Adapter::LocalModel => "local-model",
            },
            c.descriptor.provider.as_str(),
            c.descriptor.model.as_str()
        ));
    }

    passed.push_str(&format!(
        "execution_authority: {}\nPreflight PASSED. NO MODEL CALL WAS MADE. NO SESSION WAS SAVED.\n",
        report.plan.execution_authority
    ));
    Output {
        stdout: passed,
        stderr: String::new(),
        code: EXIT_OK,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inspect::{InspectError, Inspected, Stream};
    use crate::run::{EXIT_HISTORY_UNUSABLE, EXIT_SESSION_ID_IN_USE};
    use maia_roundtable::{
        DecisionRequest, ParticipantFailure, ParticipantRequest, ParticipantResponse, Timestamp,
    };
    use std::collections::HashSet;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener};
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    /// A registry whose identities are the ones the real adapters produce: the
    /// Claude Code provider is always `anthropic`; the local one is declared.
    const LIVE_PANEL: &str = r#"{"participants":[
        {"id":"claude-code","adapter":"claude-code","provider":"anthropic","model_ref":"claude-test-1",
         "roles":["round_table_member","adjudicator"],"assurance_levels":["A3"],"enabled":true},
        {"id":"local-qwen","adapter":"local-model","provider":"local-loopback","model_ref":"qwen-test-1",
         "roles":["round_table_member"],"assurance_levels":["A3"],"enabled":true}]}"#;

    fn loaded() -> LoadedRegistry {
        load_registry(LIVE_PANEL).unwrap()
    }

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("maia-invoke-live-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A host whose executable is a real (fake) file, under `root`.
    fn host_in(root: &Path, endpoint: &str) -> HostConfig {
        let exe = root.join("claude-fake.exe");
        std::fs::write(&exe, "not a real executable").unwrap();
        let work = root.join("work");
        std::fs::create_dir_all(&work).unwrap();
        HostConfig::parse(
            exe.to_str().unwrap(),
            root.join("audit").to_str().unwrap(),
            work.to_str().unwrap(),
            endpoint,
        )
        .unwrap()
    }

    // ---- fakes: the consultation runner, the version inspector, env, terminal

    /// The provider's consultation runner. Records every invocation and starts
    /// nothing. In these tests it must see NO call unless a test drives a
    /// participant on purpose.
    struct FakeRunner {
        calls: Calls,
    }
    type Calls = Arc<Mutex<Vec<Invocation>>>;
    impl ProcessRunner for FakeRunner {
        fn run(
            &self,
            i: &Invocation,
            _: &CancellationToken,
        ) -> Result<ProcessOutcome, ProcessError> {
            self.calls.lock().unwrap().push(i.clone());
            Err(ProcessError::ExecutableNotFound)
        }
    }
    fn consult_runner() -> (SharedRunner, Calls) {
        let calls: Calls = Arc::new(Mutex::new(Vec::new()));
        (
            SharedRunner::new(FakeRunner {
                calls: Arc::clone(&calls),
            }),
            calls,
        )
    }

    /// The version inspector. Records every spec and answers with a canned result.
    struct FakeInspector {
        calls: InspectCalls,
        result: Result<Inspected, InspectError>,
    }
    type InspectCalls = Arc<Mutex<Vec<InspectSpec>>>;
    impl Inspector for FakeInspector {
        fn inspect(&self, spec: &InspectSpec) -> Result<Inspected, InspectError> {
            self.calls.lock().unwrap().push(spec.clone());
            self.result.clone()
        }
    }
    fn answered(stdout: &str, code: Option<i32>) -> Result<Inspected, InspectError> {
        Ok(Inspected {
            exit_code: code,
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    }
    fn inspector_with(result: Result<Inspected, InspectError>) -> (FakeInspector, InspectCalls) {
        let calls: InspectCalls = Arc::new(Mutex::new(Vec::new()));
        (
            FakeInspector {
                calls: Arc::clone(&calls),
                result,
            },
            calls,
        )
    }
    fn good_inspector() -> (FakeInspector, InspectCalls) {
        inspector_with(answered("2.1.273 (Claude Code)\n", Some(0)))
    }

    struct FakeEnv(HashSet<&'static str>);
    impl Env for FakeEnv {
        fn is_set(&self, key: &str) -> bool {
            self.0.contains(key)
        }
    }
    fn clear() -> FakeEnv {
        FakeEnv(HashSet::new())
    }
    fn set(keys: &[&'static str]) -> FakeEnv {
        FakeEnv(keys.iter().copied().collect())
    }
    struct Typed {
        interactive: bool,
        answer: Option<String>,
        asked: usize,
    }
    impl Typed {
        fn types(a: &str) -> Self {
            Self {
                interactive: true,
                answer: Some(a.to_owned()),
                asked: 0,
            }
        }
    }
    impl Terminal for Typed {
        fn is_interactive(&self) -> bool {
            self.interactive
        }
        fn ask(&mut self, _: &str) -> Option<String> {
            self.asked += 1;
            self.answer.clone()
        }
    }
    struct FixedClock;
    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            Timestamp(1)
        }
    }

    /// Wraps a resolver so every participant it hands out counts `invoke` calls.
    struct Counting<R> {
        inner: R,
        invokes: Arc<AtomicUsize>,
    }
    impl<R: ParticipantResolver> ParticipantResolver for Counting<R> {
        fn resolve(
            &self,
            d: &ParticipantDescriptor,
        ) -> Result<Box<dyn Participant>, ResolutionFailure> {
            Ok(Box::new(CountingParticipant {
                inner: self.inner.resolve(d)?,
                invokes: Arc::clone(&self.invokes),
            }))
        }
    }
    struct CountingParticipant {
        inner: Box<dyn Participant>,
        invokes: Arc<AtomicUsize>,
    }
    impl Participant for CountingParticipant {
        fn descriptor(&self) -> ParticipantDescriptor {
            self.inner.descriptor()
        }
        fn invoke(&self, r: ParticipantRequest) -> Result<ParticipantResponse, ParticipantFailure> {
            self.invokes.fetch_add(1, Ordering::SeqCst);
            self.inner.invoke(r)
        }
    }

    /// Everything one preflight scenario observes, each wired to THAT scenario.
    struct Observed {
        invokes: Arc<AtomicUsize>,
        runner_calls: Calls,
        inspect_calls: InspectCalls,
    }

    fn preflight_with(
        loaded: &LoadedRegistry,
        host: &HostConfig,
        history_dir: &Path,
        env: &dyn Env,
        inspector: (FakeInspector, InspectCalls),
    ) -> (Output, Observed) {
        let (runner, runner_calls) = consult_runner();
        let invokes = Arc::new(AtomicUsize::new(0));
        let (inspector, inspect_calls) = inspector;
        let history = FileHistory::new(history_dir.to_str().unwrap());
        let (l2, h2, r2, i2) = (
            loaded.clone(),
            host.clone(),
            runner.clone(),
            Arc::clone(&invokes),
        );
        let out = execute_preflight(
            &PreflightInput {
                loaded,
                level: ReasoningAssuranceLevel::A3,
                subject: "s",
                prompt: "p",
                history: history_dir.to_str().unwrap(),
                host,
            },
            env,
            &inspector,
            &history,
            &move |version| {
                Box::new(Counting {
                    inner: LiveResolver::new(&l2, &h2, version.to_owned(), r2.clone()),
                    invokes: Arc::clone(&i2),
                })
            },
        );
        (
            out,
            Observed {
                invokes,
                runner_calls,
                inspect_calls,
            },
        )
    }

    fn descriptor_in(loaded: &LoadedRegistry, id: &str) -> ParticipantDescriptor {
        round_table_descriptors(&loaded.registry, ReasoningAssuranceLevel::A3)
            .unwrap()
            .into_iter()
            .find(|d| d.id.as_str() == id)
            .unwrap()
    }
    fn descriptor_of(id: &str) -> ParticipantDescriptor {
        descriptor_in(&loaded(), id)
    }

    fn listing(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    fn decision() -> DecisionRequest {
        DecisionRequest {
            id: "rt-1".into(),
            subject: "ship it?".into(),
            prompt: "should we ship on friday".into(),
            evidence: vec![],
        }
    }

    // ---------------------------------------------------- the builder mapping

    #[test]
    fn the_builder_maps_registry_identities_onto_the_real_adapters() {
        let root = temp("mapping");
        let host = host_in(&root, "127.0.0.1:11434");
        let (runner, calls) = consult_runner();
        let resolver = LiveResolver::new(&loaded(), &host, "2.1.273".into(), runner);

        let claude = descriptor_of("claude-code");
        let local = descriptor_of("local-qwen");
        assert_eq!(claude.provider.as_str(), "anthropic");
        assert_eq!(claude.model.as_str(), "claude-test-1");
        let built = resolve_verified(&resolver, &claude).expect("claude-code participant");
        assert_eq!(built.descriptor(), claude);
        let built = resolve_verified(&resolver, &local).expect("local participant");
        assert_eq!(built.descriptor(), local);
        assert_eq!(local.model.as_str(), "qwen-test-1");
        assert!(
            calls.lock().unwrap().is_empty(),
            "construction runs nothing"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_claude_model_is_the_registry_model_ref_and_nothing_else() {
        let root = temp("model");
        let host = host_in(&root, "127.0.0.1:11434");
        let config = claude_config(&host, "claude-test-1");
        assert_eq!(config.model.as_deref(), Some("claude-test-1"));
        assert_eq!(config.executable, host.claude_exe);
        assert_eq!(config.working_directory, host.working_dir);
        assert_eq!(config.audit_directory, host.claude_audit_dir);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// One-shot fake local runtime on a loopback port. It answers the first
    /// request in the shape the local adapter expects and hands back the parsed
    /// JSON body it received. It never touches a real Ollama.
    fn fake_local_runtime() -> (
        SocketAddr,
        std::thread::JoinHandle<Option<serde_json::Value>>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let handle = std::thread::spawn(move || {
            let give_up = Instant::now() + Duration::from_secs(20);
            let mut stream = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() > give_up {
                            return None;
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => return None,
                }
            };
            stream.set_nonblocking(false).ok()?;
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .ok()?;
            let mut buf = Vec::new();
            let mut chunk = [0u8; 1024];
            let (head_end, length) = loop {
                let n = stream.read(&mut chunk).ok()?;
                if n == 0 {
                    return None;
                }
                buf.extend_from_slice(&chunk[..n]);
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..pos]).to_ascii_lowercase();
                    let length = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok())?;
                    break (pos + 4, length);
                }
            };
            while buf.len() < head_end + length {
                let n = stream.read(&mut chunk).ok()?;
                if n == 0 {
                    return None;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            let body: serde_json::Value =
                serde_json::from_slice(&buf[head_end..head_end + length]).ok()?;
            let reply = r#"{"response":"fake local answer"}"#;
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{reply}"
            );
            Some(body)
        });
        (addr, handle)
    }

    #[test]
    fn the_registry_model_ref_reaches_the_actual_local_request() {
        // The production resolver builds the local participant; the request that
        // participant really sends is captured by a fake loopback runtime.
        for model in ["qwen-test-1", "another-model:7b"] {
            let root = temp("localreq");
            let (addr, server) = fake_local_runtime();
            let host = host_in(&root, &addr.to_string());
            let registry = load_registry(&LIVE_PANEL.replace("qwen-test-1", model)).unwrap();
            let (runner, calls) = consult_runner();
            let resolver = LiveResolver::new(&registry, &host, "v".into(), runner);
            let participant =
                resolve_verified(&resolver, &descriptor_in(&registry, "local-qwen")).unwrap();

            let response = participant
                .invoke(ParticipantRequest {
                    session_id: "rt-1".into(),
                    decision: decision(),
                    max_output_tokens: 64,
                })
                .expect("the fake runtime answered");
            assert_eq!(response.response_text, "fake local answer");

            let body = server.join().unwrap().expect("a request was captured");
            assert_eq!(
                body["model"], model,
                "the registry model_ref is what is sent"
            );
            assert_eq!(body["stream"], false);
            assert!(
                body["prompt"]
                    .as_str()
                    .unwrap()
                    .contains("should we ship on friday")
            );
            assert_eq!(body["options"]["num_predict"], 64);
            assert!(calls.lock().unwrap().is_empty(), "no Claude process either");
            std::fs::remove_dir_all(&root).unwrap();
        }
    }

    #[test]
    fn an_unregistered_or_mismatched_identity_is_refused_never_substituted() {
        let root = temp("mismatch");
        let host = host_in(&root, "127.0.0.1:11434");
        let (runner, _) = consult_runner();
        // The Claude adapter is always provider `anthropic`; registering it under
        // another provider name must fail, not be papered over.
        let wrong = LIVE_PANEL.replace(r#""provider":"anthropic""#, r#""provider":"claude-code""#);
        let loaded = load_registry(&wrong).unwrap();
        let resolver = LiveResolver::new(&loaded, &host, "v".into(), runner.clone());
        let err = verify_identities(&resolver, &loaded, ReasoningAssuranceLevel::A3).unwrap_err();
        assert!(
            err.contains("claude-code") && err.contains("not be the registered identity"),
            "{err}"
        );

        let stranger = ParticipantDescriptor {
            id: maia_roundtable::ParticipantId::new("stranger").unwrap(),
            ..descriptor_of("claude-code")
        };
        let resolver = LiveResolver::new(&self::loaded(), &host, "v".into(), runner);
        assert!(matches!(
            resolver.resolve(&stranger),
            Err(ResolutionFailure::UnknownParticipant)
        ));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn no_provider_names_leak_into_orchestration_or_composition() {
        fn production(source: &str) -> String {
            let cut = source.find("#[cfg(test)]").unwrap_or(source.len());
            source[..cut]
                .lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n")
                .to_ascii_lowercase()
        }
        for (name, source) in [
            (
                "run.rs (the invoker's orchestration)",
                include_str!("run.rs"),
            ),
            (
                "composition",
                include_str!("../../../composition/roundtable-composition/src/lib.rs"),
            ),
        ] {
            let code = production(source);
            for provider in [
                "claude",
                "qwen",
                "ollama",
                "anthropic",
                "local-model",
                "loopback",
            ] {
                assert!(
                    !code.contains(provider),
                    "{name} names provider `{provider}`; only live.rs may"
                );
            }
        }
    }

    #[test]
    fn a_non_loopback_endpoint_cannot_reach_a_local_participant() {
        // `HostConfig::parse` already refuses it; the fields are public, so the
        // adapter's own check is the second lock.
        let root = temp("nonloop");
        let mut host = host_in(&root, "127.0.0.1:11434");
        host.local_endpoint = "192.168.1.10:11434".parse().unwrap();
        let (runner, _) = consult_runner();
        let resolver = LiveResolver::new(&loaded(), &host, "v".into(), runner);
        assert!(matches!(
            resolver.resolve(&descriptor_of("local-qwen")),
            Err(ResolutionFailure::ProviderUnavailable)
        ));
        std::fs::remove_dir_all(&root).unwrap();
    }

    // ------------------------------------------------- own marker, real config

    #[test]
    fn the_claude_child_receives_both_markers_through_the_real_builder_config() {
        // The participant is built by the production resolver from the production
        // config. The runner is a fake that starts no process, so no model is
        // reached; it only records the child environment the provider computed.
        let root = temp("markers");
        let host = host_in(&root, "127.0.0.1:11434");
        let (runner, calls) = consult_runner();
        let resolver = LiveResolver::new(&loaded(), &host, "2.1.273".into(), runner);
        let participant = resolve_verified(&resolver, &descriptor_of("claude-code")).unwrap();
        let _ = participant.invoke(ParticipantRequest {
            session_id: "rt-1".into(),
            decision: decision(),
            max_output_tokens: 8,
        });
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        let child = &calls[0];
        assert_eq!(child.program, host.claude_exe);
        assert_eq!(child.working_directory, host.working_dir);
        assert_eq!(
            child.env_set.get(PROVIDER_MARKER).map(String::as_str),
            Some("1")
        );
        assert_eq!(child.env_set.get(OWN_MARKER).map(String::as_str), Some("1"));
        assert_eq!(
            child.env_set.len(),
            2,
            "no other variable is passed through"
        );
        assert!(child.env_remove.iter().any(|k| k == "ANTHROPIC_API_KEY"));
        assert!(
            std::env::var_os(OWN_MARKER).is_none(),
            "this process is untouched"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_counting_wrapper_and_fake_runner_really_observe_a_participant() {
        // Guards the zero-invoke assertions elsewhere: the same wiring does count
        // and does record when a participant is driven.
        let root = temp("wiring");
        let host = host_in(&root, "127.0.0.1:11434");
        let (runner, runner_calls) = consult_runner();
        let invokes = Arc::new(AtomicUsize::new(0));
        let resolver = Counting {
            inner: LiveResolver::new(&loaded(), &host, "v".into(), runner),
            invokes: Arc::clone(&invokes),
        };
        let participant = resolve_verified(&resolver, &descriptor_of("claude-code")).unwrap();
        let _ = participant.invoke(ParticipantRequest {
            session_id: "rt-1".into(),
            decision: decision(),
            max_output_tokens: 8,
        });
        assert_eq!(invokes.load(Ordering::SeqCst), 1);
        assert_eq!(runner_calls.lock().unwrap().len(), 1);
        std::fs::remove_dir_all(&root).unwrap();
    }

    // ---------------------------------------------------- the version query

    #[test]
    fn the_version_query_is_a_bounded_inspection_of_exactly_the_supplied_executable() {
        let root = temp("vspec");
        let host = host_in(&root, "127.0.0.1:11434");
        let (inspector, calls) = good_inspector();
        assert_eq!(
            query_cli_version(&inspector, &host).unwrap(),
            "2.1.273 (Claude Code)"
        );
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        let spec = &calls[0];
        assert_eq!(spec.program, host.claude_exe, "exactly the explicit path");
        assert_eq!(spec.args, ["--version"], "no other argument");
        assert_eq!(spec.working_directory, host.working_dir);
        assert_eq!(spec.scratch_dir, host.claude_audit_dir);
        assert!(spec.env_set.contains_key(PROVIDER_MARKER));
        assert!(spec.env_set.contains_key(OWN_MARKER));
        assert_eq!(spec.env_set.len(), 2);
        assert!(spec.env_remove.iter().any(|k| k == "ANTHROPIC_API_KEY"));
        assert!(spec.limits.deadline <= Duration::from_secs(10));
        assert!(spec.limits.max_stdout_bytes <= 4096 && spec.limits.max_stderr_bytes <= 4096);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn an_unusable_version_answer_is_a_classified_failure() {
        let root = temp("vbad");
        let host = host_in(&root, "127.0.0.1:11434");
        let cases = [
            answered("", Some(0)),
            answered("   \n", Some(0)),
            answered("2.1\u{7}", Some(0)),
            answered(&"x".repeat(500), Some(0)),
            answered("2.1.273", Some(1)),
            answered("2.1.273", None),
            Ok(Inspected {
                exit_code: Some(0),
                stdout: vec![0xff, 0xfe],
                stderr: vec![],
            }),
            Err(InspectError::Timeout),
            Err(InspectError::ExecutableNotFound),
            Err(InspectError::SpawnFailed),
            Err(InspectError::OutputTooLarge(Stream::Stdout)),
            Err(InspectError::OutputTooLarge(Stream::Stderr)),
            Err(InspectError::Scratch),
        ];
        for bad in cases {
            let (inspector, _) = inspector_with(bad.clone());
            assert!(query_cli_version(&inspector, &host).is_err(), "{bad:?}");
        }
        // The classification reaches the person.
        let (inspector, _) = inspector_with(Err(InspectError::Timeout));
        let msg = query_cli_version(&inspector, &host).unwrap_err();
        assert!(msg.contains("deadline"), "{msg}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    // ------------------------------------- failures before any model call

    #[test]
    fn a_missing_or_invalid_claude_executable_fails_before_any_process_or_model() {
        let root = temp("noexe");
        let mut host = host_in(&root, "127.0.0.1:11434");
        for bad in [root.join("gone.exe"), root.join("work")] {
            host.claude_exe = bad;
            let (runner, runner_calls) = consult_runner();
            let (inspector, inspect_calls) = good_inspector();
            let err = build_live(
                &loaded(),
                ReasoningAssuranceLevel::A3,
                &host,
                &runner,
                &inspector,
            )
            .err()
            .expect("must fail");
            assert!(matches!(err, BuildError::HostUnusable(ref m) if m.contains("--claude-exe")));
            assert!(inspect_calls.lock().unwrap().is_empty(), "no inspection");
            assert!(runner_calls.lock().unwrap().is_empty(), "no consultation");
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn an_unwritable_audit_directory_fails_before_any_process_or_model() {
        let root = temp("noaudit");
        let mut host = host_in(&root, "127.0.0.1:11434");
        // A regular file where the audit directory would have to be.
        let blocker = root.join("audit-is-a-file");
        std::fs::write(&blocker, "x").unwrap();
        host.claude_audit_dir = blocker.clone();
        let (runner, runner_calls) = consult_runner();
        let (inspector, inspect_calls) = good_inspector();
        let err = build_live(
            &loaded(),
            ReasoningAssuranceLevel::A3,
            &host,
            &runner,
            &inspector,
        )
        .err()
        .expect("must fail");
        assert!(matches!(err, BuildError::HostUnusable(ref m) if m.contains("audit directory")));
        assert!(inspect_calls.lock().unwrap().is_empty());
        assert!(runner_calls.lock().unwrap().is_empty());
        assert_eq!(std::fs::read(&blocker).unwrap(), b"x", "untouched");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_failing_version_inspection_fails_the_build_after_only_that_one_inspection() {
        let root = temp("vfail");
        let host = host_in(&root, "127.0.0.1:11434");
        let (runner, runner_calls) = consult_runner();
        let (inspector, inspect_calls) = inspector_with(Err(InspectError::Timeout));
        let err = build_live(
            &loaded(),
            ReasoningAssuranceLevel::A3,
            &host,
            &runner,
            &inspector,
        )
        .err()
        .expect("must fail");
        assert!(matches!(err, BuildError::HostUnusable(ref m) if m.contains("deadline")));
        assert_eq!(inspect_calls.lock().unwrap().len(), 1);
        assert!(runner_calls.lock().unwrap().is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_good_build_constructs_participants_and_inspects_only_the_version() {
        let root = temp("goodbuild");
        let host = host_in(&root, "127.0.0.1:11434");
        let (runner, runner_calls) = consult_runner();
        let (inspector, inspect_calls) = good_inspector();
        let built = build_live(
            &loaded(),
            ReasoningAssuranceLevel::A3,
            &host,
            &runner,
            &inspector,
        );
        assert!(built.is_ok());
        let inspect_calls = inspect_calls.lock().unwrap();
        assert_eq!(inspect_calls.len(), 1, "only `--version`");
        assert_eq!(inspect_calls[0].args, ["--version"]);
        assert!(
            runner_calls.lock().unwrap().is_empty(),
            "no consultation process"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    // ------------------------------------------------------------- `run`

    fn run_args(root: &Path, host: HostConfig, history: &Path) -> RunArgs {
        RunArgs {
            plan: crate::cli::PlanArgs {
                registry: "unused".into(),
                level: ReasoningAssuranceLevel::A3,
                prompt_file: "unused".into(),
                subject: "s".into(),
            },
            history: history.join("hist").to_str().unwrap().to_owned(),
            host: HostConfig {
                claude_audit_dir: root.join("audit"),
                ..host
            },
        }
    }

    #[test]
    fn run_still_requires_the_typed_session_id_and_does_nothing_until_it_is_typed() {
        let root = temp("run-declined");
        let host = host_in(&root, "127.0.0.1:11434");
        let args = run_args(&root, host, &root);
        let (runner, runner_calls) = consult_runner();
        let (inspector, inspect_calls) = good_inspector();
        let loaded = loaded();
        for (env, mut term, code) in [
            (
                set(&["CLAUDECODE"]),
                Typed::types("rt-x"),
                crate::gate::EXIT_GUARD_REFUSED,
            ),
            (
                clear(),
                Typed {
                    interactive: false,
                    answer: Some("rt-x".into()),
                    asked: 0,
                },
                crate::gate::EXIT_CONFIRMATION_UNAVAILABLE,
            ),
            (
                clear(),
                Typed::types("y"),
                crate::gate::EXIT_CONFIRMATION_DECLINED,
            ),
            (
                clear(),
                Typed::types(""),
                crate::gate::EXIT_CONFIRMATION_DECLINED,
            ),
            (
                clear(),
                Typed {
                    interactive: true,
                    answer: None,
                    asked: 0,
                },
                crate::gate::EXIT_CONFIRMATION_DECLINED,
            ),
        ] {
            let out = run_live(
                &args,
                &loaded,
                "p",
                &env,
                &mut term,
                &FixedClock,
                &runner,
                &inspector,
                "rt-x",
            );
            assert_eq!(out.code, code, "{}", out.stderr);
        }
        assert!(
            inspect_calls.lock().unwrap().is_empty(),
            "not even `--version`"
        );
        assert!(runner_calls.lock().unwrap().is_empty());
        assert!(!Path::new(&args.history).exists(), "no history directory");
        assert!(!root.join("audit").exists(), "no audit directory");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn run_with_a_bad_host_refuses_before_asking_anyone() {
        let root = temp("run-badhost");
        let mut host = host_in(&root, "127.0.0.1:11434");
        host.claude_exe = root.join("gone.exe");
        let args = run_args(&root, host, &root);
        let (runner, runner_calls) = consult_runner();
        let (inspector, inspect_calls) = good_inspector();
        let mut term = Typed::types("rt-x");
        let out = run_live(
            &args,
            &loaded(),
            "p",
            &clear(),
            &mut term,
            &FixedClock,
            &runner,
            &inspector,
            "rt-x",
        );
        assert_eq!(out.code, EXIT_NOT_CONSTRUCTED);
        assert_eq!(term.asked, 0);
        assert!(inspect_calls.lock().unwrap().is_empty());
        assert!(runner_calls.lock().unwrap().is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_confirmed_run_whose_version_inspection_fails_calls_no_model_and_saves_nothing() {
        // Goes through the real `execute_run` and the real builder up to the point
        // where a model call would begin, and stops there because the inspection
        // fails. The consultation runner is never touched.
        let root = temp("run-vfail");
        let host = host_in(&root, "127.0.0.1:11434");
        let args = run_args(&root, host, &root);
        let (runner, runner_calls) = consult_runner();
        let (inspector, inspect_calls) = inspector_with(answered("", Some(3)));
        let mut term = Typed::types("rt-x");
        let out = run_live(
            &args,
            &loaded(),
            "p",
            &clear(),
            &mut term,
            &FixedClock,
            &runner,
            &inspector,
            "rt-x",
        );
        assert_eq!(out.code, EXIT_NOT_CONSTRUCTED, "{}", out.stderr);
        assert_eq!(term.asked, 1, "run asked for the typed id");
        assert_eq!(inspect_calls.lock().unwrap().len(), 1);
        assert!(runner_calls.lock().unwrap().is_empty());
        // Established and kept, empty: no session was written.
        assert_eq!(listing(Path::new(&args.history)), Vec::<String>::new());
        assert_ne!(out.code, EXIT_SESSION_ID_IN_USE);
        std::fs::remove_dir_all(&root).unwrap();
    }

    // ------------------------------------ the outer guard comes before any read

    #[test]
    fn an_active_guard_stops_run_and_preflight_before_any_input_file_is_opened() {
        // Every input path points at a file that does not exist. If the inputs
        // were read first the result would be "unreadable" (exit 4); the guard
        // must win, and nothing else may have been touched.
        let root = temp("guard-first");
        let host = host_in(&root, "127.0.0.1:11434");
        let missing = root.join("no-such-input");
        let mut args = run_args(&root, host, &root);
        args.plan.registry = missing.to_str().unwrap().to_owned();
        args.plan.prompt_file = missing.to_str().unwrap().to_owned();

        for marker in ["CLAUDECODE", PROVIDER_MARKER, OWN_MARKER] {
            let (runner, runner_calls) = consult_runner();
            let (inspector, inspect_calls) = good_inspector();
            let env = set(&[marker]);

            let mut term = Typed::types("rt-x");
            let out = run_command_in(
                &args,
                &env,
                &mut term,
                &FixedClock,
                &runner,
                &inspector,
                "rt-x",
            );
            assert_eq!(out.code, crate::gate::EXIT_GUARD_REFUSED, "run / {marker}");
            assert_eq!(term.asked, 0);

            let out = preflight_command_in(&args, &env, &runner, &inspector);
            assert_eq!(
                out.code,
                crate::gate::EXIT_GUARD_REFUSED,
                "preflight / {marker}"
            );

            assert!(
                inspect_calls.lock().unwrap().is_empty(),
                "{marker}: no version run"
            );
            assert!(
                runner_calls.lock().unwrap().is_empty(),
                "{marker}: no provider"
            );
            assert!(
                !Path::new(&args.history).exists(),
                "{marker}: history untouched"
            );
            assert!(!root.join("audit").exists(), "{marker}: audit untouched");
        }

        // Sanity: with the guard clear, the same paths ARE read and fail as
        // unreadable, so the differential above really shows reads did not happen.
        let (runner, _) = consult_runner();
        let (inspector, _) = good_inspector();
        let out = preflight_command_in(&args, &clear(), &runner, &inspector);
        assert_eq!(out.code, EXIT_INPUT_UNREADABLE);
        std::fs::remove_dir_all(&root).unwrap();
    }

    // --------------------------------------------------------- preflight

    #[test]
    fn a_passing_preflight_needs_no_confirmation_calls_no_model_and_saves_no_session() {
        let root = temp("pf-ok");
        // A listener that must never be contacted: the local endpoint.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let host = host_in(
            &root,
            &format!("127.0.0.1:{}", listener.local_addr().unwrap().port()),
        );
        let history = root.join("hist");

        // No terminal is passed to `execute_preflight` at all: there is nothing
        // to type and nothing that could prompt.
        let (out, seen) = preflight_with(&loaded(), &host, &history, &clear(), good_inspector());
        assert_eq!(out.code, EXIT_OK, "{}\n{}", out.stdout, out.stderr);
        assert!(!out.stdout.contains("type this"), "{}", out.stdout);
        assert!(
            out.stdout
                .contains("Preflight PASSED. NO MODEL CALL WAS MADE. NO SESSION WAS SAVED.")
        );
        assert!(out.stdout.contains("execution_authority: false"));
        assert!(out.stdout.contains("2.1.273 (Claude Code)"));
        assert!(out.stdout.contains("claude-test-1") && out.stdout.contains("qwen-test-1"));
        assert!(out.stdout.contains(OWN_MARKER) && out.stdout.contains(PROVIDER_MARKER));

        // Zero Participant::invoke, zero consultation process.
        assert_eq!(seen.invokes.load(Ordering::SeqCst), 0);
        assert!(seen.runner_calls.lock().unwrap().is_empty());
        // Exactly one bounded inspection: `--version`.
        let inspected = seen.inspect_calls.lock().unwrap();
        assert_eq!(inspected.len(), 1);
        assert_eq!(inspected[0].args, ["--version"]);
        // The local model was never contacted.
        assert!(matches!(
            listener.accept(),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock
        ));
        // No session, no probe; the established directories remain.
        assert_eq!(listing(&history), Vec::<String>::new());
        assert!(history.is_dir());
        assert_eq!(
            listing(&root.join("audit")),
            Vec::<String>::new(),
            "no audit record: nothing was consulted"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_preflight_says_what_it_may_do_on_success_and_on_failure() {
        let root = temp("pf-notice");
        let host = host_in(&root, "127.0.0.1:11434");
        let (ok, _) = preflight_with(
            &loaded(),
            &host,
            &root.join("hist"),
            &clear(),
            good_inspector(),
        );
        let (bad, _) = preflight_with(
            &loaded(),
            &host,
            &root.join("hist2"),
            &clear(),
            inspector_with(Err(InspectError::Timeout)),
        );
        for out in [&ok, &bad] {
            for phrase in [
                "may:",
                "execute `",
                "--version",
                "bounded local executable inspection",
                "local filesystem readiness checks",
                "construct configuration objects",
                "will NOT invoke a model",
            ] {
                assert!(
                    out.stdout.contains(phrase),
                    "missing {phrase:?}:\n{}",
                    out.stdout
                );
            }
        }
        assert_eq!(bad.code, EXIT_PREFLIGHT_FAILED);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_preflight_stops_at_an_active_guard_before_touching_anything() {
        let root = temp("pf-guard");
        let host = host_in(&root, "127.0.0.1:11434");
        let history = root.join("hist");
        for marker in ["CLAUDECODE", PROVIDER_MARKER, OWN_MARKER] {
            let (out, seen) = preflight_with(
                &loaded(),
                &host,
                &history,
                &set(&[marker]),
                good_inspector(),
            );
            assert_eq!(out.code, crate::gate::EXIT_GUARD_REFUSED, "{marker}");
            assert!(out.stdout.is_empty());
            assert!(seen.inspect_calls.lock().unwrap().is_empty());
            assert!(seen.runner_calls.lock().unwrap().is_empty());
        }
        assert!(!history.exists() && !root.join("audit").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_preflight_with_a_missing_executable_fails_before_any_inspection() {
        let root = temp("pf-noexe");
        let mut host = host_in(&root, "127.0.0.1:11434");
        host.claude_exe = root.join("gone.exe");
        let (out, seen) = preflight_with(
            &loaded(),
            &host,
            &root.join("hist"),
            &clear(),
            good_inspector(),
        );
        assert_eq!(out.code, EXIT_PREFLIGHT_FAILED);
        assert!(out.stderr.contains("host configuration") && out.stderr.contains("--claude-exe"));
        assert!(seen.inspect_calls.lock().unwrap().is_empty());
        assert_eq!(seen.invokes.load(Ordering::SeqCst), 0);
        assert!(!root.join("hist").exists(), "nothing was established");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_preflight_names_a_failed_version_inspection_and_calls_no_model() {
        let root = temp("pf-vfail");
        let host = host_in(&root, "127.0.0.1:11434");
        for (result, expect) in [
            (Err(InspectError::Timeout), "deadline"),
            (
                Err(InspectError::OutputTooLarge(Stream::Stdout)),
                "more standard output",
            ),
            (answered("", Some(9)), "exited with"),
        ] {
            let (out, seen) = preflight_with(
                &loaded(),
                &host,
                &root.join("hist"),
                &clear(),
                inspector_with(result),
            );
            assert_eq!(out.code, EXIT_PREFLIGHT_FAILED);
            assert!(out.stderr.contains("claude version"), "{}", out.stderr);
            assert!(out.stderr.contains(expect), "{}", out.stderr);
            assert!(out.stderr.contains("No model was called"));
            assert_eq!(seen.inspect_calls.lock().unwrap().len(), 1);
            assert_eq!(seen.invokes.load(Ordering::SeqCst), 0);
            assert!(seen.runner_calls.lock().unwrap().is_empty());
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_preflight_names_a_mismatched_registry_identity_and_invokes_nothing() {
        // Every counter below belongs to THIS scenario: `preflight_with` builds
        // the runner, the invoke counter and the inspector fresh, and wires the
        // same ones into the resolver, so a zero here cannot come from elsewhere.
        // (`the_counting_wrapper_and_fake_runner_really_observe_a_participant`
        // shows the same wiring does count when a participant is driven.)
        let root = temp("pf-mismatch");
        let host = host_in(&root, "127.0.0.1:11434");
        let wrong = LIVE_PANEL.replace(r#""provider":"anthropic""#, r#""provider":"x""#);
        let mismatched = load_registry(&wrong).unwrap();
        let (out, seen) = preflight_with(
            &mismatched,
            &host,
            &root.join("hist"),
            &clear(),
            good_inspector(),
        );
        assert_eq!(out.code, EXIT_PREFLIGHT_FAILED);
        assert!(
            out.stderr.contains("participant identities"),
            "{}",
            out.stderr
        );
        assert_eq!(seen.invokes.load(Ordering::SeqCst), 0);
        assert!(seen.runner_calls.lock().unwrap().is_empty());
        let inspected = seen.inspect_calls.lock().unwrap();
        assert_eq!(inspected.len(), 1, "it got as far as the version check");
        assert_eq!(inspected[0].args, ["--version"]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_preflight_that_cannot_establish_history_fails_closed_before_any_inspection() {
        let root = temp("pf-history");
        let host = host_in(&root, "127.0.0.1:11434");
        // A regular file where a parent directory would have to be.
        let blocker = root.join("blocker");
        std::fs::write(&blocker, "x").unwrap();
        let (out, seen) = preflight_with(
            &loaded(),
            &host,
            &blocker.join("sub"),
            &clear(),
            good_inspector(),
        );
        assert_eq!(out.code, EXIT_PREFLIGHT_FAILED, "{}", out.stderr);
        assert!(out.stderr.contains("history directory"));
        assert!(seen.inspect_calls.lock().unwrap().is_empty(), "nothing ran");
        std::fs::remove_dir_all(&root).unwrap();
    }

    // ------------------------ a timed-out consultation, end to end (M0.15.7e.1)

    /// A consultation runner whose every call times out, counting the calls. It
    /// stands in for the runner outcome the bounded real runner now produces when a
    /// `claude` child hangs past its deadline.
    struct TimedOutRunner {
        calls: Arc<AtomicUsize>,
    }
    impl ProcessRunner for TimedOutRunner {
        fn run(
            &self,
            _: &Invocation,
            _: &CancellationToken,
        ) -> Result<ProcessOutcome, ProcessError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(ProcessOutcome {
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                duration: Duration::from_secs(180),
                timed_out: true,
                cancelled: false,
            })
        }
    }
    struct AnswersLocally(ParticipantDescriptor);
    impl Participant for AnswersLocally {
        fn descriptor(&self) -> ParticipantDescriptor {
            self.0.clone()
        }
        fn invoke(&self, _: ParticipantRequest) -> Result<ParticipantResponse, ParticipantFailure> {
            Ok(ParticipantResponse {
                participant: self.0.clone(),
                response_text: "a local view".into(),
                evidence: vec![],
                provider_request_id: None,
                usage: None,
                model_ref_used: None,
            })
        }
    }
    /// The real Claude Code provider over a runner that times out, and a local
    /// participant that answers. Everything else is the production pipeline.
    struct TimingOutClaude {
        host: HostConfig,
        claude_calls: Arc<AtomicUsize>,
    }
    impl ParticipantResolver for TimingOutClaude {
        fn resolve(
            &self,
            d: &ParticipantDescriptor,
        ) -> Result<Box<dyn Participant>, ResolutionFailure> {
            if d.id.as_str() == "claude-code" {
                let provider = ClaudeCodeProvider::new_without_recursion_check(
                    d.id.as_str(),
                    claude_config(&self.host, d.model.as_str()),
                    "2.1.278".into(),
                    TimedOutRunner {
                        calls: Arc::clone(&self.claude_calls),
                    },
                    FileAuditSink::new(&self.host.claude_audit_dir),
                )
                .map_err(|_| ResolutionFailure::ProviderUnavailable)?;
                Ok(Box::new(provider))
            } else {
                Ok(Box::new(AnswersLocally(d.clone())))
            }
        }
    }

    #[test]
    fn a_timed_out_consultation_is_a_failed_contribution_and_the_session_fails_on_quorum() {
        use crate::run::EXIT_SESSION_FAILED;
        use maia_roundtable::{
            ContributionResult, ParticipantFailureKind, SessionFailureKind, SessionStore,
        };
        use maia_roundtable_store::FileSessionStore;

        let root = temp("timeout-e2e");
        let host = HostConfig {
            claude_audit_dir: root.join("audit"),
            ..host_in(&root, "127.0.0.1:11434")
        };
        let history = root.join("hist");
        let claude_calls = Arc::new(AtomicUsize::new(0));
        let resolver = TimingOutClaude {
            host: host.clone(),
            claude_calls: Arc::clone(&claude_calls),
        };
        let loaded = loaded();
        let mut term = Typed::types("rt-x");
        let started = Instant::now();
        let out = execute_run(
            &RunRequest {
                loaded: &loaded,
                level: ReasoningAssuranceLevel::A3,
                subject: "s",
                prompt: "p",
                history: history.to_str().unwrap(),
                session_id: "rt-x",
            },
            &clear(),
            &mut term,
            &FixedClock,
            |_| participants_with_leader(Box::new(resolver), &loaded, ReasoningAssuranceLevel::A3),
        );
        // It returns; it does not hang, and it does not pretend to have a decision.
        assert!(started.elapsed() < Duration::from_secs(30));
        assert_eq!(
            out.code, EXIT_SESSION_FAILED,
            "{}{}",
            out.stdout, out.stderr
        );
        assert!(out.stdout.contains("did not reach a decision"));

        // No retry: the consultation was attempted exactly once.
        assert_eq!(claude_calls.load(Ordering::SeqCst), 1);

        // The failure is first-class, persisted evidence with provenance.
        let record = FileSessionStore::new(&history)
            .load("rt-x")
            .unwrap()
            .expect("the failed session is saved");
        assert!(matches!(
            record.failure,
            Some(SessionFailureKind::QuorumNotMet(_))
        ));
        let claude = record
            .outcomes
            .iter()
            .find(|o| o.participant.id.as_str() == "claude-code")
            .expect("the timed-out attempt is recorded");
        assert!(matches!(
            claude.result,
            ContributionResult::Failed {
                kind: ParticipantFailureKind::Transient,
                ..
            }
        ));
        let local = record
            .outcomes
            .iter()
            .find(|o| o.participant.id.as_str() == "local-qwen")
            .expect("the other participant was still asked");
        assert!(local.responded());
        assert!(record.adjudication.is_none(), "nothing was adjudicated");

        // The provider's own audit record says it timed out.
        let audits: Vec<_> = std::fs::read_dir(root.join("audit"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        assert_eq!(audits.len(), 1);
        let audit: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&audits[0]).unwrap()).unwrap();
        assert_eq!(audit["outcome"], "Timeout");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_history_check_codes_stay_distinct_from_preflight() {
        assert_ne!(EXIT_PREFLIGHT_FAILED, EXIT_HISTORY_UNUSABLE);
    }
}
