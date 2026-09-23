//! Capability Mesh and no-remediation boundary for M0.15.16.

const QUALIFICATION_MANIFEST: &str = include_str!("../Cargo.toml");
const QUALIFICATION_LIB: &str = include_str!("../src/lib.rs");
const LEDGER_MANIFEST: &str = include_str!("../../local-intelligence-hypothesis-ledger/Cargo.toml");
const DIAGNOSTICS_MANIFEST: &str = include_str!("../../local-intelligence-diagnostics/Cargo.toml");
const HEALTH_MANIFEST: &str = include_str!("../../local-intelligence-health/Cargo.toml");
const LOCAL_MODEL_MANIFEST: &str = include_str!("../../local-model/Cargo.toml");
const GENERATOR_MANIFEST: &str =
    include_str!("../../local-intelligence-hypothesis-generator/Cargo.toml");

fn normal_dependencies(manifest: &str) -> Vec<&str> {
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

#[test]
fn qualification_depends_only_on_public_diagnostics_and_ledger_contracts() {
    assert_eq!(
        normal_dependencies(QUALIFICATION_MANIFEST),
        [
            "maia-local-intelligence-diagnostics",
            "maia-local-intelligence-hypothesis-ledger",
        ]
    );
}

#[test]
fn no_monitored_or_upstream_component_depends_on_qualification() {
    for manifest in [
        LEDGER_MANIFEST,
        DIAGNOSTICS_MANIFEST,
        HEALTH_MANIFEST,
        LOCAL_MODEL_MANIFEST,
        GENERATOR_MANIFEST,
    ] {
        assert!(
            !normal_dependencies(manifest)
                .contains(&"maia-local-intelligence-hypothesis-qualification")
        );
    }
}

#[test]
fn production_source_contains_no_remediation_or_provider_invocation() {
    let production = QUALIFICATION_LIB.split("#[cfg(test)]").next().unwrap();
    for forbidden in [
        "std::process::Command",
        "process::Command",
        ".kill()",
        "TerminateProcess",
        "std::fs::write",
        "std::fs::remove",
        "maia_local_model::",
        "maia_roundtable::",
        "TcpStream",
        "reqwest::",
        "record_hypothesis(",
        "record_status_sample(",
        "record_inference_failure(",
    ] {
        assert!(
            !production.contains(forbidden),
            "forbidden authority or provider API: {forbidden}"
        );
    }
}
