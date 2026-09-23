//! C008 — capability absence by construction.
//!
//! v5.1 §6.2 requires that self-modification-adjacent capability be absent by
//! construction rather than disabled by a runtime boolean. These tests verify
//! the structural property, not just a flag.
//!
//! Sources are embedded with `include_str!` rather than read at runtime, so the
//! tests are path-independent: they do not depend on where the binary was built
//! and cannot pass or fail because of a stale artifact or a moved tree. Cargo
//! also treats each embedded file as a rebuild dependency.

use maia_claude_code::capability_present;

const RUNNER_RS: &str = include_str!("../src/runner.rs");
const LIB_RS: &str = include_str!("../src/lib.rs");
const CONTRACT_RS: &str = include_str!("../src/contract.rs");
const AUDIT_RS: &str = include_str!("../src/audit.rs");
const OWN_MANIFEST: &str = include_str!("../Cargo.toml");
const WORKSPACE_MANIFEST: &str = include_str!("../../../Cargo.toml");

/// MAIA Core crates. Embedded by literal path so a missing file is a compile
/// error, never a silently skipped check.
const CORE_MANIFESTS: &[(&str, &str)] = &[
    (
        "core/domain",
        include_str!("../../../core/domain/Cargo.toml"),
    ),
    (
        "core/policy",
        include_str!("../../../core/policy/Cargo.toml"),
    ),
    (
        "core/orchestrator",
        include_str!("../../../core/orchestrator/Cargo.toml"),
    ),
    ("core/store", include_str!("../../../core/store/Cargo.toml")),
    (
        "core/runtime",
        include_str!("../../../core/runtime/Cargo.toml"),
    ),
    (
        "core/executor",
        include_str!("../../../core/executor/Cargo.toml"),
    ),
    // core/roundtable moved to roundtable/ (M0.15.8, ADR-0046 physical extraction):
    // it was never really MAIA Core (see core_independence.rs, which has always
    // classified it as OTHER, not CORE) and no longer lives under core/, so it is
    // deliberately not listed here.
    (
        "core/assurance-router",
        include_str!("../../../core/assurance-router/Cargo.toml"),
    ),
    (
        "core/briefing",
        include_str!("../../../core/briefing/Cargo.toml"),
    ),
];

/// Shipped applications. A DEPLOYMENT_LOCKED artifact is built from these, so
/// none may pull the provider into its build graph.
const APP_MANIFESTS: &[(&str, &str)] = &[
    (
        "apps/desktop",
        include_str!("../../../apps/desktop/Cargo.toml"),
    ),
    (
        "apps/local-briefing-host",
        include_str!("../../../apps/local-briefing-host/Cargo.toml"),
    ),
    // M0.15.14: the Local Intelligence Recorder host -- a shipped
    // application composition root; must have no path to the provider.
    (
        "apps/local-intelligence-host",
        include_str!("../../../apps/local-intelligence-host/Cargo.toml"),
    ),
    // M0.15.4: not a shipped application (D009 keeps it out of `APPS`), but it
    // lives under apps/ and must be covered here. Listing it asserts the read-only
    // viewer has no direct path to the provider.
    (
        "apps/roundtable-viewer",
        include_str!("../../../apps/roundtable-viewer/Cargo.toml"),
    ),
];

#[test]
fn capability_flag_tracks_the_build_feature_not_a_runtime_toggle() {
    assert_eq!(
        capability_present(),
        cfg!(feature = "development-evolution")
    );
}

/// In a DEPLOYMENT_LOCKED build *no* constructor in this crate may produce a
/// provider — not the public one and not the test-facing one.
#[cfg(not(feature = "development-evolution"))]
#[test]
fn no_constructor_can_produce_a_provider_in_a_locked_build() {
    use maia_claude_code::{
        ClaudeCodeConfig, ClaudeCodeError, ClaudeCodeProvider,
        audit::{AuditError, AuditRecord, AuditSink},
        runner::{CancellationToken, Invocation, ProcessError, ProcessOutcome, ProcessRunner},
    };

    struct NoRunner;
    impl ProcessRunner for NoRunner {
        fn run(
            &self,
            _: &Invocation,
            _: &CancellationToken,
        ) -> Result<ProcessOutcome, ProcessError> {
            unreachable!("a locked build must never reach a process runner")
        }
    }
    struct NoAudit;
    impl AuditSink for NoAudit {
        fn persist(&self, _: &AuditRecord) -> Result<(), AuditError> {
            Ok(())
        }
    }

    let config = ClaudeCodeConfig::new("claude", ".", ".");
    assert_eq!(
        ClaudeCodeProvider::new("claude-code", config.clone(), "x".into(), NoRunner, NoAudit).err(),
        Some(ClaudeCodeError::CapabilityAbsent)
    );
    assert_eq!(
        ClaudeCodeProvider::new_without_recursion_check(
            "claude-code",
            config,
            "x".into(),
            NoRunner,
            NoAudit
        )
        .err(),
        Some(ClaudeCodeError::CapabilityAbsent),
        "the test-facing constructor must not be a DEPLOYMENT_LOCKED bypass"
    );
}

