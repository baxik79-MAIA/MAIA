//! Capability Mesh acceptance test (M0.15.9).
//!
//! Mechanically proves the properties the Local Intelligence Runtime
//! migration directive required at the end of the milestone, the same way
//! `roundtable/tests/core_independence.rs` mechanically proves Round Table's
//! independence: by parsing real, checked-in manifests and source via
//! `include_str!` rather than trusting a hand-written claim in a report.
//! Every assertion here fails loudly (a compile-time `include_str!` error, or
//! a panicking `assert!`) if the property it names stops holding — it is not
//! observational, it is load-bearing.

/// This crate's own manifest: proves no hidden cloud-provider dependency
/// exists ANYWHERE in the capability's dependency graph. The capability's
/// "runtime absent" behavior (see `absence_fails_explicitly_never_a_silent_fallback`
/// below) is backed by the fact that there is nothing else in this crate's
/// dependency tree that COULD serve a request instead — the loopback
/// `TcpStream` in `std` is the only way this crate talks to anything.
const LOCAL_MODEL_MANIFEST: &str = include_str!("../Cargo.toml");
const LOCAL_MODEL_LIB: &str = include_str!("../src/lib.rs");
const CORE_BRIEFING_MANIFEST: &str = include_str!("../../../core/briefing/Cargo.toml");
const DESKTOP_MANIFEST: &str = include_str!("../../../apps/desktop/Cargo.toml");

/// A dependency-graph proof, same technique `core_independence.rs` uses: a
/// crate's manifest lists its dependency names as `name = ` / `name =` table
/// keys under `[dependencies]`. Good enough for this workspace's manifests
/// (no build-dependency/dev-dependency sections on the crates checked here),
/// and precise enough to fail if a dependency is ever added or removed.
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

/// No cloud-provider or generic-HTTP-client crate anywhere in this
/// capability's own dependencies. This is the structural half of "capability
/// absence fails explicitly, never a hidden network fallback": there is
/// nothing else in the dependency graph that could even serve a substitute
/// answer. The allowlist is deliberately exhaustive (not a denylist of known
/// bad names) so a newly added dependency of any kind forces a conscious
/// update here, not a silent pass.
#[test]
fn the_capability_has_no_dependency_capable_of_a_hidden_network_fallback() {
    let deps = dependency_names(LOCAL_MODEL_MANIFEST);
    let allowed = ["maia-briefing", "serde_json"];
    for dep in &deps {
        assert!(
            allowed.contains(dep),
            "infra/local-model gained an unreviewed dependency ({dep}); the Capability \
             Mesh acceptance test's \"no hidden cloud fallback\" claim is a fact about the \
             dependency graph and must be re-verified, not silently invalidated, if this is \
             intentional"
        );
    }
}

/// Core (`core/briefing`, which owns `LocalModelProvider`/`LocalProviderFailure`)
/// never depends on this capability's implementation crate. The dependency
/// direction is capability -> Core (implements Core's trait), never the
/// reverse — the same shape Round Table's `core_independence.rs` proves for
/// itself. This is what "implementation can evolve without forcing unrelated
/// Core changes" means mechanically: Core cannot even name this crate.
#[test]
fn core_briefing_never_depends_on_the_local_model_capability() {
    let deps = dependency_names(CORE_BRIEFING_MANIFEST);
    assert!(
        !deps.contains(&"maia-local-model"),
        "core/briefing must not depend on infra/local-model; Core is provider-neutral \
         and this capability is one replaceable adapter among several"
    );
}

/// Desktop remains functional without Round Table: its manifest carries no
/// Round Table dependency at all, so the binary cannot reach into Round
/// Table even by accident.
#[test]
fn desktop_has_no_round_table_dependency() {
    let deps = dependency_names(DESKTOP_MANIFEST);
    assert!(
        !deps.iter().any(|dep| dep.contains("roundtable")),
        "apps/desktop must build and run with Round Table entirely absent from its own \
         dependency graph, not merely unused at runtime"
    );
    assert!(
        deps.contains(&"maia-local-model"),
        "apps/desktop is expected to consume the Local Intelligence Runtime capability \
         directly (through its public surface only — see the privacy assertions below)"
    );
}

