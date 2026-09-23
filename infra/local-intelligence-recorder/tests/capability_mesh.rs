//! Capability Mesh acceptance test (M0.15.13), in the same spirit as the
//! M0.15.9/M0.15.10/M0.15.11/M0.15.12 acceptance tests and
//! `roundtable/tests/core_independence.rs`: mechanical proof via real,
//! checked-in manifests and source, not narrative claim.

const RECORDER_MANIFEST: &str = include_str!("../Cargo.toml");
const RECORDER_LIB: &str = include_str!("../src/lib.rs");
const DIAGNOSTICS_MANIFEST: &str = include_str!("../../local-intelligence-diagnostics/Cargo.toml");
const HEALTH_MANIFEST: &str = include_str!("../../local-intelligence-health/Cargo.toml");
const LEDGER_MANIFEST: &str = include_str!("../../local-intelligence-hypothesis-ledger/Cargo.toml");
const LOCAL_MODEL_MANIFEST: &str = include_str!("../../local-model/Cargo.toml");
const CORE_BRIEFING_MANIFEST: &str = include_str!("../../../core/briefing/Cargo.toml");

/// Same technique every prior Local Intelligence acceptance test uses: a
/// crate's manifest lists its dependency names as `name = ` table keys
/// under `[dependencies]`, skipping comment lines and stopping before
/// `[dev-dependencies]`.
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

/// Nothing upstream (diagnostics, health, ledger, local-model,
/// core/briefing) acquires a dependency on the recorder merely to
/// function -- the M0.15.13 directive's explicit "Nothing upstream may
/// depend on the recorder merely to function" and "Core must remain
/// operational with Local Intelligence recorder absent".
#[test]
fn nothing_upstream_depends_on_the_recorder() {
    for (name, manifest) in [
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
            !deps.contains(&"maia-local-intelligence-recorder"),
            "{name} must never depend on the Diagnostic Evidence Recorder"
        );
    }
}

/// The recorder's own real (non-dev) dependency graph is exactly the
/// approximate chain the directive describes: recorder -> health's public
/// read contract -> diagnostics' public snapshot contract -> ledger's
/// public record contract. Nothing else -- no Round Table, no
/// cloud/provider crate, no process-control crate.
#[test]
fn the_recorder_depends_only_on_the_three_local_intelligence_capabilities() {
    let deps = dependency_names(RECORDER_MANIFEST);
    let allowed = [
        "maia-local-intelligence-diagnostics",
        "maia-local-intelligence-health",
        "maia-local-intelligence-hypothesis-ledger",
    ];
    for dep in &deps {
        assert!(
            allowed.contains(dep),
            "infra/local-intelligence-recorder gained an unreviewed dependency ({dep}); the \
             Capability Mesh acceptance test's \"no hidden cloud fallback / no process \
             control / no Round Table\" claim is a fact about the dependency graph and must \
             be re-verified, not silently invalidated"
        );
    }
    assert!(
        !deps.iter().any(|d| d.contains("roundtable")),
        "no Round Table dependency is allowed, per the M0.15.13 directive"
    );
}

/// Production source only: everything before `#[cfg(test)]`, comment
/// lines stripped.
fn production_code_without_comments() -> String {
    let production = RECORDER_LIB
        .split("#[cfg(test)]")
        .next()
        .unwrap_or(RECORDER_LIB);
    production
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The recorder never references any of its dependencies' private
/// implementation identifiers -- it uses public contracts only.
#[test]
fn the_recorder_never_references_private_implementation_of_its_dependencies() {
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
    ];
    for item in private_items {
        assert!(
            !production.contains(item),
            "infra/local-intelligence-recorder/src/lib.rs's production code must never \
             reference `{item}` — a private implementation detail of one of its dependencies"
        );
    }
}

/// No process-control API is referenced anywhere -- the recorder has no
/// action authority whatsoever, over the local runtime or anything else.
#[test]
fn the_recorder_never_references_process_control_apis() {
    let forbidden = [
        "std::process::Command",
        "process::Command",
        "Child::kill",
        ".kill()",
        "TerminateProcess",
    ];
    for pattern in forbidden {
        assert!(
            !RECORDER_LIB.contains(pattern),
            "infra/local-intelligence-recorder/src/lib.rs must never reference `{pattern}` — \
             this capability has no action authority whatsoever, including over the local \
             runtime it observes"
        );
    }
}

/// The recorder never writes directly into the health store's tables — it
/// only ever calls `DiagnosticSnapshot::build` (a read) and never any
/// `HealthStore::record_*` method.
#[test]
fn the_recorder_production_code_never_calls_a_health_store_record_method() {
    let production = production_code_without_comments();
    for forbidden in [
        "record_status_sample",
        "record_inference_success",
        "record_inference_failure",
    ] {
        assert!(
            !production.contains(forbidden),
            "infra/local-intelligence-recorder/src/lib.rs's production code must never call \
             `{forbidden}` on a health store — the recorder does not write health history, \
             only DiagnosticSnapshot -> Ledger::record"
        );
    }
}

/// No causal hypothesis text generation exists — the recorder persists
/// `DiagnosticSnapshot` values exactly as computed by
/// `infra/local-intelligence-diagnostics`, with no free-text annotation
/// and no LLM/model inference call anywhere. Checked against production
/// code only (comments legitimately discuss `infra/claude-code` as a
/// discovery finding — see `lib.rs`'s own module docs — explaining why it
/// is NOT a dependency, which is the actual, already-proven fact; the
/// dependency allowlist test above is the structural proof that no
/// provider/LLM crate is ever pulled in).
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
            "infra/local-intelligence-recorder/src/lib.rs's production code must never call \
             `{pattern}` — no model/LLM inference call and no causal reasoning exists in this \
             capability"
        );
    }
}

/// The public recorder contract stays public.
#[test]
fn the_public_recorder_contract_stays_public() {
    let public_items = [
        "pub fn record_cycle",
        "pub fn cadence_slot_index",
        "pub fn slot_observation_id",
        "pub struct RecorderConfig",
        "pub struct Recorder",
        "pub fn start_observed",
        "pub enum CycleOutcome",
        "pub enum StopOutcome",
        "pub const DEFAULT_CADENCE",
        "pub const DEFAULT_WINDOW",
        "pub const DEFAULT_SHUTDOWN_BOUND",
    ];
    for item in public_items {
        assert!(
            RECORDER_LIB.contains(item),
            "expected `{item}` to remain part of the recorder's public contract"
        );
    }
}
