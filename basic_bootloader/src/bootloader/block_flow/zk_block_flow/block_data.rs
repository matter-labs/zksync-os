use crypto::MiniDigest;
use zk_ee::utils::Bytes32;

/// ZKsync-specific block data keeper.
#[derive(Debug)]
pub struct ZKBasicBlockDataKeeper {
    /// Current transaction number within the block
    pub current_transaction_number: u32,
    /// Rolling Keccak hash of all transaction hashes in execution order
    pub transaction_hashes_accumulator: TransactionsRollingKeccakHasher,
    /// Blake2s accumulator for L1->L2 transaction hashes (enforced transactions)
    pub enforced_transaction_hashes_accumulator: AccumulatingBlake2sTransactionsHasher,
    /// Records the hash of any upgrade transaction (max one per block)
    pub upgrade_tx_recorder: UpgradeTx,
    /// Total gas consumed by all transactions in the block
    pub block_gas_used: u64,
    /// Total pubdata produced by all transactions
    pub block_pubdata_used: u64,
    /// Total native computational resources used by all transactions
    pub block_computational_native_used: u64,
}

impl ZKBasicBlockDataKeeper {
    pub fn new() -> Self {
        Self {
            current_transaction_number: 0,
            transaction_hashes_accumulator: TransactionsRollingKeccakHasher {
                inner: Bytes32::ZERO,
                hasher: crypto::sha3::Keccak256::new(),
            },
            enforced_transaction_hashes_accumulator: AccumulatingBlake2sTransactionsHasher {
                hasher: crypto::blake2s::Blake2s256::new(),
            },
            upgrade_tx_recorder: UpgradeTx {
                inner: Bytes32::ZERO,
            },
            block_gas_used: 0,
            block_pubdata_used: 0,
            block_computational_native_used: 0,
        }
    }
}

/// Rolling Keccak256 hash accumulator for transaction hashes.
#[derive(Debug)]
pub struct TransactionsRollingKeccakHasher {
    inner: Bytes32,
    hasher: crypto::sha3::Keccak256,
}

impl TransactionsRollingKeccakHasher {
    /// Adds a new transaction hash to the rolling accumulator.
    ///
    /// First transaction becomes the initial value. Subsequent transactions
    /// are hashed with the current accumulator: hash(accumulator || new_tx_hash).
    pub fn add_tx_hash(&mut self, tx_hash: &Bytes32) {
        if self.inner.is_zero() {
            // First transaction - use its hash directly
            self.inner = *tx_hash;
        } else {
            // Roll the hash: hash(current || new_tx_hash)
            self.inner = Bytes32::from_array({
                self.hasher.update(self.inner.as_u8_array_ref());
                self.hasher.update(tx_hash.as_u8_array_ref());
                self.hasher.finalize_reset()
            });
        }
    }

    /// Returns the final accumulated hash value.
    pub fn finish(self) -> Bytes32 {
        self.inner
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

impl AccumulatingBlake2sTransactionsHasher {
    /// Adds an L1->L2 transaction hash to the Blake2s accumulator.
    ///
    /// All enforced transaction hashes are concatenated and hashed together.
    pub fn add_tx_hash(&mut self, tx_hash: &Bytes32) {
        self.hasher.update(tx_hash.as_u8_array_ref());
    }

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
}
