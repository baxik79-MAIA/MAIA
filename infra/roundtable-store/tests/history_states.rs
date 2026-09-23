//! M0.15.3 — the states of Round Table history, against a real filesystem.
//!
//! `composition/roundtable-observability` proves the distinctions on doubles.
//! This file proves that the real store produces them, and that nothing on the
//! read path (or in merely constructing a store) brings Round Table storage
//! into existence. MAIA must operate with the Round Table absent, so the store
//! may appear only when a session is deliberately saved.

use maia_domain::ReasoningAssuranceLevel;
use maia_roundtable::{DecisionRequest, SessionFailureKind, SessionRecord, SessionStore};
use maia_roundtable_observability::{
    Availability, ObservabilityState, UnavailableReason, load_panel, load_state,
};
use maia_roundtable_store::{FileSessionStore, StoredSessionQuery};

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("maia-history-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&dir);
    dir
}

fn minimal_record(id: &str) -> SessionRecord {
    SessionRecord {
        id: id.into(),
        assurance: ReasoningAssuranceLevel::A3,
        decision: DecisionRequest {
            id: "d".into(),
            subject: "s".into(),
            prompt: "synthetic".into(),
            evidence: vec![],
        },
        outcomes: vec![],
        leader: None,
        disagreements: vec![],
        adjudication: None,
        failure: Some(SessionFailureKind::NotARoundTableLevel),
    }
}

#[test]
fn constructing_stores_and_queries_creates_nothing() {
    let dir = scratch("construct");
    let _store = FileSessionStore::new(&dir);
    let _query = StoredSessionQuery::new(&dir);
    assert!(!dir.exists(), "construction must not touch the filesystem");
}

#[test]
fn every_read_path_leaves_absent_history_absent() {
    let dir = scratch("read-absent");
    let store = FileSessionStore::new(&dir);
    let query = StoredSessionQuery::new(&dir);

    assert_eq!(store.list().unwrap(), Vec::<String>::new());
    assert!(store.load("anything").unwrap().is_none());
    let _ = load_state(&query);
    assert!(matches!(load_panel(&query, "anything"), Ok(None)));

    assert!(
        !dir.exists(),
        "no read path may create Round Table storage as a side effect"
    );
}

#[test]
fn only_a_deliberate_save_brings_storage_into_existence() {
    let dir = scratch("save");
    let store = FileSessionStore::new(&dir);
    assert!(!dir.exists());

    store.save(&minimal_record("first")).expect("save");
    assert!(dir.exists(), "saving is the one act that creates storage");
    assert_eq!(store.list().unwrap(), vec!["first".to_string()]);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn never_installed_and_healthy_empty_are_different_states() {
    // A build without the Round Table says so explicitly. It cannot be inferred
    // from the store, because a build without one has no store to ask.
    let never_installed = ObservabilityState::not_installed();

    // A build with the Round Table and a store that exists but holds nothing.
    let dir = scratch("healthy-empty");
    std::fs::create_dir_all(&dir).unwrap();
    let healthy_empty = load_state(&StoredSessionQuery::new(&dir));

    assert!(never_installed.sessions.is_empty());
    assert!(healthy_empty.sessions.is_empty());
    assert_eq!(never_installed.availability, Availability::NotInstalled);
    assert_eq!(healthy_empty.availability, Availability::Available);
    assert_ne!(never_installed.availability, healthy_empty.availability);

    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        0,
        "reading an empty store must not put anything in it"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn installed_but_not_yet_used_is_healthy_empty_and_stays_unmaterialised() {
    // Round Table present, nothing ever recorded, no directory yet. That is a
    // healthy empty history — not a fault and not "not installed" — and looking
    // at it must not change that.
    let dir = scratch("unused");
    let state = load_state(&StoredSessionQuery::new(&dir));

    assert_eq!(state.availability, Availability::Available);
    assert!(state.sessions.is_empty());
    assert_ne!(state.availability, Availability::NotInstalled);
    assert!(!dir.exists());
}

#[test]
fn unreachable_storage_is_a_fault_not_an_empty_history() {
    // The history location exists but is a regular file, so it cannot be read
    // as a store. Reporting this as "no sessions yet" would hide a broken
    // deployment behind a healthy-looking empty list.
    let path = scratch("unreachable");
    std::fs::write(&path, b"not a directory").unwrap();

    let state = load_state(&StoredSessionQuery::new(&path));
    assert_eq!(
        state.availability,
        Availability::Unavailable(UnavailableReason::HistoryUnreachable)
    );
    assert_ne!(state.availability, Availability::Available);
    assert!(state.sessions.is_empty());

    let _ = std::fs::remove_file(&path);
}

#[test]
fn asking_for_one_session_in_unreachable_storage_is_a_fault_not_a_missing_session() {
    // Same broken store as above, reached through `show` rather than `list`.
    // "No such session" would tell an operator the history is fine.
    let path = scratch("unreachable-load");
    std::fs::write(&path, b"not a directory").unwrap();

    assert!(FileSessionStore::new(&path).load("any").is_err());
    assert_eq!(
        load_panel(&StoredSessionQuery::new(&path), "any").unwrap_err(),
        Availability::Unavailable(UnavailableReason::HistoryUnreachable)
    );

    let _ = std::fs::remove_file(&path);
}

#[test]
fn history_that_exists_but_is_entirely_unreadable_is_not_healthy_empty() {
    // Files are present and none can be read. An empty list here would say "no
    // sessions" about a store that plainly has some.
    let dir = scratch("all-corrupt");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.json"), b"{ this is not a session").unwrap();
    std::fs::write(dir.join("b.json"), b"").unwrap();

    let state = load_state(&StoredSessionQuery::new(&dir));
    assert_eq!(
        state.availability,
        Availability::Unavailable(UnavailableReason::HistoryCorrupt)
    );
    assert_ne!(state.availability, Availability::Available);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn non_session_files_do_not_count_as_history() {
    // Only `*.json` is a session. Other files sitting in the directory neither
    // make the history corrupt nor make it non-empty.
    let dir = scratch("stray");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("README.txt"), b"notes").unwrap();

    let state = load_state(&StoredSessionQuery::new(&dir));
    assert_eq!(state.availability, Availability::Available);
    assert!(state.sessions.is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}
