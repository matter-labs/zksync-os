use crate::block::Block;
use crate::block_hashes::BlockHashes;
use crate::calltrace::CallTrace;
use crate::dump_utils::AccountStateDiffs;
use crate::native_model::compute_ratio;
use crate::post_check::{post_check, post_check_ext};
use crate::prestate::{populate_prestate, DiffTrace, PrestateTrace};
use crate::receipts::{BlockReceipts, TransactionReceipt};
use alloy::consensus::Header;
use alloy::eips::eip4844::BlobTransactionSidecarItem;
use alloy_primitives::U256;
use alloy_rlp::Encodable;
use alloy_rpc_types_eth::Withdrawal;
use forward_system::run::output::map_tx_results;
use rig::log::info;
use rig::*;
use std::fs::{self, File};
use std::io::BufReader;

#[allow(clippy::too_many_arguments)]
fn run<const RANDOMIZED: bool>(
    mut chain: Chain<RANDOMIZED>,
    block_context: BlockContext,
    block_number: u64,
    miner: alloy::primitives::Address,
    ps_trace: PrestateTrace,
    transactions: Vec<Vec<u8>>,
    receipts: Vec<TransactionReceipt>,
    diff_trace: DiffTrace,
    calltrace: CallTrace,
    block_hashes: Option<BlockHashes>,
    witness_output_dir: Option<String>,
    withdrawals: &[Withdrawal],
) -> anyhow::Result<()> {
    chain.set_last_block_number(block_number - 1);

    if let Some(block_hashes) = block_hashes {
        chain.set_block_hashes(block_hashes.into_array(block_number))
    }

    let prestate_cache = populate_prestate(&mut chain, ps_trace, &calltrace);

    let output_path = witness_output_dir.map(|dir| {
        let mut suffix = block_number.to_string();
        suffix.push_str("_witness");
        std::path::Path::new(&dir).join(suffix)
    });

    #[cfg(feature = "risc_v_logs")]
    const BIN_NAME: &str = "evm_replay_with_logs";

    #[cfg(not(feature = "risc_v_logs"))]
    const BIN_NAME: &str = "evm_replay";

    let (output, stats) = chain.run_block_with_extra_stats(
        transactions,
        Some(block_context),
        None,
        output_path,
        Some(BIN_NAME.to_string()),
    );

    let _ratio = compute_ratio(stats);

    post_check(
        output,
        receipts,
        diff_trace,
        prestate_cache,
        ruint::aliases::B160::from_be_bytes(miner.into()),
        withdrawals,
    )
    .unwrap();

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn eth_run<const PROOF_ENV: bool>(
    mut chain: Chain<false>,
    header: Header,
    block_number: u64,
    miner: alloy::primitives::Address,
    ps_trace: PrestateTrace,
    transactions: Vec<Vec<u8>>,
    receipts: Vec<TransactionReceipt>,
    diff_trace: DiffTrace,
    calltrace: CallTrace,
    block_hashes: Vec<U256>,
    witness: alloy_rpc_types_debug::ExecutionWitness,
    withdrawals: &[Withdrawal],
    withdrawals_encoding: Vec<u8>,
    account_diffs: Vec<AccountStateDiffs>,
    blobs: Vec<BlobTransactionSidecarItem>,
) -> anyhow::Result<()> {
    chain.set_last_block_number(block_number - 1);

    chain.set_block_hashes(block_hashes.try_into().unwrap());

    let prestate_cache = populate_prestate(&mut chain, ps_trace, &calltrace);

    let witness_output_dir = {
        let mut suffix = block_number.to_string();
        suffix.push_str("_witness");
        std::path::PathBuf::from(&suffix)
    };

    let mut result_keeper = chain.run_eth_block::<PROOF_ENV>(
        transactions,
        witness,
        header,
        withdrawals_encoding,
        Some(witness_output_dir),
        None,
    );

    if PROOF_ENV {
        for el in account_diffs.into_iter() {
            use basic_system::system_implementation::cache_structs::BitsOrd160;
            use ruint::aliases::B160;
            let address = B160::from_be_bytes(el.address.0 .0);
            let Some(output) = result_keeper
                .account_encodings
                .remove(&BitsOrd160::from(address))
            else {
                use crate::single_run::log::error;
                error!(
                    "No account leaf encoding for 0x{}",
                    hex::encode(el.address.0 .0)
                );
                // panic!("No account leaf encoding for {}", &el.address);
                continue;
            };
            if !hex::decode(&el.post_leaf_encoding[2..])
                .unwrap()
                .ends_with(&output)
            {
                use crate::single_run::log::error;
                error!(
                    "Expected leaf encoding for 0x{} is\n{}\nbut output contains\n0x{}",
                    hex::encode(el.address.0 .0),
                    &el.post_leaf_encoding,
                    hex::encode(&output),
                );
                // panic!(
                //     "Expected leaf encoding for 0x{} is\n{}\nbut output contains\n0x{}",
                //     hex::encode(el.address.0.0),
                //     &el.post_leaf_encoding,
                //     hex::encode(&output),
                // );
            } else {
                use crate::single_run::log::info;
                info!("Account leaf data matches for {}", &el.address);
            }
        }
    }

    let tx_results = map_tx_results(&result_keeper);
    let storage_writes = result_keeper
        .storage_writes
        .iter()
        .map(|s| (*s).into())
        .collect();

    post_check_ext(
        tx_results,
        receipts,
        result_keeper.account_diffs,
        storage_writes,
        result_keeper.new_preimages,
        diff_trace,
        prestate_cache,
        ruint::aliases::B160::from_be_bytes(miner.into()),
        withdrawals,
    )
    .unwrap();

    Ok(())
}

pub fn single_run(
    block_dir: String,
    block_hashes: Option<String>,
    randomized: bool,
    witness_output_dir: Option<String>,
    chain_id: Option<u64>,
) -> anyhow::Result<()> {
    use std::path::Path;
    let dir = Path::new(&block_dir);
    let block = fs::read_to_string(dir.join("block.json"))?;
    // TODO: ensure there are no calls to unsupported precompiles
    let calltrace_file = File::open(dir.join("calltrace.json"))?;
    let calltrace_reader = BufReader::new(calltrace_file);
    let receipts = fs::read_to_string(dir.join("receipts.json"))?;
    let ps_file = File::open(dir.join("prestatetrace.json"))?;
    let ps_reader = BufReader::new(ps_file);
    let ps_trace: PrestateTrace = serde_json::from_reader(ps_reader)?;
    let receipts: BlockReceipts = serde_json::from_str(&receipts).expect("valid receipts JSON");
    let diff_file = File::open(dir.join("difftrace.json"))?;
    let diff_reader = BufReader::new(diff_file);
    let diff_trace: DiffTrace = serde_json::from_reader(diff_reader)?;
    let block_hashes: Option<BlockHashes> = block_hashes.map(|path| {
        let hashes = fs::read_to_string(&path).expect("valid block hashes path");
        serde_json::from_str(&hashes).expect("valid block hashes JSON")
    });

    let calltrace: CallTrace = serde_json::from_reader(calltrace_reader)?;
    let block: Block = serde_json::from_str(&block).expect("valid block JSON");
    let block_number = block.result.header.number;
    info!("Running block: {block_number}");
    info!("Block gas used: {}", block.result.header.gas_used);
    // assert!(block.result.header.gas_used <= 11_000_000);
    let miner = block.result.header.beneficiary;

    let block_context = block.get_block_context();
    // let withdrawals = block.result.withdrawals.clone().map(|el| el.0).unwrap_or_default();
    let withdrawals = vec![];
    let (transactions, skipped) = block.get_transactions(&calltrace);

    let receipts = receipts
        .result
        .into_iter()
        .enumerate()
        .filter_map(|(i, x)| if skipped.contains(&i) { None } else { Some(x) })
        .collect();

    let ps_trace = PrestateTrace {
        result: ps_trace
            .result
            .into_iter()
            .enumerate()
            .filter_map(|(i, x)| if skipped.contains(&i) { None } else { Some(x) })
            .collect(),
    };

    let diff_trace = DiffTrace {
        result: diff_trace
            .result
            .into_iter()
            .enumerate()
            .filter_map(|(i, x)| if skipped.contains(&i) { None } else { Some(x) })
            .collect(),
    };

    let calltrace = CallTrace {
        result: calltrace
            .result
            .into_iter()
            .enumerate()
            .filter_map(|(i, x)| if skipped.contains(&i) { None } else { Some(x) })
            .collect(),
    };

    if randomized {
        let chain = Chain::empty_randomized(Some(chain_id.unwrap_or(1)));
        run(
            chain,
            block_context,
            block_number,
            miner,
            ps_trace,
            transactions,
            receipts,
            diff_trace,
            calltrace,
            block_hashes,
            witness_output_dir,
            &withdrawals,
        )
    } else {
        let chain = Chain::empty(Some(1));
        run(
            chain,
            block_context,
            block_number,
            miner,
            ps_trace,
            transactions,
            receipts,
            diff_trace,
            calltrace,
            block_hashes,
            witness_output_dir,
            &withdrawals,
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
struct BlobTransactionQuasiSidecarItem {
    pub kzg_commitment: alloy_primitives::FixedBytes<48>,
    pub kzg_proof: alloy_primitives::FixedBytes<48>,
}

pub fn single_eth_run<const PROOF_ENV: bool>(
    block_dir: String,
    chain_id: Option<u64>,
) -> anyhow::Result<()> {
    use crate::live_run::rpc::JsonResponse;
    use alloy_primitives::U256;

    use std::path::Path;
    let dir = Path::new(&block_dir);
    let block = fs::read_to_string(dir.join("block.json"))?;
    let witness = fs::File::open(dir.join("witness.json"))?;
    // TODO: ensure there are no calls to unsupported precompiles
    let calltrace_file = File::open(dir.join("calltrace.json"))?;
    let calltrace_reader = BufReader::new(calltrace_file);
    let receipts = fs::read_to_string(dir.join("receipts.json"))?;
    let ps_file = File::open(dir.join("prestatetrace.json"))?;
    let ps_reader = BufReader::new(ps_file);
    let ps_trace: PrestateTrace = serde_json::from_reader(ps_reader)?;
    let receipts: BlockReceipts = serde_json::from_str(&receipts).expect("valid receipts JSON");
    let diff_file = File::open(dir.join("difftrace.json"))?;
    let diff_reader = BufReader::new(diff_file);
    let diff_trace: DiffTrace = serde_json::from_reader(diff_reader)?;
    let block_hashes = fs::File::open(dir.join("block_hashes.json"))?;
    let block_hashes: Vec<U256> = serde_json::from_reader(block_hashes)?;

    let rpc_result: JsonResponse<alloy_rpc_types_debug::ExecutionWitness> =
        serde_json::from_reader(witness)?;
    let witness = rpc_result.result;

    let calltrace: CallTrace = serde_json::from_reader(calltrace_reader)?;
    let block: Block = serde_json::from_str(&block).expect("valid block JSON");
    let block_number = block.result.header.number;
    info!("Running block: {block_number}");
    info!("Block gas used: {}", block.result.header.gas_used);
    // assert!(block.result.header.gas_used <= 11_000_000);
    let miner = block.result.header.beneficiary;

    let account_diffs_file = File::open(dir.join("account_diffs.json"))?;
    let account_diffs: Vec<AccountStateDiffs> = serde_json::from_reader(account_diffs_file)?;

    // let blobs_file = File::open(dir.join("blobs.json"))?;
    // let blobs: Vec<BlobTransactionQuasiSidecarItem> = serde_json::from_reader(blobs_file)?;

    // let blobs: Vec<BlobTransactionSidecarItem> = blobs
    //     .into_iter()
    //     .enumerate()
    //     .map(|(idx, el)| BlobTransactionSidecarItem {
    //         index: idx as u64,
    //         blob: Box::default(),
    //         kzg_commitment: el.kzg_commitment,
    //         kzg_proof: el.kzg_proof,
    //     })
    //     .collect();

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
    let (transactions, skipped) = block.get_raw_transactions(&calltrace);
    assert!(skipped.is_empty());

    let receipts: Vec<TransactionReceipt> = receipts
        .result
        .into_iter()
        .enumerate()
        .filter_map(|(i, x)| if skipped.contains(&i) { None } else { Some(x) })
        .collect();

    assert_eq!(receipts.len(), transactions.len());

    let ps_trace = PrestateTrace {
        result: ps_trace
            .result
            .into_iter()
            .enumerate()
            .filter_map(|(i, x)| if skipped.contains(&i) { None } else { Some(x) })
            .collect(),
    };

    let diff_trace = DiffTrace {
        result: diff_trace
            .result
            .into_iter()
            .enumerate()
            .filter_map(|(i, x)| if skipped.contains(&i) { None } else { Some(x) })
            .collect(),
    };

    let calltrace = CallTrace {
        result: calltrace
            .result
            .into_iter()
            .enumerate()
            .filter_map(|(i, x)| if skipped.contains(&i) { None } else { Some(x) })
            .collect(),
    };

    let chain = Chain::empty(chain_id);
    eth_run::<PROOF_ENV>(
        chain,
        header,
        block_number,
        miner,
        ps_trace,
        transactions,
        receipts,
        diff_trace,
        calltrace,
        block_hashes,
        witness,
        &withdrawals,
        withdrawals_encoding,
        account_diffs,
        vec![],
    )
}
