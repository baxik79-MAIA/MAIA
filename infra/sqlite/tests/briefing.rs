use maia_briefing::{
    BriefingClaim, BriefingPacket, ClaimClass, PacketEvidence, ValidatedBriefingResult,
    packet_hash, result_hash,
};
use maia_domain::*;
use maia_sqlite::SqliteStore;
use maia_store::{
    ArtifactRepository, AuditRepository, BriefingPersistence, BriefingRepository, audit_event,
};
fn id<T: std::str::FromStr<Err = DomainError>>(s: &str) -> T {
    s.parse().unwrap()
}
const WS: &str = "01900000-0000-7000-8000-000000000000";
const NOW: &str = "2026-09-13T12:34:56.789Z";
fn record() -> BriefingPersistence {
    let packet = BriefingPacket {
        id: "packet-1".into(),
        workspace_id: WS.into(),
        objective: "brief".into(),
        instructions: "cite".into(),
        template_version: "1".into(),
        response_contract: "claims".into(),
        privacy_classification: "local".into(),
        assurance: "A1".into(),
        routing_constraints: "local".into(),
        evidence: vec![PacketEvidence {
            citation_id: "e1".into(),
            artifact_id: "a1".into(),
            content_hash: "h1".into(),
            excerpt: "fact".into(),
        }],
    };
    let packet_hash = packet_hash(&packet).unwrap();
    let result = ValidatedBriefingResult {
        packet_id: "packet-1".into(),
        packet_hash: packet_hash.clone(),
        claims: vec![BriefingClaim {
            text: "known".into(),
            class: ClaimClass::SupportedByEvidence,
            citations: vec!["e1".into()],
        }],
        execution_authority: false,
    };
    BriefingPersistence {
        result_id: "brief-1".into(),
        workspace_id: WS.into(),
        packet,
        packet_hash,
        requested_provider: "local".into(),
        requested_model: "model".into(),
        actual_provider: Some("local".into()),
        actual_model: Some("model".into()),
        assurance: "A1".into(),
        result_hash: result_hash(&result),
        result,
        created_at: id(NOW),
    }
}
#[test]
fn accepted_briefing_roundtrips_after_reopen() {
    let path = std::env::temp_dir().join(format!("maia-brief-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let now: Timestamp = id(NOW);
    let store = SqliteStore::open(&path, id(WS), now.clone()).unwrap();
    let event = audit_event(
        id("01900000-0000-7000-8000-000000000099"),
        id(WS),
        now,
        Some(ActorRef::new("test").unwrap()),
        AuditEventType::new("briefing.accepted").unwrap(),
        AuditSubjectType::new("briefing_result").unwrap(),
        None,
        None,
        None,
        None,
    );
    store.persist_briefing(&record(), &event).unwrap();
    drop(store);
    let reopened = SqliteStore::open(&path, id(WS), id(NOW)).unwrap();
    assert_eq!(reopened.get_briefing("brief-1").unwrap(), record());
    assert_eq!(reopened.list_audit(&id(WS)).unwrap().len(), 1);
}

#[test]
fn artifact_snapshot_roundtrips_and_is_workspace_scoped() {
    let path = std::env::temp_dir().join(format!("maia-artifact-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let now: Timestamp = id(NOW);
    let store = SqliteStore::open(&path, id(WS), now.clone()).unwrap();
    let artifact = Artifact::new(
        id("01900000-0000-7000-8000-000000000010"),
        id(WS),
        ArtifactKind::ImportedTextEvidence,
        NonEmptyString::new("evidence.txt").unwrap(),
        EvidenceMediaType::Utf8PlainText,
        "evidence".into(),
        Sha256Hex::new("ee8250fb76e094b34b471f13a73dbbe51d1ae142e9df59d7c0d31ec20f0a0a8e").unwrap(),
        ByteCount::new(8).unwrap(),
        OpaqueRef::new("private:/evidence").unwrap(),
        Sha256Hex::new("ccf1f42eff5ed96d01e86b514d19cc16f2c4b2fc8916c8dd1cc40b7c919b2d78").unwrap(),
        id("01900000-0000-7000-8000-000000000011"),
        OpaqueRef::new("trusted").unwrap(),
        OpaqueRef::new("internal").unwrap(),
        now.clone(),
    )
    .unwrap();
    let event = audit_event(
        id("01900000-0000-7000-8000-000000000012"),
        id(WS),
        now,
        None,
        AuditEventType::new("workspace_evidence_imported").unwrap(),
        AuditSubjectType::new("artifact").unwrap(),
        Some(OpaqueRef::new(artifact.id().as_str()).unwrap()),
        None,
        None,
        None,
    );
    assert_eq!(store.persist_artifact(&artifact, &event).unwrap(), artifact);
    assert_eq!(store.get_artifact(artifact.id()).unwrap(), artifact);
    assert_eq!(store.list_artifacts().unwrap(), vec![artifact]);
}
