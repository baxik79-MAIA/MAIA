//! D009 — MAIA Core does not depend on the Round Table implementation.
//!
//! ADR-0046 makes Round Table an optional external module. The dependency
//! direction is:
//!
//!     Round Table  ->  public MAIA contracts
//!
//! and never the reverse. Architecture Desk ruling 2026-09-17 refined the target
//! of this test: what matters is the **dependency graph**, not where crates
//! happen to sit in the workspace layout. Round Table does not have to vanish
//! from the workspace; MAIA Core and the shipped applications simply must build
//! and operate with the Round Table implementation absent from their dependency
//! graph.
//!
//! This test therefore computes the transitive closure of normal (non-dev)
//! dependencies for every Core crate and every shipped application, and asserts
//! the Round Table implementation never appears in it. Dev-dependencies are
//! excluded deliberately: they do not ship, so they cannot make a released Core
//! require Round Table.
//!
//! Manifests are embedded with `include_str!`, so the test is path-independent
//! and a missing manifest is a compile error rather than a silently skipped
//! check.

/// The Round Table implementation crates. Core must never reach these.
///
/// `maia-roundtable-store` is here for a concrete reason: Round Table
/// persistence deliberately does NOT live in `infra/sqlite`, because both
/// shipped applications depend on that crate and putting it there would pull the
/// Round Table into their dependency graphs. This list is what keeps that
/// decision enforced rather than merely remembered.
const ROUND_TABLE_CRATES: &[&str] = &[
    "maia-roundtable",
    "maia-anthropic",
    "maia-claude-code",
    "maia-roundtable-store",
    // M0.14: the local-model Round Table adapter. Like maia-anthropic and
    // maia-claude-code, it depends on maia-roundtable to implement
    // Participant/ParticipantResolver; infra/local-model itself does not
    // and must never gain that dependency (see
    // `local_model_never_depends_on_the_round_table_implementation` below).
    "maia-local-model-roundtable",
    // M0.15.2: the read-only observability model. Also a Round Table
    // implementation crate, so Core and the shipped apps must not reach it.
    "maia-roundtable-observability",
    // M0.15.4: the read-only command-line viewer. It lives under apps/ but is
    // NOT a shipped application: it is a Round Table implementation crate, so
    // Core and the shipped apps must not reach it. It is in OTHER, never APPS.
    "maia-roundtable-viewer",
    // M0.15.7: the explicit invocation CLI. Not a shipped application: it lives
    // under ops/, is in OTHER (never APPS), and constructs providers, so Core,
    // the shipped apps, the viewer, composition and observability must not reach it.
    "maia-roundtable-invoke",
];

/// MAIA Core. Each must remain complete with Round Table absent.
const CORE: &[(&str, &str)] = &[
    ("maia-domain", include_str!("../../core/domain/Cargo.toml")),
    ("maia-policy", include_str!("../../core/policy/Cargo.toml")),
    (
        "maia-orchestrator",
        include_str!("../../core/orchestrator/Cargo.toml"),
    ),
    ("maia-store", include_str!("../../core/store/Cargo.toml")),
    (
        "maia-runtime",
        include_str!("../../core/runtime/Cargo.toml"),
    ),
    (
        "maia-executor",
        include_str!("../../core/executor/Cargo.toml"),
    ),
    (
        "maia-briefing",
        include_str!("../../core/briefing/Cargo.toml"),
    ),
    (
        "maia-assurance-router",
        include_str!("../../core/assurance-router/Cargo.toml"),
    ),
];

/// Shipped applications: a DEPLOYMENT_LOCKED artifact is built from these.
const APPS: &[(&str, &str)] = &[
    (
        "maia-desktop",
        include_str!("../../apps/desktop/Cargo.toml"),
    ),
    (
        "maia-local-briefing-host",
        include_str!("../../apps/local-briefing-host/Cargo.toml"),
    ),
    // M0.15.14: the Local Intelligence Recorder host -- a plain
    // application composition root that keeps one Recorder running
    // continuously. No Round Table dependency (it depends only on the
    // Local Intelligence infra chain: recorder/diagnostics/health/ledger).
    (
        "maia-local-intelligence-host",
        include_str!("../../apps/local-intelligence-host/Cargo.toml"),
    ),
];

