//! Native harness for the Glamsterdam state consistency check
//! (`basic_bootloader::...::ethereum_glamsterdam::state_consistency`) over the
//! stored fixtures: builds the oracle a fixture implies and runs the check.

use crate::glamsterdam::GlamsterdamBlock;
use alloy::consensus::Header;
use alloy_rlp::Decodable;
use anyhow::Context;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::block_header::BlobSchedule;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::linkage::ProgramOutput;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::state_consistency::{
    verify_state_consistency, StateConsistencyOutput,
};
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::oracle_queries::GLAMSTERDAM_TRANSACTION_COUNT_QUERY_ID;
use forward_system::run::query_processors::{
    EthereumBlockAccessListResponder, EthereumCLResponder, EthereumTargetBlockHeaderResponder,
    GenericPreimageResponder, UARTPrintResponder,
};
use forward_system::run::test_impl::InMemoryPreimageSource;
use oracle_provider::{
    respond_to_every_target, OracleQueryProcessor, ReadWitnessSource, RunMode,
    ZkEENonDeterminismSource,
};
use rig::crypto::MiniDigest;
use std::alloc::Global;
use std::path::Path;
use zk_ee::oracle::memory_io::host::{QuerierMemory, WriteQueryOutput};
use zk_ee::utils::Bytes32;

/// Serves the number of transactions of the block, which the check proves
/// against the index tries (their nodes come from the preimage responder)
#[derive(Clone, Debug)]
pub struct TransactionCountResponder {
    pub transaction_count: u32,
}

impl OracleQueryProcessor for TransactionCountResponder {
    fn supported_memory_query_ids(&self) -> Vec<u32> {
        vec![GLAMSTERDAM_TRANSACTION_COUNT_QUERY_ID]
    }

    fn process_memory_query(
        &mut self,
        query_id: u32,
        _input_word: usize,
        _memory: &dyn QuerierMemory,
        mode: RunMode,
        native_run_responses: &mut Vec<u32>,
        guest_run_responses: &mut Vec<u32>,
    ) {
        assert_eq!(query_id, GLAMSTERDAM_TRANSACTION_COUNT_QUERY_ID);
        let mut response = Vec::new();
        self.transaction_count.write_output(&mut response);
        respond_to_every_target(mode, response, native_run_responses, guest_run_responses);
    }
}

/// The check does not account resources, so only a sanity bound on the gas limit applies
const MAX_BLOCK_GAS_LIMIT: u64 = 1 << 32;

/// The RLP encoding of the fixture's block access list, as the node hashes it
pub fn access_list_encoding(fixture: &GlamsterdamBlock) -> Vec<u8> {
    let mut encoding = Vec::new();
    alloy_rlp::encode_list(&fixture.block_access_list, &mut encoding);
    encoding
}

/// The oracle of the state consistency check for a fixture: the target header,
/// the parent headers of the witness, the witness preimages, the nodes of the
/// transactions and receipts tries (the check proves the transaction count
/// against them) and the block access list.
pub fn make_state_oracle(
    fixture: &GlamsterdamBlock,
    mode: RunMode,
) -> anyhow::Result<ZkEENonDeterminismSource> {
    let target_header = Header::decode(&mut fixture.raw_header.as_ref())?;
    let parents: Vec<(Header, Vec<u8>)> = fixture
        .witness
        .headers
        .iter()
        .map(|raw| Ok((Header::decode(&mut raw.as_ref())?, raw.to_vec())))
        .collect::<Result<_, alloy_rlp::Error>>()?;
    let tries = crate::glamsterdam_range::IndexTries::of(fixture)?;
    let preimages = fixture
        .witness
        .state
        .iter()
        .chain(fixture.witness.codes.iter())
        .map(|node| node.to_vec())
        .chain(tries.nodes.values().cloned());
    Ok(make_state_oracle_from_parts(
        (target_header, fixture.raw_header.to_vec()),
        parents,
        preimages,
        access_list_encoding(fixture),
        Vec::new(),
        tries.transactions.len() as u32,
        mode,
    ))
}

