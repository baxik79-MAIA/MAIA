//! Read-only projection of reported Supervisor facts and a fresh OS query.
//! These facts never enter briefing evidence, Task/Run state or execution gates.
use maia_supervisor_observation::{ProcessIdentity, ProcessObservation};
#[cfg(any(windows, test))]
use serde_json::Value;
#[cfg(windows)]
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
#[cfg(windows)]
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[cfg(windows)]
const ROOT: &str = r"C:\MAIA\orchestrator";
#[cfg(windows)]
const MAX_STATE: u64 = 65_536;
#[cfg(windows)]
const MAX_RECORD: u64 = 16_384;
const SECOND: i128 = 1_000_000_000;
const MAX_HEARTBEAT_AGE: i128 = 60 * SECOND;
pub(crate) const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
pub(crate) const OBSERVATION_TTL: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Activity {
    Unknown,
    ProcessObservedAlive,
    ProcessObservedExited,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Semantic {
    Unconfirmed,
    ReportedFailure,
    ReportedSuccess,
}

#[derive(Clone, Debug)]
pub(crate) struct ReportedFacts {
    pub run_id: Option<String>,
    pub phase: String,
    pub worker_kind: Option<String>,
    pub pid: Option<u32>,
    pub identity: Option<ProcessIdentity>,
    pub heartbeat_at: Option<String>,
    pub heartbeat_source: &'static str,
    pub heartbeat_unix: Option<i128>,
    pub log_bytes: Option<u64>,
    pub log_changed_at: Option<String>,
    pub semantic_run_id: Option<String>,
    pub semantic_status: Option<String>,
    pub is_error: Option<bool>,
    pub exit_code: Option<i64>,
}

impl Default for ReportedFacts {
    fn default() -> Self {
        Self {
            run_id: None,
            phase: "UNKNOWN".into(),
            worker_kind: None,
            pid: None,
            identity: None,
            heartbeat_at: None,
            heartbeat_source: "unavailable",
            heartbeat_unix: None,
            log_bytes: None,
            log_changed_at: None,
            semantic_run_id: None,
            semantic_status: None,
            is_error: None,
            exit_code: None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Projection {
    pub activity: Activity,
    pub semantic: Semantic,
    pub reported: ReportedFacts,
    pub reason: &'static str,
    pub sampled_at: String,
    // Conservative original sample origin, before file/native queries. Channel
    // delivery and UI receipt never renew this observation's lifetime.
    monotonic_sample_started: Instant,
    pub last_confirmed_at: Option<String>,
    pub last_confirmed_process: Option<ProcessIdentity>,
    pub last_confirmed_activity: Option<Activity>,
    pub worker_binding_proven: bool,
    pub executed_image_proven: bool,
    pub native_image_chain_proven: bool,
    pub native_run_ready: bool,
    pub apply_permitted: bool,
    pub durability_proven: bool,
}

impl Projection {
    pub fn unknown(reason: &'static str) -> Self {
        Self {
            activity: Activity::Unknown,
            semantic: Semantic::Unconfirmed,
            reported: ReportedFacts::default(),
            reason,
            sampled_at: String::new(),
            monotonic_sample_started: Instant::now(),
            last_confirmed_at: None,
            last_confirmed_process: None,
            last_confirmed_activity: None,
            worker_binding_proven: false,
            executed_image_proven: false,
            native_image_chain_proven: false,
            native_run_ready: false,
            apply_permitted: false,
            durability_proven: false,
        }
    }

    pub fn expire(&mut self) {
        self.activity = Activity::Unknown;
        self.reason = "OBSERVATION_STALE";
    }

    pub fn remaining_lifetime_at(&self, now: Instant) -> Duration {
        OBSERVATION_TTL.saturating_sub(now.saturating_duration_since(self.monotonic_sample_started))
    }

    pub fn expire_at(&mut self, now: Instant) {
        if self.remaining_lifetime_at(now).is_zero() {
            self.expire();
        }
    }

    pub fn for_display(mut self, now: Instant, previous: &Self) -> Self {
        self.expire_at(now);
        self.preserve_confirmation_from(previous);
        self
    }

    pub fn preserve_confirmation_from(&mut self, previous: &Self) {
        if self.last_confirmed_at.is_none() {
            self.last_confirmed_at
                .clone_from(&previous.last_confirmed_at);
            self.last_confirmed_process = previous.last_confirmed_process;
            self.last_confirmed_activity = previous.last_confirmed_activity;
        }
    }
}

pub(crate) fn project(facts: ReportedFacts, observed: ProcessObservation, now: i128) -> Projection {
    let mut out = Projection::unknown("OBSERVATION_UNAVAILABLE");
    out.sampled_at = OffsetDateTime::from_unix_timestamp_nanos(now)
        .ok()
        .and_then(|v| v.format(&Rfc3339).ok())
        .unwrap_or_default();
    // This is reported semantics, never an authenticated execution receipt.
    let same_run = facts.run_id.is_some() && facts.run_id == facts.semantic_run_id;
    out.semantic = if same_run
        && (facts.is_error == Some(true) || facts.exit_code.is_some_and(|code| code != 0))
    {
        Semantic::ReportedFailure
    } else if same_run
        && facts.is_error == Some(false)
        && facts.exit_code == Some(0)
        && facts.semantic_status.as_deref() == Some("SUCCESS")
    {
        Semantic::ReportedSuccess
    } else {
        Semantic::Unconfirmed
    };
    out.reason = match (facts.identity, facts.heartbeat_unix) {
        (None, _) => "IDENTITY_UNAVAILABLE",
        (_, None) => "CONTROLLER_TELEMETRY_UNAVAILABLE",
        (Some(_), Some(hb)) if hb > now => "CONTROLLER_TELEMETRY_FUTURE",
        (Some(_), Some(hb)) if now.saturating_sub(hb) > MAX_HEARTBEAT_AGE => {
            "CONTROLLER_TELEMETRY_STALE"
        }
        (Some(expected), Some(_)) => match observed {
            ProcessObservation::Alive(actual) if actual == expected => {
                out.activity = Activity::ProcessObservedAlive;
                out.last_confirmed_at = Some(out.sampled_at.clone());
                "REPORTED_PROCESS_IDENTITY_OBSERVED_ALIVE"
            }
            ProcessObservation::Exited { identity, .. } if identity == expected => {
                out.activity = Activity::ProcessObservedExited;
                out.last_confirmed_at = Some(out.sampled_at.clone());
                "REPORTED_PROCESS_IDENTITY_OBSERVED_EXITED"
            }
            ProcessObservation::Alive(_) | ProcessObservation::Exited { .. } => {
                "PROCESS_BIRTH_MISMATCH"
            }
            ProcessObservation::Unavailable => "NATIVE_QUERY_UNAVAILABLE",
        },
    };
    if out.activity != Activity::Unknown {
        out.last_confirmed_process = facts.identity;
        out.last_confirmed_activity = Some(out.activity);
    }
    out.reported = facts;
    out
}

#[cfg(windows)]
fn text(value: &Value, key: &str, limit: usize) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= limit)
        .map(str::to_owned)
}

#[cfg(any(windows, test))]
fn lossless_u64(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        value
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 20 && s.bytes().all(|c| c.is_ascii_digit()))?
            .parse()
            .ok()
    })
}

