use std::collections::HashMap;

use alloy::consensus::TxEip1559;
use alloy::dyn_abi::{DynSolType, DynSolValue, Specifier};
use alloy::primitives::{Address, TxKind, U256 as AlloyU256};
use alloy::signers::local::PrivateKeySigner;
use anyhow::{bail, Context};
use rig::chain::RunConfig;
use rig::TestingFramework;
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

use crate::compiler::CompiledContract;
use crate::report::{Report, StepResult};
use crate::scenario::{BlockDef, Scenario, Step};

const CHAIN_ID: u64 = 37;
const DEFAULT_GAS_LIMIT: u64 = 5_000_000;
const DEFAULT_MAX_FEE: u128 = 1_000;
const DEFAULT_PRIORITY_FEE: u128 = 1_000;

pub fn run_scenario(
    scenario: &Scenario,
    artifacts: &HashMap<String, CompiledContract>,
) -> anyhow::Result<Report> {
    // Create signers for each named account.
    let mut signers: HashMap<String, PrivateKeySigner> = HashMap::new();
    for name in scenario.accounts.keys() {
        signers.insert(name.clone(), PrivateKeySigner::random());
    }
    // Also create signers for any "from" references not in accounts.
    for step in &scenario.steps {
        let from = match step {
            Step::Deploy { from, .. } => from,
            Step::Call { from, .. } => from,
        };
        signers
            .entry(from.clone())
            .or_insert_with(PrivateKeySigner::random);
    }

    // Set up the testing framework.
    // Enable REVM consistency check with independent gas computation.
    // Combined with the `unlimited_native` feature, this makes ZKsync OS
    // gas accounting equivalent to standard EVM, enabling true gas comparison.
    let mut run_config = RunConfig::without_riscv_run();
    run_config.enable_revm_consistency_check();
    run_config.revm_independent_gas = true;

    let mut tester = TestingFramework::new().with_run_config(run_config);

    // Fund accounts.
    for (name, def) in &scenario.accounts {
        let signer = &signers[name];
        let balance = parse_u256(&def.balance)
            .with_context(|| format!("invalid balance for account '{name}'"))?;
        tester.set_balance(signer.address(), balance);
    }
    // Ensure all senders have at least some balance.
    for (name, signer) in &signers {
        if !scenario.accounts.contains_key(name) {
            // Default fund so they can pay gas.
            tester.set_balance(
                signer.address(),
                ruint::aliases::U256::from(1_000_000_000_000_000_u64),
            );
        }
    }

    // Apply block context if specified.
    if let Some(ref block) = scenario.block {
        tester.set_block_context(Some(make_block_context(block)));
    }

    // Build transactions.
    let mut transactions = Vec::new();
    let mut nonces: HashMap<String, u64> = HashMap::new();
    let mut deployed_addresses: Vec<Address> = Vec::new();
    let mut step_descriptions: Vec<String> = Vec::new();

    for step in &scenario.steps {
        match step {
            Step::Deploy {
                contract,
                from,
                args,
                gas,
                value,
            } => {
                let signer = signers
                    .get(from)
                    .with_context(|| format!("unknown sender '{from}'"))?;
                let artifact = artifacts
                    .get(contract)
                    .with_context(|| format!("no compiled artifact for contract '{contract}'"))?;

                let nonce = nonces.entry(from.clone()).or_insert(0);
                let deploy_addr = signer.address().create(*nonce);

                // Encode constructor args if any.
                let mut init_code = artifact.bytecode.clone();
                if !args.is_empty() {
                    let constructor = artifact
                        .abi
                        .constructor()
                        .with_context(|| {
                            format!("contract '{contract}' has no constructor but args were given")
                        })?;
                    let encoded_args = encode_args(&constructor.inputs, args)?;
                    init_code.extend_from_slice(&encoded_args);
                }

                let value = match value {
                    Some(v) => parse_alloy_u256(v)?,
                    None => AlloyU256::ZERO,
                };

                let tx = TxEip1559 {
                    chain_id: CHAIN_ID,
                    nonce: *nonce,
                    max_fee_per_gas: DEFAULT_MAX_FEE,
                    max_priority_fee_per_gas: DEFAULT_PRIORITY_FEE,
                    gas_limit: gas.unwrap_or(DEFAULT_GAS_LIMIT),
                    to: TxKind::Create,
                    value,
                    access_list: Default::default(),
                    input: init_code.into(),
                };

                transactions.push(ZKsyncTxEnvelope::from_eth_tx(tx, signer.clone()));
                deployed_addresses.push(deploy_addr);
                step_descriptions.push(format!("deploy {contract} -> {deploy_addr}"));
                *nonce += 1;
            }
            Step::Call {
                to,
                from,
                function,
                args,
                gas,
                value,
            } => {
                let signer = signers
                    .get(from)
                    .with_context(|| format!("unknown sender '{from}'"))?;
                let nonce = nonces.entry(from.clone()).or_insert(0);

                let target = resolve_address(to, &deployed_addresses, &signers)?;

                // Encode function call.
                let calldata = encode_function_call(function, args, artifacts)?;

                let value = match value {
                    Some(v) => parse_alloy_u256(v)?,
                    None => AlloyU256::ZERO,
                };

                let tx = TxEip1559 {
                    chain_id: CHAIN_ID,
                    nonce: *nonce,
                    max_fee_per_gas: DEFAULT_MAX_FEE,
                    max_priority_fee_per_gas: DEFAULT_PRIORITY_FEE,
                    gas_limit: gas.unwrap_or(DEFAULT_GAS_LIMIT),
                    to: TxKind::Call(target),
                    value,
                    access_list: Default::default(),
                    input: calldata.into(),
                };

                transactions.push(ZKsyncTxEnvelope::from_eth_tx(tx, signer.clone()));
                step_descriptions.push(format!("call {function} on {target}"));
                *nonce += 1;
            }
        }
    }

    // Execute block.
    let result = tester.execute_block_no_panic(transactions);

    // Build report.
    let mut report = Report {
        status: "match".to_string(),
        steps: Vec::new(),
        state_diffs: None,
        error: None,
    };

    match result {
        Ok(block_output) => {
            for (i, desc) in step_descriptions.iter().enumerate() {
                let tx_result = block_output.tx_results.get(i);
                let (success, gas_used) = match tx_result {
                    Some(Ok(output)) => (output.is_success(), Some(output.gas_used)),
                    Some(Err(_)) => (false, None),
                    None => (false, None),
                };
                report.steps.push(StepResult {
                    description: desc.clone(),
                    success,
                    gas_used,
                });
            }
            report.status = "match".to_string();
        }
        Err(err) => {
            let err_str = format!("{err:?}");
            if err_str.contains("REVM consistency") {
                report.status = "divergence".to_string();
                report.error = Some(err_str);
            } else {
                report.status = "execution_error".to_string();
                report.error = Some(err_str);
            }
        }
    }

    Ok(report)
}

