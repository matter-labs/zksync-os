use crate::bootloader::errors::InvalidTransaction;
use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::{
    FixedList, HomList, Rlp, RlpItemDecode, RlpListDecode,
};
use crate::bootloader::transaction::rlp_encoded::transaction_types::EthereumTxType;
use ruint::aliases::B160;
use ruint::aliases::U256;

#[derive(Clone, Copy, Debug)]
pub(crate) struct EIP2930Tx<'a> {
    pub(crate) chain_id: u64,
    pub(crate) nonce: u64,
    pub(crate) gas_price: U256,
    pub(crate) gas_limit: u64,
    pub(crate) to: &'a [u8], // NOTE: it may be empty for deployments
    pub(crate) value: U256,
    pub(crate) data: &'a [u8],
    pub(crate) access_list: AccessList<'a>,
}

impl<'a> EthereumTxType for EIP2930Tx<'a> {
    const TX_TYPE: u8 = 1;
}

impl<'a> RlpListDecode<'a> for EIP2930Tx<'a> {
    /// Decode the 8-field EIP-2930 list body:
    /// [chainId, nonce, gasPrice, gasLimit, to, value, data, accessList]
    fn decode_list_body(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        let chain_id = r.u64()?;
        let nonce = r.u64()?;
        let gas_price = r.u256()?;
        let gas_limit = r.u64()?;
        let to = {
            let s = r.bytes()?;
            if s.is_empty() || s.len() == 20 {
                s
            } else {
                return Err(InvalidTransaction::InvalidStructure);
            }
        };
        let value = r.u256()?;
        let data = r.bytes()?;
        let access_list = AccessList::decode_list_from(r)?;
        Ok(Self {
            chain_id,
            nonce,
            gas_price,
            gas_limit,
            to,
            value,
            data,
            access_list,
        })
    }
}

pub type StorageSlotsList<'a> = FixedList<'a, &'a [u8; 32]>;

#[derive(Clone, Copy, Debug)]
pub struct AccessListForAddress<'a> {
    pub address: B160,
    pub slots_list: StorageSlotsList<'a>,
}

impl<'a> RlpItemDecode<'a> for AccessListForAddress<'a> {
    fn decode_from_item(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        let mut it = r.list()?; // [address, storageKeys]
        let addr = B160::decode_from_item(&mut it)?; // address as string
        let slots = StorageSlotsList::decode_list_from(&mut it)?; // list of 32-byte strings
        if !it.is_empty() {
            return Err(InvalidTransaction::InvalidStructure);
        }
        Ok(Self {
            address: addr,
            slots_list: slots,
        })
    }
}

pub type AccessList<'a> = HomList<'a, AccessListForAddress<'a>, true>;
#[cfg(test)]
mod tests_eip2930 {
    use super::*;
    use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::RlpListDecode;
    use crate::bootloader::transaction::rlp_encoded::rlp::test_helpers::*;

    // Alloy imports
    use alloy::consensus::TxEip2930;
    use alloy::eips::eip2930::{AccessList as AlloyAccessList, AccessListItem};
    use alloy_primitives::{address, b256, Bytes, TxKind, U256};
    use alloy_rlp::Encodable;

    use ruint::aliases::U256 as RuintU256;

    fn encode_alloy_access_list(items: Vec<AccessListItem>) -> Vec<u8> {
        let al = AlloyAccessList(items);
        let mut out = Vec::new();
        al.encode(&mut out);
        out
    }

    // Build an EIP-2930 signing payload:
    // [chainId, nonce, gasPrice, gasLimit, to, value, data, accessList]
    fn alloy_eip2930_payload_with_access_list(
        chain_id: u64,
        nonce: u64,
        gas_price: u128,
        gas_limit: u64,
        to_kind: TxKind,
        value: u128,
        input: Bytes,
        access_list: AlloyAccessList,
    ) -> Vec<u8> {
        let tx = TxEip2930 {
            chain_id,
            nonce,
            gas_price,
            gas_limit,
            to: to_kind,
            value: U256::from(value),
            access_list,
            input,
        };
        let mut out = Vec::new();
        tx.encode(&mut out);
        out
    }

    #[test]
    fn access_list_empty() {
        let bytes = encode_alloy_access_list(vec![]);
        let al = AccessList::decode_list_full(&bytes).expect("should parse empty list");
        assert_eq!(al.count, Some(0));
        assert!(al.iter().next().is_none());
    }

