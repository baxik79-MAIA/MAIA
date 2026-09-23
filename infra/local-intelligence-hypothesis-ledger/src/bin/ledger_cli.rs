//! `local-intelligence-ledger` — the minimal one-shot CLI the M0.15.12
//! directive requires ("A lightweight one-shot CLI is acceptable... The
//! CLI must expose the SAME typed ledger contract used by machine
//! consumers"). No scheduler, no daemon: every invocation performs exactly
//! one explicit operation and exits.
//!
//! Usage:
//!   local-intelligence-ledger record --hours N [--id ID]
//!   local-intelligence-ledger latest
//!   local-intelligence-ledger pattern <PatternName> [--n N]
//!
//! Reads/writes the same ledger file across invocations (default
//! `C:\MAIA\data\maia-local-intelligence-ledger.sqlite3`, overridable via
//! `MAIA_LOCAL_INTELLIGENCE_LEDGER_DB`), and builds diagnostic snapshots
//! from the same health store Desktop writes to (default
//! `MAIA_LOCAL_INTELLIGENCE_HEALTH_DB`, matching `apps/desktop`'s and
//! `local-intelligence-inspect`'s own convention).
use maia_local_intelligence_diagnostics::{DiagnosticPattern, DiagnosticSnapshot};
use maia_local_intelligence_health::{HealthStore, RetentionPolicy as HealthRetentionPolicy};
use maia_local_intelligence_hypothesis_ledger::{
    Ledger, ObservationId, RetentionPolicy as LedgerRetentionPolicy,
};
use std::{path::Path, time::SystemTime};

const DEFAULT_HEALTH_DB: &str = r"C:\MAIA\data\maia-local-intelligence-health.sqlite3";
const DEFAULT_LEDGER_DB: &str = r"C:\MAIA\data\maia-local-intelligence-ledger.sqlite3";
const DEFAULT_WINDOW_HOURS: u64 = 24;
const DEFAULT_EVIDENCE_LIMIT: usize = 5_000;
const DEFAULT_SCAN_LIMIT: usize = 1_000;

fn open_health_store() -> HealthStore {
    let path = std::env::var("MAIA_LOCAL_INTELLIGENCE_HEALTH_DB")
        .unwrap_or_else(|_| DEFAULT_HEALTH_DB.into());
    match HealthStore::open(Path::new(&path), HealthRetentionPolicy::default()) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("could not open health store at {path}: {error:?}");
            std::process::exit(1);
        }
    }
}

fn open_ledger() -> Ledger {
    let path = std::env::var("MAIA_LOCAL_INTELLIGENCE_LEDGER_DB")
        .unwrap_or_else(|_| DEFAULT_LEDGER_DB.into());
    match Ledger::open(Path::new(&path), LedgerRetentionPolicy::default()) {
        Ok(ledger) => ledger,
        Err(error) => {
            eprintln!("could not open ledger at {path}: {error:?}");
            std::process::exit(1);
        }
    }
}

fn parse_pattern(name: &str) -> Option<DiagnosticPattern> {
    match name {
        "IsolatedFailure" => Some(DiagnosticPattern::IsolatedFailure),
        "RepeatedUnavailability" => Some(DiagnosticPattern::RepeatedUnavailability),
        "RepeatedTimeout" => Some(DiagnosticPattern::RepeatedTimeout),
        "IntermittentAvailability" => Some(DiagnosticPattern::IntermittentAvailability),
        "MalformedResponseCluster" => Some(DiagnosticPattern::MalformedResponseCluster),
        "ProviderErrorCluster" => Some(DiagnosticPattern::ProviderErrorCluster),
        "LatencyDegradation" => Some(DiagnosticPattern::LatencyDegradation),
        _ => None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("record") => cmd_record(&args[1..]),
        Some("latest") => cmd_latest(),
        Some("pattern") => cmd_pattern(&args[1..]),
        _ => {
            eprintln!(
                "usage: local-intelligence-ledger <record --hours N [--id ID] | latest | pattern <Name> [--n N]>"
            );
            std::process::exit(1);
        }
    }
}

fn cmd_record(args: &[String]) {
    let mut hours = DEFAULT_WINDOW_HOURS;
    let mut id: Option<String> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--hours" => {
                if let Some(value) = iter.next() {
                    if let Ok(parsed) = value.parse::<u64>() {
                        hours = parsed;
                    }
                }
            }
            "--id" => id = iter.next().cloned(),
            _ => {}
        }
    }

    let health = open_health_store();
    let ledger = open_ledger();
    let until = SystemTime::now();
    let since = until - std::time::Duration::from_secs(hours * 3600);
    let snapshot = match DiagnosticSnapshot::build(&health, since, until, DEFAULT_EVIDENCE_LIMIT) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("could not build a diagnostic snapshot: {error:?}");
            std::process::exit(1);
        }
    };
    let observation_id_value = id.unwrap_or_else(|| {
        format!(
            "cli-{}",
            until
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        )
    });
    let observation_id = match ObservationId::new(observation_id_value) {
        Ok(id) => id,
        Err(error) => {
            eprintln!("invalid observation id: {error:?}");
            std::process::exit(1);
        }
    };
    let observation = maia_local_intelligence_hypothesis_ledger::DiagnosticObservation {
        observation_id,
        observed_at: until,
        snapshot,
    };
    match ledger.record(&observation) {
        Ok(outcome) => println!("{outcome:?}"),
        Err(error) => {
            eprintln!("could not record observation: {error:?}");
            std::process::exit(1);
        }
    }
}

fn cmd_latest() {
    let ledger = open_ledger();
    match ledger.latest() {
        Ok(Some(observation)) => {
            println!(
                "observation_id={} observed_at={:?} evidence_level={:?} patterns={:?}",
                observation.observation_id.as_str(),
                observation.observed_at,
                observation.snapshot.evidence_level,
                observation.snapshot.detected_patterns,
            );
        }
        Ok(None) => println!("no observations recorded yet"),
        Err(error) => {
            eprintln!("could not read the ledger: {error:?}");
            std::process::exit(1);
        }
    }
}

fn cmd_pattern(args: &[String]) {
    let Some(name) = args.first() else {
        eprintln!("usage: local-intelligence-ledger pattern <Name> [--n N]");
        std::process::exit(1);
    };
    let Some(pattern) = parse_pattern(name) else {
        eprintln!("unknown pattern: {name}");
        std::process::exit(1);
    };
    let mut n = 3usize;
    let mut iter = args[1..].iter();
    while let Some(arg) = iter.next() {
        if arg == "--n" {
            if let Some(value) = iter.next() {
                if let Ok(parsed) = value.parse::<usize>() {
                    n = parsed;
                }
            }
        }
    }
    let ledger = open_ledger();
    let run = match ledger.consecutive_supported_run(pattern, DEFAULT_SCAN_LIMIT) {
        Ok(run) => run,
        Err(error) => {
            eprintln!("could not compute consecutive run: {error:?}");
            std::process::exit(1);
        }
    };
    let eligible = match ledger.eligible_support_over_last_n(pattern, n, DEFAULT_SCAN_LIMIT) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("could not compute eligible support: {error:?}");
            std::process::exit(1);
        }
    };
    println!("consecutive_supported_run: {run:?}");
    println!("eligible_support_over_last_{n}: {eligible:?}");
}