/// Every occurrence of `needle` in `source` must appear after `gate`.
/// Returns the offending byte offset so a failure is actionable.
fn offenders_before_gate(source: &str, gate: &str, needle: &str) -> Vec<usize> {
    let Some(gate_at) = source.find(gate) else {
        return vec![0];
    };
    let mut found = Vec::new();
    let mut from = 0usize;
    while let Some(hit) = source[from..].find(needle) {
        let at = from + hit;
        if at < gate_at {
            found.push(at);
        }
        from = at + needle.len();
    }
    found
}

/// The whole point of §6.2: a DEPLOYMENT_LOCKED build must contain no process
/// spawning code for this capability. Assert that every construct able to start
/// a child lives inside the feature-gated module.
#[test]
fn all_subprocess_code_is_behind_the_development_evolution_feature() {
    let gate = "#[cfg(feature = \"development-evolution\")]";
    assert!(
        RUNNER_RS.contains(gate),
        "runner.rs must gate its real implementation"
    );

    for needle in [
        "process::Command",
        "Command::new",
        "spawn()",
        "Stdio::piped",
    ] {
        let offenders = offenders_before_gate(RUNNER_RS, gate, needle);
        assert!(
            offenders.is_empty(),
            "`{needle}` appears at byte offset(s) {offenders:?} in runner.rs, \
             outside the development-evolution gate"
        );
    }

    // No other module in the crate may spawn a process at all.
    for (name, body) in [
        ("lib.rs", LIB_RS),
        ("contract.rs", CONTRACT_RS),
        ("audit.rs", AUDIT_RS),
    ] {
        assert!(
            !body.contains("Command::new"),
            "{name} must not spawn a process; subprocess code belongs in runner.rs"
        );
    }
}

/// The gate scanner must actually be able to fail, or the test above proves nothing.
#[test]
fn gate_scanner_detects_a_violation() {
    let gate = "#[cfg(feature = \"x\")]";
    let compliant = format!("mod a {{}}\n{gate}\nmod real {{ Command::new(); }}");
    let violating = format!("Command::new();\n{gate}\nmod real {{}}");
    let ungated = "Command::new();";

    assert!(offenders_before_gate(&compliant, gate, "Command::new").is_empty());
    assert_eq!(
        offenders_before_gate(&violating, gate, "Command::new").len(),
        1
    );
    // A missing gate is itself a violation, never a silent pass.
    assert!(!offenders_before_gate(ungated, gate, "Command::new").is_empty());
}

/// MAIA Core must never depend on this adapter (ADR-0046 dependency direction).
#[test]
fn core_does_not_depend_on_the_provider() {
    for (name, manifest) in CORE_MANIFESTS {
        assert!(
            !manifest.contains("maia-claude-code"),
            "{name} must not depend on maia-claude-code"
        );
    }
}

/// No shipped application may pull the provider into its build graph.
#[test]
fn no_shipped_application_depends_on_the_provider() {
    for (name, manifest) in APP_MANIFESTS {
        assert!(
            !manifest.contains("maia-claude-code"),
            "{name} must not depend on maia-claude-code"
        );
    }
}

/// Guard against a newly added core or app crate silently escaping the two
/// checks above, which would let the dependency rule rot without any test failing.
#[test]
fn every_core_and_app_crate_is_covered_by_the_dependency_checks() {
    let members = workspace_members(WORKSPACE_MANIFEST);
    assert!(
        !members.is_empty(),
        "workspace members must be parseable from the root manifest"
    );

    let covered: Vec<&str> = CORE_MANIFESTS
        .iter()
        .chain(APP_MANIFESTS.iter())
        .map(|(name, _)| *name)
        .collect();

    for member in members
        .iter()
        .filter(|m| m.starts_with("core/") || m.starts_with("apps/"))
    {
        assert!(
            covered.contains(&member.as_str()),
            "workspace member `{member}` is not covered by the dependency checks; \
             add its manifest to CORE_MANIFESTS or APP_MANIFESTS"
        );
    }
}

/// The capability must not be switched on by default.
#[test]
fn development_evolution_is_not_a_default_feature() {
    assert!(
        OWN_MANIFEST.contains("development-evolution"),
        "the feature must exist to be meaningfully off by default"
    );
    assert!(
        OWN_MANIFEST.contains("default = []"),
        "development-evolution must never be a default feature"
    );
}

/// Minimal `members = [...]` extractor for the workspace manifest.
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
        .filter_map(|entry| {
            let entry = entry.trim().trim_matches('"').trim();
            (!entry.is_empty()).then(|| entry.to_string())
        })
        .collect()
}

#[test]
fn workspace_member_parser_handles_the_real_manifest() {
    let members = workspace_members(WORKSPACE_MANIFEST);
    assert!(members.contains(&"roundtable".to_string()));
    assert!(members.contains(&"infra/claude-code".to_string()));
    assert!(workspace_members("no members here").is_empty());
}
