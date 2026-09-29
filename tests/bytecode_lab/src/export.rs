//! Exporting a case as an `evm_divergence_validator` scenario.
//!
//! The scenario uses `send_raw` steps so no Solidity is involved: a first step
//! deploys the program (a CREATE with the initcode), and each following step
//! calls the deployed contract at `$deployed:0`. All steps run from a single
//! funded account, `alice`, in one block, which is what the validator supports.
//! The output is a reproducer the VM team can confirm with their own tool; it
//! exercises the same program and calls, though it uses one sender in one block
//! rather than the lab's two signers across two blocks.

use std::collections::BTreeMap;

use anyhow::{ensure, Context, Result};
use serde::Serialize;

use crate::case::CaseRecord;

/// Block gas limit for the single-block scenario, comfortably above the sum of
/// the transactions' gas.
const EXPORT_BLOCK_GAS_LIMIT: u64 = 500_000_000;
/// Deploy gas, under the default per-tx cap (EIP-7825: 2^24 on Osaka).
const EXPORT_DEPLOY_GAS: u64 = 16_000_000;
/// A generous balance for `alice`, in wei.
const EXPORT_ALICE_BALANCE: &str = "1000000000000000000000000";

#[derive(Serialize)]
struct ExportScenario {
    accounts: BTreeMap<String, ExportAccount>,
    block: ExportBlock,
    steps: Vec<ExportStep>,
}

#[derive(Serialize)]
struct ExportAccount {
    balance: String,
}

#[derive(Serialize)]
struct ExportBlock {
    basefee: u128,
    gas_limit: u64,
    timestamp: u64,
}

/// A flat step with an explicit `type` field, matching the validator's
/// `#[serde(tag = "type")]` `Step` on the deserializing side. A flat struct is
/// used rather than a tagged enum because `serde_yaml` serializes internally
/// tagged enums unreliably.
#[derive(Serialize)]
struct ExportStep {
    #[serde(rename = "type")]
    step_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<String>,
    from: String,
    data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    gas: u64,
}

/// Render `record` as a validator scenario in YAML.
pub fn export_scenario(record: &CaseRecord) -> Result<String> {
    ensure!(
        record.mode != program_gen::Mode::Delegation7702,
        "delegation-7702 export is unsupported; use replay to preserve the authorization sequence"
    );
    let mut steps = Vec::with_capacity(1 + record.program.calls.len());

    // Deployment step first: a CREATE (no `to`) carrying the initcode.
    steps.push(ExportStep {
        step_type: "send_raw".to_string(),
        to: None,
        from: "alice".to_string(),
        data: format!("0x{}", hex::encode(&record.program.initcode)),
        value: None,
        gas: EXPORT_DEPLOY_GAS,
    });

    // One call per CallPlan, against the deployed contract.
    for call in &record.program.calls {
        steps.push(ExportStep {
            step_type: "send_raw".to_string(),
            to: Some("$deployed:0".to_string()),
            from: "alice".to_string(),
            data: format!("0x{}", hex::encode(&call.calldata)),
            value: (call.value != 0).then(|| call.value.to_string()),
            gas: call.gas,
        });
    }

    let mut accounts = BTreeMap::new();
    accounts.insert(
        "alice".to_string(),
        ExportAccount {
            balance: EXPORT_ALICE_BALANCE.to_string(),
        },
    );

    // The validator forces native_price = 0 and blob_fee = 1 itself; only the
    // basefee and timestamp carry over from the case's block context.
    let basefee = record
        .block_context
        .eip1559_basefee
        .parse::<u128>()
        .context("invalid basefee in case record")?;

    let scenario = ExportScenario {
        accounts,
        block: ExportBlock {
            basefee,
            gas_limit: EXPORT_BLOCK_GAS_LIMIT,
            timestamp: record.block_context.timestamp,
        },
        steps,
    };

    let body = serde_yaml::to_string(&scenario)?;
    let header = format!(
        "# Exported by bytecode-lab from case index {} (case_seed {}, mode {:?}, fork {}).\n\
         # Run from tests/evm_divergence_validator: cargo run --release -- <this-file>\n",
        record.case_index, record.case_seed, record.mode, record.fork
    );
    Ok(format!("{header}{body}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        case::{BlockContextRecord, LAB_FORK},
        classify::Outcome,
        runner::block_context_for_mode,
    };
    use program_gen::{generate, Fork, Mode};

    fn record(mode: Mode) -> CaseRecord {
        CaseRecord {
            master_seed: 1,
            case_seed: 2,
            case_index: 0,
            fork: LAB_FORK.into(),
            mode,
            program: generate(2, Fork::Osaka, mode),
            block_context: BlockContextRecord::from_block_context(&block_context_for_mode(mode)),
            outcome: Outcome::Ok,
        }
    }

    #[test]
    fn exports_deployment_and_calls() {
        let record = record(Mode::GasAgnostic);
        let yaml: serde_yaml::Value =
            serde_yaml::from_str(&export_scenario(&record).unwrap()).unwrap();
        let steps = yaml["steps"].as_sequence().unwrap();
        assert_eq!(steps.len(), 1 + record.program.calls.len());
        assert_eq!(
            steps[0]["data"].as_str(),
            Some(format!("0x{}", hex::encode(&record.program.initcode)).as_str())
        );
        assert_eq!(steps[1]["to"].as_str(), Some("$deployed:0"));
    }

    #[test]
    fn does_not_export_delegation_without_authorizations() {
        assert!(export_scenario(&record(Mode::Delegation7702)).is_err());
    }
}
