//! Capability Mesh acceptance test (M0.15.11), in the same spirit as the
//! M0.15.9/M0.15.10 acceptance tests and `roundtable/tests/core_independence.rs`:
//! mechanical proof via real, checked-in manifests and source, not
//! narrative claim.

const DIAGNOSTICS_MANIFEST: &str = include_str!("../Cargo.toml");
const DIAGNOSTICS_LIB: &str = include_str!("../src/lib.rs");
const LOCAL_MODEL_MANIFEST: &str = include_str!("../../local-model/Cargo.toml");
const LOCAL_INTELLIGENCE_HEALTH_MANIFEST: &str =
    include_str!("../../local-intelligence-health/Cargo.toml");
const CORE_BRIEFING_MANIFEST: &str = include_str!("../../../core/briefing/Cargo.toml");

/// Same technique the M0.15.9/M0.15.10 acceptance tests use: a crate's
/// manifest lists its dependency names as `name = ` table keys under
/// `[dependencies]`. Dev-dependencies are deliberately excluded (this
/// function stops at the first `[` after `[dependencies]`), since this
/// crate's own test-only `maia-briefing`/`maia-local-model` dev-dependencies
/// (needed only to construct enum values in tests) must never be mistaken
/// for a real runtime dependency direction.
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

/// This crate's real (non-dev) dependency graph contains nothing capable
/// of a hidden cloud call or process control. The allowlist is exhaustive,
/// so a newly added dependency of any kind forces a conscious update here.
/// Notably absent: `maia-local-model`, `maia-briefing` (both are
/// dev-dependencies only, for tests — see `dependency_names`'s docs) and
/// any Round Table crate.
#[test]
fn the_diagnostics_capability_has_no_dependency_capable_of_a_hidden_cloud_call_or_process_control()
{
    let deps = dependency_names(DIAGNOSTICS_MANIFEST);
    let allowed = ["maia-local-intelligence-health", "serde", "serde_json"];
    for dep in &deps {
        assert!(
            allowed.contains(dep),
            "infra/local-intelligence-diagnostics gained an unreviewed dependency ({dep}); \
             the Capability Mesh acceptance test's \"no hidden cloud fallback / no process \
             control\" claim is a fact about the dependency graph and must be re-verified, \
             not silently invalidated"
        );
    }
    assert!(
        !deps.iter().any(|d| d.contains("roundtable")),
        "no dependency toward Round Table is allowed, per the M0.15.11 directive"
    );
}

/// `infra/local-model` never depends on diagnostics — the diagnostics
/// capability must remain fully absent-able without affecting Local
/// Intelligence inference at all.
#[test]
fn local_model_has_no_dependency_on_diagnostics() {
    let deps = dependency_names(LOCAL_MODEL_MANIFEST);
    assert!(
        !deps.contains(&"maia-local-intelligence-diagnostics"),
        "infra/local-model must never depend on infra/local-intelligence-diagnostics"
    );
}

/// `infra/local-intelligence-health` never depends on diagnostics either —
/// explicitly required by the M0.15.11 directive ("maia-local-intelligence-health
/// SHALL NOT gain a reverse dependency on diagnostics"). The dependency
/// direction is strictly diagnostics -> health, never the reverse.
#[test]
fn local_intelligence_health_has_no_reverse_dependency_on_diagnostics() {
    let deps = dependency_names(LOCAL_INTELLIGENCE_HEALTH_MANIFEST);
    assert!(
        !deps.contains(&"maia-local-intelligence-diagnostics"),
        "infra/local-intelligence-health must not depend on infra/local-intelligence-diagnostics \
         (M0.15.11 directive, explicit)"
    );
}

/// Core (`core/briefing`) never depends on the diagnostics implementation.
#[test]
fn core_briefing_never_depends_on_diagnostics() {
    let deps = dependency_names(CORE_BRIEFING_MANIFEST);
    assert!(
        !deps.contains(&"maia-local-intelligence-diagnostics"),
        "core/briefing must not depend on infra/local-intelligence-diagnostics"
    );
}