/// Resolve a target address from a string like "$deployed:0", a named account, or a hex address.
fn resolve_address(
    target: &str,
    deployed: &[Address],
    signers: &HashMap<String, PrivateKeySigner>,
) -> anyhow::Result<Address> {
    if let Some(idx_str) = target.strip_prefix("$deployed:") {
        let idx: usize = idx_str
            .parse()
            .with_context(|| format!("invalid deploy index: {idx_str}"))?;
        deployed
            .get(idx)
            .copied()
            .with_context(|| format!("no deploy at index {idx} (only {} deploys so far)", deployed.len()))
    } else if let Some(signer) = signers.get(target) {
        Ok(signer.address())
    } else if target.starts_with("0x") || target.starts_with("0X") {
        target
            .parse::<Address>()
            .with_context(|| format!("invalid address: {target}"))
    } else {
        bail!("cannot resolve target '{target}': not a $deployed ref, named account, or hex address");
    }
}

/// Encode function call from a signature like "transfer(address,uint256)" and JSON args.
fn encode_function_call(
    sig: &str,
    args: &[serde_json::Value],
    _artifacts: &HashMap<String, CompiledContract>,
) -> anyhow::Result<Vec<u8>> {
    // Parse the function signature to get selector and param types.
    let func = alloy::json_abi::Function::parse(sig)
        .with_context(|| format!("failed to parse function signature: {sig}"))?;

    let selector = func.selector();

    let encoded_args = encode_args(&func.inputs, args)?;

    let mut calldata = Vec::with_capacity(4 + encoded_args.len());
    calldata.extend_from_slice(selector.as_slice());
    calldata.extend_from_slice(&encoded_args);
    Ok(calldata)
}

/// ABI-encode arguments given param definitions and JSON values.
fn encode_args(
    params: &[alloy::json_abi::Param],
    args: &[serde_json::Value],
) -> anyhow::Result<Vec<u8>> {
    if params.len() != args.len() {
        bail!(
            "argument count mismatch: expected {}, got {}",
            params.len(),
            args.len()
        );
    }

    let mut values = Vec::with_capacity(args.len());
    for (param, arg) in params.iter().zip(args) {
        let sol_type: DynSolType = param
            .resolve()
            .with_context(|| format!("failed to resolve type for param '{}'", param.name))?;
        let value = json_to_dyn_sol_value(&sol_type, arg)
            .with_context(|| format!("failed to convert arg for param '{}'", param.name))?;
        values.push(value);
    }

    Ok(DynSolValue::Tuple(values).abi_encode_params())
}

