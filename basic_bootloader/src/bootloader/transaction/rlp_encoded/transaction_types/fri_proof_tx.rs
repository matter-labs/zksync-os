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

#[cfg(test)]
mod test {
    use super::*;
    use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::RlpListDecode;
    use crate::bootloader::transaction::rlp_encoded::rlp::test_helpers::*;

    use alloy::eips::eip2930::{AccessList as AlloyAccessList, AccessListItem};
    use alloy_primitives::{address, b256, Address, FixedBytes};
    use alloy_rlp::encode;
    use ruint::aliases::U256 as RuintU256;

    /// Full FRI proof tx body encoder. Mirrors the 10-field signed-body
    /// layout (`[chainId, nonce, maxPriorityFeePerGas, maxFeePerGas,
    /// gasLimit, to, value, data, accessList, statementVersionedHashes]`)
    /// using the hand-rolled RLP helpers — no alloy facade exists for
    /// this tx type since it's zksync-os-specific.
    #[allow(clippy::too_many_arguments)]
    fn encode_fri_proof_payload(
        chain_id: u64,
        nonce: u64,
        max_priority: u128,
        max_fee: u128,
        gas_limit: u64,
        to: Address,
        value: u128,
        data: &[u8],
        access_list: AlloyAccessList,
        statement_hashes: Vec<FixedBytes<32>>,
    ) -> Vec<u8> {
        rlp_list(&[
            rlp_uint(chain_id as u128),
            rlp_uint(nonce as u128),
            rlp_uint(max_priority),
            rlp_uint(max_fee),
            rlp_uint(gas_limit as u128),
            rlp_bytes(to.as_slice()),
            rlp_uint(value),
            rlp_bytes(data),
            encode(&access_list),
            encode(&statement_hashes),
        ])
    }

    #[test]
    fn statement_versioned_hashes_list_empty() {
        let bytes = encode::<Vec<FixedBytes<32>>>(vec![]);
        let list: StatementVersionedHashesList =
            StatementVersionedHashesList::decode_list_full(&bytes)
                .expect("empty list should parse");
        assert_eq!(list.count, 0);
        assert!(list.iter().next().is_none());
    }

    #[test]
    fn statement_versioned_hashes_list_two_entries() {
        let h0 = b256!("0x0101010101010101010101010101010101010101010101010101010101010101");
        let h1 = b256!("0x0202020202020202020202020202020202020202020202020202020202020202");

        let bytes = encode::<Vec<FixedBytes<32>>>(vec![h0, h1]);
        let list: StatementVersionedHashesList =
            StatementVersionedHashesList::decode_list_full(&bytes).expect("should parse");

        assert_eq!(list.count, 2);
        let mut it = list.iter();

        let x0 = it.next().unwrap().unwrap();
        assert_eq!(&x0[..], h0.as_slice());

        let x1 = it.next().unwrap().unwrap();
        assert_eq!(&x1[..], h1.as_slice());

        assert!(it.next().is_none());
    }

    #[test]
    fn statement_versioned_hashes_list_invalid_element_length_fails() {
        // Each entry must be exactly 32 bytes. A 31-byte entry must
        // reject the whole list so the tx fails structural validation
        // before any FRI oracle lookup can run.
        let bad = vec![0xAB; 31];
        let good = vec![0xCD; 32];
        let bytes = rlp_list(&[rlp_bytes(&bad), rlp_bytes(&good)]);
        let res: Result<StatementVersionedHashesList, _> =
            StatementVersionedHashesList::decode_list_full(&bytes);
        assert!(res.is_err());
    }

    #[test]
    fn parses_fri_proof_tx_with_nonempty_lists() {
        let to = address!("0x1234567890abcdef1234567890abcdef12345678");

        let al_item = AccessListItem {
            address: address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            storage_keys: vec![b256!(
                "0x1111111111111111111111111111111111111111111111111111111111111111"
            )],
        };
        let access_list = AlloyAccessList(vec![al_item]);

        let h0 = b256!("0x0101010101010101010101010101010101010101010101010101010101010101");
        let h1 = b256!("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");

        let bytes = encode_fri_proof_payload(
            99,      // chainId
            7,       // nonce
            5,       // maxPriorityFeePerGas
            9,       // maxFeePerGas
            250_000, // gasLimit
            to,
            0, // value
            &[0xDE, 0xAD, 0xBE, 0xEF],
            access_list,
            vec![h0, h1], // statementVersionedHashes
        );

        let tx: FriProofTx = RlpListDecode::decode_list_full(&bytes).expect("parse should succeed");

        assert_eq!(tx.chain_id, 99);
        assert_eq!(tx.nonce, 7);
        assert_eq!(tx.gas_limit, 250_000);
        assert_eq!(tx.max_priority_fee_per_gas, RuintU256::from(5u128));
        assert_eq!(tx.max_fee_per_gas, RuintU256::from(9u128));
        assert_eq!(tx.to, to.as_slice());
        assert_eq!(tx.value, RuintU256::from(0u128));
        assert_eq!(tx.data, &[0xDE, 0xAD, 0xBE, 0xEF]);

        assert_eq!(tx.access_list.count, Some(1));
        let first_al = tx.access_list.iter().next().unwrap();
        assert_eq!(
            first_al.address.to_be_bytes(),
            <[u8; 20]>::try_from(address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").as_slice())
                .unwrap()
        );
        assert_eq!(first_al.slots_list.count, 1);
        let mut slots = first_al.slots_list.iter();
        let s0 = slots.next().unwrap().unwrap();
        assert_eq!(s0.len(), 32);
        assert!(slots.next().is_none());

        assert_eq!(tx.statement_versioned_hashes.count, 2);
        let mut it = tx.statement_versioned_hashes.iter();
        let sh0 = it.next().unwrap().unwrap();
        let sh1 = it.next().unwrap().unwrap();
        assert_eq!(&sh0[..], h0.as_slice());
        assert_eq!(&sh1[..], h1.as_slice());
        assert!(it.next().is_none());
    }

    #[test]
    fn fri_proof_tx_rejects_bad_to_length() {
        let to_bad = vec![0x11u8; 19];
        let access_list = encode(&AlloyAccessList::default());
        let statement_hashes = encode::<Vec<FixedBytes<32>>>(vec![]);
        let bytes = rlp_list(&[
            rlp_uint(1),
            rlp_uint(0),
            rlp_uint(1),
            rlp_uint(1),
            rlp_uint(21_000),
            rlp_bytes(&to_bad), // invalid — must be exactly 20 bytes
            rlp_uint(0),
            rlp_bytes(&[]),
            access_list,
            statement_hashes,
        ]);
        let res: Result<FriProofTx, _> = RlpListDecode::decode_list_full(&bytes);
        assert!(res.is_err());
    }
}