/// Remaining workspace members, needed to resolve the transitive closure.
const OTHER: &[(&str, &str)] = &[
    ("maia-roundtable", include_str!("../Cargo.toml")),
    (
        "maia-anthropic",
        include_str!("../../infra/anthropic/Cargo.toml"),
    ),
    (
        "maia-claude-code",
        include_str!("../../infra/claude-code/Cargo.toml"),
    ),
    (
        "maia-local-model",
        include_str!("../../infra/local-model/Cargo.toml"),
    ),
    // M0.15.10: the Local Intelligence observability/health-history
    // capability. Depends only on maia-local-model and maia-briefing
    // (Core); no Round Table crate depends on it and it depends on no
    // Round Table crate, so it belongs here, never in the Round Table list
    // above.
    (
        "maia-local-intelligence-health",
        include_str!("../../infra/local-intelligence-health/Cargo.toml"),
    ),
    // M0.15.11: the read-only diagnostic evidence/snapshot capability.
    // Depends only on maia-local-intelligence-health (dev-dependencies on
    // maia-briefing/maia-local-model are test-fixture-only and excluded by
    // `normal_maia_deps`'s dependency-section scan below); no Round Table
    // crate depends on it and it depends on no Round Table crate.
    (
        "maia-local-intelligence-diagnostics",
        include_str!("../../infra/local-intelligence-diagnostics/Cargo.toml"),
    ),
    // M0.15.12: the persistent hypothesis evidence ledger. Depends on
    // maia-local-intelligence-diagnostics (its public DiagnosticSnapshot
    // contract) and maia-local-intelligence-health (used only by its CLI
    // binary, to build a fresh snapshot before recording it -- never by
    // its library); no Round Table crate depends on it and it depends on
    // no Round Table crate.
    (
        "maia-local-intelligence-hypothesis-ledger",
        include_str!("../../infra/local-intelligence-hypothesis-ledger/Cargo.toml"),
    ),
    // M0.15.13: the bounded-cadence diagnostic evidence recorder. Depends
    // on all three prior Local Intelligence capabilities (health,
    // diagnostics, ledger) and nothing else; no Round Table crate depends
    // on it and it depends on no Round Table crate.
    (
        "maia-local-intelligence-recorder",
        include_str!("../../infra/local-intelligence-recorder/Cargo.toml"),
    ),
    ("maia-sqlite", include_str!("../../infra/sqlite/Cargo.toml")),
    (
        "maia-roundtable-store",
        include_str!("../../infra/roundtable-store/Cargo.toml"),
    ),
    // M0.14: the composition layer that realizes an AssurancePlan against
    // the Round Table runtime. It legitimately depends on both
    // maia-assurance-router (Core) and maia-roundtable (Round Table
    // implementation) — that is its entire purpose — so it belongs here,
    // never in CORE or APPS, and its presence must never let a CORE or APPS
    // crate reach it and, through it, the Round Table implementation.
    (
        "maia-roundtable-observability",
        include_str!("../../composition/roundtable-observability/Cargo.toml"),
    ),
    (
        "maia-roundtable-composition",
        include_str!("../../composition/roundtable-composition/Cargo.toml"),
    ),
    (
        "maia-local-model-roundtable",
        include_str!("../../infra/local-model-roundtable/Cargo.toml"),
    ),
    (
        "maia-roundtable-viewer",
        include_str!("../../apps/roundtable-viewer/Cargo.toml"),
    ),
    (
        "maia-roundtable-invoke",
        include_str!("../../ops/roundtable-invoke/Cargo.toml"),
    ),
];

const WORKSPACE_MANIFEST: &str = include_str!("../../Cargo.toml");

fn all_manifests() -> Vec<(&'static str, &'static str)> {
    let mut v = Vec::new();
    v.extend_from_slice(CORE);
    v.extend_from_slice(APPS);
    v.extend_from_slice(OTHER);
    v
}

