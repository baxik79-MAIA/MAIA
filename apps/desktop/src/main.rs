//! M0.11 native MAIA desktop surface over the existing local Rust ports.
#![forbid(unsafe_code)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{self, Color32, RichText, TextEdit};
use maia_briefing::{
    BriefingError, BriefingPacket, LocalModelProvider, LocalProviderFailure, MAXIMUM_OUTPUT_TOKENS,
    PacketEvidence, consult_and_validate, packet_hash, result_hash,
};
use maia_domain::*;
use maia_local_intelligence_health::{HealthStore, OperationClass, RetentionPolicy};
use maia_local_model::LoopbackLocalProvider;
use maia_sqlite::SqliteStore;
use maia_store::{ArtifactRepository, BriefingPersistence, BriefingRepository, audit_event};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    fs,
    net::SocketAddr,
    path::Path,
    time::{Duration, Instant},
};
use time::OffsetDateTime;
use uuid::Uuid;
mod capture;
mod design;

const LOCAL_ENDPOINT: &str = "127.0.0.1:11434";
const LOCAL_MODEL: &str = "qwen3:4b-instruct-2507-q4_K_M";
const DEFAULT_WORKSPACE: &str = "01900000-0000-7000-8000-000000000000";
const RECENT_REQUESTS_CAP: usize = 20;
/// How often `local_available` is re-probed after startup (Architecture
/// Desk - "GO - PROCEED AUTONOMOUSLY" Step 4). Bounded, event-driven-ish via
/// `request_repaint_after` rather than continuous polling: the probe itself
/// is a single 250ms-bounded TCP connect (see `local_status()`), so a 5s
/// cadence stays well clear of "aggressive polling" while letting the UI
/// recover Offline -> Online without a rebuild or manual relaunch.
const LOCAL_PROBE_INTERVAL: Duration = Duration::from_secs(5);
// A reduced CPU-first output ceiling was tried here (384) and REVERTED on
// measured evidence. Paired probes over identical demanding evidence, on the
// CPU-only host:
//
//   num_predict  384 -> done_reason "length", eval_count exactly 384, and the
//                       structured result came back as INVALID JSON, cut mid
//                       string inside a claim. In MAIA that is not a shorter
//                       briefing, it is MalformedResponse: 269s of CPU spent
//                       and then a failed briefing.
//   num_predict 1024 -> did not finish an exhaustive answer within a 300s
//                       probe limit at all.
//
// So no evidence supports a ceiling below the contract bound, and one measured
// case refutes 384 outright. The desktop therefore asks for
// MAXIMUM_OUTPUT_TOKENS, restoring the prior behaviour. Note that the same run
// also measured generation at 1.9 tok/s (against 4.2 tok/s in the earlier
// single-claim run), so output rate varies by more than 2x with the shape of
// the answer — any future budget must be derived from the slow end, not the
// fast one.

/// How a failed briefing attempt should be presented / whether a retry is
/// reasonable. Derived from `BriefingFailureKind` (never decided
/// independently of it) — see `disposition_for_kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureDisposition {
    /// A slow/loaded local model. An explicit user retry is reasonable.
    Retryable,
    /// Runtime/config/validation failure. Retrying immediately will not help.
    Terminal,
    /// Protocol/schema mismatch or a truncated response; not assumed
    /// recoverable (Architecture Desk amendment #8/Batch 1 review #2).
    Unknown,
}

/// Typed, non-localized failure category. Product logic (single-flight,
/// reconciliation, disposition) branches on this — never on a raw string or
/// on `format!("{error:?}")` (Architecture Desk Batch 1 review #2/#6).
/// Granularity follows the existing `BriefingError`/`LocalProviderFailure`
/// variants plus the desktop layer's own precondition checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BriefingFailureKind {
    Timeout,
    MalformedResponse,
    Unavailable,
    ModelUnavailable,
    ProviderError,
    EmptyPrompt,
    NoSources,
    InvalidWorkspace,
    InvalidRuntimeConfig,
    ExecutionAuthorityRejected,
    ClockError,
    PersistenceError,
    Other,
}

/// Per-attempt outcome tracked in the UI-local ledger. Deliberately omits
/// `Queued` (indistinguishable from `Processing` here) and `Cancelled`
/// (nothing here can actually cancel the worker thread) — truthful state
/// over a future-proofed enum, per Architecture Desk amendment #7.
#[derive(Debug, Clone, PartialEq, Eq)]
enum BriefingRequestState {
    Processing,
    Completed {
        result_id: String,
    },
    Failed {
        kind: BriefingFailureKind,
        disposition: FailureDisposition,
        /// Raw technical detail (e.g. `format!("{error:?}")`) for the
        /// Details/Diagnostics surface only — never used as, or in place of,
        /// localized user-facing copy (Architecture Desk Batch 1 review #6).
        technical_reason: Option<String>,
    },
}

/// Message sent back from the worker thread. Carries only semantic outcome
/// data — no presentation copy. The UI thread renders current-language text
/// from `kind` at render time via `tr()`, so a language switch while a
/// briefing is in flight can never leave a stale-language string on screen
/// (Architecture Desk Batch 1 review #1).
#[derive(Debug, Clone)]
enum BriefingOutcome {
    Succeeded {
        attempt_id: String,
        result_id: String,
    },
    Failed {
        attempt_id: String,
        kind: BriefingFailureKind,
        disposition: FailureDisposition,
        technical_reason: Option<String>,
    },
}

/// One entry per submission. `attempt_id` (= `packet.id`) is execution
/// identity; `packet_hash` is semantic identity — the two are never
/// conflated. Invariant (Architecture Desk amendments #1-#5, Batch 1 review
/// #3-#5): an exact `attempt_id` match against persisted history is the only
/// thing that may transition `state` (Processing or Failed) to `Completed`;
/// a shared `packet_hash` with a *different* `attempt_id` only ever sets
/// `related_result_id` (informational association) and never rewrites
/// `state`.
#[derive(Debug, Clone)]
struct RecentBriefingRequest {
    attempt_id: String,
    packet_hash: String,
    state: BriefingRequestState,
    related_result_id: Option<String>,
}

