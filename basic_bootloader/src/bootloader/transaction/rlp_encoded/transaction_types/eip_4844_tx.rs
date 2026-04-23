use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::{
    FixedList, Rlp, RlpListDecode,
};
use crate::bootloader::transaction::rlp_encoded::transaction_types::EthereumTxType;
use crate::bootloader::{
    errors::InvalidTransaction,
    transaction::rlp_encoded::transaction_types::eip_2930_tx::AccessList,
};
use ruint::aliases::U256;

pub type BlobHashesList<'a> = FixedList<'a, &'a [u8; 32]>;

/// EIP-4844 payload (type 0x03) layout: [chainId, nonce, maxPriorityFeePerGas, maxFeePerGas, gasLimit, to(20 bytes, not zero), value, data, accessList, maxFeePerBlobGas, blobVersionedHashes(32-byte items)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct EIP4844Tx<'a> {
    #[cfg_attr(
        not(feature = "eip-4844"),
        expect(
            dead_code,
            reason = "chain_id is only validated when eip-4844 parsing is enabled"
        )
    )]
    pub(crate) chain_id: u64,
    pub(crate) nonce: u64,
    pub(crate) max_priority_fee_per_gas: U256,
    pub(crate) max_fee_per_gas: U256,
    pub(crate) gas_limit: u64,
    pub(crate) to: &'a [u8; 20], // NOTE: Can not be empty
    pub(crate) value: U256,
    pub(crate) data: &'a [u8],
    pub(crate) access_list: AccessList<'a>,
    pub(crate) max_fee_per_blob_gas: U256,
    pub(crate) blob_versioned_hashes: BlobHashesList<'a>,
}

impl<'a> EthereumTxType for EIP4844Tx<'a> {
    const TX_TYPE: u8 = 3;
}