/// The oracle of the state consistency check from its parts: the target
/// header with its encoding, the headers of the parent chain (any order), the
/// witness preimages (the index trie nodes among them), the RLP of the block
/// access list, the withdrawals and the transaction count
pub fn make_state_oracle_from_parts(
    (target_header, target_header_encoding): (Header, Vec<u8>),
    mut parents: Vec<(Header, Vec<u8>)>,
    preimages: impl IntoIterator<Item = Vec<u8>>,
    access_list_encoding: Vec<u8>,
    withdrawals_encoding: Vec<u8>,
    transaction_count: u32,
    mode: RunMode,
) -> ZkEENonDeterminismSource {
    // depth 0 is the parent
    parents.sort_by_key(|(header, _)| core::cmp::Reverse(header.number));
    assert_eq!(
        parents.first().map(|(h, _)| h.number + 1),
        Some(target_header.number),
        "the parent header must be given"
    );

    let mut preimage_source = InMemoryPreimageSource::default();
    let mut hasher = rig::crypto::sha3::Keccak256::new();
    for node in preimages {
        hasher.update(&node);
        let hash = hasher.finalize_reset();
        preimage_source
            .inner
            .insert(Bytes32::from_array(*hash), node);
    }

    let (parent_headers_list, parent_headers_encodings_list) = parents.into_iter().unzip();
    let mut oracle = ZkEENonDeterminismSource::new(mode);
    oracle.add_external_processor(EthereumTargetBlockHeaderResponder {
        target_header,
        target_header_encoding,
    });
    oracle.add_external_processor(EthereumCLResponder {
        withdrawals_list: withdrawals_encoding,
        parent_headers_list,
        parent_headers_encodings_list,
    });
    oracle.add_external_processor(GenericPreimageResponder { preimage_source });
    oracle.add_external_processor(EthereumBlockAccessListResponder {
        access_list_encoding,
    });
    oracle.add_external_processor(TransactionCountResponder { transaction_count });
    oracle.add_external_processor(UARTPrintResponder);
    oracle
}

/// Runs the state consistency check natively over a fixture
pub fn run_state_consistency(
    fixture: &GlamsterdamBlock,
) -> anyhow::Result<StateConsistencyOutput<Global>> {
    run_state_consistency_recording(fixture).map(|(output, _)| output)
}

/// Runs the check natively while recording the oracle's responses as the
/// RISC-V guest reads them: the words the `glamsterdam_pre_execution` program
/// runs from in the simulator (`QuasiUARTSource::new_with_reads`)
pub fn run_state_consistency_recording(
    fixture: &GlamsterdamBlock,
) -> anyhow::Result<(StateConsistencyOutput<Global>, Vec<u32>)> {
    let oracle = make_state_oracle(fixture, RunMode::NativeRunSavingForRiscV)?;
    let mut source = ReadWitnessSource::new(oracle);
    let output = verify_state_consistency(
        &mut source,
        &BlobSchedule::BPO2,
        MAX_BLOCK_GAS_LIMIT,
        Global,
    )
    .map_err(|err| anyhow::anyhow!("state consistency check failed: {err:?}"))?;
    let words = source.get_read_items().borrow().clone();
    Ok((output, words))
}

/// One line per fixture: what the check found and how big the initial value tables are
pub fn report(output: &StateConsistencyOutput<Global>) -> String {
    let written = output
        .slots
        .iter()
        .filter(|slot| {
            slot.flags
                & basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::state_consistency::SLOT_WRITTEN
                != 0
        })
        .count();
    let fresh = (0..output.counts.accounts as u32)
        .filter(|index| output.is_fresh(*index))
        .count();
    format!(
        "block {}: state root {:?} OK; {} accounts ({} fresh), {} slots ({} written, {} read), \
         {} distinct slot keys, access list {} bytes; initial_accounts {} bytes, initial_slots {} bytes",
        output.header.base.number,
        output.computed_state_root,
        output.counts.accounts,
        fresh,
        output.counts.slots,
        written,
        output.counts.slots - written,
        output.slot_keys.len(),
        output.access_list_encoding.len(),
        output.initial_accounts_bytes().len(),
        output.initial_slots_bytes().len(),
    )
}