struct MaiaDesktop {
    workspace: String,
    database_path: String,
    evidence_path: String,
    prompt: String,
    status: String,
    artifacts: Vec<Artifact>,
    briefings: Vec<BriefingPersistence>,
    selected_briefing: Option<String>,
    language: String,
    dark_theme: bool,
    page: usize,
    context_tab: usize,
    selected_citation: Option<String>,
    show_import: bool,
    initialized: bool,
    capture_frame: usize,
    briefing_job: Option<std::sync::mpsc::Receiver<BriefingOutcome>>,
    recent_requests: VecDeque<RecentBriefingRequest>,
    /// The failure kind behind the current `feedback == Some("briefing")`
    /// banner, if any. Looked up fresh every frame via `failure_copy()` so
    /// the displayed text always matches the current `self.language`.
    current_failure_kind: Option<BriefingFailureKind>,
    feedback: Option<&'static str>,
    local_available: bool,
    /// Last time a re-probe of `local_available` was *started* (the probe
    /// itself completes asynchronously — see `local_probe_job`). Drives the
    /// bounded periodic re-probe cadence in `update()` — see
    /// `LOCAL_PROBE_INTERVAL`.
    last_local_probe: Instant,
    /// Set while a background local-model probe thread is in flight. The
    /// probe (`local_status()`) does a TCP connect with a 250ms timeout —
    /// bounded, but still blocking I/O, so it must never run inline on the
    /// egui UI thread. Polled non-blockingly via `try_recv()` in `update()`,
    /// same channel/thread shape as `briefing_job`.
    local_probe_job: Option<std::sync::mpsc::Receiver<bool>>,
    /// Whether the model has already been asked to load this session, so
    /// focusing the question field repeatedly costs nothing. See
    /// `warm_local_model()`.
    warmed_local_model: bool,
    /// When the in-flight briefing was submitted, so the wait can show real
    /// elapsed time instead of an opaque spinner.
    briefing_started: Option<Instant>,
}