/// Production source only: everything before the `#[cfg(test)]` module,
/// with pure comment lines (`///`/`//`) stripped. Doc comments legitimately
/// name things in prose for explanatory purposes (e.g. "unlike X's
/// `rich_failure_label`, this crate does Y"), and the test module
/// legitimately constructs fixture data via the health store's own
/// `record_*` methods to set up scenarios — neither is the production
/// dependency this test exists to catch, which is actual code in the
/// non-test API surface.
fn production_code_without_comments() -> String {
    let production = DIAGNOSTICS_LIB
        .split("#[cfg(test)]")
        .next()
        .unwrap_or(DIAGNOSTICS_LIB);
    production
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Diagnostics depends only on the health capability's PUBLIC query
/// contract — verified both structurally (the manifest allowlist above)
/// and at the source level: production code never references
/// `infra/local-intelligence-health`'s or `infra/local-model`'s private
/// implementation identifiers.
#[test]
fn diagnostics_never_references_private_implementation_of_its_dependencies() {
    let production = production_code_without_comments();
    let private_items = [
        // infra/local-model private items
        "send_prompt",
        "write_bounded",
        "read_bounded",
        "perform_request",
        "is_timeout_like",
        "response_timeout",
        "TimedWrite",
        "TimedRead",
        // infra/local-intelligence-health private items
        "span_and_count",
        "enforce_retention",
        "rich_failure_label",
        "briefing_failure_label",
        "validate_model",
    ];
    for item in private_items {
        assert!(
            !production.contains(item),
            "infra/local-intelligence-diagnostics/src/lib.rs's production code must never \
             reference `{item}` — a private implementation detail of one of its dependencies"
        );
    }
}

/// No action authority: the diagnostics crate never references process-
/// control APIs, mirroring the same proof in
/// `infra/local-intelligence-health/tests/capability_mesh.rs`. Strictly
/// observational, per the M0.15.11 directive's explicit "no action
/// authority" requirement.
#[test]
fn diagnostics_never_references_process_control_apis() {
    let forbidden = [
        "std::process::Command",
        "process::Command",
        "Child::kill",
        ".kill()",
        "TerminateProcess",
    ];
    for pattern in forbidden {
        assert!(
            !DIAGNOSTICS_LIB.contains(pattern),
            "infra/local-intelligence-diagnostics/src/lib.rs must never reference `{pattern}` \
             — this capability has no action authority whatsoever"
        );
    }
}

/// The diagnostics crate's PRODUCTION code never calls any `record_*`
/// method — it is strictly read-only against the health store, never
/// mutating the evidence it reads (a diagnostic failure must never modify
/// health history, per the M0.15.11 directive). The test module is
/// exempted: tests legitimately seed an in-memory `HealthStore` via its
/// own public `record_*` methods to construct fixtures — that is calling
/// the health capability's own public write API from a test, not the
/// diagnostics capability gaining write behavior.
#[test]
fn diagnostics_production_code_never_calls_a_record_method_on_the_health_store() {
    let production = production_code_without_comments();
    let forbidden = [
        "record_status_sample",
        "record_inference_success",
        "record_inference_failure",
    ];
    for pattern in forbidden {
        assert!(
            !production.contains(pattern),
            "infra/local-intelligence-diagnostics/src/lib.rs's production code must never \
             call `{pattern}` — this capability is strictly read-only"
        );
    }
}

/// The public diagnostic contract stays public, so a future Hypothesis
/// Ledger (or the CLI in this same crate) can actually reach it.
#[test]
fn the_public_diagnostic_contract_stays_public() {
    let public_items = [
        "pub fn build",
        "pub struct DiagnosticSnapshot",
        "pub struct WindowCoverage",
        "pub struct LatencySummary",
        "pub struct LatencyComparison",
        "pub enum TaxonomyFidelity",
        "pub enum DiagnosticPattern",
        "pub enum EvidenceLevel",
        "pub enum DiagnosticError",
    ];
    for item in public_items {
        assert!(
            DIAGNOSTICS_LIB.contains(item),
            "expected `{item}` to remain part of the diagnostics capability's public contract"
        );
    }
}
