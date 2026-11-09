//! Explicit file locations for the desktop's local preferences and diagnostics.
//! Existing defaults are unchanged. A present invalid/empty override never
//! falls back to a different profile; its I/O fails under existing best effort.
use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

fn selected_path(value: Option<OsString>, default: &str) -> PathBuf {
    value
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
}

pub(super) fn preferences_path() -> PathBuf {
    selected_path(
        std::env::var_os("MAIA_ALPHA1_PREFERENCES"),
        r"C:\MAIA\data\maia-alpha1-ui.json",
    )
}

pub(super) fn diagnostics_path() -> PathBuf {
    selected_path(
        std::env::var_os("MAIA_ALPHA1_DIAGNOSTICS"),
        r"C:\MAIA\reports\maia_failure_diagnostics.log",
    )
}

fn create_parent(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub(super) fn write_preferences(path: &Path, language: &str, dark: bool) -> io::Result<()> {
    create_parent(path)?;
    let raw = serde_json::to_vec(&serde_json::json!({"language": language, "dark": dark}))
        .map_err(io::Error::other)?;
    fs::write(path, raw)
}

pub(super) fn append_diagnostics(path: &Path, line: &str) -> io::Result<()> {
    use io::Write as _;
    create_parent(path)?;
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(line.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("maia-profile-{}", uuid::Uuid::now_v7()));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn preferences_roundtrip_stays_in_selected_profile() {
        let f = Fixture::new();
        let other = f.0.join("other.json");
        fs::write(&other, b"untouched-other-profile").unwrap();
        let path = f.0.join("selected/nested/ui.json");
        write_preferences(&path, "pl", true).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(value, serde_json::json!({"language":"pl","dark":true}));
        assert_eq!(fs::read(other).unwrap(), b"untouched-other-profile");
    }

    #[test]
    fn diagnostics_append_stays_in_selected_profile() {
        let f = Fixture::new();
        let other = f.0.join("other.log");
        fs::write(&other, b"existing-history\n").unwrap();
        let path = f.0.join("selected/diagnostics.log");
        append_diagnostics(&path, "fixture-first\n").unwrap();
        append_diagnostics(&path, "fixture-second\n").unwrap();
        assert_eq!(fs::read(path).unwrap(), b"fixture-first\nfixture-second\n");
        assert_eq!(fs::read(other).unwrap(), b"existing-history\n");
    }

    #[test]
    fn present_empty_override_does_not_use_default_profile() {
        let f = Fixture::new();
        let default = f.0.join("default.json");
        fs::write(&default, b"original").unwrap();
        let selected = selected_path(Some(OsString::new()), default.to_str().unwrap());
        assert!(selected.as_os_str().is_empty());
        assert!(write_preferences(&selected, "en", false).is_err());
        assert_eq!(fs::read(default).unwrap(), b"original");
    }

    #[test]
    fn invalid_selected_parent_never_falls_back_to_another_profile() {
        let f = Fixture::new();
        let blocked = f.0.join("not-a-directory");
        fs::write(&blocked, b"sentinel").unwrap();
        let path = blocked.join("ui.json");
        assert!(write_preferences(&path, "en", false).is_err());
        assert!(append_diagnostics(&path, "fixture\n").is_err());
        assert_eq!(fs::read(blocked).unwrap(), b"sentinel");
    }
}
