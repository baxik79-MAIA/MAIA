//! M0.15.4 — the viewer, exercised as a real process.
//!
//! The properties that matter are about what the process does to the world:
//! exit codes an operator can script against, wording that keeps absent, empty
//! and broken apart, and above all that it never creates or changes history.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use maia_domain::ReasoningAssuranceLevel;
use maia_roundtable::{
    DecisionRequest, RoundTableDecision, SessionFailureKind, SessionRecord, SessionStore,
};
use maia_roundtable_store::FileSessionStore;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("maia-viewer-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&dir);
    dir
}

fn viewer(cwd: &Path, args: &[&str]) -> Output {
    std::fs::create_dir_all(cwd).unwrap();
    Command::new(env!("CARGO_BIN_EXE_maia-roundtable-viewer"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("viewer runs")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn decision(id: &str) -> DecisionRequest {
    DecisionRequest {
        id: id.into(),
        subject: "should we ship".into(),
        prompt: "synthetic".into(),
        evidence: vec![],
    }
}

fn failed_record() -> SessionRecord {
    SessionRecord {
        id: "session-failed".into(),
        assurance: ReasoningAssuranceLevel::A3,
        decision: decision("d-failed"),
        outcomes: vec![],
        leader: None,
        disagreements: vec![],
        adjudication: None,
        failure: Some(SessionFailureKind::NotARoundTableLevel),
    }
}

fn decided_record() -> SessionRecord {
    SessionRecord {
        id: "session-decided".into(),
        assurance: ReasoningAssuranceLevel::A3,
        decision: decision("d-ok"),
        outcomes: vec![],
        leader: None,
        disagreements: vec![],
        adjudication: Some(RoundTableDecision {
            session_id: "session-decided".into(),
            conclusion: "they broadly agree".into(),
            evidence: vec![],
            disagreement_ids: vec![],
            execution_authority: false,
        }),
        failure: None,
    }
}

fn seeded(name: &str) -> PathBuf {
    let dir = scratch(name);
    let store = FileSessionStore::new(&dir);
    store.save(&failed_record()).unwrap();
    store.save(&decided_record()).unwrap();
    dir
}

fn fingerprint(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| {
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

fn dir_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

// ------------------------------------------------------------------- usage

#[test]
fn there_is_no_default_history_location() {
    let cwd = scratch("no-default-cwd");
    let out = viewer(&cwd, &["list"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("--history is required"));
    assert!(out.stdout.is_empty());
    assert_eq!(
        std::fs::read_dir(&cwd).unwrap().count(),
        0,
        "a viewer with no location must not guess one or create anything"
    );
    let _ = std::fs::remove_dir_all(&cwd);
}

#[test]
fn unrecognised_input_is_refused_rather_than_ignored() {
    let cwd = scratch("usage-cwd");
    let dir = dir_str(&scratch("usage-history"));
    for args in [
        vec!["--history", dir.as_str()],
        vec!["--history", dir.as_str(), "delete"],
        vec!["--history", dir.as_str(), "show"],
        vec!["--history", dir.as_str(), "list", "--force"],
        vec!["--history"],
        vec!["--history", dir.as_str(), "list", "extra"],
    ] {
        let out = viewer(&cwd, &args);
        assert_eq!(out.status.code(), Some(2), "args {args:?}");
        assert!(out.stdout.is_empty(), "args {args:?}");
    }
    let _ = std::fs::remove_dir_all(&cwd);
}

#[test]
fn help_states_that_it_is_read_only() {
    let cwd = scratch("help-cwd");
    let out = viewer(&cwd, &["--help"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(text(&out.stdout).contains("Read-only"));
    let _ = std::fs::remove_dir_all(&cwd);
}

// ----------------------------------------------------------- history states

#[test]
fn absent_history_is_healthy_empty_and_stays_absent() {
    let cwd = scratch("absent-cwd");
    let dir = scratch("absent-history");
    assert!(!dir.exists());

    let list = viewer(&cwd, &["--history", &dir_str(&dir), "list"]);
    assert_eq!(list.status.code(), Some(0));
    assert!(text(&list.stdout).contains("No sessions have been recorded yet"));
    assert!(!text(&list.stdout).contains("Action needed"));

    let show = viewer(&cwd, &["--history", &dir_str(&dir), "show", "anything"]);
    assert_eq!(show.status.code(), Some(1));
    assert!(text(&show.stderr).contains("No session with id `anything` was found."));

    assert!(
        !dir.exists(),
        "viewing must not create the history directory"
    );
    let _ = std::fs::remove_dir_all(&cwd);
}

#[test]
fn unreadable_history_is_a_fault_with_its_own_exit_code() {
    let cwd = scratch("fault-cwd");
    let file = scratch("fault-history");
    std::fs::write(&file, b"not a directory").unwrap();

    let list = viewer(&cwd, &["--history", &dir_str(&file), "list"]);
    assert_eq!(list.status.code(), Some(3));
    assert!(text(&list.stdout).contains("Action needed"));
    assert!(!text(&list.stdout).contains("No sessions have been recorded"));

    let show = viewer(&cwd, &["--history", &dir_str(&file), "show", "x"]);
    assert_eq!(show.status.code(), Some(3));
    assert!(text(&show.stderr).contains("Action needed"));

    let _ = std::fs::remove_file(&file);
    let _ = std::fs::remove_dir_all(&cwd);
}

#[test]
fn history_with_only_unreadable_records_is_a_fault_not_empty() {
    let cwd = scratch("corrupt-cwd");
    let dir = scratch("corrupt-history");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.json"), b"{ nope").unwrap();

    let list = viewer(&cwd, &["--history", &dir_str(&dir), "list"]);
    assert_eq!(list.status.code(), Some(3));
    assert!(text(&list.stdout).contains("none can be read"));

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&cwd);
}

// ------------------------------------------------------------------ viewing

#[test]
fn lists_and_shows_real_sessions() {
    let cwd = scratch("view-cwd");
    let dir = seeded("view-history");
    let h = dir_str(&dir);

    let list = viewer(&cwd, &["--history", &h, "list"]);
    assert_eq!(list.status.code(), Some(0));
    let listing = text(&list.stdout);
    assert!(listing.contains("2 session(s)"));
    assert!(listing.contains("session-decided"));
    assert!(listing.contains("session-failed"));

    let show = viewer(&cwd, &["--history", &h, "show", "session-decided"]);
    assert_eq!(show.status.code(), Some(0));
    let panel = text(&show.stdout);
    assert!(panel.contains("Conclusion: they broadly agree"));
    assert!(panel.contains("Execution authority: none"));

    let failed = viewer(&cwd, &["--history", &h, "show", "session-failed"]);
    assert_eq!(failed.status.code(), Some(0));
    assert!(text(&failed.stdout).contains("did not reach a decision"));

    let missing = viewer(&cwd, &["--history", &h, "show", "no-such"]);
    assert_eq!(missing.status.code(), Some(1));

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&cwd);
}

#[test]
fn viewing_leaves_every_stored_byte_and_the_working_directory_unchanged() {
    let cwd = scratch("readonly-cwd");
    let dir = seeded("readonly-history");
    let h = dir_str(&dir);
    let before = fingerprint(&dir);
    assert_eq!(before.len(), 2);

    for _ in 0..3 {
        for args in [
            vec!["--history", h.as_str(), "list"],
            vec!["--history", h.as_str(), "show", "session-decided"],
            vec!["--history", h.as_str(), "show", "session-failed"],
            vec!["--history", h.as_str(), "show", "no-such"],
        ] {
            viewer(&cwd, &args);
        }
    }

    assert_eq!(
        before,
        fingerprint(&dir),
        "the viewer must not create, delete, rewrite or touch any stored byte"
    );
    assert_eq!(
        std::fs::read_dir(&cwd).unwrap().count(),
        0,
        "the viewer must not write into its working directory"
    );

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&cwd);
}