impl Default for MaiaDesktop {
    fn default() -> Self {
        let preferences = fs::read_to_string(r"C:\MAIA\data\maia-alpha1-ui.json")
            .ok()
            .and_then(|v| serde_json::from_str::<serde_json::Value>(&v).ok());
        Self {
            page: 0,
            context_tab: 0,
            selected_citation: None,
            show_import: false,
            initialized: false,
            capture_frame: 0,
            briefing_job: None,
            recent_requests: VecDeque::new(),
            current_failure_kind: None,
            feedback: None,
            local_available: local_status().contains("Ready"),
            last_local_probe: Instant::now(),
            local_probe_job: None,
            warmed_local_model: false,
            briefing_started: None,
            workspace: DEFAULT_WORKSPACE.into(),
            database_path: std::env::var("MAIA_ALPHA1_DATABASE")
                .unwrap_or_else(|_| r"C:\MAIA\data\maia-alpha1.db".into()),
            evidence_path: String::new(),
            prompt: String::new(),
            status: local_status(),
            artifacts: Vec::new(),
            briefings: Vec::new(),
            selected_briefing: None,
            language: preferences
                .as_ref()
                .and_then(|v| v.get("language"))
                .and_then(|v| v.as_str())
                .unwrap_or(
                    if std::env::var("LANG").unwrap_or_default().starts_with("pl") {
                        "pl"
                    } else {
                        "en"
                    },
                )
                .into(),
            dark_theme: preferences
                .as_ref()
                .and_then(|v| v.get("dark"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }
    }
}

/// Builds the outgoing briefing packet. Shared by the pre-spawn ledger entry
/// (which only needs `packet_hash`) and the worker's actual provider call,
/// so both always compute identical semantic content for the same inputs.
/// `attempt_id` becomes `packet.id` directly — no separate SubmissionId type.
fn build_packet(
    workspace: &WorkspaceId,
    attempt_id: &str,
    objective: &str,
    artifacts: &[Artifact],
) -> BriefingPacket {
    let evidence = artifacts
        .iter()
        .take(8)
        .enumerate()
        .map(|(index, artifact)| PacketEvidence {
            citation_id: format!("e{}", index + 1),
            artifact_id: artifact.id().as_str().into(),
            content_hash: artifact.content_sha256().as_str().into(),
            excerpt: truncate(artifact.content().as_str(), 4_000),
        })
        .collect();
    BriefingPacket {
        id: attempt_id.into(),
        workspace_id: workspace.as_str().into(),
        objective: objective.into(),
        instructions: "Use only supplied evidence. Separate evidence-supported claims, inferences and unknowns. Cite every supported claim. Do not propose or execute actions.".into(),
        template_version: "m0.10-local-briefing-v1".into(),
        response_contract: "validated_cited_claims".into(),
        privacy_classification: "workspace-private".into(),
        assurance: "A1".into(),
        routing_constraints: "loopback-only; no-cloud-fallback; no-actions".into(),
        evidence,
    }
}

/// Maps a provider failure to its typed kind. Product logic branches on this
/// enum, never on `format!("{error:?}")`.
fn classify_provider_failure(error: &BriefingError) -> BriefingFailureKind {
    match error {
        BriefingError::Provider(LocalProviderFailure::Timeout) => BriefingFailureKind::Timeout,
        BriefingError::Provider(LocalProviderFailure::MalformedResponse) => {
            BriefingFailureKind::MalformedResponse
        }
        BriefingError::Provider(LocalProviderFailure::Unavailable) => {
            BriefingFailureKind::Unavailable
        }
        BriefingError::Provider(LocalProviderFailure::ModelUnavailable) => {
            BriefingFailureKind::ModelUnavailable
        }
        BriefingError::Provider(LocalProviderFailure::ProviderError) => {
            BriefingFailureKind::ProviderError
        }
        _ => BriefingFailureKind::Other,
    }
}

/// The single source of truth for kind -> disposition. Only a genuine
/// provider Timeout is `Retryable`; `MalformedResponse` is `Unknown` (never
/// assumed background-recoverable — Architecture Desk amendment #8).
fn disposition_for_kind(kind: BriefingFailureKind) -> FailureDisposition {
    match kind {
        BriefingFailureKind::Timeout => FailureDisposition::Retryable,
        BriefingFailureKind::MalformedResponse => FailureDisposition::Unknown,
        _ => FailureDisposition::Terminal,
    }
}

/// DEVELOPMENT_EVOLUTION diagnostic instrumentation only (Architecture Desk,
/// M0.11 Run 1 failure diagnosis, 2026-09-16). Appends one line per failed
/// briefing attempt to a local, developer-only log, because today nothing
/// else preserves a failure's classified kind or technical detail once the
/// UI banner is dismissed: the Diagnostics/Szczegóły panel does not surface
/// `technical_reason` (separate, later scope), and the exe is launched with
/// no attached console and no panic hook, so nothing is otherwise ever
/// captured. Carries no evidence content, no provider raw-response text, and
/// no secrets/credentials — only the already-typed `kind`/`disposition` and
/// the existing `technical_reason`, which was already reduced to
/// `format!("{error:?}")` of a typed error before reaching here, never a raw
/// response body (Architecture Desk Batch 1 review #6 — unchanged by this
/// function). Best-effort only: every I/O result is discarded via `let _ =`,
/// so a logging failure can never affect briefing behavior, persistence
/// semantics, timeout policy, or retry behavior, and this function never
/// itself retries or falls back.
///
/// Capability Mesh note: this observability surface conceptually belongs to
/// the Local Intelligence Runtime / persistence capability, not ad-hoc
/// Desktop code. Placed here only for minimal, local M0.11 diagnosis;
/// recorded as migration debt in `LOCAL_INTELLIGENCE_CAPABILITY_RECORD.md`.
fn log_failure_diagnostics(
    attempt_id: &str,
    kind: BriefingFailureKind,
    disposition: FailureDisposition,
    technical_reason: Option<&str>,
) {
    let format = time::format_description::parse(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z",
    )
    .ok();
    let ts = format
        .and_then(|f| OffsetDateTime::now_utc().format(&f).ok())
        .unwrap_or_else(|| "unknown-time".to_owned());
    let line = format!(
        "{ts}\tattempt_id={attempt_id}\tkind={kind:?}\tdisposition={disposition:?}\ttechnical_reason={}\n",
        technical_reason.unwrap_or("<none>")
    );
    let _ = fs::create_dir_all(r"C:\MAIA\reports");
    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(r"C:\MAIA\reports\maia_failure_diagnostics.log")
    {
        use std::io::Write as _;
        let _ = file.write_all(line.as_bytes());
    }
}

impl MaiaDesktop {
    fn text(&self, key: &str) -> &'static str {
        match (self.language.as_str(), key) {
            ("pl", "today") => "Dzisiaj",
            ("pl", "workspaces") => "Obszary robocze",
            ("pl", "history") => "Briefingi i historia",
            ("pl", "sources") => "Źródła",
            ("pl", "ask") => "O czym chcesz porozmawiać?",
            ("pl", "add") => "Dodaj źródła",
            ("pl", "local") => "MAIA lokalna",
            ("pl", "ready") => "Gotowa",
            ("pl", "import") => "Importuj źródło",
            ("pl", "open") => "Otwórz obszar roboczy",
            ("pl", "citations") => "Źródła i cytowania",
            ("pl", "empty") => "Zacznij od dodania źródeł do obszaru roboczego.",
            ("pl", "theme") => "Motyw",
            ("pl", "language") => "Język",
            (_, "today") => "Today",
            (_, "workspaces") => "Workspaces",
            (_, "history") => "Briefings & history",
            (_, "sources") => "Sources",
            (_, "ask") => "What would you like to work on?",
            (_, "add") => "Add sources",
            (_, "local") => "MAIA Local",
            (_, "ready") => "Ready",
            (_, "import") => "Import source",
            (_, "open") => "Open workspace",
            (_, "citations") => "Sources & citations",
            (_, "empty") => "Start by adding sources to your workspace.",
            (_, "theme") => "Theme",
            (_, "language") => "Language",
            _ => "",
        }
    }
    fn save_preferences(&self) {
        let _ = fs::create_dir_all(r"C:\MAIA\data");
        let _ = fs::write(
            r"C:\MAIA\data\maia-alpha1-ui.json",
            format!(
                r#"{{"language":"{}","dark":{}}}"#,
                self.language, self.dark_theme
            ),
        );
    }
    fn store(&self) -> Result<SqliteStore, String> {
        let workspace = WorkspaceId::new(self.workspace.trim()).map_err(|_| {
            self.tr(
                "Identyfikator obszaru roboczego musi być UUIDv7.",
                "Workspace ID must be UUIDv7.",
            )
            .to_owned()
        })?;
        // Reaches the UI only via self.status, which is now shown solely in
        // the Diagnostics panel (Details/Szczegóły tab) — so the raw
        // {error:?} technical detail is preserved here deliberately, not
        // leaked into normal UI (Architecture Desk Batch 1 review #6).
        SqliteStore::open(&self.database_path, workspace, timestamp()?).map_err(|error| {
            format!(
                "{} {error:?}",
                self.tr("Obszar roboczy niedostępny:", "Workspace unavailable:")
            )
        })
    }

    /// Navigate to `page`, reloading persisted state from the store whenever
    /// the destination is Today (0) or Briefings & History (2) and the page
    /// actually changed. Never reloads on every frame — only on an actual
    /// page transition into one of those two screens.
    fn navigate_to(&mut self, page: usize) {
        let changed = self.page != page;
        self.page = page;
        if changed && (page == 0 || page == 2) {
            self.reload();
        }
    }

    fn push_recent_request(&mut self, entry: RecentBriefingRequest) {
        self.recent_requests.push_back(entry);
        while self.recent_requests.len() > RECENT_REQUESTS_CAP {
            self.recent_requests.pop_front();
        }
    }

    /// Two-tier reconciliation, run after every `reload()`.
    ///
    /// INVARIANT: exact execution identity (`attempt_id == packet.id` of a
    /// durably persisted result) is the ONLY thing that may transition a
    /// ledger entry's `state` — from `Processing` OR `Failed` — to
    /// `Completed`. This also covers the case where the outcome channel
    /// message was lost/unreceived but the worker still persisted
    /// successfully (the UI would otherwise think it's still `Processing`
    /// forever).
    ///
    /// A shared `packet_hash` with a *different* `attempt_id` is semantic
    /// association only: it may set `related_result_id` on a `Failed` entry,
    /// and NEVER changes `state`. Attempt A remains Attempt A; Attempt B
    /// remains Attempt B.
    fn reconcile_recent_requests(&mut self) {
        let briefings = &self.briefings;
        for entry in self.recent_requests.iter_mut() {
            if matches!(entry.state, BriefingRequestState::Completed { .. }) {
                continue;
            }
            if let Some(exact) = briefings.iter().find(|b| b.packet.id == entry.attempt_id) {
                entry.state = BriefingRequestState::Completed {
                    result_id: exact.result_id.clone(),
                };
                entry.related_result_id = None;
                continue;
            }
            if let BriefingRequestState::Failed { .. } = &entry.state {
                if entry.related_result_id.is_none() {
                    if let Some(semantic) = briefings
                        .iter()
                        .find(|b| b.packet_hash == entry.packet_hash)
                    {
                        entry.related_result_id = Some(semantic.result_id.clone());
                    }
                }
            }
        }
    }

    fn reload(&mut self) {
        match self.store() {
            Ok(store) => match (store.list_artifacts(), store.list_briefings()) {
                (Ok(artifacts), Ok(briefings)) => {
                    self.artifacts = artifacts;
                    self.briefings = briefings;
                    self.status = format!(
                        "{} · {} {} · {} {}",
                        self.tr("Obszar roboczy gotowy", "Workspace ready"),
                        self.artifacts.len(),
                        self.tr("kopii dowodów", "evidence snapshots"),
                        self.briefings.len(),
                        self.tr("briefingów", "briefings"),
                    );
                    self.reconcile_recent_requests();
                }
                _ => {
                    self.status = self
                        .tr(
                            "Nie można wczytać niezmiennej historii obszaru roboczego.",
                            "Workspace could not load its immutable history.",
                        )
                        .into();
                    self.feedback = Some("store");
                }
            },
            Err(error) => {
                self.status = error;
                self.feedback = Some("store");
            }
        }
    }

    fn import_evidence(&mut self) {
        let path = Path::new(self.evidence_path.trim());
        let metadata = match fs::metadata(path) {
            Ok(value) if value.is_file() => value,
            _ => {
                self.status = self
                    .tr(
                        "Wybierz istniejący lokalny plik .txt lub .md.",
                        "Choose one existing local .txt or .md file.",
                    )
                    .into();
                return;
            }
        };
        if metadata.len() > 262_144 {
            self.status = self
                .tr(
                    "W wersji Alpha 1 dowody są ograniczone do 256 KiB.",
                    "Evidence is limited to 256 KiB for Alpha 1.",
                )
                .into();
            return;
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let media_type = match extension.as_str() {
            "txt" => EvidenceMediaType::Utf8PlainText,
            "md" => EvidenceMediaType::Markdown,
            _ => {
                self.status = self
                    .tr(
                        "Wersja Alpha 1 akceptuje tylko dowody UTF-8 w formacie .txt lub .md.",
                        "Alpha 1 accepts explicit UTF-8 .txt or .md evidence only.",
                    )
                    .into();
                return;
            }
        };
        let content = match fs::read_to_string(path) {
            Ok(value) if !value.trim().is_empty() => value,
            Ok(_) => {
                self.status = self
                    .tr("Dowód nie może być pusty.", "Evidence cannot be empty.")
                    .into();
                return;
            }
            Err(_) => {
                self.status = self
                    .tr(
                        "Dowód musi być czytelnym tekstem UTF-8.",
                        "Evidence must be readable UTF-8 text.",
                    )
                    .into();
                return;
            }
        };
        let name = match path
            .file_name()
            .and_then(|value| value.to_str())
            .and_then(|value| NonEmptyString::new(value).ok())
        {
            Some(value) => value,
            None => {
                self.status = self
                    .tr(
                        "Nazwa pliku dowodowego jest nieprawidłowa.",
                        "Evidence file name is invalid.",
                    )
                    .into();
                return;
            }
        };
        let source = self.evidence_path.trim();
        let workspace = match WorkspaceId::new(self.workspace.trim()) {
            Ok(value) => value,
            Err(_) => {
                self.status = self
                    .tr(
                        "Identyfikator obszaru roboczego musi być UUIDv7.",
                        "Workspace ID must be UUIDv7.",
                    )
                    .into();
                return;
            }
        };
        let now = match timestamp() {
            Ok(value) => value,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        let artifact = match Artifact::new(
            typed_id(),
            workspace.clone(),
            ArtifactKind::ImportedTextEvidence,
            name,
            media_type,
            content.clone(),
            sha256(&content),
            ByteCount::new(content.len() as u64).expect("non-negative byte length"),
            OpaqueRef::new(source).expect("non-empty local source"),
            sha256(source),
            typed_id(),
            OpaqueRef::new("local-user-selected").expect("constant"),
            OpaqueRef::new("workspace-private").expect("constant"),
            now.clone(),
        ) {
            Ok(value) => value,
            Err(_) => {
                self.status = self
                    .tr(
                        "MAIA odrzuciła metadane dowodu.",
                        "MAIA rejected the evidence metadata.",
                    )
                    .into();
                return;
            }
        };
        let event = audit_event(
            typed_id(),
            workspace,
            now,
            Some(ActorRef::new("desktop-user").expect("constant")),
            AuditEventType::new("workspace_evidence_imported").expect("constant"),
            AuditSubjectType::new("artifact").expect("constant"),
            Some(OpaqueRef::new(artifact.id().as_str()).expect("UUIDv7")),
            None,
            None,
            None,
        );
        match self.store().and_then(|store| {
            store.persist_artifact(&artifact, &event).map_err(|error| {
                format!(
                    "{} {error:?}",
                    self.tr("Dowód nie został zapisany:", "Evidence was not persisted:")
                )
            })
        }) {
            Ok(_) => {
                let confirmation = format!(
                    "{} {}… · {}",
                    self.tr("Zaimportowano kopię · hash", "Snapshot imported · hash"),
                    &artifact.content_sha256().as_str()[..8],
                    self.tr("niezmienna", "immutable"),
                );
                self.evidence_path.clear();
                self.reload();
                self.status = confirmation;
            }
            Err(error) => self.status = error,
        }
    }

    fn ask_maia(&mut self) {
        if self.briefing_job.is_some() {
            return;
        }
        if self.prompt.trim().is_empty() {
            self.feedback = Some("question");
            return;
        }
        if self.artifacts.is_empty() {
            self.feedback = Some("sources");
            return;
        }
        let attempt_id = Uuid::now_v7().to_string();
        let packet_hash_value = WorkspaceId::new(self.workspace.trim())
            .ok()
            .map(|workspace| {
                build_packet(&workspace, &attempt_id, self.prompt.trim(), &self.artifacts)
            })
            .and_then(|packet| packet_hash(&packet).ok())
            .unwrap_or_default();
        self.push_recent_request(RecentBriefingRequest {
            attempt_id: attempt_id.clone(),
            packet_hash: packet_hash_value,
            state: BriefingRequestState::Processing,
            related_result_id: None,
        });
        // Only cheap clones happen on the UI thread here. The throwaway
        // worker is built INSIDE the spawned thread because `Self::default()`
        // calls `local_status()` twice (two TCP connects, 250ms timeout each)
        // and reads the preferences file — up to ~500ms of blocking I/O that
        // previously ran on the egui thread on every single send, stalling
        // the frame at the exact moment the user interacts (Architecture Desk
        // UI-thread-blocking review before Alpha 1 sign-off).
        let workspace = self.workspace.clone();
        let database_path = self.database_path.clone();
        let prompt = self.prompt.clone();
        let artifacts = self.artifacts.clone();
        let (send, receive) = std::sync::mpsc::channel();
        self.briefing_job = Some(receive);
        self.briefing_started = Some(Instant::now());
        self.feedback = None;
        self.current_failure_kind = None;
        std::thread::spawn(move || {
            let mut worker = Self {
                workspace,
                database_path,
                prompt,
                artifacts,
                ..Self::default()
            };
            let outcome = worker.perform_briefing(&attempt_id);
            let _ = send.send(outcome);
        });
    }

    /// Prepare -> execute -> validate -> persist -> return a typed outcome.
    /// Deliberately does NOT set `self.status`, `self.page`,
    /// `self.selected_briefing`, or call `self.reload()` — this runs on a
    /// throwaway worker clone (see `ask_maia`), and none of those
    /// presentation-state transitions are required for persistence or
    /// result construction. The live UI thread performs them, once, on
    /// receiving the returned `BriefingOutcome` (Architecture Desk Batch 1
    /// review #10).
    fn perform_briefing(&mut self, attempt_id: &str) -> BriefingOutcome {
        let fail = |kind: BriefingFailureKind, technical_reason: Option<String>| {
            let disposition = disposition_for_kind(kind);
            log_failure_diagnostics(attempt_id, kind, disposition, technical_reason.as_deref());
            BriefingOutcome::Failed {
                attempt_id: attempt_id.to_string(),
                disposition,
                kind,
                technical_reason,
            }
        };
        if self.prompt.trim().is_empty() {
            return fail(BriefingFailureKind::EmptyPrompt, None);
        }
        if self.artifacts.is_empty() {
            return fail(BriefingFailureKind::NoSources, None);
        }
        let workspace = match WorkspaceId::new(self.workspace.trim()) {
            Ok(value) => value,
            Err(_) => return fail(BriefingFailureKind::InvalidWorkspace, None),
        };
        let packet = build_packet(&workspace, attempt_id, self.prompt.trim(), &self.artifacts);
        let provider = match local_provider() {
            Some(value) => value,
            None => return fail(BriefingFailureKind::InvalidRuntimeConfig, None),
        };
        // M0.15.10: best-effort health recording around the one call that
        // actually reaches the local runtime. Measured here, not inside the
        // capability, because `consult_and_validate`/`consult()` return
        // Briefing's own `RawProviderResponse`/`LocalProviderFailure`
        // shapes, neither of which carries timing the way
        // `complete_detailed()`'s `CompletionOutcome` does. Recording never
        // changes which branch below is taken or what `fail`/`value`
        // becomes -- every `record_*` call's own `Result` is discarded.
        let consult_started = Instant::now();
        let result = match consult_and_validate(&provider, &packet, MAXIMUM_OUTPUT_TOKENS) {
            Ok(value) if !value.execution_authority => {
                if let Some(store) = health_store() {
                    let _ = store.record_inference_success(
                        provider.requested_model(),
                        OperationClass::Consult,
                        consult_started.elapsed(),
                        std::time::SystemTime::now(),
                    );
                }
                value
            }
            Ok(_) => {
                // The provider itself succeeded (a valid response came
                // back); recorded as a Local Intelligence Runtime success,
                // distinct from the separate Core policy rejection below.
                if let Some(store) = health_store() {
                    let _ = store.record_inference_success(
                        provider.requested_model(),
                        OperationClass::Consult,
                        consult_started.elapsed(),
                        std::time::SystemTime::now(),
                    );
                }
                return fail(BriefingFailureKind::ExecutionAuthorityRejected, None);
            }
            Err(error) => {
                let kind = classify_provider_failure(&error);
                // Only `BriefingError::Provider(inner)` is an actual Local
                // Intelligence Runtime failure. The other `BriefingError`
                // variants (`DuplicateCitation`, `UnsupportedClaim`,
                // `ExecutionAuthority`) mean the provider answered and Core's
                // own content validation rejected it -- recorded as a
                // runtime success, the same reasoning as the `Ok(_)` branch
                // above, not a `LocalProviderFailure` that never happened.
                if let Some(store) = health_store() {
                    match &error {
                        BriefingError::Provider(inner) => {
                            let _ = store.record_inference_failure_from_briefing(
                                provider.requested_model(),
                                OperationClass::Consult,
                                inner.clone(),
                                Some(consult_started.elapsed()),
                                std::time::SystemTime::now(),
                            );
                        }
                        _ => {
                            let _ = store.record_inference_success(
                                provider.requested_model(),
                                OperationClass::Consult,
                                consult_started.elapsed(),
                                std::time::SystemTime::now(),
                            );
                        }
                    }
                }
                return fail(kind, Some(format!("{error:?}")));
            }
        };
        let now = match timestamp() {
            Ok(value) => value,
            Err(error) => return fail(BriefingFailureKind::ClockError, Some(error)),
        };
        let record = BriefingPersistence {
            result_id: Uuid::now_v7().to_string(),
            workspace_id: workspace.as_str().into(),
            packet: packet.clone(),
            packet_hash: packet_hash(&packet).expect("validated packet"),
            requested_provider: provider.requested_provider().into(),
            requested_model: provider.requested_model().into(),
            actual_provider: Some(provider.requested_provider().into()),
            actual_model: Some(provider.requested_model().into()),
            assurance: "A1".into(),
            result_hash: result_hash(&result),
            result,
            created_at: now.clone(),
        };
        let event = audit_event(
            typed_id(),
            workspace,
            now,
            Some(ActorRef::new("desktop-user").expect("constant")),
            AuditEventType::new("briefing.accepted").expect("constant"),
            AuditSubjectType::new("briefing_result").expect("constant"),
            Some(OpaqueRef::new(record.result_id.as_str()).expect("UUIDv7")),
            None,
            None,
            None,
        );
        match self.store().and_then(|store| {
            store
                .persist_briefing(&record, &event)
                .map_err(|error| format!("{error:?}"))
        }) {
            Ok(()) => BriefingOutcome::Succeeded {
                attempt_id: attempt_id.to_string(),
                result_id: record.result_id,
            },
            Err(error) => fail(BriefingFailureKind::PersistenceError, Some(error)),
        }
    }
}

impl eframe::App for MaiaDesktop {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        // Bounded periodic re-probe of local-model availability (Architecture
        // Desk - "GO - PROCEED AUTONOMOUSLY" Step 4; hardened per the
        // follow-up UI-thread-blocking review before Alpha 1 sign-off).
        // `local_available` was previously set only once, in
        // `Default::default()`, so an already running Ollama runtime that
        // started (or became reachable) after MAIA launched was never
        // reflected without a full relaunch. This does not touch
        // `self.status`, which reload()/import_evidence() own exclusively
        // (see the Diagnostics comment in design.rs).
        //
        // The probe (`local_status()`) does a TCP connect with a 250ms
        // timeout — bounded, but still synchronous I/O, so it never runs
        // inline on this (egui UI) thread. It always runs on its own
        // short-lived background thread and reports back through
        // `local_probe_job`, polled here with a non-blocking `try_recv()` —
        // the same channel/thread shape `briefing_job` already uses for the
        // (much longer) provider call.
        if let Some(job) = &self.local_probe_job {
            if let Ok(available) = job.try_recv() {
                self.local_available = available;
                self.local_probe_job = None;
            }
        }
        if self.local_probe_job.is_none() && self.last_local_probe.elapsed() >= LOCAL_PROBE_INTERVAL
        {
            self.last_local_probe = Instant::now();
            let (send, receive) = std::sync::mpsc::channel();
            self.local_probe_job = Some(receive);
            std::thread::spawn(move || {
                let probe_started = Instant::now();
                let status = local_provider().map(|provider| provider.status());
                let probe_duration = probe_started.elapsed();
                let available = status.as_ref().map(|s| s.available).unwrap_or(false);
                // Best-effort recording (M0.15.10): never affects `available`,
                // which the UI receives regardless of whether this succeeds.
                if let (Some(status), Some(store)) = (&status, health_store()) {
                    let _ = store.record_status_sample(
                        status,
                        Some(probe_duration),
                        std::time::SystemTime::now(),
                    );
                }
                let _ = send.send(available);
            });
        }
        // While a briefing is in flight the wait shows elapsed seconds, so the
        // UI needs to tick once a second; otherwise the 5s probe cadence is
        // enough to keep status live without busy-repainting an idle window.
        ctx.request_repaint_after(if self.briefing_job.is_some() {
            Duration::from_secs(1)
        } else {
            LOCAL_PROBE_INTERVAL
        });
        self.render(ctx);
        self.capture_review(ctx);
    }
}

fn typed_id<T: std::str::FromStr<Err = DomainError>>() -> T {
    Uuid::now_v7().to_string().parse().expect("UUIDv7")
}
fn sha256(text: &str) -> Sha256Hex {
    Sha256Hex::new(format!("{:x}", Sha256::digest(text.as_bytes()))).expect("SHA-256")
}
fn timestamp() -> Result<Timestamp, String> {
    let format = time::format_description::parse(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z",
    )
    .map_err(|_| "Clock formatting failed.".to_owned())?;
    Timestamp::new(
        OffsetDateTime::now_utc()
            .format(&format)
            .map_err(|_| "Clock formatting failed.".to_owned())?,
    )
    .map_err(|_| "Clock value is invalid.".into())
}
fn truncate(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}
/// Asks the local runtime to load the model into memory, without requesting any
/// generation, and discards the reply.
///
/// Measured on the CPU-only host: a cold load costs 21.02s of the 73.78s a real
/// briefing took — 28.5% of the wait, spent before any work on the user's
/// question. Ollama evicts an idle model, so the briefing most likely to pay it
/// is the first one after launch. Warming on intent (the user focusing the
/// question field) moves that cost into the seconds the user spends typing, and
/// brings the measured packet from 73.78s to 52.76s — under even the original
/// 60s budget — with no change to the model, the provider, or the evidence.
///
/// Fire-and-forget: it runs on its own thread, and any failure is ignored,
/// because this is purely an optimization. If it does not complete in time the
/// briefing simply pays the load itself, exactly as it did before.
/// Constructs the Local Intelligence Runtime capability instance for the
/// MAIA-owned endpoint/model this desktop is configured against (M0.15.9:
/// desktop owns WHICH endpoint/model to use — consumer-specific
/// configuration — the capability owns everything about talking to it).
/// Returns `None` only when the constant configuration itself is invalid
/// (unparsable endpoint or non-loopback address), never as a
/// runtime-unavailable signal — that comes from `LocalIntelligenceStatus`.
fn local_provider() -> Option<LoopbackLocalProvider> {
    LOCAL_ENDPOINT
        .parse::<SocketAddr>()
        .ok()
        .and_then(|endpoint| LoopbackLocalProvider::new(endpoint, LOCAL_MODEL).ok())
}
/// Fire-and-forget model warm-up, delegated to the capability
/// (`LoopbackLocalProvider::warm`, itself moved here from this exact function
/// in M0.15.9). Any failure — including an invalid constant configuration —
/// is silently ignored, because this is purely an optimization: a caller
/// that never warmed, or whose warm attempt failed, still gets a correct (if
/// slower) answer from `perform_briefing`, which pays the load cost itself.
fn warm_local_model() {
    if let Some(provider) = local_provider() {
        provider.warm();
    }
}
fn local_status() -> String {
    match local_provider().map(|provider| provider.status()) {
        Some(status) if status.available => {
            "MAIA Local: Ready · Qwen configured · offline-only".into()
        }
        _ => "Local intelligence unavailable · start the MAIA-owned runtime".into(),
    }
}

const DEFAULT_HEALTH_DB: &str = r"C:\MAIA\data\maia-local-intelligence-health.sqlite3";

/// The Local Intelligence health-history store (M0.15.10), opened once for
/// the process and shared from then on. Opening a fresh `HealthStore` per
/// call would reintroduce the exact "fresh store per call races another
/// thread for the same lock" anti-pattern already flagged for `SqliteStore`
/// in `docs/development/LOCAL_INTELLIGENCE_CAPABILITY_RECORD.md`'s Run 1
/// postmortem -- see `maia_local_intelligence_health`'s own crate docs.
///
/// Returns `None` if the store could not be opened at all (bad path,
/// unwritable directory, disk issue). Every call site treats `None` and a
/// per-call recording failure identically: silently skip recording, never
/// affect the inference result being described. This is the concrete
/// implementation of the migration directive's "failure of observability
/// must not break inference" requirement -- recording always happens AFTER
/// the real result is already computed, and its own `Result` is always
/// discarded (`let _ = ...`), never observed by the caller's return path.
/// A one-time `eprintln!` gives a developer truthful visibility into an
/// open failure without building a UI surface for it (out of proportion for
/// this milestone).
fn health_store() -> Option<&'static HealthStore> {
    static STORE: std::sync::OnceLock<Option<HealthStore>> = std::sync::OnceLock::new();
    STORE
        .get_or_init(|| {
            let path = std::env::var("MAIA_LOCAL_INTELLIGENCE_HEALTH_DB")
                .unwrap_or_else(|_| DEFAULT_HEALTH_DB.into());
            match HealthStore::open(Path::new(&path), RetentionPolicy::default()) {
                Ok(store) => Some(store),
                Err(error) => {
                    eprintln!(
                        "MAIA: Local Intelligence health store unavailable ({path}): {error:?}; \
                         continuing without recorded health history"
                    );
                    None
                }
            }
        })
        .as_ref()
}
fn main() -> eframe::Result<()> {
    eframe::run_native(
        "MAIA",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_maximized(true)
                .with_min_inner_size([1100.0, 720.0]),
            ..Default::default()
        },
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();
            if let Ok(data) = fs::read(r"C:\Windows\Fonts\segoeui.ttf") {
                fonts
                    .font_data
                    .insert("maia-ui".into(), egui::FontData::from_owned(data).into());
                fonts
                    .families
                    .entry(egui::FontFamily::Proportional)
                    .or_default()
                    .insert(0, "maia-ui".into());
            }
            let mut semibold = fonts.families[&egui::FontFamily::Proportional].clone();
            if let Ok(data) = fs::read(r"C:\Windows\Fonts\seguisb.ttf") {
                fonts.font_data.insert(
                    "maia-semibold".into(),
                    egui::FontData::from_owned(data).into(),
                );
                semibold.insert(0, "maia-semibold".into());
            }
            fonts
                .families
                .insert(egui::FontFamily::Name("maia-semibold".into()), semibold);
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::<MaiaDesktop>::default())
        }),
    )
}

