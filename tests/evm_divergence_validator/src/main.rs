#![allow(incomplete_features)]
#![feature(allocator_api)]

mod compiler;
mod report;
mod runner;
mod scenario;

use std::ffi::OsStr;
use std::path::PathBuf;
use std::process;

use anyhow::Context;

fn main() {
    // Both engines recurse natively once per EVM call frame. On the main thread's default
    // stack, a scenario nesting more than a few hundred frames aborts the process before a
    // report is produced, which makes the 1024 call-depth limit impossible to exercise.
    // Run the work on a thread with a large stack instead.
    let child = std::thread::Builder::new()
        .stack_size(1024 * 1024 * 1024)
        .spawn(real_main)
        .expect("failed to spawn worker thread");
    match child.join() {
        Ok(()) => {}
        Err(_) => process::exit(2),
    }
}

fn real_main() {
    // rig::init_logger() is called by TestingFramework::new(), so we don't
    // initialize env_logger here to avoid double-init panics.
    // Users can still set RUST_LOG to control verbosity.

    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: evm-divergence-validator <scenario.yaml|json>");
        eprintln!();
        eprintln!("Executes a scenario on ZKsync OS and REVM, reports divergences.");
        eprintln!();
        eprintln!("Accepts YAML (.yaml/.yml) or JSON (.json) scenario files.");
        process::exit(2);
    }

    let scenario_path = PathBuf::from(&args[1]);

    match run(&scenario_path) {
        Ok(report) => {
            let json = serde_json::to_string_pretty(&report).expect("failed to serialize report");
            println!("{json}");
            match report.status.as_str() {
                report::STATUS_MATCH => process::exit(0),
                report::STATUS_DIVERGENCE => process::exit(1),
                _ => process::exit(2),
            }
        }
        Err(err) => {
            let report = report::Report {
                status: report::STATUS_ERROR.to_string(),
                steps: vec![],
                error: Some(format!("{err:#}")),
            };
            let json = serde_json::to_string_pretty(&report).expect("failed to serialize report");
            println!("{json}");
            process::exit(2);
        }
    }
}

fn run(scenario_path: &PathBuf) -> anyhow::Result<report::Report> {
    let content = std::fs::read_to_string(scenario_path)
        .with_context(|| format!("failed to read scenario file: {}", scenario_path.display()))?;

    let is_yaml = scenario_path
        .extension()
        .is_some_and(|ext| ext == OsStr::new("yaml") || ext == OsStr::new("yml"));

    let scenario: scenario::Scenario = if is_yaml {
        serde_yaml::from_str(&content).context("failed to parse scenario YAML")?
    } else {
        serde_json::from_str(&content).context("failed to parse scenario JSON")?
    };

    let scenario_dir = scenario_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));

    // Compile contracts.
    let artifacts = compiler::compile_contracts(&scenario.contracts, scenario_dir)?;

    // Run scenario.
    runner::run_scenario(&scenario, &artifacts)
}
