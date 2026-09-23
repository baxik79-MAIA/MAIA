//! `local-intelligence-recorder` — the lightweight operator surface the
//! M0.15.13 directive allows (§12). No repair/restart actions are exposed
//! — there are none to expose; this crate has no action authority.
//!
//! Usage:
//!   local-intelligence-recorder record-once [--recorder-id ID]
//!   local-intelligence-recorder run --seconds N [--recorder-id ID]
//!   local-intelligence-recorder config
//!
//! `run` starts the bounded cadence loop for exactly `--seconds N` (a
//! finite demonstration run, not a Windows service/daemon — wiring this
//! into an always-on host process is a separate, later integration
//! decision, explicitly out of scope here) and stops it cleanly.
use maia_local_intelligence_health::{HealthStore, RetentionPolicy as HealthRetentionPolicy};
use maia_local_intelligence_hypothesis_ledger::{Ledger, RetentionPolicy as LedgerRetentionPolicy};
use maia_local_intelligence_recorder::{Recorder, RecorderConfig, record_cycle};
use std::{path::Path, sync::Arc, time::SystemTime};

const DEFAULT_HEALTH_DB: &str = r"C:\MAIA\data\maia-local-intelligence-health.sqlite3";
const DEFAULT_LEDGER_DB: &str = r"C:\MAIA\data\maia-local-intelligence-ledger.sqlite3";

fn open_health_store() -> Arc<HealthStore> {
    let path = std::env::var("MAIA_LOCAL_INTELLIGENCE_HEALTH_DB")
        .unwrap_or_else(|_| DEFAULT_HEALTH_DB.into());
    match HealthStore::open(Path::new(&path), HealthRetentionPolicy::default()) {
        Ok(store) => Arc::new(store),
        Err(error) => {
            eprintln!("could not open health store at {path}: {error:?}");
            std::process::exit(1);
        }
    }
}

fn open_ledger() -> Arc<Ledger> {
    let path = std::env::var("MAIA_LOCAL_INTELLIGENCE_LEDGER_DB")
        .unwrap_or_else(|_| DEFAULT_LEDGER_DB.into());
    match Ledger::open(Path::new(&path), LedgerRetentionPolicy::default()) {
        Ok(ledger) => Arc::new(ledger),
        Err(error) => {
            eprintln!("could not open ledger at {path}: {error:?}");
            std::process::exit(1);
        }
    }
}

fn config_from_args(args: &[String]) -> RecorderConfig {
    let mut config = RecorderConfig::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--recorder-id" {
            if let Some(value) = iter.next() {
                config.recorder_id = value.clone();
            }
        }
    }
    config
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("record-once") => cmd_record_once(&args[1..]),
        Some("run") => cmd_run(&args[1..]),
        Some("config") => cmd_config(),
        _ => {
            eprintln!(
                "usage: local-intelligence-recorder <record-once [--recorder-id ID] | run --seconds N [--recorder-id ID] | config>"
            );
            std::process::exit(1);
        }
    }
}

fn cmd_record_once(args: &[String]) {
    let config = config_from_args(args);
    let health = open_health_store();
    let ledger = open_ledger();
    let outcome = record_cycle(&health, &ledger, &config, SystemTime::now());
    println!("{outcome:?}");
}

fn cmd_run(args: &[String]) {
    let mut seconds = 60u64;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--seconds" {
            if let Some(value) = iter.next() {
                if let Ok(parsed) = value.parse::<u64>() {
                    seconds = parsed;
                }
            }
        }
    }
    let config = config_from_args(args);
    let health = open_health_store();
    let ledger = open_ledger();
    println!(
        "starting recorder (recorder_id={}, cadence={:?}) for {seconds}s...",
        config.recorder_id, config.cadence
    );
    let recorder = Recorder::start(health, ledger, config);
    std::thread::sleep(std::time::Duration::from_secs(seconds));
    let outcome = recorder.stop();
    println!("stopped: {outcome:?}");
}

fn cmd_config() {
    let config = RecorderConfig::default();
    println!("recorder_id: {}", config.recorder_id);
    println!("cadence: {:?}", config.cadence);
    println!("window: {:?}", config.window);
    println!("evidence_limit: {}", config.evidence_limit);
    let health_path = std::env::var("MAIA_LOCAL_INTELLIGENCE_HEALTH_DB")
        .unwrap_or_else(|_| DEFAULT_HEALTH_DB.into());
    let ledger_path = std::env::var("MAIA_LOCAL_INTELLIGENCE_LEDGER_DB")
        .unwrap_or_else(|_| DEFAULT_LEDGER_DB.into());
    println!("health_db: {health_path}");
    println!("ledger_db: {ledger_path}");
}