#[cfg(any(windows, test))]
fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
}

#[cfg(windows)]
fn ordinary(path: &Path) -> Result<(), &'static str> {
    for p in path.ancestors() {
        let metadata = p.symlink_metadata().map_err(|_| "SOURCE_UNAVAILABLE")?;
        if metadata.file_type().is_symlink() {
            return Err("REPARSE_SOURCE");
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("REPARSE_SOURCE");
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn bytes(path: &Path, cap: u64, tail: bool) -> Result<Vec<u8>, &'static str> {
    ordinary(path)?;
    let mut file = File::open(path).map_err(|_| "SOURCE_UNAVAILABLE")?;
    let size = file.metadata().map_err(|_| "SOURCE_UNAVAILABLE")?.len();
    if !tail && size > cap {
        return Err("SOURCE_TOO_LARGE");
    }
    if tail {
        file.seek(SeekFrom::Start(size.saturating_sub(cap)))
            .map_err(|_| "SOURCE_UNAVAILABLE")?;
    }
    let mut raw = Vec::new();
    file.take(cap + 1)
        .read_to_end(&mut raw)
        .map_err(|_| "SOURCE_UNAVAILABLE")?;
    if raw.len() as u64 > cap {
        return Err("SOURCE_TOO_LARGE");
    }
    Ok(raw)
}

#[cfg(windows)]
fn object(raw: &[u8]) -> Result<Value, &'static str> {
    // PS5 Add-Content -Encoding UTF8 may put a BOM on the first heartbeat
    // line. Hashes and readback retain original bytes; only JSON parsing skips it.
    let raw = if raw.starts_with(b"\xEF\xBB\xBF") {
        &raw[3..]
    } else {
        raw
    };
    let value: Value = serde_json::from_slice(raw).map_err(|_| "SOURCE_JSON_INVALID")?;
    if !value.is_object() {
        return Err("SOURCE_JSON_INVALID");
    }
    Ok(value)
}

