use crate::bootloader::block_flow::ethereum::{
    rlp_ordering_and_key_for_index, short_digits_from_key, CellEnvelope, ReceiptEncoder,
};
use crate::bootloader::transaction_flow::ethereum::LogsBloom;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use basic_system::system_implementation::ethereum_storage_model::vec_trait::VecCtor;
use basic_system::system_implementation::ethereum_storage_model::{
    BoxInterner, ByteBuffer, EthereumMPT, LazyEncodable, LazyLeafValue, LeafValue,
    MPTInternalCapacities, Path,
};
use core::alloc::Allocator;
use crypto::MiniDigest;
use zk_ee::common_structs::skip_list_quasi_vec::ListVec;
use zk_ee::common_structs::GenericEventContentRef;
use zk_ee::memory::stack_trait::Stack;
use zk_ee::system::{EthereumLikeTypes, IOSubsystemExt, IOTeardown, System};
use zk_ee::utils::Bytes32;

/// Opaque transaction encoding stored as a trie leaf.
///
/// For RLP-encoded txs this is the canonical EIP-2718 envelope as parsed from
/// the oracle. For ABI-encoded txs (ZKsync L1->L2 / upgrade) this is the raw
/// ABI buffer as received from the oracle — a canonical typed-envelope format
/// is TODO; for the draft we just use the bytes as-is.
pub struct RawTxLeaf<A: Allocator> {
    bytes: Vec<u8, A>,
}

impl<A: Allocator> core::fmt::Debug for RawTxLeaf<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RawTxLeaf")
            .field("len", &self.bytes.len())
            .finish()
    }
}

impl<A: Allocator> RawTxLeaf<A> {
    pub fn new(bytes: Vec<u8, A>) -> Self {
        Self { bytes }
    }
}

impl<A: Allocator> LazyEncodable for RawTxLeaf<A> {
    fn encoding_len_and_first_byte(&self) -> (usize, u8) {
        // Transactions are never of length 1 in practice, so the first byte
        // marker (used by the MPT's hash-vs-inline decision) can be a dummy.
        debug_assert!(self.bytes.len() != 1);
        (self.bytes.len(), 0xff)
    }

    fn encode(&self, into: &mut dyn ByteBuffer) {
        into.write_slice(&self.bytes);
    }
}

/// Computed block-header values derived from accumulated per-tx data.
#[derive(Debug)]
pub struct ZkBlockHeaderRoots {
    pub transactions_root: Bytes32,
    pub receipts_root: Bytes32,
    pub block_bloom: LogsBloom,
}

/// ZKsync-specific block data keeper.
#[derive(Debug)]
pub struct ZKBasicBlockDataKeeper<A: Allocator + Clone, EA: TxHashesAccumulator> {
    /// Current transaction number within the block
    pub current_transaction_number: u32,
    /// Rolling Keccak hash of all transaction hashes in execution order
    pub transaction_hashes_accumulator: TransactionsRollingKeccakHasher,
    /// Accumulator for L1->L2 transaction hashes (enforced transactions)
    /// It's generic as it needs to be different for different post-ops(sequencing, proving aggregation, proving batch, etc).
    pub enforced_transaction_hashes_accumulator: EA,
    /// Records the hash of any upgrade transaction (max one per block)
    pub upgrade_tx_recorder: UpgradeTx,
    /// Total gas consumed by all transactions in the block
    pub block_gas_used: u64,
    /// Total pubdata produced by all transactions
    pub block_pubdata_used: u64,
    /// Total native computational resources used by all transactions
    pub block_computational_native_used: u64,
    /// Amount of blob gas used in the block
    pub block_blob_gas_used: u64,
    /// Per-tx data needed to compute receipts trie root + block bloom.
    /// Tuple: (status, cumulative_gas_used, num_events, per_tx_bloom)
    pub per_tx_data: ListVec<(bool, u64, usize, LogsBloom), 32, A>,
    /// Opaque transaction encodings, one per included transaction, ordered by
    /// inclusion. Used as leaves of the transactions trie.
    pub executed_transactions: ListVec<RawTxLeaf<A>, 32, A>,
}

impl<A: Allocator + Clone, EA: TxHashesAccumulator> ZKBasicBlockDataKeeper<A, EA> {
    pub fn new_in(allocator: A) -> Self {
        Self {
            current_transaction_number: 0,
            transaction_hashes_accumulator: TransactionsRollingKeccakHasher::empty(),
            enforced_transaction_hashes_accumulator: EA::empty(),
            upgrade_tx_recorder: UpgradeTx {
                inner: Bytes32::ZERO,
            },
            block_gas_used: 0,
            block_pubdata_used: 0,
            block_computational_native_used: 0,
            block_blob_gas_used: 0,
            per_tx_data: ListVec::new_in(allocator.clone()),
            executed_transactions: ListVec::new_in(allocator),
        }
    }

