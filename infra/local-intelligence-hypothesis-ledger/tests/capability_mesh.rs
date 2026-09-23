//! Capability Mesh acceptance test (M0.15.12), in the same spirit as the
//! M0.15.9/M0.15.10/M0.15.11 acceptance tests and
//! `roundtable/tests/core_independence.rs`: mechanical proof via real,
//! checked-in manifests and source, not narrative claim.

const LEDGER_MANIFEST: &str = include_str!("../Cargo.toml");
const LEDGER_LIB: &str = include_str!("../src/lib.rs");
const DIAGNOSTICS_MANIFEST: &str = include_str!("../../local-intelligence-diagnostics/Cargo.toml");
const HEALTH_MANIFEST: &str = include_str!("../../local-intelligence-health/Cargo.toml");
const LOCAL_MODEL_MANIFEST: &str = include_str!("../../local-model/Cargo.toml");
const CORE_BRIEFING_MANIFEST: &str = include_str!("../../../core/briefing/Cargo.toml");

/// Same technique the M0.15.9/M0.15.10/M0.15.11 acceptance tests use: a
/// crate's manifest lists its dependency names as `name = ` table keys
/// under `[dependencies]`. Dev-dependencies are excluded (this function
/// stops at the first `[` after `[dependencies]`).
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

/// Diagnostics, health, local-model and core/briefing must never depend on
/// the ledger — the M0.15.12 directive's explicit "Never: diagnostics ->
/// ledger. Never: health -> ledger. Never: local-model -> ledger", plus
/// Core independence generally.
#[test]
fn nothing_upstream_depends_on_the_ledger() {
    for (name, manifest) in [
        ("infra/local-intelligence-diagnostics", DIAGNOSTICS_MANIFEST),
        ("infra/local-intelligence-health", HEALTH_MANIFEST),
        ("infra/local-model", LOCAL_MODEL_MANIFEST),
        ("core/briefing", CORE_BRIEFING_MANIFEST),
    ] {
        let deps = dependency_names(manifest);
        assert!(
            !deps.contains(&"maia-local-intelligence-hypothesis-ledger"),
            "{name} must never depend on the Hypothesis Evidence Ledger"
        );
    }
}

/// The ledger's own real (non-dev) dependency graph contains nothing
/// capable of a hidden cloud call, process control, or Round Table. The
/// allowlist is exhaustive. `maia-local-intelligence-health` is present
/// (used only by the CLI binary, to build a fresh `DiagnosticSnapshot`
/// before recording it — never by `lib.rs`, see
/// `the_ledger_library_never_references_health_or_local_model_types`),
/// which is the allowed direction (`ledger -> diagnostics`/`health`), not
/// the forbidden one.
#[test]
fn the_ledger_has_no_dependency_capable_of_a_hidden_cloud_call_or_process_control() {
    let deps = dependency_names(LEDGER_MANIFEST);
    let allowed = [
        "maia-local-intelligence-diagnostics",
        "maia-local-intelligence-health",
        "rusqlite",
        "serde",
        "serde_json",
    ];
    for dep in &deps {
        assert!(
            allowed.contains(dep),
            "infra/local-intelligence-hypothesis-ledger gained an unreviewed dependency \
             ({dep}); the Capability Mesh acceptance test's \"no hidden cloud fallback / no \
             process control\" claim is a fact about the dependency graph and must be \
             re-verified, not silently invalidated"
        );
    }
    assert!(
        !deps.iter().any(|d| d.contains("roundtable")),
        "no dependency toward Round Table is allowed, per the M0.15.12 directive"
    );
}