#[cfg(windows)]
fn same_path(reported: &str, actual: &Path) -> bool {
    reported
        .replace('\\', "/")
        .eq_ignore_ascii_case(&actual.to_string_lossy().replace('\\', "/"))
}

#[cfg(windows)]
fn identity_for(
    state: &Value,
    folder: &Path,
    run_id: &str,
    pid: Option<u32>,
) -> Option<ProcessIdentity> {
    let expected = text(state, "current_worker_identity_sha256", 64)?;
    if expected.len() != 64 || !expected.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let raw = bytes(&folder.join("worker-identity.json"), MAX_RECORD, false).ok()?;
    if format!("{:x}", Sha256::digest(&raw)) != expected {
        return None;
    }
    let identity = object(&raw).ok()?;
    if identity.get("run_id").and_then(Value::as_str) != Some(run_id)
        || identity.get("prompt_id") != state.get("current_prompt_id")
        || identity.get("pid").and_then(lossless_u64) != pid.map(u64::from)
    {
        return None;
    }
    let birth = identity.get("created_ticks").and_then(lossless_u64)?;
    if birth == 0 {
        return None;
    }
    Some(ProcessIdentity {
        pid: pid?,
        birth_ticks: birth,
    })
}

#[cfg(windows)]
// Supervisor Monitor-Run emits these samples; they are controller telemetry,
// not evidence that the worker itself sent a heartbeat.
fn heartbeat_for(folder: &Path, pid: Option<u32>) -> Option<(String, i128)> {
    let raw = bytes(&folder.join("heartbeat.jsonl"), MAX_RECORD, true).ok()?;
    let line = raw.split(|c| *c == b'\n').rev().find(|v| !v.is_empty())?;
    let heartbeat = object(line).ok()?;
    if heartbeat.get("worker_pid").and_then(lossless_u64) != pid.map(u64::from)
        || heartbeat.get("worker_alive").and_then(Value::as_bool) != Some(true)
    {
        return None;
    }
    let at = text(&heartbeat, "at_utc", 64)?;
    let unix = OffsetDateTime::parse(&at, &Rfc3339)
        .ok()?
        .unix_timestamp_nanos();
    Some((at, unix))
}

#[cfg(windows)]
fn read_reported(root: &Path) -> Result<ReportedFacts, &'static str> {
    let state_path = root.join("state/state.json");
    let raw = bytes(&state_path, MAX_STATE, false)?;
    let state = object(&raw)?;
    let run_id = text(&state, "current_run_id", 128).filter(|id| safe_id(id));
    let pid = state
        .get("current_worker_pid")
        .and_then(lossless_u64)
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v != 0);
    let mut facts = ReportedFacts {
        run_id,
        phase: text(&state, "status", 64).unwrap_or_else(|| "UNKNOWN".into()),
        pid,
        worker_kind: text(&state, "current_worker_kind", 80),
        heartbeat_at: text(&state, "last_heartbeat_utc", 64),
        heartbeat_source: "state.last_heartbeat_utc (reported controller telemetry)",
        log_bytes: state.get("last_log_size").and_then(lossless_u64),
        log_changed_at: text(&state, "last_log_change_utc", 64),
        ..ReportedFacts::default()
    };
    if let Some(id) = &facts.run_id {
        let folder: PathBuf = root.join("runs").join(id);
        let matched = state
            .get("current_run_dir")
            .and_then(Value::as_str)
            .is_some_and(|s| same_path(s, &folder));
        if matched {
            // Only fixed basenames below one validated current run; never follow
            // a log path, worktree, command or executable from untrusted metadata.
            facts.identity = identity_for(&state, &folder, id, pid);
            if let Some((at, unix)) = heartbeat_for(&folder, pid) {
                facts.heartbeat_at = Some(at);
                facts.heartbeat_source = "heartbeat.jsonl (reported controller telemetry)";
                facts.heartbeat_unix = Some(unix);
            }
            if let Ok(summary) =
                bytes(&folder.join("summary.json"), MAX_RECORD, false).and_then(|raw| object(&raw))
            {
                facts.semantic_run_id = text(&summary, "run_id", 128);
                facts.semantic_status = text(&summary, "status", 32);
                facts.is_error = summary.get("is_error").and_then(Value::as_bool);
                facts.exit_code = summary.get("exit_code").and_then(Value::as_i64);
            }
        }
    }
    if bytes(&state_path, MAX_STATE, false)? != raw {
        return Err("SOURCE_CHANGED_DURING_READ");
    }
    Ok(facts)
}

