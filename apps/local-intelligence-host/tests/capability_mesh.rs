//! Capability Mesh acceptance test (M0.15.14), in the same spirit as every
//! prior Local Intelligence acceptance test and
//! `roundtable/tests/core_independence.rs`: mechanical proof via real,
//! checked-in manifests and source, not narrative claim.

const HOST_MANIFEST: &str = include_str!("../Cargo.toml");
const HOST_MAIN: &str = include_str!("../src/main.rs");
const RECORDER_MANIFEST: &str =
    include_str!("../../../infra/local-intelligence-recorder/Cargo.toml");
const DIAGNOSTICS_MANIFEST: &str =
    include_str!("../../../infra/local-intelligence-diagnostics/Cargo.toml");
const HEALTH_MANIFEST: &str = include_str!("../../../infra/local-intelligence-health/Cargo.toml");
const LEDGER_MANIFEST: &str =
    include_str!("../../../infra/local-intelligence-hypothesis-ledger/Cargo.toml");
const LOCAL_MODEL_MANIFEST: &str = include_str!("../../../infra/local-model/Cargo.toml");
const CORE_BRIEFING_MANIFEST: &str = include_str!("../../../core/briefing/Cargo.toml");

fn dependency_names(manifest: &str) -> Vec<&str> {
    manifest
        .lines()
        .skip_while(|line| line.trim() != "[dependencies]")
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .filter(|line| !line.trim_start().starts_with('#'))
        .filter_map(|line| line.split('=').next())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect()
}

/// Nothing in Core, and none of the four Local Intelligence infrastructure
/// crates, may depend on the application host — the M0.15.14 directive's
/// explicit "Nothing in Core may depend on the host. Recorder must not
/// depend on the host. No reverse dependency from Local Intelligence
/// infrastructure to the app host."
#[test]
fn nothing_in_core_or_local_intelligence_infrastructure_depends_on_the_host() {
    for (name, manifest) in [
        ("infra/local-intelligence-recorder", RECORDER_MANIFEST),
        ("infra/local-intelligence-diagnostics", DIAGNOSTICS_MANIFEST),
        ("infra/local-intelligence-health", HEALTH_MANIFEST),
        (
            "infra/local-intelligence-hypothesis-ledger",
            LEDGER_MANIFEST,
        ),
        ("infra/local-model", LOCAL_MODEL_MANIFEST),
        ("core/briefing", CORE_BRIEFING_MANIFEST),
    ] {
        let deps = dependency_names(manifest);
        assert!(
            !deps.contains(&"maia-local-intelligence-host"),
            "{name} must never depend on the Local Intelligence Host application"
        );
    }
}

/// The host's own real dependency graph is exactly the expected chain:
/// host -> recorder -> diagnostics -> health -> ledger, plus `ctrlc` for
/// graceful shutdown (which does not let this process control any OTHER
/// process — it only lets this process react to its own termination
/// signal, categorically different from process-control authority over
/// something else). No Round Table, cloud/provider, or unreviewed
/// dependency.
#[test]
fn the_host_depends_only_on_the_expected_local_intelligence_chain_plus_ctrlc() {
    let deps = dependency_names(HOST_MANIFEST);
    let allowed = [
        "ctrlc",
        "maia-local-intelligence-health",
        "maia-local-intelligence-hypothesis-ledger",
        "maia-local-intelligence-recorder",
    ];
    for dep in &deps {
        assert!(
            allowed.contains(dep),
            "apps/local-intelligence-host gained an unreviewed dependency ({dep}); the \
             Capability Mesh acceptance test's \"no hidden cloud fallback / no process \
             control / no Round Table\" claim is a fact about the dependency graph and must \
             be re-verified, not silently invalidated"
        );
    }
    assert!(
        !deps.iter().any(|d| d.contains("roundtable")),
        "no Round Table dependency is allowed, per the M0.15.14 directive"
    );
}