    #[test]
    fn access_list_single_item_no_keys() {
        let addr = address!("0x1111111111111111111111111111111111111111");
        let bytes = encode_alloy_access_list(vec![AccessListItem {
            address: addr,
            storage_keys: vec![],
        }]);

        let al = AccessList::decode_list_full(&bytes).expect("should parse");
        assert_eq!(al.count, Some(1));
        let mut it = al.iter();
        let item = it.next().unwrap();
        assert_eq!(
            item.address.to_be_bytes(),
            <[u8; 20]>::try_from(addr.as_slice()).unwrap()
        );
        assert_eq!(item.slots_list.count, 0);
        assert!(item.slots_list.iter().next().is_none());
        assert!(it.next().is_none());
    }

    #[test]
    fn access_list_single_item_two_keys() {
        use alloy_primitives::B256;
        let addr = address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let k0: B256 = b256!("0x1111111111111111111111111111111111111111111111111111111111111111");
        let k1: B256 = b256!("0x2222222222222222222222222222222222222222222222222222222222222222");
        let bytes = encode_alloy_access_list(vec![AccessListItem {
            address: addr,
            storage_keys: vec![k0, k1],
        }]);

        let al = AccessList::decode_list_full(&bytes).expect("should parse");
        let item = al.iter().next().unwrap();
        assert_eq!(
            item.address.to_be_bytes(),
            <[u8; 20]>::try_from(addr.as_slice()).unwrap()
        );
        assert_eq!(item.slots_list.count, 2);

        let mut slots = item.slots_list.iter();
        let s0 = slots.next().unwrap().unwrap();
        let s1 = slots.next().unwrap().unwrap();
        assert_eq!(s0.len(), 32);
        assert_eq!(s1.len(), 32);
        assert_eq!(&s0[0..2], &[0x11, 0x11]);
        assert_eq!(&s1[0..2], &[0x22, 0x22]);
        assert!(slots.next().is_none());
    }