/// Convert a JSON value to a DynSolValue based on the expected Solidity type.
fn json_to_dyn_sol_value(
    sol_type: &DynSolType,
    value: &serde_json::Value,
) -> anyhow::Result<DynSolValue> {
    match sol_type {
        DynSolType::Address => {
            let s = value.as_str().context("expected string for address")?;
            let addr: Address = s.parse().context("invalid address")?;
            Ok(DynSolValue::Address(addr))
        }
        DynSolType::Bool => {
            let b = value.as_bool().context("expected bool")?;
            Ok(DynSolValue::Bool(b))
        }
        DynSolType::Uint(bits) => {
            let s = match value {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s.clone(),
                _ => bail!("expected number or string for uint"),
            };
            let v: AlloyU256 = if s.starts_with("0x") || s.starts_with("0X") {
                AlloyU256::from_str_radix(&s[2..], 16)?
            } else {
                AlloyU256::from_str_radix(&s, 10)?
            };
            Ok(DynSolValue::Uint(v, *bits))
        }
        DynSolType::Int(bits) => {
            let s = match value {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s.clone(),
                _ => bail!("expected number or string for int"),
            };
            let v: alloy::primitives::I256 = s.parse().context("invalid int")?;
            Ok(DynSolValue::Int(v, *bits))
        }
        DynSolType::Bytes => {
            let s = value.as_str().context("expected hex string for bytes")?;
            let s = s.strip_prefix("0x").unwrap_or(s);
            let bytes = hex::decode(s).context("invalid hex for bytes")?;
            Ok(DynSolValue::Bytes(bytes))
        }
        DynSolType::FixedBytes(len) => {
            let s = value.as_str().context("expected hex string for bytes")?;
            let s = s.strip_prefix("0x").unwrap_or(s);
            let bytes = hex::decode(s).context("invalid hex for fixed bytes")?;
            if bytes.len() != *len {
                bail!("expected {len} bytes, got {}", bytes.len());
            }
            Ok(DynSolValue::FixedBytes(
                alloy::primitives::FixedBytes::from_slice(&bytes),
                *len,
            ))
        }
        DynSolType::String => {
            let s = value.as_str().context("expected string")?;
            Ok(DynSolValue::String(s.to_string()))
        }
        DynSolType::Array(inner) => {
            let arr = value.as_array().context("expected array")?;
            let vals: Vec<DynSolValue> = arr
                .iter()
                .map(|v| json_to_dyn_sol_value(inner, v))
                .collect::<anyhow::Result<_>>()?;
            Ok(DynSolValue::Array(vals))
        }
        DynSolType::Tuple(types) => {
            let arr = value.as_array().context("expected array for tuple")?;
            if arr.len() != types.len() {
                bail!("tuple length mismatch: expected {}, got {}", types.len(), arr.len());
            }
            let vals: Vec<DynSolValue> = types
                .iter()
                .zip(arr)
                .map(|(t, v)| json_to_dyn_sol_value(t, v))
                .collect::<anyhow::Result<_>>()?;
            Ok(DynSolValue::Tuple(vals))
        }
        _ => bail!("unsupported Solidity type: {sol_type:?}"),
    }
}

fn parse_u256(s: &str) -> anyhow::Result<ruint::aliases::U256> {
    if s.starts_with("0x") || s.starts_with("0X") {
        ruint::aliases::U256::from_str_radix(&s[2..], 16).context("invalid hex U256")
    } else {
        ruint::aliases::U256::from_str_radix(s, 10).context("invalid decimal U256")
    }
}

fn parse_alloy_u256(s: &str) -> anyhow::Result<AlloyU256> {
    if s.starts_with("0x") || s.starts_with("0X") {
        AlloyU256::from_str_radix(&s[2..], 16).context("invalid hex U256")
    } else {
        AlloyU256::from_str_radix(s, 10).context("invalid decimal U256")
    }
}

fn make_block_context(block: &BlockDef) -> rig::BlockContext {
    rig::BlockContext {
        eip1559_basefee: ruint::aliases::U256::from(
            block.basefee.unwrap_or(DEFAULT_MAX_FEE),
        ),
        native_price: ruint::aliases::U256::from(10u64),
        pubdata_price: Default::default(),
        timestamp: block.timestamp.unwrap_or(1_700_000_000),
        gas_limit: block.gas_limit.unwrap_or(30_000_000),
        pubdata_limit: u64::MAX,
        coinbase: Default::default(),
        mix_hash: Default::default(),
        blob_fee: Default::default(),
    }
}