/// Runs the check over one fixture directory, or every one under `path`
pub fn check_path(path: &Path) -> anyhow::Result<usize> {
    let dirs = crate::glamsterdam::fixture_dirs(path)?;
    for dir in &dirs {
        let fixture = GlamsterdamBlock::from_dir(dir)
            .with_context(|| format!("Failed to load fixture {}", dir.display()))?;
        let output = run_state_consistency(&fixture)
            .with_context(|| format!("fixture {}", dir.display()))?;
        println!("{}", report(&output));
    }
    Ok(dirs.len())
}

/// Runs the check on the RISC-V simulator with the `app` program over one
/// fixture directory, or every one under `path`, after checking each fixture
/// natively, and prints the cycles per block
pub fn run_on_simulator(path: &Path, app: &str) -> anyhow::Result<()> {
    let dirs = crate::glamsterdam::fixture_dirs(path)?;
    let program = crate::ethproofs::load_program_for(app)?;
    let mut total = 0u64;
    println!(
        "{:>10} {:>10} {:>9} {:>9} {:>14}",
        "block", "gas", "accounts", "slots", "cycles"
    );
    for dir in &dirs {
        let fixture = GlamsterdamBlock::from_dir(dir)?;
        let output = run_state_consistency(&fixture)
            .with_context(|| format!("fixture {}", dir.display()))?;
        let mut builder = program.transpiler_runner();
        if cfg!(target_arch = "x86_64") {
            builder = builder.with_jit();
        }
        let runner = builder
            .build()
            .context("failed to build the transpiler runner")?;
        let execution = runner
            .run_with_source(make_state_oracle(&fixture, RunMode::RiscVRun)?)
            .context("transpiler execution failed")?;
        anyhow::ensure!(execution.reached_end, "the program did not reach the end");
        // the program publishes the hash of its linkage output, with a clear access mask
        anyhow::ensure!(
            execution.receipt.output == ProgramOutput::of_pre_execution(&output).hash_words(),
            "the program's output is not the hash of the linkage output"
        );
        total += execution.cycles_executed as u64;
        println!(
            "{:>10} {:>10} {:>9} {:>9} {:>14}",
            output.header.base.number,
            output.header.base.gas_used,
            output.counts.accounts,
            output.counts.slots,
            execution.cycles_executed
        );
    }
    println!(
        "{:>10} {:>10} {:>9} {:>9} {:>14}",
        "total", "", "", "", total
    );
    Ok(())
}

/// Profiles the check on the RISC-V simulator over one fixture with the `app`
/// program (built with debug info, see `zksync_os/debug_symbols.toml`) and
/// writes a flamegraph. Returns the executed cycles.
pub fn flamegraph(
    block_dir: &Path,
    app: &str,
    output: &Path,
    sampling_rate: usize,
    inverse: bool,
) -> anyhow::Result<u64> {
    let fixture = GlamsterdamBlock::from_dir(block_dir)?;
    let native = run_state_consistency(&fixture)?;
    let program = crate::ethproofs::load_program_for(app)?;
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let runner = program
        .transpiler_runner()
        .with_flamegraph(airbender_host::FlamegraphConfig {
            output: output.to_path_buf(),
            sampling_rate,
            inverse,
            elf_path: Some(program.app_elf().to_path_buf()),
        })
        .build()
        .context("failed to build the transpiler runner")?;
    let start = std::time::Instant::now();
    let execution = runner
        .run_with_source(make_state_oracle(&fixture, RunMode::RiscVRun)?)
        .context("transpiler execution with profiling failed")?;
    anyhow::ensure!(execution.reached_end, "the program did not reach the end");
    let root: Vec<u8> = execution
        .receipt
        .output
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    anyhow::ensure!(
        root == native.computed_state_root.as_u8_array_ref(),
        "the guest returned a different state root"
    );
    println!(
        "block {} ({} gas, {} accounts, {} slots): {} cycles, profiled in {:?} (1 sample / {} cycles), flamegraph written to {}",
        native.header.base.number,
        native.header.base.gas_used,
        native.counts.accounts,
        native.counts.slots,
        execution.cycles_executed,
        start.elapsed(),
        sampling_rate,
        output.display()
    );
    Ok(execution.cycles_executed as u64)
}

