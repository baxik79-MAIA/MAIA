use std::collections::BTreeMap;

use maia_domain::{
    AuditEventType, AuditId, AuditRecord, AuditSubjectType, OpaqueRef, Sha256Hex, Timestamp,
    WorkspaceId,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::{StoreError, StoreResult};

/// Event facts supplied by a mutation transaction.  Sequence, previous hash
/// and record hash are allocated by the authoritative adapter in that same
/// transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEventDraft {
    pub audit_id: AuditId,
    pub workspace_id: WorkspaceId,
    pub recorded_at: Timestamp,
    pub actor_ref: Option<maia_domain::ActorRef>,
    pub event_type: AuditEventType,
    pub subject_type: AuditSubjectType,
    pub subject_ref: Option<OpaqueRef>,
    pub operation_ref: Option<OpaqueRef>,
    pub decision_ref: Option<OpaqueRef>,
    pub metadata_hash: Option<Sha256Hex>,
}

/// RFC8785-compatible projection for the current AuditRecordV1 field set.
/// Keys are ASCII and values are strings, integers or null, so a sorted JSON
/// object is deterministic and has no floating-point or exponent ambiguity.
pub fn audit_record_projection(record: &AuditRecord) -> String {
    let mut values = BTreeMap::<String, Value>::new();
    values.insert("actor_ref".into(), optional_string(record.actor_ref()));
    values.insert(
        "audit_id".into(),
        Value::String(record.audit_id().to_string()),
    );
    values.insert(
        "decision_ref".into(),
        optional_string(record.decision_ref()),
    );
    values.insert(
        "event_type".into(),
        Value::String(record.event_type().to_string()),
    );
    values.insert(
        "metadata_hash".into(),
        optional_string(record.metadata_hash()),
    );
    values.insert(
        "operation_ref".into(),
        optional_string(record.operation_ref()),
    );
    values.insert(
        "prev_hash".into(),
        Value::String(record.prev_hash().to_string()),
    );
    values.insert(
        "recorded_at".into(),
        Value::String(record.recorded_at().to_string()),
    );
    values.insert(
        "sequence".into(),
        Value::Number((*record.sequence()).get().into()),
    );
    values.insert("subject_ref".into(), optional_string(record.subject_ref()));
    values.insert(
        "subject_type".into(),
        Value::String(record.subject_type().to_string()),
    );
    values.insert(
        "workspace_id".into(),
        Value::String(record.workspace_id().to_string()),
    );
    let object: Map<String, Value> = values.into_iter().collect();
    serde_json::to_string(&object).expect("AuditRecordV1 projection is JSON-serializable")
}

fn optional_string<T: ToString>(value: &Option<T>) -> Value {
    value
        .as_ref()
        .map(ToString::to_string)
        .map(Value::String)
        .unwrap_or(Value::Null)
}

pub fn compute_audit_hash(record: &AuditRecord) -> StoreResult<Sha256Hex> {
    let digest = Sha256::digest(audit_record_projection(record).as_bytes());
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    Sha256Hex::new(encoded).map_err(StoreError::InvalidInput)
}

pub fn make_audit_record(
    draft: &AuditEventDraft,
    audit_id: AuditId,
    sequence: maia_domain::AuditSequence,
    prev_hash: Sha256Hex,
) -> StoreResult<AuditRecord> {
    // record_hash is excluded from the projection, therefore a placeholder
    // with the correct shape is safe while computing the canonical hash.
    let placeholder = Sha256Hex::new("0".repeat(64)).map_err(StoreError::InvalidInput)?;
    let candidate = AuditRecord::new(
        audit_id,
        draft.workspace_id.clone(),
        sequence,
        draft.recorded_at.clone(),
        draft.actor_ref.clone(),
        draft.event_type.clone(),
        draft.subject_type.clone(),
        draft.subject_ref.clone(),
        draft.operation_ref.clone(),
        draft.decision_ref.clone(),
        draft.metadata_hash.clone(),
        prev_hash,
        placeholder,
    )
    .map_err(StoreError::InvalidInput)?;
    let hash = compute_audit_hash(&candidate)?;
    AuditRecord::new(
        candidate.audit_id().clone(),
        candidate.workspace_id().clone(),
        *candidate.sequence(),
        candidate.recorded_at().clone(),
        candidate.actor_ref().clone(),
        candidate.event_type().clone(),
        candidate.subject_type().clone(),
        candidate.subject_ref().clone(),
        candidate.operation_ref().clone(),
        candidate.decision_ref().clone(),
        candidate.metadata_hash().clone(),
        candidate.prev_hash().clone(),
        hash,
    )
    .map_err(StoreError::InvalidInput)
}
