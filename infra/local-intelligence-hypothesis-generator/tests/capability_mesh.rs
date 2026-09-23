//! Dependency and authority boundary for deterministic hypothesis generation.

const GENERATOR_MANIFEST: &str = include_str!("../Cargo.toml");
const GENERATOR_LIB: &str = include_str!("../src/lib.rs");
const LEDGER_MANIFEST: &str = include_str!("../../local-intelligence-hypothesis-ledger/Cargo.toml");
const DIAGNOSTICS_MANIFEST: &str = include_str!("../../local-intelligence-diagnostics/Cargo.toml");
const HEALTH_MANIFEST: &str = include_str!("../../local-intelligence-health/Cargo.toml");
const LOCAL_MODEL_MANIFEST: &str = include_str!("../../local-model/Cargo.toml");

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
fn generator_has_only_read_contract_and_deterministic_hash_dependencies() {
    let deps = normal_dependencies(GENERATOR_MANIFEST);
    assert_eq!(
        deps,
        [
            "maia-local-intelligence-diagnostics",
            "maia-local-intelligence-hypothesis-ledger",
            "sha2"
        ]
    );
}

#[test]
fn monitored_capabilities_cannot_depend_on_generator() {
    for manifest in [
        LEDGER_MANIFEST,
        DIAGNOSTICS_MANIFEST,
        HEALTH_MANIFEST,
        LOCAL_MODEL_MANIFEST,
    ] {
        assert!(
            !normal_dependencies(manifest)
                .contains(&"maia-local-intelligence-hypothesis-generator")
        );
    }
}

#[test]
fn generator_has_no_process_or_provider_invocation_source() {
    let production = GENERATOR_LIB.split("#[cfg(test)]").next().unwrap();
    for forbidden in [
        "std::process::Command",
        "process::Command",
        ".kill()",
        "TerminateProcess",
        "maia_local_model::",
        "maia_roundtable::",
        "reqwest::",
        "TcpStream",
    ] {
        assert!(
            !production.contains(forbidden),
            "forbidden authority or provider API: {forbidden}"
        );
    }
}