/// Normal (non-dev, non-build) `maia-*` dependencies declared by a manifest.
///
/// Dev-dependencies are excluded on purpose: they never ship, so they cannot
/// make a released Core require the Round Table implementation.
fn normal_maia_deps(manifest: &str) -> Vec<String> {
    let mut deps = Vec::new();
    let mut in_normal = false;
    for raw in manifest.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            // Section headers we care about: [dependencies] and
            // [target.'...'.dependencies]. Everything else (dev-dependencies,
            // build-dependencies, features, package) turns collection off.
            in_normal = line == "[dependencies]"
                || (line.starts_with('[')
                    && line.ends_with(".dependencies]")
                    && !line.contains("dev-"));
            continue;
        }
        if !in_normal || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.split('=').next() {
            let name = name.trim().trim_matches('"');
            if name.starts_with("maia-") {
                deps.push(name.to_string());
            }
        }
    }
    deps
}

fn manifest_for(crate_name: &str) -> Option<&'static str> {
    all_manifests()
        .into_iter()
        .find(|(n, _)| *n == crate_name)
        .map(|(_, m)| m)
}

/// Transitive closure of normal dependencies, starting from `root`.
fn closure(root: &str) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut stack = vec![root.to_string()];
    while let Some(current) = stack.pop() {
        let Some(manifest) = manifest_for(&current) else {
            // A dependency outside the embedded set would silently shrink the
            // closure and weaken the guarantee, so refuse rather than skip.
            panic!(
                "dependency `{current}` has no embedded manifest; add it to this \
                 test so the dependency closure stays complete"
            );
        };
        for dep in normal_maia_deps(manifest) {
            if !seen.contains(&dep) {
                seen.push(dep.clone());
                stack.push(dep);
            }
        }
    }
    seen
}

#[test]
fn core_does_not_depend_on_the_round_table_implementation() {
    for (name, _) in CORE {
        let reached = closure(name);
        for forbidden in ROUND_TABLE_CRATES {
            assert!(
                !reached.iter().any(|d| d == forbidden),
                "ADR-0046 violation: MAIA Core crate `{name}` reaches Round Table \
                 implementation `{forbidden}` through {reached:?}. The dependency \
                 direction must be Round Table -> public MAIA contracts, never the reverse."
            );
        }
    }
}

#[test]
fn shipped_applications_do_not_depend_on_the_round_table_implementation() {
    for (name, _) in APPS {
        let reached = closure(name);
        for forbidden in ROUND_TABLE_CRATES {
            assert!(
                !reached.iter().any(|d| d == forbidden),
                "shipped application `{name}` reaches Round Table implementation \
                 `{forbidden}` through {reached:?}; MAIA must be a complete product \
                 with Round Table entirely absent."
            );
        }
    }
}

/// The allowed direction must actually exist, otherwise the two tests above
/// could pass simply because nothing depends on anything.
#[test]
fn adapters_do_depend_on_the_round_table_port() {
    for adapter in [
        "maia-anthropic",
        "maia-claude-code",
        "maia-roundtable-store",
        "maia-local-model-roundtable",
        "maia-roundtable-observability",
        "maia-roundtable-viewer",
        "maia-roundtable-invoke",
    ] {
        let reached = closure(adapter);
        assert!(
            reached.iter().any(|d| d == "maia-roundtable"),
            "`{adapter}` is expected to depend on maia-roundtable (adapter -> port); \
             if this changed, the direction assertions above may be vacuous"
        );
    }
}

/// The composition layer's whole purpose is to depend on both sides: it
/// realizes a Core plan (`maia-assurance-router`) against the Round Table
/// implementation (`maia-roundtable`). Assert both edges exist, so a future
/// change that quietly drops one of them (e.g. reimplementing resolution
/// inline instead of depending on the port) is caught here rather than
/// discovered only when composition stops working.
#[test]
fn composition_depends_on_both_the_core_plan_and_the_round_table_port() {
    let reached = closure("maia-roundtable-composition");
    assert!(
        reached.iter().any(|d| d == "maia-assurance-router"),
        "maia-roundtable-composition is expected to depend on maia-assurance-router \
         (composition -> assurance-router)"
    );
    assert!(
        reached.iter().any(|d| d == "maia-roundtable"),
        "maia-roundtable-composition is expected to depend on maia-roundtable \
         (composition -> roundtable)"
    );
}