impl<'a> RlpListDecode<'a> for EIP4844Tx<'a> {
    fn decode_list_body(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        let chain_id = r.u64()?;
        let nonce = r.u64()?;
        let max_priority_fee_per_gas = r.u256()?;
        let max_fee_per_gas = r.u256()?;
        let gas_limit = r.u64()?;
        // to must be exactly 20 bytes
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
        let max_fee_per_blob_gas = r.u256()?;
        let blob_versioned_hashes = BlobHashesList::decode_list_from(r)?;

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
            max_fee_per_blob_gas,
            blob_versioned_hashes,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::RlpListDecode;
    use crate::bootloader::transaction::rlp_encoded::rlp::test_helpers::*;

    // Alloy imports
    use alloy::consensus::TxEip4844;
    use alloy::eips::eip2930::{AccessList, AccessListItem};
    use alloy_primitives::{address, b256, Address, Bytes, FixedBytes, U256 as AlloyU256};
    use alloy_rlp::{encode, Encodable};
    use ruint::aliases::U256 as RuintU256;

    fn encode_blob_hashes_list(items: Vec<FixedBytes<32>>) -> Vec<u8> {
        encode(items)
    }

    fn encode_eip4844_payload(
        chain_id: u64,
        nonce: u64,
        max_priority: u128,
        max_fee: u128,
        gas_limit: u64,
        to: Address,
        value: u128,
        data: &[u8],
        access_list: AccessList,
        max_fee_per_blob_gas: u128,
        blob_hashes: Vec<FixedBytes<32>>,
    ) -> Vec<u8> {
        let tx = TxEip4844 {
            chain_id,
            nonce,
            gas_limit,
            max_priority_fee_per_gas: max_priority,
            max_fee_per_gas: max_fee,
            to,
            value: AlloyU256::from(value),
            input: Bytes::copy_from_slice(data),
            access_list,
            max_fee_per_blob_gas,
            blob_versioned_hashes: blob_hashes,
        };
        let mut out = Vec::new();
        tx.encode(&mut out);
        out
    }

    #[test]
    fn blob_hashes_list_empty() {
        let bytes = encode_blob_hashes_list(vec![]);
        let list: BlobHashesList =
            BlobHashesList::decode_list_full(&bytes).expect("empty list should parse");
        assert_eq!(list.count, 0);
        assert!(list.iter().next().is_none());
    }

    #[test]
    fn blob_hashes_list_two_entries() {
        let h0 = b256!("0x0101010101010101010101010101010101010101010101010101010101010101");
        let h1 = b256!("0x0202020202020202020202020202020202020202020202020202020202020202");

        let bytes = encode_blob_hashes_list(vec![h0, h1]);
        let list: BlobHashesList = BlobHashesList::decode_list_full(&bytes).expect("should parse");

        assert_eq!(list.count, 2);
        let mut it = list.iter();

        let x0 = it.next().unwrap().unwrap();
        assert_eq!(&x0[..], h0.as_slice());

        let x1 = it.next().unwrap().unwrap();
        assert_eq!(&x1[..], h1.as_slice());

        assert!(it.next().is_none());
    }

    #[test]
    fn blob_hashes_list_invalid_element_length_fails() {
        // First element is 31 bytes (invalid), second is 32 bytes.
        let bad = vec![0xAB; 31];
        let good = vec![0xCD; 32];
        let bytes = rlp_list(&[rlp_bytes(&bad), rlp_bytes(&good)]);
        let res: Result<BlobHashesList, _> = BlobHashesList::decode_list_full(&bytes);
        assert!(res.is_err());
    }

    #[test]
    fn parses_eip4844_transfer_with_nonempty_lists() {
        let to = address!("0x1234567890abcdef1234567890abcdef12345678");

        let al_item = AccessListItem {
            address: address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            storage_keys: vec![b256!(
                "0x1111111111111111111111111111111111111111111111111111111111111111"
            )],
        };
        let access_list = AccessList(vec![al_item]);

        let h0 = b256!("0x0101010101010101010101010101010101010101010101010101010101010101");
        let h1 = b256!("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");

        let bytes = encode_eip4844_payload(
            99,      // chainId
            7,       // nonce
            5,       // maxPriorityFeePerGas
            9,       // maxFeePerGas
            250_000, // gasLimit
            to,
            0, // value
            &[0xDE, 0xAD, 0xBE, 0xEF],
            access_list,
            12,           // maxFeePerBlobGas
            vec![h0, h1], // blobVersionedHashes
        );

        let tx: EIP4844Tx = RlpListDecode::decode_list_full(&bytes).expect("parse should succeed");

        assert_eq!(tx.chain_id, 99);
        assert_eq!(tx.nonce, 7);
        assert_eq!(tx.gas_limit, 250_000);
        assert_eq!(tx.max_priority_fee_per_gas, RuintU256::from(5u128));
        assert_eq!(tx.max_fee_per_gas, RuintU256::from(9u128));
        assert_eq!(tx.max_fee_per_blob_gas, RuintU256::from(12u128));
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

        assert_eq!(tx.blob_versioned_hashes.count, 2);
        let mut it = tx.blob_versioned_hashes.iter();
        let bh0 = it.next().unwrap().unwrap();
        let bh1 = it.next().unwrap().unwrap();
        assert_eq!(&bh0[..], h0.as_slice());
        assert_eq!(&bh1[..], h1.as_slice());
        assert!(it.next().is_none());
    }

    #[test]
    fn eip4844_rejects_bad_to_length() {
        let to_bad = vec![0x11u8; 19];
        let access_list = encode(&AccessList::default());
        let blob_hashes = encode::<Vec<FixedBytes<32>>>(vec![]);
        let bytes = rlp_list(&[
            rlp_uint(1),
            rlp_uint(0),
            rlp_uint(1),
            rlp_uint(1),
            rlp_uint(21_000),
            rlp_bytes(&to_bad), // invalid
            rlp_uint(0),
            rlp_bytes(&[]),
            access_list,
            rlp_uint(1),
            blob_hashes,
        ]);
        let res: Result<EIP4844Tx, _> = RlpListDecode::decode_list_full(&bytes);
        assert!(res.is_err());
    }
}