    /// Record per-tx data after the block-limit check has passed and
    /// `block_gas_used` has been updated to its new cumulative value.
    pub fn record_tx_result<S: EthereumLikeTypes>(
        &mut self,
        system: &System<S>,
        raw_tx_bytes: Vec<u8, A>,
        status: bool,
    ) where
        S::IO: IOSubsystemExt + IOTeardown<S::IOTypes>,
    {
        // Compute per-tx logs bloom from this tx's emitted events.
        let mut bloom = LogsBloom::default();
        let events_it = system.io.events_in_this_tx_iterator();
        let num_events = events_it.len();
        let mut hasher = crypto::sha3::Keccak256::new();
        bloom.mark_events(&mut hasher, events_it);

        self.per_tx_data
            .push((status, self.block_gas_used, num_events, bloom));
        self.executed_transactions.push(RawTxLeaf::new(raw_tx_bytes));
    }

    /// Compute transactions/receipts trie roots and the block-level bloom.
    ///
    /// Mirrors `EthereumBasicTransactionDataKeeper::compute_header_values` but
    /// uses opaque raw-bytes leaves for the transactions trie and consumes the
    /// ZK keeper's per-tx data.
    pub fn compute_header_roots<S: EthereumLikeTypes<Allocator = A>>(
        &self,
        system: &System<S>,
    ) -> ZkBlockHeaderRoots
    where
        S::IO: IOSubsystemExt + IOTeardown<S::IOTypes>,
    {
        let allocator = system.get_allocator();
        let mut hasher = crypto::sha3::Keccak256::new();
        let mut block_bloom = LogsBloom::default();

        // Reorder by RLP-encoded index so we can insert sequentially.
        let mut tmp_map = BTreeMap::new_in(allocator.clone());

        let mut all_events_it = system.io.events_iterator();
        for (tx_number, ((tx_status, cumulative_gas, num_events, bloom), tx_leaf)) in self
            .per_tx_data
            .iter()
            .zip(self.executed_transactions.iter())
            .enumerate()
        {
            let events_it = all_events_it.clone().take(*num_events).map(move |el| {
                debug_assert_eq!(tx_number, el.tx_number as usize);
                GenericEventContentRef {
                    address: el.address,
                    topics: el.topics,
                    data: el.data,
                }
            });
            for _ in 0..*num_events {
                let _ = all_events_it.next().unwrap();
            }

            block_bloom.merge(bloom);

            // Receipt format follows EIP-2718; for ZK we use tx_type=0
            // (legacy-style receipt envelope) because the per-tx data here
            // does not carry the original tx type. TODO: thread the tx type
            // through so L1/upgrade txs can be tagged 0x7e/0x7f.
            let receipt_encoder = ReceiptEncoder::new_from_fields(
                0u8,
                tx_status,
                cumulative_gas,
                bloom,
                events_it,
            );

            let (ordering_key, tx_number_rlp) = rlp_ordering_and_key_for_index(tx_number as u32);
            tmp_map.insert(
                ordering_key,
                (tx_number_rlp, CellEnvelope::new(receipt_encoder), tx_leaf),
            );
        }

        let num_txs = self.current_transaction_number as usize;
        let mut interner = BoxInterner::with_capacity_in(1 << 20, allocator.clone());
        let receipts_mpt_capacity = MPTInternalCapacities::<S::Allocator, VecCtor>::with_capacity_in(
            num_txs,
            allocator.clone(),
        );
        let mut receipts_mpt = EthereumMPT::<_, _, true>::empty_with_preallocated_capacities(
            receipts_mpt_capacity,
            allocator.clone(),
        );
        let transactions_mpt_capacity =
            MPTInternalCapacities::<S::Allocator, VecCtor>::with_capacity_in(
                num_txs,
                allocator.clone(),
            );
        let mut transactions_mpt = EthereumMPT::<_, _, true>::empty_with_preallocated_capacities(
            transactions_mpt_capacity,
            allocator.clone(),
        );

        for (_, ((key, len), receipt, tx_leaf)) in tmp_map.iter() {
            let digits = short_digits_from_key(key);
            let path = Path::new(&digits[..(*len * 2)]);
            let receipt_value = LeafValue::LazyEncodable {
                value: LazyLeafValue::from_value(receipt),
                cached_encoding_len_with_metadata: 0,
            };
            receipts_mpt
                .insert_lazy_value(path, receipt_value, &mut (), &mut interner, &mut hasher)
                .expect("must insert receipt leaf");

            let tx_value = LeafValue::LazyEncodable {
                value: LazyLeafValue::from_value(*tx_leaf),
                cached_encoding_len_with_metadata: 0,
            };
            transactions_mpt
                .insert_lazy_value(path, tx_value, &mut (), &mut interner, &mut hasher)
                .expect("must insert tx leaf");
        }
        receipts_mpt
            .recompute(&mut (), &mut interner, &mut hasher)
            .expect("must compute receipts root");
        let receipts_root = Bytes32::from_array(receipts_mpt.root(&mut hasher));
        transactions_mpt
            .recompute(&mut (), &mut interner, &mut hasher)
            .expect("must compute transactions root");
        let transactions_root = Bytes32::from_array(transactions_mpt.root(&mut hasher));

        ZkBlockHeaderRoots {
            transactions_root,
            receipts_root,
            block_bloom,
        }
    }
}

