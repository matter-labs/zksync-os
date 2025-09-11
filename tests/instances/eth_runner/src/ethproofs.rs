use crate::live_run::rpc;
use alloy::consensus::Header;
use alloy_primitives::U256;
use alloy_rlp::Encodable;
use anyhow::Context;
use anyhow::Ok;
use rig::log::info;
use rig::*;
use std::thread::sleep;
use std::time::Duration;

const ETH_CHAIN_ID: u64 = 1;

#[allow(clippy::too_many_arguments)]
fn eth_run(
    mut chain: Chain<false>,
    header: Header,
    block_number: u64,
    transactions: Vec<Vec<u8>>,
    block_hashes: Vec<U256>,
    witness: alloy_rpc_types_debug::ExecutionWitness,
    withdrawals_encoding: Vec<u8>,
) -> anyhow::Result<()> {
    chain.set_last_block_number(block_number - 1);

    chain.set_block_hashes(block_hashes.try_into().unwrap());

    let witness_output_dir = {
        let mut suffix = block_number.to_string();
        suffix.push_str("_witness");
        std::path::PathBuf::from(&suffix)
    };
    let _result_keeper = chain.run_eth_block::<true>(
        transactions,
        witness,
        header,
        withdrawals_encoding,
        Some(witness_output_dir),
        None,
    );

    Ok(())
}

pub fn ethproofs_run(block_number: u64, reth_endpoint: &str) -> anyhow::Result<()> {
    // Fetch data from RPC endpoints
    let block = rpc::get_block(reth_endpoint, block_number)
        .context(format!("Failed to fetch block for {block_number}"))?;
    let witness = rpc::get_witness(reth_endpoint, block_number)
        .context(format!("Failed to fetch witness for {block_number}"))?
        .result;
    let mut headers: Vec<Header> = witness
        .headers
        .iter()
        .map(|el| alloy_rlp::decode_exact(&el[..]).expect("must decode headers from witness"))
        .collect();
    assert!(!headers.is_empty());
    assert!(headers.is_sorted_by(|a, b| { a.number < b.number }));
    headers.reverse();

    assert_eq!(headers[0].number, block_number - 1);
    let mut block_hashes: Vec<U256> = headers
        .iter()
        .map(|el| U256::from_be_bytes(el.hash_slow().0))
        .collect();
    block_hashes.resize(256, U256::ZERO); // those will not be accessed

    info!("Running block: {block_number}");
    info!("Block gas used: {}", block.result.header.gas_used);
    let miner = block.result.header.beneficiary;

    let header = block.result.header.clone().into();
    let withdrawals = block
        .result
        .withdrawals
        .clone()
        .map(|el| el.0)
        .unwrap_or_default();
    let withdrawals_encoding = if let Some(withdrawals) = block.result.withdrawals.clone() {
        let mut buff = vec![];
        withdrawals.encode(&mut buff);

        buff
    } else {
        Vec::new()
    };
    let transactions = block.get_all_raw_transactions();

    let chain = Chain::empty(Some(ETH_CHAIN_ID));
    eth_run(
        chain,
        header,
        block_number,
        transactions,
        block_hashes,
        witness,
        withdrawals_encoding,
    )
}

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const CONFIRMATIONS: u64 = 2;

pub fn ethproofs_live_run(reth_endpoint: &str) -> anyhow::Result<()> {
    let mut next = rpc::get_block_number(reth_endpoint)?.saturating_sub(CONFIRMATIONS);

    ethproofs_run(next, reth_endpoint)?;

    loop {
        let head = rpc::get_block_number(reth_endpoint)?.saturating_sub(CONFIRMATIONS);
        if head > next {
            for n in (next + 1)..=head {
                ethproofs_run(n, reth_endpoint)?;
            }
            next = head;
        } else {
            sleep(POLL_INTERVAL);
        }
    }
}
