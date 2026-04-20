use crate::bootloader::errors::InvalidTransaction;
use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::{
    FixedList, Rlp, RlpListDecode,
};
use crate::bootloader::transaction::rlp_encoded::transaction_types::eip_2930_tx::AccessList;
use crate::bootloader::transaction::rlp_encoded::transaction_types::EthereumTxType;
use ruint::aliases::U256;

/// List of 32-byte `statement_versioned_hash` entries carried by a
/// `FriProofTx` in its signed body. Each entry commits to one proved
/// statement (not to a specific proof artifact). The raw proof bytes
/// travel out-of-band via the FRI proof sidecar and are resolved at
/// execution time through `FRI_PROOF_QUERY_ID`.
pub type StatementVersionedHashesList<'a> = FixedList<'a, &'a [u8; 32]>;

/// FRI proof transaction (type `FRI_PROOF_TX_TYPE`) — Gateway-only.
///
/// RLP list layout of the signed body (10 fields):
/// `[chainId, nonce, maxPriorityFeePerGas, maxFeePerGas, gasLimit, to,
///   value, data, accessList, statementVersionedHashes]`
///
/// Followed by the standard EIP-2718 signature triplet `(yParity, r, s)`.
///
/// Execution semantics:
/// - System verifies each `statement_versioned_hash` against the sidecar
///   proof bytes before the tx body runs, and records successful
///   verifications in tx-scoped transient state.
/// - After verification, the tx body runs as a normal contract call to
///   `to` with `data`, just like EIP-1559. The called contract may query
///   `FRI_PRECOMPILE_ADDRESS` to ask whether a specific
///   `statement_versioned_hash` was verified earlier in this tx.
///
/// `to` must be exactly 20 bytes — a FRI proof tx cannot deploy a
/// contract, because the whole point is to run against a contract that
/// knows how to consume the FRI precompile result.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FriProofTx<'a> {
    pub(crate) chain_id: u64,
    pub(crate) nonce: u64,
    pub(crate) max_priority_fee_per_gas: U256,
    pub(crate) max_fee_per_gas: U256,
    pub(crate) gas_limit: u64,
    pub(crate) to: &'a [u8; 20],
    pub(crate) value: U256,
    pub(crate) data: &'a [u8],
    pub(crate) access_list: AccessList<'a>,
    pub(crate) statement_versioned_hashes: StatementVersionedHashesList<'a>,
}

/// FRI proof transaction type byte. Chosen as the next unused slot in
/// the zksync-os typed-tx `0x7x` block (below the existing
/// `SERVICE_TX_TYPE = 0x7d`, `UPGRADE_TX_TYPE = 0x7e`,
/// `L1_L2_TX_TYPE = 0x7f`).
pub const FRI_PROOF_TX_TYPE: u8 = 0x7c;

impl<'a> EthereumTxType for FriProofTx<'a> {
    const TX_TYPE: u8 = FRI_PROOF_TX_TYPE;
}

impl<'a> RlpListDecode<'a> for FriProofTx<'a> {
    fn decode_list_body(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        let chain_id = r.u64()?;
        let nonce = r.u64()?;
        let max_priority_fee_per_gas = r.u256()?;
        let max_fee_per_gas = r.u256()?;
        let gas_limit = r.u64()?;

        // `to` must be exactly 20 bytes — FRI proof txs cannot deploy.
        let to_slice = r.bytes()?;
        if to_slice.len() != 20 {
            return Err(InvalidTransaction::InvalidStructure);
        }
        let to: &'a [u8; 20] = to_slice
            .try_into()
            .map_err(|_| InvalidTransaction::InvalidStructure)?;

        let value = r.u256()?;
        let data = r.bytes()?;
        let access_list = AccessList::decode_list_from(r)?;
        let statement_versioned_hashes = StatementVersionedHashesList::decode_list_from(r)?;

        Ok(Self {
            chain_id,
            nonce,
            max_priority_fee_per_gas,
            max_fee_per_gas,
            gas_limit,
            to,
            value,
            data,
            access_list,
            statement_versioned_hashes,
        })
    }
}