#[cfg(windows)]
fn read_at(root: &Path) -> Projection {
    match read_reported(root) {
        Ok(facts) => {
            let observation = facts
                .identity
                .map(|v| maia_supervisor_observation::observe(v.pid))
                .unwrap_or(ProcessObservation::Unavailable);
            project(
                facts,
                observation,
                OffsetDateTime::now_utc().unix_timestamp_nanos(),
            )
        }
        Err(reason) => Projection::unknown(reason),
    }
}

pub(crate) fn read_default() -> Projection {
    // Include filesystem/query time and time queued for the UI in the TTL.
    let sample_started = Instant::now();
    #[cfg(not(windows))]
    let mut out = {
        let mut out = project(
            ReportedFacts::default(),
            ProcessObservation::Unavailable,
            OffsetDateTime::now_utc().unix_timestamp_nanos(),
        );
        out.reason = "PLATFORM_OBSERVATION_UNAVAILABLE";
        out
    };
    #[cfg(windows)]
    let mut out = read_at(Path::new(ROOT));
    out.monotonic_sample_started = sample_started;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn facts() -> ReportedFacts {
        ReportedFacts {
            run_id: Some("current-run".into()),
            phase: "ACTIVE".into(),
            pid: Some(42),
            identity: Some(ProcessIdentity {
                pid: 42,
                birth_ticks: 134_000_000_000_000_001,
            }),
            heartbeat_at: Some("1970-01-01T00:16:40Z".into()),
            heartbeat_unix: Some(1_000 * SECOND),
            ..ReportedFacts::default()
        }
    }
    #[test]
    fn pid_or_phase_alone_never_means_alive() {
        let mut f = facts();
        f.identity = None;
        assert_eq!(
            project(
                f.clone(),
                ProcessObservation::Alive(facts().identity.unwrap()),
                1_001 * SECOND
            )
            .activity,
            Activity::Unknown
        );
        f.phase = "PIPELINE_COMPLETE".into();
        f.exit_code = Some(0);
        let p = project(f, ProcessObservation::Unavailable, 1_001 * SECOND);
        assert_eq!(p.activity, Activity::Unknown);
        assert_eq!(p.semantic, Semantic::Unconfirmed);
    }
    #[test]
    fn observed_alive_is_only_process_observation() {
        let f = facts();
        let p = project(
            f.clone(),
            ProcessObservation::Alive(f.identity.unwrap()),
            1_001 * SECOND,
        );
        assert_eq!(p.activity, Activity::ProcessObservedAlive);
        assert!(
            !p.worker_binding_proven
                && !p.executed_image_proven
                && !p.native_image_chain_proven
                && !p.native_run_ready
                && !p.apply_permitted
                && !p.durability_proven
        );
    }
    #[test]
    fn reused_pid_missing_stale_future_and_denied_are_unknown() {
        let mut identity = facts().identity.unwrap();
        identity.birth_ticks += 1;
        assert_eq!(
            project(facts(), ProcessObservation::Alive(identity), 1_001 * SECOND).reason,
            "PROCESS_BIRTH_MISMATCH"
        );
        for hb in [
            None,
            Some(900 * SECOND),
            Some(1_002 * SECOND),
            Some(1_001 * SECOND + 1),
        ] {
            let mut f = facts();
            f.heartbeat_unix = hb;
            assert_eq!(
                project(
                    f.clone(),
                    ProcessObservation::Alive(f.identity.unwrap()),
                    1_001 * SECOND
                )
                .activity,
                Activity::Unknown
            );
        }
        assert_eq!(
            project(facts(), ProcessObservation::Unavailable, 1_001 * SECOND).activity,
            Activity::Unknown
        );
    }
    #[test]
    fn a_signaled_process_with_exit_259_is_exited() {
        let f = facts();
        let p = project(
            f.clone(),
            ProcessObservation::Exited {
                identity: f.identity.unwrap(),
                exit_code: Some(259),
            },
            1_001 * SECOND,
        );
        assert_eq!(p.activity, Activity::ProcessObservedExited);
    }
    #[test]
    fn semantic_error_overrides_exit_zero_and_attempts_do_not_alias() {
        let mut f = facts();
        f.semantic_run_id = f.run_id.clone();
        f.semantic_status = Some("SUCCESS".into());
        f.exit_code = Some(0);
        f.is_error = Some(true);
        assert_eq!(
            project(f.clone(), ProcessObservation::Unavailable, 1_001 * SECOND).semantic,
            Semantic::ReportedFailure
        );
        f.is_error = Some(false);
        assert_eq!(
            project(f.clone(), ProcessObservation::Unavailable, 1_001 * SECOND).semantic,
            Semantic::ReportedSuccess
        );
        f.semantic_run_id = Some("previous-run".into());
        assert_eq!(
            project(f, ProcessObservation::Unavailable, 1_001 * SECOND).semantic,
            Semantic::Unconfirmed
        );
    }
    #[test]
    fn refresh_failure_clears_alive_but_preserves_last_observation() {
        let f = facts();
        let old = project(
            f.clone(),
            ProcessObservation::Alive(f.identity.unwrap()),
            1_001 * SECOND,
        );
        let mut next = Projection::unknown("SOURCE_UNAVAILABLE");
        next.preserve_confirmation_from(&old);
        assert_eq!(next.activity, Activity::Unknown);
        assert_eq!(next.last_confirmed_at, old.last_confirmed_at);
        assert_eq!(next.last_confirmed_process, old.last_confirmed_process);
        assert_eq!(next.last_confirmed_activity, old.last_confirmed_activity);
        let mut stale = old;
        stale.expire();
        assert_eq!(stale.activity, Activity::Unknown);
    }
    #[test]
    fn queued_delivery_keeps_original_age_instead_of_renewing_ttl() {
        let origin = Instant::now();
        let f = facts();
        let mut sampled = project(
            f.clone(),
            ProcessObservation::Alive(f.identity.unwrap()),
            1_001 * SECOND,
        );
        sampled.monotonic_sample_started = origin;
        let previous = Projection::unknown("NOT_OBSERVED");
        let delivered = sampled.for_display(origin + Duration::from_secs(9), &previous);
        assert_eq!(delivered.activity, Activity::ProcessObservedAlive);
        assert_eq!(delivered.monotonic_sample_started, origin);
        assert_eq!(
            delivered.remaining_lifetime_at(origin + Duration::from_secs(9)),
            Duration::from_secs(1),
        );
    }
    #[test]
    fn delayed_delivery_is_already_unknown_without_losing_confirmation() {
        let origin = Instant::now();
        let f = facts();
        let mut sampled = project(
            f.clone(),
            ProcessObservation::Alive(f.identity.unwrap()),
            1_001 * SECOND,
        );
        sampled.monotonic_sample_started = origin;
        let confirmation = sampled.last_confirmed_at.clone();
        let delivered = sampled.for_display(
            origin + OBSERVATION_TTL + Duration::from_nanos(1),
            &Projection::unknown("NOT_OBSERVED"),
        );
        assert_eq!(delivered.activity, Activity::Unknown);
        assert_eq!(delivered.reason, "OBSERVATION_STALE");
        assert_eq!(delivered.last_confirmed_at, confirmation);
        assert_eq!(delivered.last_confirmed_process, f.identity);
        assert_eq!(delivered.monotonic_sample_started, origin);
    }
    #[test]
    fn near_ttl_repaint_budget_and_exact_expiry_use_original_sample() {
        let origin = Instant::now();
        let f = facts();
        let mut sampled = project(
            f.clone(),
            ProcessObservation::Alive(f.identity.unwrap()),
            1_001 * SECOND,
        );
        sampled.monotonic_sample_started = origin;
        let near = origin + OBSERVATION_TTL - Duration::from_millis(1);
        sampled.expire_at(near);
        assert_eq!(sampled.activity, Activity::ProcessObservedAlive);
        assert_eq!(
            sampled.remaining_lifetime_at(near),
            Duration::from_millis(1)
        );
        sampled.expire_at(origin + OBSERVATION_TTL);
        assert_eq!(sampled.activity, Activity::Unknown);
        assert_eq!(
            sampled.remaining_lifetime_at(origin + OBSERVATION_TTL),
            Duration::ZERO
        );
        assert_eq!(sampled.monotonic_sample_started, origin);
    }
    #[test]
    fn birth_ticks_are_lossless_and_untrusted_paths_are_not_ids() {
        assert_eq!(
            lossless_u64(&serde_json::json!("134000000000000001")),
            Some(134_000_000_000_000_001)
        );
        assert!(lossless_u64(&serde_json::json!(1.5)).is_none());
        for path in ["..", "../secret", "C:\\other", "/tmp/other"] {
            assert!(!safe_id(path));
        }
    }

    #[cfg(windows)]
    struct Fixture(PathBuf);
    #[cfg(windows)]
    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("maia-status-fixture-{}", uuid::Uuid::now_v7()));
            std::fs::create_dir(&root).unwrap();
            std::fs::create_dir(root.join("state")).unwrap();
            Self(root)
        }
    }
    #[cfg(windows)]
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(windows)]
    #[test]
    fn bom_compatible_legacy_terminal_fixture_is_unknown_and_unconfirmed() {
        let fixture = Fixture::new();
        let folder = fixture.0.join("runs/legacy-run");
        std::fs::create_dir_all(&folder).unwrap();
        let state = serde_json::json!({"schema_version":1,"status":"PIPELINE_COMPLETE",
            "exit_code":0,"current_run_id":"legacy-run","current_worker_pid":42,
            "current_run_dir":folder.to_string_lossy(),
            "last_heartbeat_utc":"2026-10-08T21:25:19.8877839Z"});
        let mut raw = b"\xEF\xBB\xBF".to_vec();
        raw.extend(serde_json::to_vec(&state).unwrap());
        std::fs::write(fixture.0.join("state/state.json"), &raw).unwrap();
        std::fs::write(folder.join("heartbeat.jsonl"),
            b"\xEF\xBB\xBF{\"at_utc\":\"2026-10-08T21:25:19.8877839Z\",\"worker_pid\":42,\"worker_alive\":true}\n").unwrap();
        std::fs::write(
            folder.join("summary.json"),
            br#"{"run_id":"legacy-run","exit_code":0,"verdict":"APPROVED"}"#,
        )
        .unwrap();
        assert!(heartbeat_for(&folder, Some(42)).is_some());
        let projection = read_at(&fixture.0);
        assert_eq!(projection.activity, Activity::Unknown);
        assert_eq!(projection.semantic, Semantic::Unconfirmed);
        assert_eq!(projection.reported.phase, "PIPELINE_COMPLETE");
        assert!(projection.reported.identity.is_none());
        assert_eq!(
            projection.reported.heartbeat_at.as_deref(),
            Some("2026-10-08T21:25:19.8877839Z")
        );
        assert_eq!(
            std::fs::read(fixture.0.join("state/state.json")).unwrap(),
            raw
        );
    }

    #[cfg(windows)]
    #[test]
    fn missing_invalid_and_bounded_oversize_sources_are_unknown() {
        let fixture = Fixture::new();
        assert_eq!(read_at(&fixture.0).activity, Activity::Unknown);
        std::fs::write(fixture.0.join("state/state.json"), b"{invalid").unwrap();
        let invalid = read_at(&fixture.0);
        assert_eq!(invalid.activity, Activity::Unknown);
        assert_eq!(invalid.reason, "SOURCE_JSON_INVALID");
        std::fs::write(
            fixture.0.join("state/state.json"),
            vec![b' '; MAX_STATE as usize + 1],
        )
        .unwrap();
        let too_large = read_at(&fixture.0);
        assert_eq!(too_large.activity, Activity::Unknown);
        assert_eq!(too_large.reason, "SOURCE_TOO_LARGE");
    }
}
