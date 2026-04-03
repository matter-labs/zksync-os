#![allow(incomplete_features)]
#![feature(allocator_api)]

mod compiler;
mod report;
mod runner;
mod scenario;

use std::path::PathBuf;
use std::process;

use anyhow::Context;

fn main() {
    // rig::init_logger() is called by TestingFramework::new(), so we don't
    // initialize env_logger here to avoid double-init panics.
    // Users can still set RUST_LOG to control verbosity.

    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: evm-divergence-validator <scenario.json>");
        eprintln!();
        eprintln!("Executes a scenario on ZKsync OS and REVM, reports divergences.");
        eprintln!();
        eprintln!("The scenario JSON file should contain:");
        eprintln!("  contracts: map of contract name -> {{ source: \"...\" }} or {{ file: \"...\" }}");
        eprintln!("  accounts:  map of name -> {{ balance: \"...\" }}");
        eprintln!("  block:     optional {{ basefee, gas_limit, timestamp }}");
        eprintln!("  steps:     array of deploy/call actions");
        process::exit(2);
    }

    let scenario_path = PathBuf::from(&args[1]);

    match run(&scenario_path) {
        Ok(report) => {
            let json = serde_json::to_string_pretty(&report).expect("failed to serialize report");
            println!("{json}");
            match report.status.as_str() {
                "match" => process::exit(0),
                "divergence" => process::exit(1),
                _ => process::exit(2),
            }
        }
        Err(err) => {
            let report = report::Report {
                status: "error".to_string(),
                steps: vec![],
                state_diffs: None,
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

    let scenario: scenario::Scenario =
        serde_json::from_str(&content).context("failed to parse scenario JSON")?;

    let scenario_dir = scenario_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));

    // Compile contracts.
    let artifacts = compiler::compile_contracts(&scenario.contracts, scenario_dir)?;

    // Run scenario.
    runner::run_scenario(&scenario, &artifacts)
}
