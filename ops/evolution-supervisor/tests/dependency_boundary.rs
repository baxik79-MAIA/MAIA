//! Structural guard for the development-only contract placement.
use std::{fs, path::Path};

const OWN_MANIFEST: &str = include_str!("../Cargo.toml");
const OWN_SOURCE: &str = include_str!("../src/lib.rs");
const TIER0_SOURCE: &str = include_str!("../src/tier0.rs");

#[test]
fn no_product_or_advisory_crate_depends_on_supervisor() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for area in ["core", "infra", "composition", "apps", "roundtable"] {
        let directory = root.join(area);
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let manifest = entry.path().join("Cargo.toml");
            if manifest.is_file() {
                let text = fs::read_to_string(&manifest).unwrap();
                assert!(
                    !text.contains("maia-evolution-supervisor"),
                    "{} must not depend on the development Supervisor",
                    manifest.display()
                );
            }
        }
    }
}

#[test]
fn contract_crate_has_no_side_effect_dependencies_or_binary() {
    assert!(OWN_MANIFEST.contains("[dependencies]\n"));
    assert!(!OWN_MANIFEST.contains("[[bin]]"));
    assert!(!OWN_MANIFEST.contains("required-features"));
    let dependencies = OWN_MANIFEST.split("[dependencies]").nth(1).unwrap();
    assert!(dependencies.trim().is_empty());
    for forbidden in [
        "std::process::",
        "std::fs::",
        "std::net::",
        "Command::",
        "git2::",
        "rusqlite::",
    ] {
        assert!(
            !OWN_SOURCE.contains(forbidden) && !TIER0_SOURCE.contains(forbidden),
            "contract crate gained side-effect API {forbidden}"
        );
    }
}