/// `infra/local-model` is the credential-free loopback provider shared by
/// the shipped applications and (via its adapter) the Round Table. It must
/// stay Round-Table-unaware on its own: the adapter depends on it, never the
/// reverse, exactly like `maia-anthropic`/`maia-claude-code` never make
/// their upstream API clients Round-Table-aware.
#[test]
fn local_model_never_depends_on_the_round_table_implementation() {
    let reached = closure("maia-local-model");
    for forbidden in ROUND_TABLE_CRATES {
        assert!(
            !reached.iter().any(|d| d == forbidden),
            "infra/local-model must stay Round-Table-unaware, but its closure \
             reaches `{forbidden}` through {reached:?}"
        );
    }
}

/// The local-model Round Table adapter's whole purpose is to depend on both
/// sides: it implements the Round Table `Participant`/`ParticipantResolver`
/// ports against `maia-local-model`'s generic text-completion capability.
/// Assert both edges exist, mirroring the composition-layer check above.
#[test]
fn local_model_adapter_depends_on_both_local_model_and_the_round_table_port() {
    let reached = closure("maia-local-model-roundtable");
    assert!(
        reached.iter().any(|d| d == "maia-local-model"),
        "maia-local-model-roundtable is expected to depend on maia-local-model"
    );
    assert!(
        reached.iter().any(|d| d == "maia-roundtable"),
        "maia-local-model-roundtable is expected to depend on maia-roundtable"
    );
}

/// Guard against a new workspace member escaping the checks: every `core/` and
/// `apps/` member must be covered, and every member must have an embedded
/// manifest so closures stay complete.
#[test]
fn every_workspace_member_is_covered() {
    let members = workspace_members(WORKSPACE_MANIFEST);
    assert!(
        !members.is_empty(),
        "workspace members must be parseable from the root manifest"
    );
    let covered: Vec<&str> = all_manifests().into_iter().map(|(n, _)| n).collect();

    for member in &members {
        // maia-roundtable (this crate) is the module under test and is intentionally not in CORE.
        let expected = match member.rsplit('/').next() {
            Some(dir) => dir,
            None => continue,
        };
        let known = covered.iter().any(|c| {
            let suffix = c.strip_prefix("maia-").unwrap_or(c);
            suffix == expected || c.ends_with(expected)
        });
        assert!(
            known,
            "workspace member `{member}` has no embedded manifest in this test; \
             add it so the dependency closure cannot silently shrink"
        );
    }
}

fn workspace_members(manifest: &str) -> Vec<String> {
    let Some(start) = manifest.find("members") else {
        return Vec::new();
    };
    let Some(open) = manifest[start..].find('[') else {
        return Vec::new();
    };
    let open = start + open;
    let Some(close) = manifest[open..].find(']') else {
        return Vec::new();
    };
    manifest[open + 1..open + close]
        .split(',')
        .filter_map(|e| {
            let e = e.trim().trim_matches('"').trim();
            (!e.is_empty()).then(|| e.to_string())
        })
        .collect()
}