/// Round Table reaches this capability only through the allowed seam
/// (`infra/local-model-roundtable`'s `TextCompletion for LoopbackLocalProvider`),
/// which depends on both sides by construction. This does not prove Round
/// Table restricts itself to the public surface (the privacy assertions
/// below do that, crate-wide) — it proves the ONE sanctioned path exists and
/// is where the dependency is declared, not smuggled in through Core.
///
/// Reads the adapter's manifest at RUNTIME, not via `include_str!` (deliberately
/// unlike the other manifest checks in this file): `infra/local-model-roundtable`
/// is itself a Round Table member crate, and ADR-0046 requires Core/this
/// capability to keep building and testing with Round Table entirely absent
/// (`tools/verify_round_table_absent.py` deletes it and rebuilds). A
/// compile-time `include_str!` of a Round-Table-member manifest would make
/// this test file itself fail to compile in that configuration — the same
/// mistake `core_independence.rs` avoids by only embedding manifests that are
/// guaranteed to exist. When the adapter crate is absent, there is nothing
/// to check and the assertion below is vacuously satisfied.
#[test]
fn round_table_consumes_the_capability_only_through_the_named_adapter_crate() {
    let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../local-model-roundtable/Cargo.toml");
    let Ok(manifest) = std::fs::read_to_string(&manifest_path) else {
        return;
    };
    let deps = dependency_names(&manifest);
    assert!(
        deps.contains(&"maia-local-model") && deps.contains(&"maia-roundtable"),
        "infra/local-model-roundtable is the one crate authorized to bridge Round Table \
         and the Local Intelligence Runtime capability; both dependencies must be explicit \
         here"
    );
}

/// Private implementation detail functions/types stay private at the source
/// level — not just "unused externally by convention" but genuinely
/// unreachable from outside this crate. Checked directly against the
/// checked-in source rather than inferred, so a future `pub` added by
/// accident on any of these fails this test immediately rather than only
/// showing up as a widened surface nobody reviewed.
#[test]
fn private_implementation_functions_are_not_pub() {
    let private_items = [
        "fn send_prompt",
        "fn write_bounded",
        "fn read_bounded",
        "fn perform_request",
        "fn is_timeout_like",
        "fn response_timeout",
        "trait TimedWrite",
        "trait TimedRead",
    ];
    for item in private_items {
        assert!(
            LOCAL_MODEL_LIB.contains(item),
            "expected to find `{item}` in infra/local-model/src/lib.rs at all \
             (this assertion tracks a known private item by name; update it if the \
             item was intentionally renamed)"
        );
        assert!(
            !LOCAL_MODEL_LIB.contains(&format!("pub {item}")),
            "`{item}` must stay a private implementation detail of the Local Intelligence \
             Runtime capability (Capability Mesh ownership rule: the capability owns its \
             own local inference boundary and local failure classification internally); \
             finding `pub {item}` means private implementation detail has leaked into the \
             public capability contract"
        );
    }
}

/// The public capability contract itself stays public — the mirror image of
/// the assertion above, so this test cannot pass merely by everything having
/// become private.
#[test]
fn the_public_capability_contract_stays_public() {
    let public_items = [
        "pub fn new",
        "pub fn status",
        "pub fn warm",
        "pub fn complete",
        "pub fn complete_detailed",
        "pub enum LocalIntelligenceFailure",
        "pub struct LocalIntelligenceStatus",
        "pub struct CompletionOutcome",
    ];
    for item in public_items {
        assert!(
            LOCAL_MODEL_LIB.contains(item),
            "expected `{item}` to remain part of the Local Intelligence Runtime capability's \
             public contract; its absence means the public surface shrank without an \
             explicit migration decision"
        );
    }
}

use maia_briefing::LocalProviderFailure;
use maia_local_model::LoopbackLocalProvider;
use std::net::{SocketAddr, TcpListener};

/// Behavioral half of "capability absence fails explicitly, never a silent
/// fallback": pointed at a loopback address nothing is listening on, the
/// capability reports itself unavailable and fails closed — it never
/// substitutes a different model, a different provider or a canned answer,
/// and it never panics. The listener is bound and immediately dropped so the
/// port is a real one the OS just freed, not a guessed one that might
/// legitimately be in use.
#[test]
fn absence_fails_explicitly_never_a_silent_fallback() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
    let dead_port: SocketAddr = listener.local_addr().expect("local_addr");
    drop(listener);

    let provider =
        LoopbackLocalProvider::new(dead_port, "capability-mesh-test-model").expect("loopback");

    let status = provider.status();
    assert!(
        !status.available,
        "status() must report the runtime unavailable when nothing is listening"
    );
    assert_eq!(status.model, "capability-mesh-test-model");

    let result = provider.complete("irrelevant prompt", 16);
    assert_eq!(
        result,
        Err(LocalProviderFailure::Unavailable),
        "an absent runtime must fail as Unavailable, never succeed via any fallback path"
    );
}

/// Loopback-only boundary preserved: a non-loopback address is refused at
/// construction, before any network attempt, so no consumer can be
/// misconfigured into reaching an off-box "local" model.
#[test]
fn non_loopback_endpoints_are_refused_at_construction() {
    let public_looking: SocketAddr = "93.184.216.34:80".parse().expect("valid address");
    let result = LoopbackLocalProvider::new(public_looking, "capability-mesh-test-model");
    assert_eq!(result.err(), Some(LocalProviderFailure::Unavailable));
}