pub trait TxHashesAccumulator {
    /// Creates empty accumulator.
    fn empty() -> Self;

    /// Adds a new transaction hash to the accumulator.
    fn add_tx_hash(&mut self, tx_hash: &Bytes32);
}

#[derive(Debug)]
pub struct NopTxHashesAccumulator;

impl TxHashesAccumulator for NopTxHashesAccumulator {
    fn empty() -> Self {
        Self
    }

    fn add_tx_hash(&mut self, _tx_hash: &Bytes32) {}
}

impl TxHashesAccumulator for () {
    fn empty() -> Self {}

    fn add_tx_hash(&mut self, _tx_hash: &Bytes32) {}
}

/// Rolling Keccak256 hash accumulator for transaction hashes.
#[derive(Debug)]
pub struct TransactionsRollingKeccakHasher {
    inner: Bytes32,
    hasher: crypto::sha3::Keccak256,
    count: u32,
}

impl TxHashesAccumulator for TransactionsRollingKeccakHasher {
    fn empty() -> Self {
        // keccak256([])
        Self {
            inner: Bytes32::from([
                0xc5, 0xd2, 0x46, 0x01, 0x86, 0xf7, 0x23, 0x3c, 0x92, 0x7e, 0x7d, 0xb2, 0xdc, 0xc7,
                0x03, 0xc0, 0xe5, 0x00, 0xb6, 0x53, 0xca, 0x82, 0x27, 0x3b, 0x7b, 0xfa, 0xd8, 0x04,
                0x5d, 0x85, 0xa4, 0x70,
            ]),
            hasher: crypto::sha3::Keccak256::new(),
            count: 0,
        }
    }

    fn add_tx_hash(&mut self, tx_hash: &Bytes32) {
        self.inner = Bytes32::from_array({
            self.hasher.update(self.inner.as_u8_array_ref());
            self.hasher.update(tx_hash.as_u8_array_ref());
            self.hasher.finalize_reset()
        });
        self.count += 1;
    }
}

impl TransactionsRollingKeccakHasher {
    /// Returns the final accumulated hash value and count.
    pub fn finish(self) -> (Bytes32, u32) {
        (self.inner, self.count)
    }
}

/// Blake2s accumulator for L1->L2 enforced transaction hashes.
///
/// Unlike the rolling hash, this simply concatenates all transaction hashes
/// and produces a final Blake2s hash. Used specifically for L1->L2 transactions
#[derive(Debug)]
pub struct AccumulatingBlake2sTransactionsHasher {
    hasher: crypto::blake2s::Blake2s256,
}

impl TxHashesAccumulator for AccumulatingBlake2sTransactionsHasher {
    fn empty() -> Self {
        Self {
            hasher: crypto::blake2s::Blake2s256::new(),
        }
    }

    fn add_tx_hash(&mut self, tx_hash: &Bytes32) {
        self.hasher.update(tx_hash.as_u8_array_ref());
    }
}

impl AccumulatingBlake2sTransactionsHasher {
    /// Finalizes the Blake2s hash of all accumulated enforced transactions.
    pub fn finish(self) -> Bytes32 {
        Bytes32::from_array(self.hasher.finalize())
    }
}

/// Recorder for system upgrade transactions.
///
/// ZKsync allows at most one upgrade transaction per block. This structure
/// tracks the hash of any upgrade transaction, panicking if multiple upgrade
/// transactions are attempted in the same block.
#[derive(Debug)]
pub struct UpgradeTx {
    inner: Bytes32,
}

impl UpgradeTx {
    /// Records the hash of an upgrade transaction.
    ///
    /// Panics if an upgrade transaction was already recorded for this block.
    /// ZKsync allows at most one upgrade transaction per block.
    pub fn add_upgrade_tx_hash(&mut self, tx_hash: &Bytes32) {
        if self.inner.is_zero() == false {
            panic!("duplicate upgrade tx");
        }
        self.inner = *tx_hash;
    }

    /// Returns the upgrade transaction hash, or zero if no upgrade occurred.
    pub fn finish(self) -> Bytes32 {
        self.inner
    }

    /// Returns if an upgrade transaction has been recorded
    pub fn is_empty(&self) -> bool {
        self.inner.is_zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolling_keccak_count_increases_on_add_tx_hash() {
        let mut hasher = TransactionsRollingKeccakHasher::empty();
        let tx_hash = Bytes32::from_array([1u8; 32]);

        hasher.add_tx_hash(&tx_hash);

        let (_hash, count) = hasher.finish();
        assert_eq!(count, 1);
    }

    #[test]
    fn rolling_keccak_count_tracks_multiple_adds() {
        let mut hasher = TransactionsRollingKeccakHasher::empty();
        let tx_hash_a = Bytes32::from_array([2u8; 32]);
        let tx_hash_b = Bytes32::from_array([3u8; 32]);

        hasher.add_tx_hash(&tx_hash_a);
        hasher.add_tx_hash(&tx_hash_b);

        let (_hash, count) = hasher.finish();
        assert_eq!(count, 2);
    }
}
