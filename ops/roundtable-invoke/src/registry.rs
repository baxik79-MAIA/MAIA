//! Explicit local participant registry.
//!
//! The registry file names participants and nothing else. There is no default
//! registry and no environment lookup: a participant that is not named in the
//! file is simply unavailable, which is the router's "no hidden fallback"
//! principle applied to the file that feeds it.
//!
//! The file holds no secret. Credentials stay behind the existing boundaries
//! (the subscription CLI session, the loopback endpoint). The loader rejects a
//! file containing a credential-shaped key or value rather than trusting that
//! nobody will put one there.
//!
//! ```json
//! {"participants": [{
//!   "id": "claude-code-1", "adapter": "claude-code",
//!   "provider": "claude-code", "model_ref": "claude-opus",
//!   "roles": ["round_table_member", "adjudicator"],
//!   "assurance_levels": ["A3"], "enabled": true
//! }]}
//! ```

use maia_assurance_router::{
    AvailabilityHealth, ParticipantRegistration, ParticipantRegistry, ParticipantRole,
};
use maia_domain::ReasoningAssuranceLevel;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use std::fmt;

/// A registry file larger than this is refused unread: it names participants,
/// and a legitimate one is a few hundred bytes.
pub const MAX_REGISTRY_BYTES: usize = 64 * 1024;

/// Which adapter realizes a participant. The registry says this explicitly; the
/// invoker never infers an adapter from a provider or model name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adapter {
    ClaudeCode,
    LocalModel,
}

/// Where a participant's prompt goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locality {
    /// A loopback endpoint on this machine. No third party, no cost.
    Loopback,
    /// A third-party subscription-backed service. Cost is not known and is not zero.
    ThirdPartySubscription,
}

impl Adapter {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "claude-code" => Some(Self::ClaudeCode),
            "local-model" => Some(Self::LocalModel),
            _ => None,
        }
    }

    pub fn locality(self) -> Locality {
        match self {
            Self::ClaudeCode => Locality::ThirdPartySubscription,
            Self::LocalModel => Locality::Loopback,
        }
    }
}

/// A loaded registry: the router-facing registry plus, per registration, the
/// adapter that would realize it. Both are derived from the one file, in the
/// same order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedRegistry {
    pub registry: ParticipantRegistry,
    pub adapters: Vec<(String, Adapter)>,
}