/// Production source only: everything in `src/lib.rs` before the
/// `#[cfg(test)]` module, with pure comment lines stripped. The library
/// (as opposed to the CLI binary in `src/bin/`) must never reference
/// `infra/local-intelligence-health` or `infra/local-model` types at all —
/// it depends only on `maia-local-intelligence-diagnostics`'s already-built
/// `DiagnosticSnapshot`/`DiagnosticPattern`/`EvidenceLevel`.
fn lib_production_code_without_comments() -> String {
    let production = LEDGER_LIB
        .split("#[cfg(test)]")
        .next()
        .unwrap_or(LEDGER_LIB);
    production
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_ledger_library_never_references_health_or_local_model_types() {
    let production = lib_production_code_without_comments();
    for forbidden in [
        "maia_local_intelligence_health",
        "maia_local_model",
        "HealthStore",
        "LocalIntelligenceStatus",
        "LocalIntelligenceFailure",
    ] {
        assert!(
            !production.contains(forbidden),
            "infra/local-intelligence-hypothesis-ledger/src/lib.rs must never reference \
             `{forbidden}` — the library consumes only diagnostics' already-built \
             DiagnosticSnapshot; only the CLI binary (src/bin/ledger_cli.rs) is allowed to \
             build one from a HealthStore"
        );
    }
}

/// The ledger's production code never references any dependency's private
/// implementation identifiers.
#[test]
fn the_ledger_never_references_private_implementation_of_its_dependencies() {
    let production = lib_production_code_without_comments();
    // Only infra/local-intelligence-diagnostics' private items are checked
    // here: it is the ledger library's one real dependency besides
    // rusqlite/serde_json. infra/local-model's and
    // infra/local-intelligence-health's private item names are NOT
    // checked here -- this crate's own private helpers legitimately reuse
    // some of the same names (e.g. `enforce_retention`) as an intentional
    // naming echo of the same discipline, and since neither crate is a
    // real dependency of lib.rs at all (proven by
    // `the_ledger_library_never_references_health_or_local_model_types`),
    // there is no actual risk of accidentally calling into either one's
    // private surface.
    let private_items = [
        "midpoint",
        "is_briefing_sourced",
        "classify_taxonomy_fidelity",
        "detect_failure_patterns",
        "detect_intermittent_availability",
    ];
    for item in private_items {
        assert!(
            !production.contains(item),
            "infra/local-intelligence-hypothesis-ledger/src/lib.rs's production code must \
             never reference `{item}` — a private implementation detail of one of its \
             dependencies"
        );
    }
}

/// The ledger never references process-control APIs — strictly
/// observational, per the M0.15.12 directive's explicit "no
/// process-control authority is granted".
#[test]
fn the_ledger_never_references_process_control_apis() {
    let forbidden = [
        "std::process::Command",
        "process::Command",
        "Child::kill",
        ".kill()",
        "TerminateProcess",
    ];
    for pattern in forbidden {
        assert!(
            !LEDGER_LIB.contains(pattern),
            "infra/local-intelligence-hypothesis-ledger/src/lib.rs must never reference \
             `{pattern}` — this capability has no action authority whatsoever"
        );
    }
}

/// The ledger never writes to health history — it only ever reads an
/// already-built `DiagnosticSnapshot` value, never calling any
/// `HealthStore::record_*` method (which it cannot even name — see
/// `the_ledger_library_never_references_health_or_local_model_types`).
#[test]
fn the_ledger_production_code_never_calls_a_health_store_record_method() {
    // Test fixtures (not production code) legitimately call the health
    // store's own public record_* methods to seed scenarios -- checked
    // against production code only, same discipline as diagnostics'
    // equivalent test in M0.15.11.
    let production = lib_production_code_without_comments();
    for forbidden in [
        "record_status_sample",
        "record_inference_success",
        "record_inference_failure",
    ] {
        assert!(
            !production.contains(forbidden),
            "infra/local-intelligence-hypothesis-ledger/src/lib.rs's production code must \
             never call `{forbidden}` on a health store — the ledger does not write health \
             history"
        );
    }
}

/// M0.15.12 observation summaries cannot present overlapping reads as
/// independent confirmations or assign them confidence. A later proposed
/// hypothesis has its own qualitative confidence, so inspect only the
/// original observation result contracts here.
#[test]
fn observation_results_have_no_confidence_or_independence_claim() {
    for name in [
        "LedgerCoverage",
        "ConsecutiveRunResult",
        "EligibleSupportResult",
    ] {
        let start = LEDGER_LIB.find(&format!("pub struct {name} {{")).unwrap();
        let body = LEDGER_LIB[start..].split_once('}').unwrap().0;
        for forbidden in ["confidence", "independent_confirmation"] {
            assert!(
                !body.contains(forbidden),
                "{name} must not claim {forbidden} from overlapping observations"
            );
        }
    }
}

/// The public ledger contract stays public.
#[test]
fn the_public_ledger_contract_stays_public() {
    let public_items = [
        "pub fn record",
        "pub fn get",
        "pub fn latest",
        "pub fn latest_n",
        "pub fn observations_in_window",
        "pub fn coverage",
        "pub fn consecutive_supported_run",
        "pub fn eligible_support_over_last_n",
        "pub struct Ledger",
        "pub struct ObservationId",
        "pub struct DiagnosticObservation",
        "pub struct RetentionPolicy",
        "pub struct LedgerCoverage",
        "pub struct ConsecutiveRunResult",
        "pub struct EligibleSupportResult",
        "pub enum LedgerError",
        "pub enum RecordOutcome",
        "pub enum EvidenceState",
        "pub enum RunStopReason",
        "pub fn pattern_state",
        "pub fn record_hypothesis",
        "pub fn get_hypothesis",
        "pub fn latest_hypotheses",
        "pub fn hypotheses_in_window",
        "pub struct HypothesisId",
        "pub struct DiagnosticHypothesis",
        "pub enum HypothesisConfidence",
        "pub enum HypothesisStatus",
    ];
    for item in public_items {
        assert!(
            LEDGER_LIB.contains(item),
            "expected `{item}` to remain part of the ledger's public contract"
        );
    }
}