fn production_code_without_comments() -> String {
    let production = HOST_MAIN.split("#[cfg(test)]").next().unwrap_or(HOST_MAIN);
    production
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The host never references any of its dependencies' private
/// implementation identifiers — it uses public contracts only.
#[test]
fn the_host_never_references_private_implementation_of_its_dependencies() {
    let production = production_code_without_comments();
    let private_items = [
        // infra/local-model
        "send_prompt",
        "write_bounded",
        "read_bounded",
        "perform_request",
        // infra/local-intelligence-health
        "span_and_count",
        // infra/local-intelligence-diagnostics
        "midpoint",
        "is_briefing_sourced",
        "classify_taxonomy_fidelity",
        "detect_failure_patterns",
        "detect_intermittent_availability",
        // infra/local-intelligence-hypothesis-ledger
        "row_to_observation",
        "collect_rows",
        // infra/local-intelligence-recorder
        "run_loop",
        "start_inner",
    ];
    for item in private_items {
        assert!(
            !production.contains(item),
            "apps/local-intelligence-host/src/main.rs's production code must never reference \
             `{item}` — a private implementation detail of one of its dependencies"
        );
    }
}

/// No process-control API anywhere: the host cannot start, stop, restart
/// or inspect-for-control Ollama, Claude Code, or any other process — the
/// M0.15.14 directive's explicit, repeated "no local model lifecycle
/// authority" boundary (§11) and "no process-control authority" (§2, §20
/// stop condition). `ctrlc::set_handler` reacting to THIS process's own
/// termination signal is not process control over anything else and is
/// explicitly allowed (see the dependency-allowlist test above); this
/// check greps for controlling OTHER processes specifically.
#[test]
fn the_host_never_references_process_control_apis() {
    // Checked against production code only: the module's own discovery/
    // scope-boundary doc comments legitimately name "Ollama" in prose
    // (explaining what this crate explicitly does NOT do), the same
    // self-referential shape every prior milestone's acceptance test in
    // this sequence has hit and fixed the same way.
    let production = production_code_without_comments();
    let forbidden = [
        "std::process::Command",
        "process::Command",
        "Child::kill",
        ".kill()",
        "TerminateProcess",
        "ollama",
        "Ollama",
    ];
    for pattern in forbidden {
        assert!(
            !production.contains(pattern),
            "apps/local-intelligence-host/src/main.rs's production code must never reference \
             `{pattern}` — this application has no process-control authority whatsoever, \
             including over the local runtime it observes only through \
             infra/local-intelligence-health's public contract"
        );
    }
}

/// The host never calls a health-store `record_*` method — it only ever
/// starts a `Recorder`, which is the sole thing that ever writes health-
/// adjacent evidence (and even the Recorder only writes to the LEDGER, not
/// to health history — proven in the recorder crate's own acceptance
/// test).
#[test]
fn the_host_never_calls_a_health_store_record_method() {
    let production = production_code_without_comments();
    for forbidden in [
        "record_status_sample",
        "record_inference_success",
        "record_inference_failure",
    ] {
        assert!(
            !production.contains(forbidden),
            "apps/local-intelligence-host/src/main.rs's production code must never call \
             `{forbidden}` on a health store"
        );
    }
}

/// No causal reasoning or model/LLM inference call exists in the host —
/// it only ever starts/stops a `Recorder` and prints already-computed
/// `CycleOutcome` values.
#[test]
fn no_causal_reasoning_or_model_inference_call_exists_in_production_code() {
    let production = production_code_without_comments();
    let forbidden = [
        "TextCompletion",
        "consult(",
        "complete(",
        "complete_detailed(",
    ];
    for pattern in forbidden {
        assert!(
            !production.contains(pattern),
            "apps/local-intelligence-host/src/main.rs's production code must never call \
             `{pattern}` — no model/LLM inference call and no causal reasoning exists in this \
             application"
        );
    }
}

/// The host does not duplicate `RecorderConfig`'s cadence/window
/// constants — it must read them from `RecorderConfig::default()`, per
/// the M0.15.14 directive's explicit "Do not duplicate these constants in
/// the host."
#[test]
fn the_host_does_not_duplicate_recorder_cadence_or_window_constants() {
    assert!(
        HOST_MAIN.contains("RecorderConfig::default()"),
        "apps/local-intelligence-host/src/main.rs must derive its cadence/window defaults from \
         RecorderConfig::default(), not redeclare the 15-minute/1-hour constants itself"
    );
    assert!(
        !HOST_MAIN.contains("900") && !HOST_MAIN.contains("3600"),
        "apps/local-intelligence-host/src/main.rs must not hardcode the cadence/window values \
         as separate literals"
    );
}