#[cfg(test)]
mod tests {
    //! Artificial blocks over a synthetic pre-state, for cases the Sepolia
    //! fixtures do not exercise.

    use super::*;
    use alloy::consensus::constants::{EMPTY_OMMER_ROOT_HASH, EMPTY_ROOT_HASH, KECCAK_EMPTY};
    use alloy::eips::eip7928::{AccountChanges, BalanceChange, BlockAccessIndex};
    use alloy::primitives::{Address, Bloom, Bytes, B256, B64, U256};
    use alloy_rlp::Encodable;
    use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::GlamsterdamError;
    use basic_system::system_implementation::ethereum_storage_model::reference_trie::{
        build_trie, rlp_integer, rlp_list, rlp_string,
    };
    use std::collections::BTreeMap;

    /// A pre-state account: nonce, balance, no code, no storage
    #[derive(Clone, Copy)]
    struct Account {
        address: Address,
        nonce: u64,
        balance: U256,
    }

    fn keccak(data: &[u8]) -> [u8; 32] {
        *rig::crypto::sha3::Keccak256::digest(data)
    }

    /// The full RLP item of the account's leaf value (the list inside its string)
    fn leaf_item(account: &Account) -> Vec<u8> {
        rlp_string(&rlp_list(&[
            rlp_integer(&account.nonce.to_be_bytes()),
            rlp_integer(&account.balance.to_be_bytes::<32>()),
            rlp_string(EMPTY_ROOT_HASH.as_slice()),
            rlp_string(KECCAK_EMPTY.as_slice()),
        ]))
    }

    /// The accounts trie of `accounts`: its root and every node, by hash
    fn state_trie(accounts: &[Account]) -> ([u8; 32], BTreeMap<Bytes32, Vec<u8>>) {
        let mut store = BTreeMap::new();
        let entries: Vec<([u8; 32], Vec<u8>)> = accounts
            .iter()
            .map(|account| (keccak(account.address.as_slice()), leaf_item(account)))
            .collect();
        let root = build_trie(&entries, &mut store);
        (root, store)
    }

    /// A parent and child header pair whose fee fields are consistent
    fn headers_with_gas_limit(
        parent_state_root: [u8; 32],
        state_root: [u8; 32],
        bal_hash: [u8; 32],
        gas_limit: u64,
    ) -> (Header, Header) {
        let parent = Header {
            parent_hash: B256::repeat_byte(0x11),
            ommers_hash: EMPTY_OMMER_ROOT_HASH,
            beneficiary: Address::repeat_byte(0x22),
            state_root: B256::from(parent_state_root),
            transactions_root: EMPTY_ROOT_HASH,
            receipts_root: EMPTY_ROOT_HASH,
            logs_bloom: Bloom::ZERO,
            difficulty: U256::ZERO,
            number: 1000,
            gas_limit,
            // at the target, so the base fee stays
            gas_used: gas_limit / 2,
            timestamp: 1_700_000_000,
            extra_data: Bytes::new(),
            mix_hash: B256::ZERO,
            nonce: B64::ZERO,
            base_fee_per_gas: Some(7),
            withdrawals_root: Some(EMPTY_ROOT_HASH),
            blob_gas_used: Some(0),
            excess_blob_gas: Some(0),
            parent_beacon_block_root: Some(B256::ZERO),
            requests_hash: Some(B256::ZERO),
            block_access_list_hash: Some(B256::ZERO),
            slot_number: Some(5000),
        };
        let mut parent_encoding = Vec::new();
        parent.encode(&mut parent_encoding);
        let header = Header {
            parent_hash: B256::from(keccak(&parent_encoding)),
            state_root: B256::from(state_root),
            number: parent.number + 1,
            gas_used: 21_000,
            timestamp: parent.timestamp + 12,
            block_access_list_hash: Some(B256::from(bal_hash)),
            slot_number: Some(5001),
            ..parent.clone()
        };
        (parent, header)
    }

