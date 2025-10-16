use crate::{types_config::SystemIOTypesConfig, utils::Bytes32};
use ruint::aliases::U256;

/// Block-level metadata required by the bootloader to execute transactions.
pub trait BasicBlockMetadata<IOTypes: SystemIOTypesConfig> {
    /// Identifier of the chain/network.
    fn chain_id(&self) -> u64;

    /// Current block number.
    fn block_number(&self) -> u64;

    /// Hash of a recent historical block at `depth` (0 = parent of the current block).
    fn block_historical_hash(&self, depth: u64) -> Option<Bytes32>;

    /// Block timestamp.
    fn block_timestamp(&self) -> u64;

    /// Block randomness beacon, if available.
    fn block_randomness(&self) -> Option<Bytes32>;

    /// The block beneficiary address.
    fn coinbase(&self) -> IOTypes::Address;

    /// Per-block gas limit for computation.
    fn block_gas_limit(&self) -> u64;

    /// Max gas allowed for an individual transaction’s computation.
    fn individual_tx_gas_limit(&self) -> u64;

    /// Base fee per execution gas unit (EIP-1559 style), if supported.
    fn eip1559_basefee(&self) -> U256;

    /// Maximum number of blobs allowed per block (EIP-4844 style), if supported.
    ///
    /// Return `0` if blobs are unsupported.
    fn max_blobs(&self) -> usize {
        0
    }

    /// Block-level limit for blob gas (EIP-4844), if supported.
    ///
    /// Default is `u64::MAX`.
    fn blobs_gas_limit(&self) -> u64 {
        u64::MAX
    }

    /// Base fee per blob gas*(EIP-4844), if supported.
    ///
    /// Default is `0`.
    fn blob_base_fee_per_gas(&self) -> U256 {
        U256::ZERO
    }
}

/// Transaction-level metadata describing the currently executing transaction.
pub trait BasicTransactionMetadata<IOTypes: SystemIOTypesConfig> {
    /// `tx.origin` / EVM `ORIGIN`—the original externally-owned account that
    /// initiated this transaction.
    fn tx_origin(&self) -> IOTypes::Address;

    /// Price actually used for charging execution gas for this tx.
    fn tx_gas_price(&self) -> U256;

    /// Number of EIP-4844 blobs carried by this transaction (if any).
    fn num_blobs(&self) -> usize {
        0
    }

    /// Hash (commitment) of the `idx`-th blob for this transaction, if present.
    fn get_blob_hash(&self, _idx: usize) -> Option<Bytes32> {
        None
    }
}

/// ZKsync-specific pricing knobs that are *not* standardized by Ethereum.
pub trait ZkSpecificPricingMetadata {
    /// Price of an unit of native resources.
    fn native_price(&self) -> U256;

    /// Upper bound on total pubdata that can be used by the transaction.
    fn get_pubdata_limit(&self) -> u64;

    /// Price in base token of 1 byte of pubdata.
    fn get_pubdata_price(&self) -> U256;
}

/// Convenience super-trait for environments that expose both block- and tx-level
/// metadata, plus a pluggable transaction metadata payload.
pub trait BasicMetadata<IOTypes: SystemIOTypesConfig>:
    BasicBlockMetadata<IOTypes> + BasicTransactionMetadata<IOTypes>
{
    type TransactionMetadata;

    /// Set the metadata for the current transaction.
    fn set_transaction_metadata(&mut self, tx_level_metadata: Self::TransactionMetadata);
}