    #[test]
    fn access_list_two_items_mixed() {
        use alloy_primitives::B256;
        let a1 = address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let a2 = address!("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        let k: B256 = b256!("0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");

        let bytes = encode_alloy_access_list(vec![
            AccessListItem {
                address: a1,
                storage_keys: vec![k],
            },
            AccessListItem {
                address: a2,
                storage_keys: vec![],
            },
        ]);

        let al = AccessList::decode_list_full(&bytes).expect("should parse");
        assert_eq!(al.count, Some(2));

        let mut it = al.iter();

        let first = it.next().unwrap();
        assert_eq!(
            first.address.to_be_bytes(),
            <[u8; 20]>::try_from(a1.as_slice()).unwrap()
        );
        assert_eq!(first.slots_list.count, 1);
        let mut slots = first.slots_list.iter();
        let s = slots.next().unwrap().unwrap();
        assert_eq!(s.len(), 32);
        assert!(slots.next().is_none());

        let second = it.next().unwrap();
        assert_eq!(
            second.address.to_be_bytes(),
            <[u8; 20]>::try_from(a2.as_slice()).unwrap()
        );
        assert_eq!(second.slots_list.count, 0);
        assert!(second.slots_list.iter().next().is_none());

        assert!(it.next().is_none());
    }

    #[test]
    fn access_list_invalid_address_length_fails() {
        // Manually craft: [[address(19 bytes), []]]
        let addr_19 = vec![0xAA; 19];
        let address_enc = rlp_bytes(&addr_19); // 0x93 + 19 bytes
        let empty_keys = vec![0xC0]; // empty list
        let item = rlp_list(&[address_enc, empty_keys]);
        let outer = rlp_list(&[item]);

        let res = AccessList::decode_list_full(&outer);
        assert!(res.is_err(), "address of length != 20 must fail");
    }

    #[test]
    fn access_list_invalid_storage_key_length_fails() {
        // Manually craft: [[address(20 bytes), [key len 31]]]
        let addr_20 = vec![0xBB; 20];
        let address_enc = {
            let mut v = Vec::with_capacity(1 + 20);
            v.push(0x80 + 20); // 0x94
            v.extend_from_slice(&addr_20);
            v
        };
        let bad_key = rlp_bytes(&[0x11; 31]); // should be 32
        let keys_list = rlp_list(&[bad_key]);
        let item = rlp_list(&[address_enc, keys_list]);
        let outer = rlp_list(&[item]);

        let res = AccessList::decode_list_full(&outer);
        assert!(res.is_err(), "storage key length != 32 must fail");
    }

    #[test]
    fn parses_eip2930_transfer_with_empty_access_list() {
        let to = address!("0x2222222222222222222222222222222222222222");
        let value = 777u128;
        let data = Bytes::from(vec![0x01, 0x02, 0x03]);

        let bytes = alloy_eip2930_payload_with_access_list(
            1,                          // chain_id
            3,                          // nonce
            25_000_000,                 // gasPrice
            50_000,                     // gasLimit
            TxKind::Call(to),           // to
            value,                      // value
            data.clone(),               // data
            AlloyAccessList::default(), // empty access list
        );

        let tx: EIP2930Tx = RlpListDecode::decode_list_full(&bytes).expect("parse should succeed");

        assert_eq!(tx.chain_id, 1);
        assert_eq!(tx.nonce, 3);
        assert_eq!(tx.gas_limit, 50_000);
        assert_eq!(tx.gas_price, RuintU256::from(25_000_000u128));

        assert_eq!(tx.to.len(), 20);
        assert_eq!(tx.to, to.as_slice());
        assert_eq!(tx.value, RuintU256::from(value));
        assert_eq!(tx.data, &*data);

        // Access list should be empty. With VALIDATE=true, count is Some(len).
        assert_eq!(tx.access_list.count, Some(0));
        assert!(tx.access_list.iter().next().is_none());
    }

    #[test]
    fn parses_eip2930_transfer_with_nonempty_access_list() {
        let to = address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let value = 0u128;

        // Build a non-empty access list:
        // item 0: address A with two storage keys
        // item 1: address B with zero storage keys
        let addr_a = address!("0x1111111111111111111111111111111111111111");
        let addr_b = address!("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        let al = AlloyAccessList(vec![
            AccessListItem {
                address: addr_a,
                storage_keys: vec![
                    b256!("0x1111111111111111111111111111111111111111111111111111111111111111"),
                    b256!("0x2222222222222222222222222222222222222222222222222222222222222222"),
                ],
            },
            AccessListItem {
                address: addr_b,
                storage_keys: vec![],
            },
        ]);

        let bytes = alloy_eip2930_payload_with_access_list(
            9,                // chain_id
            1,                // nonce
            42_000_000,       // gasPrice
            210_000,          // gasLimit
            TxKind::Call(to), // to
            value,            // value
            Bytes::new(),     // data
            al,               // non-empty access list
        );

        let tx: EIP2930Tx = RlpListDecode::decode_list_full(&bytes).expect("parse should succeed");

        assert_eq!(tx.chain_id, 9);
        assert_eq!(tx.nonce, 1);
        assert_eq!(tx.gas_limit, 210_000);
        assert_eq!(tx.gas_price, RuintU256::from(42_000_000u128));

        // Access list validation
        assert_eq!(tx.access_list.count, Some(2));
        let mut it = tx.access_list.iter();

        let first = it.next().expect("first item");
        assert_eq!(
            first.address.to_be_bytes(),
            <[u8; 20]>::try_from(addr_a.as_slice()).unwrap()
        );
        assert_eq!(first.slots_list.count, 2);
        // Check both storage keys lengths and a couple of prefixes
        let mut slots = first.slots_list.iter();
        let k0 = slots.next().unwrap().unwrap();
        let k1 = slots.next().unwrap().unwrap();
        assert_eq!(k0.len(), 32);
        assert_eq!(k1.len(), 32);
        assert_eq!(&k0[0..2], &[0x11, 0x11]);
        assert_eq!(&k1[0..2], &[0x22, 0x22]);

        let second = it.next().expect("second item");
        assert_eq!(
            second.address.to_be_bytes(),
            <[u8; 20]>::try_from(addr_b.as_slice()).unwrap()
        );
        assert_eq!(second.slots_list.count, 0);
        assert!(second.slots_list.iter().next().is_none());

        assert!(it.next().is_none());
    }

    #[test]
    fn parses_eip2930_create_with_empty_access_list() {
        let initcode = Bytes::from(vec![0x60, 0x60, 0x60, 0x40, 0x52, 0xFE]);

        let bytes = alloy_eip2930_payload_with_access_list(
            1,              // chain_id
            0,              // nonce
            1_000_000,      // gasPrice
            800_000,        // gasLimit
            TxKind::Create, // to = empty
            0,              // value
            initcode.clone(),
            AlloyAccessList::default(), // empty access list
        );

        let tx: EIP2930Tx = RlpListDecode::decode_list_full(&bytes).expect("parse should succeed");

        assert_eq!(tx.chain_id, 1);
        assert_eq!(tx.to.len(), 0, "contract creation must have empty `to`");
        assert_eq!(tx.data, &*initcode);
        assert_eq!(tx.access_list.count, Some(0));
    }
}