    /// Runs the check for a block over `pre` whose access list is `changes`
    /// and whose header claims the state `post`
    fn run(
        pre: &[Account],
        post: &[Account],
        changes: Vec<AccountChanges>,
    ) -> Result<StateConsistencyOutput<Global>, GlamsterdamError> {
        run_with_gas_limit(pre, post, changes, 60_000_000)
    }

    fn run_with_gas_limit(
        pre: &[Account],
        post: &[Account],
        changes: Vec<AccountChanges>,
        gas_limit: u64,
    ) -> Result<StateConsistencyOutput<Global>, GlamsterdamError> {
        let (pre_root, store) = state_trie(pre);
        let (post_root, _) = state_trie(post);
        let mut bal = Vec::new();
        alloy_rlp::encode_list(&changes, &mut bal);
        let (parent, header) = headers_with_gas_limit(pre_root, post_root, keccak(&bal), gas_limit);
        let mut parent_encoding = Vec::new();
        parent.encode(&mut parent_encoding);
        let mut header_encoding = Vec::new();
        header.encode(&mut header_encoding);
        let oracle = make_state_oracle_from_parts(
            (header, header_encoding),
            vec![(parent, parent_encoding)],
            store.into_values(),
            bal,
            Vec::new(),
            0,
            RunMode::NativeRunSavingForRiscV,
        );
        let mut source = ReadWitnessSource::new(oracle);
        verify_state_consistency(
            &mut source,
            &BlobSchedule::BPO2,
            MAX_BLOCK_GAS_LIMIT,
            Global,
        )
    }

    /// An access list with more accounts than the header's gas limit lets a block
    /// touch is rejected before any table is allocated
    #[test]
    fn access_list_bounded_by_the_gas_limit() {
        use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::{
            max_accounts_in_block, SYSTEM_PHASE_ACCOUNTS,
        };
        // the minimum gas limit admits the system accounts and one more
        let gas_limit = 5000;
        assert_eq!(max_accounts_in_block(gas_limit), SYSTEM_PHASE_ACCOUNTS + 1);
        let accounts: Vec<Account> = (1..=SYSTEM_PHASE_ACCOUNTS as u8 + 2)
            .map(|i| Account {
                address: Address::repeat_byte(i),
                nonce: 1,
                balance: U256::from(10u64),
            })
            .collect();
        // every account reads its balance, nothing changes
        let changes: Vec<AccountChanges> = accounts
            .iter()
            .map(|account| account_changes(account.address, vec![]))
            .collect();
        let result = run_with_gas_limit(&accounts, &accounts, changes.clone(), gas_limit);
        assert!(
            matches!(result, Err(GlamsterdamError::Capacity(_))),
            "{:?}",
            result.as_ref().err()
        );
        // one account fewer fits
        let fewer: Vec<Account> = accounts[..accounts.len() - 1].to_vec();
        let changes: Vec<AccountChanges> = changes[..changes.len() - 1].to_vec();
        run_with_gas_limit(&fewer, &fewer, changes, gas_limit).expect("within the bound");
    }

    fn balance_change(index: u64, balance: U256) -> BalanceChange {
        BalanceChange {
            block_access_index: BlockAccessIndex(index),
            post_balance: balance,
        }
    }

    fn account_changes(address: Address, balance_changes: Vec<BalanceChange>) -> AccountChanges {
        AccountChanges {
            address,
            balance_changes,
            ..AccountChanges::default()
        }
    }