#[cfg(test)]
mod desktop_tests {
    use super::*;

    #[test]
    fn desktop_clock_matches_canonical_millisecond_timestamp() {
        let value = timestamp().expect("desktop clock must be accepted by domain");
        assert_eq!(value.as_str().len(), 24);
        assert_eq!(&value.as_str()[19..20], ".");
        assert!(value.as_str().ends_with('Z'));
    }
    #[test]
    fn import_rejects_unsupported_file_extension() {
        let root = std::env::temp_dir().join(format!("maia-desktop-test-{}", Uuid::now_v7()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source.pdf");
        fs::write(&source, "not really a pdf, just wrong extension").unwrap();
        let mut app = MaiaDesktop {
            database_path: root.join("workspace.db").to_string_lossy().into(),
            evidence_path: source.to_string_lossy().into(),
            ..Default::default()
        };
        app.import_evidence();
        assert!(
            app.artifacts.is_empty(),
            "Alpha 1 accepts only .txt/.md evidence"
        );
        drop(app);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn import_rejects_empty_file() {
        let root = std::env::temp_dir().join(format!("maia-desktop-test-{}", Uuid::now_v7()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source.txt");
        fs::write(&source, "").unwrap();
        let mut app = MaiaDesktop {
            database_path: root.join("workspace.db").to_string_lossy().into(),
            evidence_path: source.to_string_lossy().into(),
            ..Default::default()
        };
        app.import_evidence();
        assert!(app.artifacts.is_empty());
        drop(app);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn import_rejects_file_over_256_kib() {
        let root = std::env::temp_dir().join(format!("maia-desktop-test-{}", Uuid::now_v7()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source.txt");
        fs::write(&source, "x".repeat(262_145)).unwrap();
        let mut app = MaiaDesktop {
            database_path: root.join("workspace.db").to_string_lossy().into(),
            evidence_path: source.to_string_lossy().into(),
            ..Default::default()
        };
        app.import_evidence();
        assert!(app.artifacts.is_empty());
        drop(app);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn import_rejects_missing_file() {
        let root = std::env::temp_dir().join(format!("maia-desktop-test-{}", Uuid::now_v7()));
        fs::create_dir(&root).unwrap();
        let mut app = MaiaDesktop {
            database_path: root.join("workspace.db").to_string_lossy().into(),
            evidence_path: root.join("does-not-exist.txt").to_string_lossy().into(),
            ..Default::default()
        };
        app.import_evidence();
        assert!(app.artifacts.is_empty());
        drop(app);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn desktop_import_reopens_immutable_source_after_original_changes() {
        let root = std::env::temp_dir().join(format!("maia-desktop-test-{}", Uuid::now_v7()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source.txt");
        fs::write(&source, "Recorded evidence\nSecond line").unwrap();
        let mut app = MaiaDesktop {
            database_path: root.join("workspace.db").to_string_lossy().into(),
            evidence_path: source.to_string_lossy().into(),
            ..Default::default()
        };
        app.import_evidence();
        assert_eq!(app.artifacts.len(), 1, "{}", app.status);
        let original_hash = app.artifacts[0].content_sha256().clone();
        fs::write(&source, "Changed original").unwrap();
        app.reload();
        assert_eq!(
            app.artifacts[0].content().as_str(),
            "Recorded evidence\nSecond line"
        );
        assert_eq!(app.artifacts[0].content_sha256(), &original_hash);
        assert!(app.briefings.is_empty());
        drop(app);
        // This unique directory was created by this test under the system temp directory.
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn empty_question_and_missing_sources_do_not_start_provider_work() {
        let mut app = MaiaDesktop::default();
        app.ask_maia();
        assert!(app.briefing_job.is_none());
        assert_eq!(app.feedback, Some("question"));
        app.prompt = "Question".into();
        app.ask_maia();
        assert!(app.briefing_job.is_none());
        assert_eq!(app.feedback, Some("sources"));
    }

    fn workspace() -> WorkspaceId {
        WorkspaceId::new(DEFAULT_WORKSPACE).expect("valid UUIDv7 workspace id")
    }

    fn persisted(attempt_id: &str, result_id: &str, packet_hash: &str) -> BriefingPersistence {
        BriefingPersistence {
            result_id: result_id.into(),
            workspace_id: DEFAULT_WORKSPACE.into(),
            packet: BriefingPacket {
                id: attempt_id.into(),
                workspace_id: DEFAULT_WORKSPACE.into(),
                objective: "Question".into(),
                instructions: "Use only supplied evidence.".into(),
                template_version: "m0.10-local-briefing-v1".into(),
                response_contract: "validated_cited_claims".into(),
                privacy_classification: "workspace-private".into(),
                assurance: "A1".into(),
                routing_constraints: "loopback-only; no-cloud-fallback; no-actions".into(),
                evidence: vec![],
            },
            packet_hash: packet_hash.into(),
            requested_provider: "local-loopback".into(),
            requested_model: "test-model".into(),
            actual_provider: Some("local-loopback".into()),
            actual_model: Some("test-model".into()),
            assurance: "A1".into(),
            result_hash: "hash".into(),
            result: maia_briefing::ValidatedBriefingResult {
                packet_id: attempt_id.into(),
                packet_hash: packet_hash.into(),
                claims: vec![],
                execution_authority: false,
            },
            created_at: timestamp().expect("valid timestamp"),
        }
    }

    // ---- Batch 1 tests (kept, updated for the new BriefingFailureKind shape) ----

    #[test]
    fn same_packet_hash_different_attempt_ids_are_distinct_requests() {
        let evidence = vec![PacketEvidence {
            citation_id: "e1".into(),
            artifact_id: "a1".into(),
            content_hash: "h1".into(),
            excerpt: "fact".into(),
        }];
        let mut a = build_packet(&workspace(), "attempt-a", "Same question", &[]);
        a.evidence = evidence.clone();
        let mut b = build_packet(&workspace(), "attempt-b", "Same question", &[]);
        b.evidence = evidence;
        assert_ne!(a.id, b.id);
        assert_eq!(
            packet_hash(&a).expect("packet_hash should succeed with non-empty evidence"),
            packet_hash(&b).expect("packet_hash should succeed with non-empty evidence"),
            "identical semantic content with different attempt ids must hash identically \
             (packet_hash excludes packet.id by design)"
        );
    }

    #[test]
    fn recent_requests_ledger_is_bounded() {
        let mut app = MaiaDesktop::default();
        for i in 0..(RECENT_REQUESTS_CAP + 5) {
            app.push_recent_request(RecentBriefingRequest {
                attempt_id: format!("attempt-{i}"),
                packet_hash: "hash".into(),
                state: BriefingRequestState::Processing,
                related_result_id: None,
            });
        }
        assert_eq!(app.recent_requests.len(), RECENT_REQUESTS_CAP);
        assert_eq!(app.recent_requests.front().unwrap().attempt_id, "attempt-5");
    }

    // ---- Batch 1 review corrections: typed kind/disposition ----

    #[test]
    fn classify_provider_failure_and_disposition_map_correctly() {
        assert_eq!(
            classify_provider_failure(&BriefingError::Provider(LocalProviderFailure::Timeout)),
            BriefingFailureKind::Timeout
        );
        assert_eq!(
            disposition_for_kind(BriefingFailureKind::Timeout),
            FailureDisposition::Retryable
        );
        assert_eq!(
            classify_provider_failure(&BriefingError::Provider(
                LocalProviderFailure::MalformedResponse
            )),
            BriefingFailureKind::MalformedResponse
        );
        assert_eq!(
            disposition_for_kind(BriefingFailureKind::MalformedResponse),
            FailureDisposition::Unknown
        );
        assert_eq!(
            classify_provider_failure(&BriefingError::Provider(LocalProviderFailure::Unavailable)),
            BriefingFailureKind::Unavailable
        );
        assert_eq!(
            disposition_for_kind(BriefingFailureKind::Unavailable),
            FailureDisposition::Terminal
        );
        assert_eq!(
            classify_provider_failure(&BriefingError::EmptyPacket),
            BriefingFailureKind::Other
        );
    }

    // ---- Batch 1 review #3/#4/#5: reconciliation invariants ----

    #[test]
    fn exact_attempt_processing_to_completed_reconciliation() {
        let mut app = MaiaDesktop::default();
        app.push_recent_request(RecentBriefingRequest {
            attempt_id: "attempt-a".into(),
            packet_hash: "hash-a".into(),
            state: BriefingRequestState::Processing,
            related_result_id: None,
        });
        app.briefings = vec![persisted("attempt-a", "result-a", "hash-a")];
        app.reconcile_recent_requests();
        assert_eq!(
            app.recent_requests[0].state,
            BriefingRequestState::Completed {
                result_id: "result-a".into()
            },
            "a durably persisted result for the same attempt_id must promote \
             Processing -> Completed even if the outcome channel message was \
             lost — this covers the 'result persisted, UI never notified' gap"
        );
    }

    #[test]
    fn exact_attempt_failed_to_completed_reconciliation() {
        let mut app = MaiaDesktop::default();
        app.push_recent_request(RecentBriefingRequest {
            attempt_id: "attempt-a".into(),
            packet_hash: "hash-a".into(),
            state: BriefingRequestState::Failed {
                kind: BriefingFailureKind::Timeout,
                disposition: FailureDisposition::Retryable,
                technical_reason: Some("Provider(Timeout)".into()),
            },
            related_result_id: None,
        });
        app.briefings = vec![persisted("attempt-a", "result-a", "hash-a")];
        app.reconcile_recent_requests();
        assert_eq!(
            app.recent_requests[0].state,
            BriefingRequestState::Completed {
                result_id: "result-a".into()
            }
        );
    }

    #[test]
    fn same_packet_hash_different_attempt_id_does_not_rewrite_failed_attempt() {
        let mut app = MaiaDesktop::default();
        app.push_recent_request(RecentBriefingRequest {
            attempt_id: "attempt-a".into(),
            packet_hash: "shared-hash".into(),
            state: BriefingRequestState::Failed {
                kind: BriefingFailureKind::Timeout,
                disposition: FailureDisposition::Retryable,
                technical_reason: Some("Provider(Timeout)".into()),
            },
            related_result_id: None,
        });
        // A different attempt (B) with the SAME packet_hash succeeded.
        app.briefings = vec![persisted("attempt-b", "result-b", "shared-hash")];
        app.reconcile_recent_requests();
        assert_eq!(
            app.recent_requests[0].state,
            BriefingRequestState::Failed {
                kind: BriefingFailureKind::Timeout,
                disposition: FailureDisposition::Retryable,
                technical_reason: Some("Provider(Timeout)".into()),
            },
            "a Failed entry must never be rewritten to Completed by a different \
             attempt_id, even with an identical packet_hash"
        );
        assert_eq!(
            app.recent_requests[0].related_result_id.as_deref(),
            Some("result-b"),
            "the shared packet_hash must still be recorded as an informational \
             association, pointing at the other attempt's real result"
        );
    }

    #[test]
    fn failed_attempt_then_successful_retry_preserves_both_execution_outcomes() {
        let mut app = MaiaDesktop::default();
        app.push_recent_request(RecentBriefingRequest {
            attempt_id: "attempt-a".into(),
            packet_hash: "shared-hash".into(),
            state: BriefingRequestState::Failed {
                kind: BriefingFailureKind::Timeout,
                disposition: FailureDisposition::Retryable,
                technical_reason: None,
            },
            related_result_id: None,
        });
        app.push_recent_request(RecentBriefingRequest {
            attempt_id: "attempt-b".into(),
            packet_hash: "shared-hash".into(),
            state: BriefingRequestState::Processing,
            related_result_id: None,
        });
        app.briefings = vec![persisted("attempt-b", "result-b", "shared-hash")];
        app.reconcile_recent_requests();
        assert!(matches!(
            app.recent_requests[0].state,
            BriefingRequestState::Failed { .. }
        ));
        assert_eq!(
            app.recent_requests[1].state,
            BriefingRequestState::Completed {
                result_id: "result-b".into()
            }
        );
    }

    // ---- Batch 1 review #7: single-flight fails closed at the state layer ----

    fn single_source_app() -> (MaiaDesktop, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!("maia-desktop-test-{}", Uuid::now_v7()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source.txt");
        fs::write(&source, "Evidence").unwrap();
        let mut app = MaiaDesktop {
            database_path: root.join("workspace.db").to_string_lossy().into(),
            evidence_path: source.to_string_lossy().into(),
            ..Default::default()
        };
        app.import_evidence();
        app.prompt = "Question".into();
        (app, root)
    }

    #[test]
    fn mouse_submit_creates_exactly_one_attempt() {
        // The send-arrow click handler calls ask_maia() once per click; this
        // exercises that single call directly.
        let (mut app, root) = single_source_app();
        app.ask_maia();
        assert_eq!(app.recent_requests.len(), 1);
        drop(app);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn enter_submit_creates_exactly_one_attempt() {
        // The Enter-key handler (edit.lost_focus() && Key::Enter pressed)
        // calls the identical ask_maia() as the mouse path; there is no
        // separate code path to diverge from the mouse-submit test above.
        let (mut app, root) = single_source_app();
        app.ask_maia();
        assert_eq!(app.recent_requests.len(), 1);
        drop(app);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mouse_and_enter_in_the_same_interaction_create_exactly_one_attempt() {
        // Both the send-button click handler and the Enter-key handler
        // ultimately call ask_maia() on the same frame's self; this
        // simulates that race directly against the guard ask_maia() itself
        // enforces (in addition to the UI-level add_enabled_ui in design.rs,
        // which cannot be exercised without a live egui frame).
        let root = std::env::temp_dir().join(format!("maia-desktop-test-{}", Uuid::now_v7()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source.txt");
        fs::write(&source, "Evidence").unwrap();
        let mut app = MaiaDesktop {
            database_path: root.join("workspace.db").to_string_lossy().into(),
            evidence_path: source.to_string_lossy().into(),
            ..Default::default()
        };
        app.import_evidence();
        app.prompt = "Question".into();
        app.ask_maia();
        assert_eq!(
            app.recent_requests.len(),
            1,
            "mouse (first call) must create exactly one attempt"
        );
        app.ask_maia(); // simulated Enter firing in the same interaction
        assert_eq!(
            app.recent_requests.len(),
            1,
            "Enter firing while briefing_job.is_some() must fail closed and not \
             create a second attempt"
        );
        drop(app);
        fs::remove_dir_all(root).unwrap();
    }
}