impl LoadedRegistry {
    pub fn adapter_of(&self, id: &str) -> Option<Adapter> {
        self.adapters.iter().find(|(i, _)| i == id).map(|(_, a)| *a)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    TooLarge,
    NotJson(String),
    CredentialShaped(String),
    Shape(String),
    Empty,
    DuplicateId(String),
    BlankField { id: String, field: &'static str },
    UnknownAdapter { id: String, value: String },
    UnknownRole { id: String, value: String },
    UnknownLevel { id: String, value: String },
    NoRoles(String),
    NoLevels(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => write!(f, "registry file exceeds {MAX_REGISTRY_BYTES} bytes"),
            Self::NotJson(e) => write!(f, "registry file is not valid JSON: {e}"),
            Self::CredentialShaped(at) => write!(
                f,
                "registry file contains a credential-shaped entry at `{at}`; the registry \
                 must name participants only and hold no secret"
            ),
            Self::Shape(e) => write!(f, "registry file has the wrong shape: {e}"),
            Self::Empty => write!(f, "registry names no participants"),
            Self::DuplicateId(id) => write!(f, "participant id `{id}` appears more than once"),
            Self::BlankField { id, field } => {
                write!(f, "participant `{id}` has a blank `{field}`")
            }
            Self::UnknownAdapter { id, value } => write!(
                f,
                "participant `{id}` names unknown adapter `{value}` (known: claude-code, local-model)"
            ),
            Self::UnknownRole { id, value } => write!(
                f,
                "participant `{id}` names unknown role `{value}` (known: primary, verifier, \
                 round_table_member, adjudicator)"
            ),
            Self::UnknownLevel { id, value } => write!(
                f,
                "participant `{id}` names unknown assurance level `{value}` (known: A0-A4)"
            ),
            Self::NoRoles(id) => write!(f, "participant `{id}` declares no roles"),
            Self::NoLevels(id) => write!(f, "participant `{id}` declares no assurance levels"),
        }
    }
}

impl std::error::Error for RegistryError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRegistry {
    participants: Vec<FileParticipant>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileParticipant {
    id: String,
    adapter: String,
    provider: String,
    model_ref: String,
    roles: Vec<String>,
    assurance_levels: Vec<String>,
    enabled: bool,
}

/// Key fragments that mark a credential. Matched case-insensitively against
/// every object key at any depth.
const CREDENTIAL_KEY_FRAGMENTS: &[&str] = &[
    "secret",
    "token",
    "password",
    "passwd",
    "credential",
    "api_key",
    "apikey",
    "api-key",
    "authorization",
    "bearer",
    "private_key",
];

/// Value prefixes that mark a credential regardless of the key holding them.
const CREDENTIAL_VALUE_PREFIXES: &[&str] = &["sk-", "bearer ", "ghp_", "xoxb-"];

fn credential_shaped(value: &Value, path: &str) -> Option<String> {
    match value {
        Value::Object(map) => map.iter().find_map(|(k, v)| {
            let here = format!("{path}.{k}");
            let lower = k.to_ascii_lowercase();
            if CREDENTIAL_KEY_FRAGMENTS.iter().any(|f| lower.contains(f)) {
                Some(here)
            } else {
                credential_shaped(v, &here)
            }
        }),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .find_map(|(i, v)| credential_shaped(v, &format!("{path}[{i}]"))),
        Value::String(s) => {
            let lower = s.trim().to_ascii_lowercase();
            CREDENTIAL_VALUE_PREFIXES
                .iter()
                .any(|p| lower.starts_with(p))
                .then(|| path.to_owned())
        }
        _ => None,
    }
}

fn role(id: &str, s: &str) -> Result<ParticipantRole, RegistryError> {
    match s {
        "primary" => Ok(ParticipantRole::Primary),
        "verifier" => Ok(ParticipantRole::Verifier),
        "round_table_member" => Ok(ParticipantRole::RoundTableMember),
        "adjudicator" => Ok(ParticipantRole::Adjudicator),
        other => Err(RegistryError::UnknownRole {
            id: id.to_owned(),
            value: other.to_owned(),
        }),
    }
}

/// Parse an assurance level by its canonical wire name.
pub fn parse_level(s: &str) -> Option<ReasoningAssuranceLevel> {
    ReasoningAssuranceLevel::ALL
        .iter()
        .copied()
        .find(|l| l.as_str() == s)
}

fn level(id: &str, s: &str) -> Result<ReasoningAssuranceLevel, RegistryError> {
    parse_level(s).ok_or_else(|| RegistryError::UnknownLevel {
        id: id.to_owned(),
        value: s.to_owned(),
    })
}

fn nonblank(id: &str, field: &'static str, v: &str) -> Result<(), RegistryError> {
    if v.trim().is_empty() {
        Err(RegistryError::BlankField {
            id: id.to_owned(),
            field,
        })
    } else {
        Ok(())
    }
}

/// Load a registry from the text of a registry file.
///
/// Every field is required and unknown fields are refused, so a misspelled key
/// cannot silently fall back to a default. Health is not a file field: the
/// invoker performs no availability probe in v1, so every enabled entry is
/// presented to the router as `Available`, and an unreachable participant shows
/// up as a failed contribution in the saved session rather than being hidden.
/// `cost_metadata_capability` is always false: cost is never assumed known.
pub fn load_registry(text: &str) -> Result<LoadedRegistry, RegistryError> {
    if text.len() > MAX_REGISTRY_BYTES {
        return Err(RegistryError::TooLarge);
    }
    let value: Value =
        serde_json::from_str(text).map_err(|e| RegistryError::NotJson(e.to_string()))?;
    if let Some(at) = credential_shaped(&value, "$") {
        return Err(RegistryError::CredentialShaped(at));
    }
    let file: FileRegistry =
        serde_json::from_value(value).map_err(|e| RegistryError::Shape(e.to_string()))?;
    if file.participants.is_empty() {
        return Err(RegistryError::Empty);
    }

    let mut seen = HashSet::new();
    let mut participants = Vec::new();
    let mut adapters = Vec::new();
    for p in file.participants {
        nonblank(&p.id, "id", &p.id)?;
        if !seen.insert(p.id.clone()) {
            return Err(RegistryError::DuplicateId(p.id));
        }
        nonblank(&p.id, "provider", &p.provider)?;
        nonblank(&p.id, "model_ref", &p.model_ref)?;
        let adapter = Adapter::parse(&p.adapter).ok_or_else(|| RegistryError::UnknownAdapter {
            id: p.id.clone(),
            value: p.adapter.clone(),
        })?;
        if p.roles.is_empty() {
            return Err(RegistryError::NoRoles(p.id));
        }
        if p.assurance_levels.is_empty() {
            return Err(RegistryError::NoLevels(p.id));
        }
        let role_capabilities = p
            .roles
            .iter()
            .map(|r| role(&p.id, r))
            .collect::<Result<Vec<_>, _>>()?;
        let assurance_levels = p
            .assurance_levels
            .iter()
            .map(|l| level(&p.id, l))
            .collect::<Result<Vec<_>, _>>()?;
        adapters.push((p.id.clone(), adapter));
        participants.push(ParticipantRegistration {
            id: p.id,
            provider: p.provider,
            model_ref: p.model_ref,
            role_capabilities,
            enabled: p.enabled,
            assurance_levels,
            cost_metadata_capability: false,
            availability_health: AvailabilityHealth::Available,
        });
    }
    Ok(LoadedRegistry {
        registry: ParticipantRegistry { participants },
        adapters,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, adapter: &str) -> String {
        format!(
            r#"{{"id":"{id}","adapter":"{adapter}","provider":"p-{id}","model_ref":"m-{id}",
                "roles":["round_table_member","adjudicator"],"assurance_levels":["A3"],"enabled":true}}"#
        )
    }

