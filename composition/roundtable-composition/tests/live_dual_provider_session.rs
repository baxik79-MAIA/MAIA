//! M0.14 LIVE SUCCESS / LIVE FAILURE: an end-to-end A3 Round Table session
//! composed from two real providers — the Claude Code subscription
//! participant and the local loopback Qwen participant — through the exact
//! same `maia_roundtable::Participant` port, wired only through composition
//! and registry-declared identity. Neither test branches on a provider name;
//! the only place either provider is named is this file's own resolver,
//! which plays the role a production registry-backed resolver would play.
//!
//! Both tests are gated two ways and skipped by default:
//!
//! 1. `#[ignore]`, with the concrete prerequisites in the reason string.
//! 2. A `MAIA_LIVE_CLAUDE_CODE=1` runtime check, mirroring
//!    `infra/claude-code/tests/live_smoke.rs`, so accidentally running
//!    `cargo test -- --ignored` without opting in still skips cleanly rather
//!    than spawning a real subscription-backed subprocess.
//!
//! This file also documents why no autonomous MAIA session should flip that
//! env var on itself: `infra/claude-code`'s `RecursionGuard` exists because
//! constructing `ClaudeCodeProvider` inside an already-running Claude Code
//! process tree is exactly the nested-invocation shape it refuses. These
//! tests are meant to be run by a human, or by a process that is verifiably
//! not itself a Claude Code session — never by this repository's own
//! autonomous continuation driver turns.

#![cfg(feature = "development-evolution")]

use maia_assurance_router::{
    AssuranceFailure, AssurancePlan, AvailabilityHealth, OrchestrationPath,
    ParticipantRegistration, ParticipantRegistry, ParticipantRole,
};
use maia_claude_code::{
    ClaudeCodeConfig, ClaudeCodeProvider, audit::FileAuditSink, runner::StdProcessRunner,
};
use maia_domain::ReasoningAssuranceLevel;
use maia_local_model::LoopbackLocalProvider;
use maia_local_model_roundtable::LocalModelParticipant;
use maia_roundtable::{
    DecisionRequest, EvidenceReference, OrchestrationFailure, Participant, ParticipantAdjudicator,
    ParticipantDescriptor, ParticipantResolver, QuorumReasonCode, ResolutionFailure, SessionRecord,
    SessionStore, SystemClock,
};
use maia_roundtable_composition::realize_plan;
use maia_roundtable_store::FileSessionStore;
use std::net::SocketAddr;
use std::path::PathBuf;

/// The exact model this project has live-qualified against (see
/// `infra/local-model/tests/live_smoke.rs` and the timing measurements in
/// `infra/local-model/src/lib.rs`). Not configurable here: a live test
/// asserting real behaviour should pin the real target, not a placeholder.
const LOCAL_MODEL: &str = "qwen3:4b-instruct-2507-q4_K_M";
const LOCAL_ENDPOINT: &str = "127.0.0.1:11434";
/// A small, fast Claude model, matching the one already live-qualified by
/// `infra/claude-code/tests/live_smoke.rs`.
const CLAUDE_MODEL: &str = "claude-haiku-4-5-20251001";

fn live_claude_code_enabled() -> bool {
    std::env::var("MAIA_LIVE_CLAUDE_CODE").as_deref() == Ok("1")
}

