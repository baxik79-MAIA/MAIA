//! Read-only-hosted, loopback-only local evidence briefing command.
#![forbid(unsafe_code)]

use maia_briefing::{
    BriefingPacket, LocalModelProvider, MAXIMUM_OUTPUT_TOKENS, consult_and_validate, packet_hash,
    result_hash,
};
use maia_domain::*;
use maia_local_model::LoopbackLocalProvider;
use maia_sqlite::SqliteStore;
use maia_store::{ArtifactRepository, BriefingPersistence, BriefingRepository, audit_event};
use std::{env, fs, net::SocketAddr, process::ExitCode};

fn required(arguments: &[String], name: &str) -> Result<String, String> {
    arguments
        .windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
        .ok_or_else(|| format!("missing {name}"))
}
fn parse<T: std::str::FromStr>(value: String, name: &str) -> Result<T, String> {
    value.parse().map_err(|_| format!("invalid {name}"))
}
fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let db = required(&args, "--db")?;
    let workspace: WorkspaceId = parse(required(&args, "--workspace")?, "--workspace")?;
    let packet_path = required(&args, "--packet")?;
    let result_id = required(&args, "--result-id")?;
    let timestamp: Timestamp = parse(required(&args, "--timestamp")?, "--timestamp")?;
    let audit_id: AuditId = parse(required(&args, "--audit-id")?, "--audit-id")?;
    let endpoint: SocketAddr = parse(required(&args, "--endpoint")?, "--endpoint")?;
    let model = required(&args, "--model")?;
    let packet: BriefingPacket =
        serde_json::from_slice(&fs::read(packet_path).map_err(|_| "cannot read --packet")?)
            .map_err(|_| "invalid packet JSON")?;
    if packet.workspace_id != workspace.as_str() {
        return Err("packet workspace differs from --workspace".into());
    }
    let store = SqliteStore::open(&db, workspace.clone(), timestamp.clone())
        .map_err(|_| "cannot open authoritative store")?;
    for evidence in &packet.evidence {
        let artifact_id: ArtifactId = parse(evidence.artifact_id.clone(), "packet artifact_id")?;
        let artifact = store
            .get_artifact(&artifact_id)
            .map_err(|_| "packet references absent MAIA artifact")?;
        if artifact.workspace_id() != &workspace
            || artifact.content_sha256().as_str() != evidence.content_hash
            || !artifact.content().contains(&evidence.excerpt)
        {
            return Err("packet evidence does not bind to its MAIA artifact".into());
        }
    }
    let provider =
        LoopbackLocalProvider::new(endpoint, model).map_err(|_| "endpoint is not loopback")?;
    let accepted = consult_and_validate(&provider, &packet, MAXIMUM_OUTPUT_TOKENS)
        .map_err(|_| "provider response was not accepted")?;
    let packet_hash = packet_hash(&packet).map_err(|_| "invalid packet")?;
    let event = audit_event(
        audit_id,
        workspace.clone(),
        timestamp.clone(),
        None,
        AuditEventType::new("briefing.accepted").map_err(|_| "invalid audit event")?,
        AuditSubjectType::new("briefing_result").map_err(|_| "invalid audit subject")?,
        Some(OpaqueRef::new(result_id.clone()).map_err(|_| "invalid result id")?),
        None,
        None,
        Some(Sha256Hex::new(packet_hash.clone()).map_err(|_| "invalid packet hash")?),
    );
    let assurance = packet.assurance.clone();
    let record = BriefingPersistence {
        result_id: result_id.clone(),
        workspace_id: workspace.to_string(),
        packet,
        packet_hash,
        requested_provider: provider.requested_provider().into(),
        requested_model: provider.requested_model().into(),
        actual_provider: Some(provider.requested_provider().into()),
        actual_model: Some(provider.requested_model().into()),
        assurance,
        result_hash: result_hash(&accepted),
        result: accepted,
        created_at: timestamp,
    };
    store
        .persist_briefing(&record, &event)
        .map_err(|_| "cannot atomically persist accepted briefing")?;
    println!(
        "{{\"result_id\":\"{}\",\"packet_hash\":\"{}\",\"result_hash\":\"{}\",\"execution_authority\":false}}",
        record.result_id, record.packet_hash, record.result_hash
    );
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("maia-local-briefing-host: {error}");
            ExitCode::FAILURE
        }
    }
}
