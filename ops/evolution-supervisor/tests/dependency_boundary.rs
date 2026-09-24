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
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && matches!(area, "composition" | "roundtable") =>
            {
                continue;
            }
            Err(error) => panic!("cannot inspect {}: {error}", directory.display()),
        };
        for entry in entries {
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
    assert!(empty_dependencies(OWN_MANIFEST));
    assert!(!OWN_MANIFEST.contains("[[bin]]"));
    assert!(!OWN_MANIFEST.contains("required-features"));
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

fn empty_dependencies(manifest: &str) -> bool {
    let lines: Vec<_> = manifest.lines().map(str::trim).collect();
    let Some(section) = lines.iter().position(|line| *line == "[dependencies]") else {
        return false;
    };
    lines[section + 1..].iter().all(|line| line.is_empty())
}

#[test]
fn dependency_section_check_accepts_lf_and_crlf_but_not_dependencies() {
    assert!(empty_dependencies(
        "[package]\nname = \"x\"\n[dependencies]\n"
    ));
    assert!(empty_dependencies(
        "[package]\r\nname = \"x\"\r\n[dependencies]\r\n"
    ));
    assert!(!empty_dependencies("[dependencies]\nserde = \"1\"\n"));
    assert!(!empty_dependencies("[package]\nname = \"x\"\n"));
}