    /// EIP-8246 with EIP-6780: a contract created in the transaction on a
    /// prefunded address self-destructs to another beneficiary. At
    /// finalization the account has nonce 0, no code, no storage and a zero
    /// balance, so EIP-161 deletes its leaf. The access list shows only the
    /// balance change to zero (no nonce or code changes for a destroyed
    /// account), and the beneficiary's balance change.
    #[test]
    fn selfdestruct_in_creating_transaction_deletes_prefunded_account() {
        let eth = U256::from(10u64).pow(U256::from(18u64));
        let prefunded = Account {
            address: Address::repeat_byte(0xaa),
            nonce: 0,
            balance: eth * U256::from(5u64),
        };
        let beneficiary = Account {
            address: Address::repeat_byte(0xbb),
            nonce: 3,
            balance: eth,
        };
        let others: Vec<Account> = (1..=6u8)
            .map(|i| Account {
                address: Address::repeat_byte(i),
                nonce: i as u64,
                balance: eth * U256::from(i),
            })
            .collect();
        let mut pre = others.clone();
        pre.extend([prefunded, beneficiary]);
        let mut post = others.clone();
        post.push(Account {
            balance: beneficiary.balance + prefunded.balance,
            ..beneficiary
        });
        let mut changes = vec![
            account_changes(prefunded.address, vec![balance_change(1, U256::ZERO)]),
            account_changes(
                beneficiary.address,
                vec![balance_change(1, beneficiary.balance + prefunded.balance)],
            ),
        ];
        changes.sort_by_key(|c| c.address);

        let output = run(&pre, &post, changes.clone()).expect("the block must be consistent");
        let index = output
            .addresses
            .get(
                &basic_system::system_implementation::ethereum_storage_model::interner::AddressKey(
                    ruint::aliases::B160::from_be_bytes(prefunded.address.0 .0),
                ),
            )
            .expect("the prefunded account is in the access list");
        assert!(!output.is_fresh(index), "the prefunded account had a leaf");
        assert_eq!(
            output.initial_account(index),
            Some(&leaf_item(&prefunded)[2..]),
            "the pre-block leaf is recorded as the trie held it"
        );

        // the same access list is inconsistent with a state that keeps the leaf
        let mut kept = post.clone();
        kept.push(Account {
            balance: U256::ZERO,
            ..prefunded
        });
        assert_eq!(
            run(&pre, &kept, changes).err(),
            Some(GlamsterdamError::StateRootMismatch)
        );
    }

    /// EIP-8246: self-destructing to itself in the creating transaction keeps
    /// the balance, so the account survives as a code-less, nonce 0 leaf with
    /// the prefunded and received value.
    #[test]
    fn selfdestruct_to_itself_keeps_the_balance() {
        let eth = U256::from(10u64).pow(U256::from(18u64));
        let prefunded = Account {
            address: Address::repeat_byte(0xaa),
            nonce: 0,
            balance: eth,
        };
        let funder = Account {
            address: Address::repeat_byte(0xcc),
            nonce: 9,
            balance: eth * U256::from(4u64),
        };
        let pre = [prefunded, funder];
        let post = [
            Account {
                balance: eth * U256::from(3u64),
                ..prefunded
            },
            Account {
                nonce: 10,
                balance: eth * U256::from(2u64),
                ..funder
            },
        ];
        let mut changes = vec![
            account_changes(
                prefunded.address,
                vec![balance_change(1, eth * U256::from(3u64))],
            ),
            AccountChanges {
                nonce_changes: vec![alloy::eips::eip7928::NonceChange {
                    block_access_index: BlockAccessIndex(1),
                    new_nonce: 10,
                }],
                ..account_changes(
                    funder.address,
                    vec![balance_change(1, eth * U256::from(2u64))],
                )
            },
        ];
        changes.sort_by_key(|c| c.address);
        run(&pre, &post, changes).expect("the block must be consistent");
    }
}