    fn file(entries: &[String]) -> String {
        format!(r#"{{"participants":[{}]}}"#, entries.join(","))
    }

    #[test]
    fn loads_a_valid_registry_and_records_adapters() {
        let text = file(&[entry("a", "claude-code"), entry("b", "local-model")]);
        let loaded = load_registry(&text).unwrap();
        assert_eq!(loaded.registry.participants.len(), 2);
        assert_eq!(loaded.adapter_of("a"), Some(Adapter::ClaudeCode));
        assert_eq!(loaded.adapter_of("b"), Some(Adapter::LocalModel));
        assert_eq!(loaded.adapter_of("zzz"), None);
        let a = &loaded.registry.participants[0];
        assert!(a.enabled && !a.cost_metadata_capability);
        assert_eq!(a.availability_health, AvailabilityHealth::Available);
        assert_eq!(a.assurance_levels, vec![ReasoningAssuranceLevel::A3]);
    }

    #[test]
    fn locality_follows_the_adapter_not_a_name() {
        assert_eq!(
            Adapter::ClaudeCode.locality(),
            Locality::ThirdPartySubscription
        );
        assert_eq!(Adapter::LocalModel.locality(), Locality::Loopback);
    }

    #[test]
    fn rejects_credential_shaped_keys_at_any_depth() {
        for key in [
            "api_key",
            "Authorization",
            "session_token",
            "client_secret",
            "password",
        ] {
            let text = format!(
                r#"{{"participants":[{{"id":"a","adapter":"claude-code","provider":"p","model_ref":"m",
                    "roles":["adjudicator"],"assurance_levels":["A3"],"enabled":true,"{key}":"x"}}]}}"#
            );
            assert!(
                matches!(
                    load_registry(&text),
                    Err(RegistryError::CredentialShaped(_))
                ),
                "{key} must be rejected as credential-shaped"
            );
        }
        let nested = r#"{"participants":[],"meta":{"deep":{"apiKey":"x"}}}"#;
        assert!(matches!(
            load_registry(nested),
            Err(RegistryError::CredentialShaped(_))
        ));
    }

    #[test]
    fn rejects_credential_shaped_values_whatever_the_key() {
        let text = r#"{"participants":[{"id":"a","adapter":"claude-code","provider":"p",
            "model_ref":"sk-ant-abc123","roles":["adjudicator"],"assurance_levels":["A3"],"enabled":true}]}"#;
        assert!(matches!(
            load_registry(text),
            Err(RegistryError::CredentialShaped(_))
        ));
    }

    #[test]
    fn unknown_fields_are_refused_not_ignored() {
        let text = r#"{"participants":[{"id":"a","adapter":"claude-code","provider":"p","model_ref":"m",
            "roles":["adjudicator"],"assurance_levels":["A3"],"enabled":true,"fallback":"b"}]}"#;
        assert!(matches!(load_registry(text), Err(RegistryError::Shape(_))));
    }

    #[test]
    fn every_field_is_required_including_enabled() {
        let text = r#"{"participants":[{"id":"a","adapter":"claude-code","provider":"p","model_ref":"m",
            "roles":["adjudicator"],"assurance_levels":["A3"]}]}"#;
        assert!(matches!(load_registry(text), Err(RegistryError::Shape(_))));
    }

    #[test]
    fn rejects_bad_content() {
        assert_eq!(
            load_registry(r#"{"participants":[]}"#),
            Err(RegistryError::Empty)
        );
        assert!(matches!(
            load_registry("not json"),
            Err(RegistryError::NotJson(_))
        ));
        assert_eq!(
            load_registry(&file(&[
                entry("a", "claude-code"),
                entry("a", "local-model")
            ])),
            Err(RegistryError::DuplicateId("a".into()))
        );
        assert!(matches!(
            load_registry(&file(&[entry("a", "gpt")])),
            Err(RegistryError::UnknownAdapter { .. })
        ));
        assert!(matches!(
            load_registry(&file(&[entry("  ", "claude-code")])),
            Err(RegistryError::BlankField { field: "id", .. })
        ));
        let bad_role = entry("a", "claude-code").replace("adjudicator", "boss");
        assert!(matches!(
            load_registry(&file(&[bad_role])),
            Err(RegistryError::UnknownRole { .. })
        ));
        let bad_level = entry("a", "claude-code").replace("A3", "A9");
        assert!(matches!(
            load_registry(&file(&[bad_level])),
            Err(RegistryError::UnknownLevel { .. })
        ));
        let no_roles =
            entry("a", "claude-code").replace(r#""round_table_member","adjudicator""#, "");
        assert_eq!(
            load_registry(&file(&[no_roles])),
            Err(RegistryError::NoRoles("a".into()))
        );
    }

    #[test]
    fn oversized_input_is_refused_unparsed() {
        let big = " ".repeat(MAX_REGISTRY_BYTES + 1);
        assert_eq!(load_registry(&big), Err(RegistryError::TooLarge));
    }

    #[test]
    fn level_names_are_canonical_only() {
        assert_eq!(parse_level("A3"), Some(ReasoningAssuranceLevel::A3));
        assert_eq!(parse_level("a3"), None);
        assert_eq!(parse_level("A5"), None);
    }
}
