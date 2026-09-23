//! Capability Mesh acceptance test (M0.15.10), in the same spirit as
//! `infra/local-model/tests/capability_mesh.rs` (M0.15.9) and
//! `roundtable/tests/core_independence.rs`: mechanical proof via real,
//! checked-in manifests and source, not narrative claim.

const OBSERVER_MANIFEST: &str = include_str!("../Cargo.toml");
const OBSERVER_LIB: &str = include_str!("../src/lib.rs");
const LOCAL_MODEL_MANIFEST: &str = include_str!("../../local-model/Cargo.toml");
const CORE_BRIEFING_MANIFEST: &str = include_str!("../../../core/briefing/Cargo.toml");

/// Same technique `infra/local-model/tests/capability_mesh.rs` uses: a
/// crate's manifest lists its dependency names as `name = ` table keys under
/// `[dependencies]`.
fn dependency_names(manifest: &str) -> Vec<&str> {
    manifest
        .lines()
        .skip_while(|line| line.trim() != "[dependencies]")
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .filter_map(|line| line.split('=').next())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect()
}

/// The dependency direction the migration directive requires:
/// "Local Intelligence observer -> public local-intelligence capability
/// contract", NEVER "infra/local-model -> observer/history implementation".
/// `infra/local-model`'s own manifest must never name this crate.
#[test]
fn local_model_has_no_dependency_on_the_observer() {
    let deps = dependency_names(LOCAL_MODEL_MANIFEST);
    assert!(
        !deps.contains(&"maia-local-intelligence-health"),
        "infra/local-model must never depend on the observer; the capability must remain \
         fully usable (and buildable) with the observer entirely absent"
    );
}

/// Core (`core/briefing`) never depends on this crate's implementation —
/// the observer is an optional, external consumer of Core's contracts
/// (transitively, via `maia-local-model`), never the reverse.
#[test]
fn core_briefing_never_depends_on_the_observer() {
    let deps = dependency_names(CORE_BRIEFING_MANIFEST);
    assert!(
        !deps.contains(&"maia-local-intelligence-health"),
        "core/briefing must not depend on the Local Intelligence observer"
    );
}

/// No dependency in this crate's own graph is capable of a hidden
/// cloud-provider call or of process control. The allowlist is exhaustive
/// (not a denylist), so any newly added dependency forces a conscious
/// update here.
#[test]
fn the_observer_has_no_dependency_capable_of_a_hidden_cloud_call_or_process_control() {
    let deps = dependency_names(OBSERVER_MANIFEST);
    let allowed = ["maia-briefing", "maia-local-model", "rusqlite"];
    for dep in &deps {
        assert!(
            allowed.contains(dep),
            "infra/local-intelligence-health gained an unreviewed dependency ({dep}); the \
             Capability Mesh acceptance test's \"no hidden cloud fallback / no process \
             control\" claim is a fact about the dependency graph and must be re-verified, \
             not silently invalidated"
        );
    }
}

/// The observer never references process-control APIs at the source level:
/// it cannot start, stop, restart, replace or reconfigure the local
/// runtime, per the migration directive's explicit prohibition (section 9,
/// "NO ACTION AUTHORITY"). Checked as a source-text fact, not merely as an
/// absent dependency, since `std::process` needs no external crate.
#[test]
fn the_observer_never_references_process_control_apis() {
    let forbidden = [
        "std::process::Command",
        "process::Command",
        "Child::kill",
        ".kill()",
        "TerminateProcess",
    ];
    for pattern in forbidden {
        assert!(
            !OBSERVER_LIB.contains(pattern),
            "infra/local-intelligence-health/src/lib.rs must never reference `{pattern}` — \
             the observer is strictly observational and must have no action authority over \
             the local runtime process"
        );
    }
}

/// The observer reaches the Local Intelligence capability only through its
/// public contract: it never names any of `infra/local-model`'s known
/// private implementation items. This is the mirror image of
/// `infra/local-model/tests/capability_mesh.rs`'s
/// `private_implementation_functions_are_not_pub` — proving the same
/// boundary from the consumer's side.
#[test]
fn the_observer_never_references_local_models_private_implementation() {
    let private_items = [
        "send_prompt",
        "write_bounded",
        "read_bounded",
        "perform_request",
        "is_timeout_like",
        "response_timeout",
        "TimedWrite",
        "TimedRead",
    ];
    for item in private_items {
        assert!(
            !OBSERVER_LIB.contains(item),
            "infra/local-intelligence-health/src/lib.rs must never reference `{item}` \
             (a private implementation detail of infra/local-model); the observer consumes \
             only status()/warm()/complete()/complete_detailed()/consult() and the public \
             LocalIntelligenceStatus/LocalIntelligenceFailure/CompletionOutcome types"
        );
    }
}

/// The observer's own public contract stays public, so a future caller (a
/// Desktop wiring, or a later self-diagnosis capability) can actually reach
/// it.
#[test]
fn the_public_recording_and_query_contract_stays_public() {
    let public_items = [
        "pub fn open",
        "pub fn open_in_memory",
        "pub fn record_status_sample",
        "pub fn record_inference_success",
        "pub fn record_inference_failure",
        "pub fn record_inference_failure_from_briefing",
        "pub fn latest_status",
        "pub fn status_samples_in_window",
        "pub fn history_coverage",
        "pub struct HistoryCoverage",
        "pub fn failures_in_window",
        "pub fn failure_counts_by_class",
        "pub fn recent_latencies",
        "pub fn consecutive_unavailable_or_timeout_count",
        "pub struct HealthStore",
        "pub struct RetentionPolicy",
        "pub enum ObserverError",
        "pub enum OperationClass",
    ];
    for item in public_items {
        assert!(
            OBSERVER_LIB.contains(item),
            "expected `{item}` to remain part of the observer's public contract"
        );
    }
}