fn repo_root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn cli_version() -> String {
    let out = std::process::Command::new("claude")
        .arg("--version")
        .output()
        .expect("claude --version");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn claude_config(audit_directory: &std::path::Path) -> ClaudeCodeConfig {
    let mut config = ClaudeCodeConfig::new("claude", repo_root(), audit_directory);
    config.timeout = std::time::Duration::from_secs(180);
    config.model = Some(CLAUDE_MODEL.into());
    config
}

fn registration(
    id: &str,
    provider: &str,
    model_ref: &str,
    roles: Vec<ParticipantRole>,
) -> ParticipantRegistration {
    ParticipantRegistration {
        id: id.into(),
        provider: provider.into(),
        model_ref: model_ref.into(),
        role_capabilities: roles,
        enabled: true,
        assurance_levels: vec![ReasoningAssuranceLevel::A3],
        cost_metadata_capability: false,
        availability_health: AvailabilityHealth::Available,
    }
}

/// Claude Code is the sole `Adjudicator`-capable registration, so leader
/// selection is deterministic and does not depend on registry order.
fn dual_provider_registry() -> ParticipantRegistry {
    ParticipantRegistry {
        participants: vec![
            registration(
                "claude-code",
                "anthropic",
                CLAUDE_MODEL,
                vec![
                    ParticipantRole::RoundTableMember,
                    ParticipantRole::Adjudicator,
                ],
            ),
            registration(
                "local-qwen",
                "local-loopback",
                LOCAL_MODEL,
                vec![ParticipantRole::RoundTableMember],
            ),
        ],
    }
}

fn decision(id: &str) -> DecisionRequest {
    DecisionRequest {
        id: id.into(),
        subject: "M0.14 live composition smoke".into(),
        prompt: "In one sentence, state whether a reasoning-only Round Table \
                  participant should ever be granted unattended execution \
                  authority. This is a synthetic, non-sensitive question; do \
                  not read or modify any file."
            .into(),
        evidence: vec![EvidenceReference {
            id: "ev-invariant".into(),
            source_ref: "ADR-0046: Round Table grants no execution authority".into(),
        }],
    }
}

/// Resolves the two live identities this file exercises. This is the one
/// place a provider is named: exactly where provider-specific wiring
/// belongs, never in composition or orchestration.
struct LiveDualResolver {
    claude_descriptor: ParticipantDescriptor,
    local_descriptor: ParticipantDescriptor,
    audit_directory: PathBuf,
    cli_version: String,
    local_endpoint: SocketAddr,
    local_model: String,
}

impl ParticipantResolver for LiveDualResolver {
    fn resolve(
        &self,
        descriptor: &ParticipantDescriptor,
    ) -> Result<Box<dyn Participant>, ResolutionFailure> {
        if *descriptor == self.claude_descriptor {
            let provider = ClaudeCodeProvider::new(
                descriptor.id.as_str(),
                claude_config(&self.audit_directory),
                self.cli_version.clone(),
                StdProcessRunner,
                FileAuditSink::new(&self.audit_directory),
            )
            .map_err(|_| ResolutionFailure::ProviderUnavailable)?;
            return Ok(Box::new(provider));
        }
        if *descriptor == self.local_descriptor {
            let provider =
                LoopbackLocalProvider::new(self.local_endpoint, self.local_model.clone())
                    .map_err(|_| ResolutionFailure::ProviderUnavailable)?;
            return Ok(Box::new(LocalModelParticipant::new(
                descriptor.clone(),
                provider,
            )));
        }
        Err(ResolutionFailure::UnknownParticipant)
    }
}

fn descriptor_for(registry: &ParticipantRegistry, id: &str) -> ParticipantDescriptor {
    maia_roundtable_composition::round_table_descriptors(registry, ReasoningAssuranceLevel::A3)
        .unwrap()
        .into_iter()
        .find(|d| d.id.as_str() == id)
        .unwrap_or_else(|| panic!("registry has no Round Table member `{id}`"))
}

#[test]
#[ignore = "opt-in live M0.14 end-to-end test: requires MAIA_LIVE_CLAUDE_CODE=1, \
            an authenticated `claude` CLI subscription session, and the MAIA-owned \
            local Ollama runtime on 127.0.0.1:11434 serving qwen3:4b-instruct-2507-q4_K_M. \
            Run from a process that is not itself a nested Claude Code session — \
            infra/claude-code's RecursionGuard exists to refuse exactly that nesting, \
            and no autonomous driver turn in this repository should attempt this."]
fn live_success_a3_session_with_claude_code_and_local_qwen() {
    if !live_claude_code_enabled() {
        eprintln!("skipped: set MAIA_LIVE_CLAUDE_CODE=1 to run the live M0.14 dual-provider test");
        return;
    }

    let registry = dual_provider_registry();
    let claude_descriptor = descriptor_for(&registry, "claude-code");
    let local_descriptor = descriptor_for(&registry, "local-qwen");

    let audit_directory = repo_root().join("target/round-table-live-audit");
    let resolver = LiveDualResolver {
        claude_descriptor: claude_descriptor.clone(),
        local_descriptor,
        audit_directory: audit_directory.clone(),
        cli_version: cli_version(),
        local_endpoint: LOCAL_ENDPOINT.parse().unwrap(),
        local_model: LOCAL_MODEL.into(),
    };

    // The leader is realized through the same resolver, not constructed
    // ad hoc: its identity in the adjudication is the same descriptor the
    // first round would have addressed, had it been asked as a member.
    let leader = resolver
        .resolve(&claude_descriptor)
        .expect("leader resolves through the same port as every other participant");
    let adjudicator = ParticipantAdjudicator::new(leader, 512);

    let plan = AssurancePlan {
        required: ReasoningAssuranceLevel::A3,
        path: OrchestrationPath::A3RoundTable,
        reasons: vec![],
        execution_authority: false,
    };
    let session_id = "m0-14-live-success-1";
    let decision = decision(session_id);

    let realized = realize_plan(
        session_id,
        &plan,
        &decision,
        &registry,
        &resolver,
        &adjudicator,
        512,
        &SystemClock,
    )
    .expect("A3 plan realizes against the Round Table");

    let session = realized
        .outcome
        .as_ref()
        .unwrap_or_else(|failure| panic!("expected a decided session, got failure: {failure:?}"));

    // Both real providers resolved and both contributions were produced.
    assert_eq!(session.outcomes.len(), 2);
    assert!(
        session.outcomes.iter().all(|o| o.responded()),
        "both live participants must have produced a usable response: {:?}",
        session.outcomes
    );
    // First-round isolation: neither saw the other's answer.
    assert!(session.outcomes.iter().all(|o| o.first_round_isolated));
    // Contribution provenance is present for every attempt.
    for outcome in &session.outcomes {
        assert!(!outcome.participant.id.as_str().is_empty());
        assert!(outcome.started_at.0 <= outcome.finished_at.0);
    }
    // Deterministic leader: the sole Adjudicator-capable identity.
    assert_eq!(session.leader.leader.id.as_str(), "claude-code");
    assert_eq!(session.leader.eligible, 1);
    // Synthesis completed and never claims execution authority.
    assert!(!session.adjudication.conclusion.trim().is_empty());
    assert!(!session.adjudication.execution_authority);

    // The audit record links to the real session and never grants
    // execution authority either.
    let audit = &realized.audit;
    assert_eq!(audit.round_table_session_id.as_deref(), Some(session_id));
    assert_eq!(audit.selected_leader.as_deref(), Some("claude-code"));
    assert_eq!(audit.quorum_satisfied, Some(true));
    assert!(!audit.execution_authority);
    assert!(audit.failure_reason.is_none());

    // No provider-specific branch exists in composition: the same
    // `round_table_descriptors`/`realize_plan` call handled both real
    // providers without ever inspecting a provider string.

    // Persist and reload.
    let store_dir = repo_root().join("target/round-table-live-session-store");
    let store = FileSessionStore::new(&store_dir);
    let record = SessionRecord::from_session(session);
    store.save(&record).expect("session persists");
    let reloaded = store
        .load(session_id)
        .expect("store is reachable")
        .expect("the just-saved session reloads");
    assert_eq!(reloaded.id, session_id);
    assert!(reloaded.decided());
    assert_eq!(reloaded.outcomes.len(), 2);

    eprintln!(
        "live M0.14 success: leader={} responses={} audit_session={:?}",
        session.leader.leader.id.as_str(),
        session.outcomes.len(),
        audit.round_table_session_id
    );
}

#[test]
#[ignore = "opt-in live M0.14 failure-path test: requires MAIA_LIVE_CLAUDE_CODE=1 and \
            an authenticated `claude` CLI subscription session. The local participant is \
            deliberately misconfigured (an endpoint nothing listens on) rather than any \
            process being killed. Run from a process that is not itself a nested Claude \
            Code session, for the same RecursionGuard reason as the success test."]
fn live_failure_one_participant_unavailable_fails_closed() {
    if !live_claude_code_enabled() {
        eprintln!("skipped: set MAIA_LIVE_CLAUDE_CODE=1 to run the live M0.14 failure test");
        return;
    }

    let registry = dual_provider_registry();
    let claude_descriptor = descriptor_for(&registry, "claude-code");
    let local_descriptor = descriptor_for(&registry, "local-qwen");

    let audit_directory = repo_root().join("target/round-table-live-audit");
    let resolver = LiveDualResolver {
        claude_descriptor: claude_descriptor.clone(),
        local_descriptor,
        audit_directory: audit_directory.clone(),
        cli_version: cli_version(),
        // Deliberately unavailable: a loopback port nothing is bound to,
        // not a killed process and not a reachable-but-wrong host.
        local_endpoint: "127.0.0.1:1".parse().unwrap(),
        local_model: LOCAL_MODEL.into(),
    };

    let leader = resolver
        .resolve(&claude_descriptor)
        .expect("leader resolves even though the other participant will fail");
    let adjudicator = ParticipantAdjudicator::new(leader, 512);

    let plan = AssurancePlan {
        required: ReasoningAssuranceLevel::A3,
        path: OrchestrationPath::A3RoundTable,
        reasons: vec![],
        execution_authority: false,
    };
    let session_id = "m0-14-live-failure-1";
    let decision = decision(session_id);

    let realized = realize_plan(
        session_id,
        &plan,
        &decision,
        &registry,
        &resolver,
        &adjudicator,
        512,
        &SystemClock,
    )
    .expect("realize_plan still produces a record on a failed session");

    let failure = match &realized.outcome {
        Err(failure) => failure,
        Ok(session) => panic!(
            "expected quorum to fail with the local participant unavailable, got a \
             decided session: {session:?}"
        ),
    };

    let OrchestrationFailure::QuorumNotMet {
        insufficient,
        outcomes,
    } = failure
    else {
        panic!("expected QuorumNotMet, got {failure:?}");
    };
    assert_eq!(
        insufficient.reason_code,
        QuorumReasonCode::ParticipantUnavailable
    );
    assert_eq!(insufficient.achieved_responses, 1);
    // The surviving contribution (Claude Code) is retained in provenance,
    // not discarded just because the panel overall failed.
    assert!(outcomes.iter().any(|o| o.responded()));
    assert!(outcomes.iter().any(|o| !o.responded()));

    let audit = &realized.audit;
    assert_eq!(audit.round_table_session_id.as_deref(), Some(session_id));
    assert_eq!(audit.quorum_satisfied, Some(false));
    assert_eq!(
        audit.failure_reason,
        Some(AssuranceFailure::ParticipantUnavailable)
    );
    assert!(
        audit.adjudication.is_none(),
        "no synthesis on a failed panel"
    );
    assert!(!audit.execution_authority);

    // Persist the failed session too: a failed A3 realization is still an
    // auditable assurance event, not a discarded attempt.
    let store_dir = repo_root().join("target/round-table-live-session-store");
    let store = FileSessionStore::new(&store_dir);
    let record = SessionRecord::from_failure(session_id, plan.required, &decision, failure);
    store.save(&record).expect("failed session persists");
    let reloaded = store
        .load(session_id)
        .expect("store is reachable")
        .expect("the just-saved failed session reloads");
    assert!(!reloaded.decided());
    assert!(reloaded.failure.is_some());

    eprintln!(
        "live M0.14 failure: achieved={} required={} reason={:?}",
        insufficient.achieved_responses, insufficient.required_responses, insufficient.reason_code
    );
}
