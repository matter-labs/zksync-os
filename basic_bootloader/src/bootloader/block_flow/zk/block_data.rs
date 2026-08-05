use crypto::MiniDigest;
use zk_ee::utils::Bytes32;

/// ZKsync-specific block data keeper.
#[derive(Debug)]
pub struct ZKBasicBlockDataKeeper<EA: TxHashesAccumulator> {
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
}

impl<EA: TxHashesAccumulator> ZKBasicBlockDataKeeper<EA> {
    pub fn new() -> Self {
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