#[test]
fn dependency_parser_ignores_dev_dependencies() {
    let manifest = "\
[package]
name = \"x\"

[dependencies]
maia-domain = { path = \"../domain\" }
serde = \"1\"

[dev-dependencies]
maia-roundtable = { path = \"../roundtable\" }
";
    let deps = normal_maia_deps(manifest);
    assert_eq!(deps, vec!["maia-domain".to_string()]);
    assert!(
        !deps.iter().any(|d| d == "maia-roundtable"),
        "a dev-dependency must not count as a shipping dependency"
    );
}

/// M0.15.2: the observability model reads the Round Table contract and must not
/// reach orchestration or storage. A viewer able to reach composition could
/// invoke providers; one able to reach the store would bind a product surface
/// to a persistence format.
#[test]
fn observability_cannot_reach_orchestration_or_storage() {
    let reached = closure("maia-roundtable-observability");
    for forbidden in [
        "maia-roundtable-composition",
        "maia-roundtable-store",
        "maia-claude-code",
        "maia-anthropic",
        "maia-local-model-roundtable",
    ] {
        assert!(
            !reached.iter().any(|d| d == forbidden),
            "observability reaches `{forbidden}` through {reached:?}; it reads a contract,              it does not orchestrate or store"
        );
    }
    assert!(
        reached.iter().any(|d| d == "maia-roundtable"),
        "observability must depend on the read contract, or this check is vacuous"
    );
}

/// M0.15.4: the read-only viewer picks a storage backend for the read contract,
/// so it legitimately reaches the store and the observability model. It must not
/// reach composition or any provider adapter: a viewer able to do that could
/// invoke a session, and "read-only" would then rest on convention alone.
#[test]
fn viewer_reaches_history_but_cannot_reach_invocation() {
    let reached = closure("maia-roundtable-viewer");
    for required in [
        "maia-roundtable",
        "maia-roundtable-observability",
        "maia-roundtable-store",
    ] {
        assert!(
            reached.iter().any(|d| d == required),
            "viewer is expected to depend on `{required}`; otherwise the checks \
             below are vacuous (closure: {reached:?})"
        );
    }
    for forbidden in [
        "maia-roundtable-composition",
        "maia-claude-code",
        "maia-anthropic",
        "maia-local-model-roundtable",
        "maia-local-model",
    ] {
        assert!(
            !reached.iter().any(|d| d == forbidden),
            "viewer reaches `{forbidden}` through {reached:?}; a read-only viewer \
             must have no path to invoke a provider"
        );
    }
}

/// M0.15.7: nothing may depend on the invocation CLI. It constructs providers, so
/// a crate that reached it would gain a path to a live call. This is checked for
/// every other workspace crate, not a hand-picked few, so a new crate cannot
/// slip past by omission.
#[test]
fn nothing_reaches_the_invocation_cli() {
    for (name, _) in all_manifests() {
        if name == "maia-roundtable-invoke" {
            continue;
        }
        let reached = closure(name);
        assert!(
            !reached.iter().any(|d| d == "maia-roundtable-invoke"),
            "`{name}` reaches the invocation CLI through {reached:?}; the invoker is a \
             leaf: nothing may depend on it"
        );
    }
}

/// The allowed direction must exist, otherwise the leaf check above could pass
/// because the invoker depends on nothing. It composes and persists sessions, and
/// is the only writer; it is never a shipped application.
#[test]
fn invoker_reaches_composition_and_the_store_and_is_not_a_shipped_app() {
    let reached = closure("maia-roundtable-invoke");
    for required in [
        "maia-roundtable",
        "maia-roundtable-composition",
        "maia-roundtable-store",
        "maia-roundtable-observability",
        "maia-assurance-router",
    ] {
        assert!(
            reached.iter().any(|d| d == required),
            "invoker is expected to depend on `{required}`; otherwise the leaf check \
             is vacuous (closure: {reached:?})"
        );
    }
    assert!(
        !APPS.iter().any(|(n, _)| *n == "maia-roundtable-invoke"),
        "the invoker must never be in APPS: it is not a shipped application"
    );
    assert!(
        !WORKSPACE_MANIFEST.contains("apps/roundtable-invoke")
            && WORKSPACE_MANIFEST.contains("ops/roundtable-invoke"),
        "the invoker lives under ops/, outside the apps/* guard in deployment_lock.rs"
    );
}

/// The provider must be absent by construction in a build without
/// `development-evolution`: the dependency is optional, the feature is off by
/// default, and the binary is not produced without it.
#[test]
fn invoker_provider_is_absent_by_construction_without_the_feature() {
    let manifest = manifest_for("maia-roundtable-invoke").expect("invoker manifest embedded");
    assert!(
        manifest.contains("default = []"),
        "development-evolution must never be a default feature"
    );
    let provider_line = manifest
        .lines()
        .find(|l| l.trim_start().starts_with("maia-claude-code ="))
        .expect("invoker declares the provider dependency");
    assert!(
        provider_line.contains("optional = true"),
        "the provider dependency must be optional"
    );
    let bin_at = manifest
        .find("[[bin]]")
        .expect("invoker declares its binary");
    assert!(
        manifest[bin_at..].contains(r#"required-features = ["development-evolution"]"#),
        "the binary must require development-evolution so a locked build does not produce it"
    );
}
