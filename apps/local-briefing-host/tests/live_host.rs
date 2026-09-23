use maia_briefing::{BriefingPacket, PacketEvidence};
use maia_domain::*;
use maia_sqlite::SqliteStore;
use maia_store::{ArtifactRepository, BriefingRepository, audit_event};
use sha2::{Digest, Sha256};
use std::{fs, process::Command};
fn id<T: std::str::FromStr<Err = DomainError>>(s: &str) -> T {
    s.parse().unwrap()
}
fn hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
const WS: &str = "01900000-0000-7000-8000-000000000000";
const NOW: &str = "2026-09-13T13:40:00.000Z";
#[test]
#[ignore = "requires MAIA-owned local Ollama on 127.0.0.1:11434"]
fn live_host_persists_packet_result_citations_and_provenance_after_reopen() {
    let dir = std::env::temp_dir().join(format!("maia-m010-host-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let db = dir.join("maia.db");
    let now: Timestamp = id(NOW);
    let store = SqliteStore::open(&db, id(WS), now.clone()).unwrap();
    let content = "MAIA local briefing is advisory only.";
    let artifact = Artifact::new(
        id("01900000-0000-7000-8000-000000000010"),
        id(WS),
        ArtifactKind::ImportedTextEvidence,
        NonEmptyString::new("e.txt").unwrap(),
        EvidenceMediaType::Utf8PlainText,
        content.into(),
        Sha256Hex::new(hash(content)).unwrap(),
        ByteCount::new(content.len() as u64).unwrap(),
        OpaqueRef::new("private:/m010/e.txt").unwrap(),
        Sha256Hex::new(hash("private:/m010/e.txt")).unwrap(),
        id("01900000-0000-7000-8000-000000000011"),
        OpaqueRef::new("trusted").unwrap(),
        OpaqueRef::new("internal").unwrap(),
        now.clone(),
    )
    .unwrap();
    let event = audit_event(
        id("01900000-0000-7000-8000-000000000012"),
        id(WS),
        now.clone(),
        None,
        AuditEventType::new("workspace_evidence_imported").unwrap(),
        AuditSubjectType::new("artifact").unwrap(),
        Some(OpaqueRef::new(artifact.id().as_str()).unwrap()),
        None,
        None,
        None,
    );
    store.persist_artifact(&artifact, &event).unwrap();
    drop(store);
    let packet = BriefingPacket {
        id: "m010-live-host".into(),
        workspace_id: WS.into(),
        objective: "State the supplied fact in one advisory claim.".into(),
        instructions: "Use only supplied evidence and cite it.".into(),
        template_version: "briefing-v1".into(),
        response_contract: "claims JSON".into(),
        privacy_classification: "internal".into(),
        assurance: "A1".into(),
        routing_constraints: "local_loopback_only".into(),
        evidence: vec![PacketEvidence {
            citation_id: "e1".into(),
            artifact_id: artifact.id().to_string(),
            content_hash: artifact.content_sha256().to_string(),
            excerpt: content.into(),
        }],
    };
    let file = dir.join("packet.json");
    fs::write(&file, serde_json::to_vec(&packet).unwrap()).unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_maia-local-briefing-host"))
        .args([
            "--db",
            db.to_str().unwrap(),
            "--workspace",
            WS,
            "--packet",
            file.to_str().unwrap(),
            "--result-id",
            "m010-live-result",
            "--timestamp",
            NOW,
            "--audit-id",
            "01900000-0000-7000-8000-000000000013",
            "--endpoint",
            "127.0.0.1:11434",
            "--model",
            "qwen3:4b-instruct-2507-q4_K_M",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let reopened = SqliteStore::open(&db, id(WS), id(NOW)).unwrap();
    let record = reopened.get_briefing("m010-live-result").unwrap();
    assert_eq!(record.packet, packet);
    assert!(!record.result.execution_authority);
    assert_eq!(
        record.actual_model.as_deref(),
        Some("qwen3:4b-instruct-2507-q4_K_M")
    );
}
