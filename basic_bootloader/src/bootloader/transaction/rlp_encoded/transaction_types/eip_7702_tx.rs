use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::{
    HomList, Rlp, RlpListDecode,
};
use crate::bootloader::transaction::rlp_encoded::transaction_types::EthereumTxType;
use crate::bootloader::{
    errors::InvalidTransaction,
    transaction::rlp_encoded::transaction_types::eip_2930_tx::AccessList,
};
use ruint::aliases::U256;

/// Authorization entry (EIP-7702 style) encoded as a list:
/// [chain_id, address(20), nonce, y_parity, r, s]
#[derive(Clone, Copy, Debug)]
pub struct AuthorizationEntry<'a> {
    pub chain_id: U256,
    pub address: &'a [u8; 20], // NOTE: Can not be empty
    pub nonce: u64,
    pub y_parity: u8, // not bool
    pub r: &'a [u8],  // not fixed size
    pub s: &'a [u8],  // not fixed size
}

impl<'a> RlpListDecode<'a> for AuthorizationEntry<'a> {
    fn decode_list_body(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        let chain_id = r.u256()?;
        let addr_bytes = r.bytes()?;
        if addr_bytes.len() != 20 {
            return Err(InvalidTransaction::InvalidStructure);
        }
        let address: &'a [u8; 20] = addr_bytes
            .try_into()
            .map_err(|_| InvalidTransaction::InvalidStructure)?;
        let nonce = r.u64()?;
        let y_parity = r.u8()?;
        let r_bytes = r.bytes()?;
        let s_bytes = r.bytes()?;
        Ok(Self {
            chain_id,
            address,
            nonce,
            y_parity,
            r: r_bytes,
            s: s_bytes,
        })
    }
}

pub type AuthorizationList<'a> = HomList<'a, AuthorizationEntry<'a>, true>;

// EIP-7702 payload (type 0x04) used for signing.
// Layout:
// [chainId, nonce, maxPriorityFeePerGas, maxFeePerGas, gasLimit,
//  to(20 bytes, not empty), value, data, accessList, authorizationList]
#[derive(Clone, Copy, Debug)]
pub(crate) struct EIP7702Tx<'a> {
    pub(crate) chain_id: u64,
    pub(crate) nonce: u64,
    pub(crate) max_priority_fee_per_gas: U256,
    pub(crate) max_fee_per_gas: U256,
    pub(crate) gas_limit: u64,
    pub(crate) to: &'a [u8; 20], // NOTE: Can not be empty
    pub(crate) value: U256,
    pub(crate) data: &'a [u8],
    pub(crate) access_list: AccessList<'a>,
    pub(crate) authorization_list: AuthorizationList<'a>,
}

impl<'a> EthereumTxType for EIP7702Tx<'a> {
    const TX_TYPE: u8 = 4;
}

// If you don't already have it:
// pub type AuthorizationList<'a> = HomList<'a, AuthorizationEntry<'a>, true>;

impl<'a> RlpListDecode<'a> for EIP7702Tx<'a> {
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

        if to.iter().all(|&b| b == 0) {
            // Deployment transactions are not allowed in EIP-7702
            return Err(InvalidTransaction::EIP7702HasNullDestination);
        }

        let value = r.u256()?;
        let data = r.bytes()?;
        let access_list = AccessList::decode_list_from(r)?;
        let authorization_list = AuthorizationList::decode_list_from(r)?;

        if authorization_list.count == Some(0) {
            return Err(InvalidTransaction::AuthListIsEmpty);
        }
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
            authorization_list,
        })
    }
}

// tests for AuthorizationList parsing + EIP-7702 payloads using Alloy types
// place this next to your EIP-7702/authorization-list implementation

#[cfg(test)]
mod test {
    use super::*;
    use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::RlpListDecode;
    use crate::bootloader::transaction::rlp_encoded::rlp::test_helpers::*;

    // Alloy imports
    use alloy::consensus::TxEip7702;
    use alloy::eips::eip2930::{AccessList, AccessListItem};
    use alloy::eips::eip7702::Authorization;
    use alloy::rpc::types::SignedAuthorization;
    use alloy_primitives::{
        address, b256, Address, Bytes, FixedBytes, Signature, U256 as AlloyU256,
    };
    use alloy_rlp::{encode, Encodable};
    use ruint::aliases::U256 as RuintU256;

    fn signed_auth_from_rs(
        chain_id: u64,
        address: Address,
        nonce: u64,
        y_parity: u8, // 0/1
        r_be: &[u8],  // big-endian, variable-length
        s_be: &[u8],  // big-endian, variable-length
    ) -> SignedAuthorization {
        let auth = Authorization {
            chain_id: AlloyU256::from(chain_id),
            address,
            nonce,
        };

        let mut r = [0u8; 32];
        let mut s = [0u8; 32];

        let rl = r_be.len().min(32);
        let sl = s_be.len().min(32);

        r[32 - rl..].copy_from_slice(&r_be[r_be.len() - rl..]);
        s[32 - sl..].copy_from_slice(&s_be[s_be.len() - sl..]);

        let sig = Signature::from_scalars_and_parity(
            FixedBytes::<32>::from(r),
            FixedBytes::<32>::from(s),
            y_parity != 0,
        );
        auth.into_signed(sig)
    }

    fn encode_auth_list(all: Vec<SignedAuthorization>) -> Vec<u8> {
        encode(all)
    }

    fn encode_eip7702_payload(
        chain_id: u64,
        nonce: u64,
        max_priority: u128,
        max_fee: u128,
        gas_limit: u64,
        to: Address,
        value: u128,
        data: &[u8],
        access_list: AccessList,
        auth_list: Vec<SignedAuthorization>,
    ) -> Vec<u8> {
        let tx = TxEip7702 {
            chain_id,
            nonce,
            gas_limit,
            max_priority_fee_per_gas: max_priority,
            max_fee_per_gas: max_fee,
            to,
            value: AlloyU256::from(value),
            input: Bytes::copy_from_slice(data),
            access_list,
            authorization_list: auth_list,
        };
        let mut out = Vec::new();
        tx.encode(&mut out);
        out
    }

    #[test]
    fn authorization_list_empty() {
        let bytes = encode_auth_list(vec![]);
        let al: AuthorizationList =
            AuthorizationList::decode_list_full(&bytes).expect("empty list should parse");
        assert_eq!(al.count, Some(0));
        assert!(al.iter().next().is_none());
    }

    #[test]
    fn authorization_list_single_entry() {
        let addr = address!("0x1111111111111111111111111111111111111111");
        // r = 0x0102, s = 0x03, parity = 1
        let signed = signed_auth_from_rs(1, addr, 5, 1, &[0x01, 0x02], &[0x03]);
        let bytes = encode_auth_list(vec![signed]);

        let al: AuthorizationList =
            AuthorizationList::decode_list_full(&bytes).expect("should parse");
        assert_eq!(al.count, Some(1));

        let mut it = al.iter();
        let e = it.next().unwrap();

        assert_eq!(e.chain_id, RuintU256::from(1u64));
        assert_eq!(e.nonce, 5);
        assert_eq!(e.y_parity, 1);
        assert_eq!(&e.r, &[0x01, 0x02]); // Alloy trims leading zeros in RLP
        assert_eq!(&e.s, &[0x03]);
        assert_eq!(&e.address[..], addr.as_slice());
        assert!(it.next().is_none());
    }

    #[test]
    fn authorization_list_two_entries_mixed_lengths() {
        let a0 = address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let a1 = address!("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");

        // First auth: r = 0 (empty in RLP), s = 32 bytes of 0xFF, y = 0
        let e0 = signed_auth_from_rs(9, a0, 0, 0, &[], &[0xFF; 32]);
        // Second: r = 0x01, s = 0x020304, y = 1
        let e1 = signed_auth_from_rs(9, a1, 1, 1, &[0x01], &[0x02, 0x03, 0x04]);

        let bytes = encode_auth_list(vec![e0, e1]);
        let al: AuthorizationList =
            AuthorizationList::decode_list_full(&bytes).expect("should parse");

        assert_eq!(al.count, Some(2));
        let mut it = al.iter();

        let x0 = it.next().unwrap();
        assert_eq!(x0.chain_id, RuintU256::from(9u64));
        assert_eq!(&x0.address[..], a0.as_slice());
        assert_eq!(x0.y_parity, 0);
        assert_eq!(x0.r.len(), 0); // zero -> empty RLP
        assert_eq!(x0.s.len(), 32);

        let x1 = it.next().unwrap();
        assert_eq!(&x1.address[..], a1.as_slice());
        assert_eq!(x1.y_parity, 1);
        assert_eq!(&x1.r, &[0x01]);
        assert_eq!(&x1.s, &[0x02, 0x03, 0x04]);

        assert!(it.next().is_none());
    }

    #[test]
    fn authorization_list_invalid_address_len_fails() {
        // Deliberately craft an invalid entry with 19-byte address.
        // Alloy types won't let us build this, so we encode by hand.
        let addr_19 = vec![0xAA; 19];
        let entry = rlp_list(&[
            rlp_uint(1),
            rlp_bytes(&addr_19), // invalid (should be 20)
            rlp_uint(0),
            vec![0x01],
            rlp_bytes(&[0x11]),
            rlp_bytes(&[0x22]),
        ]);
        let bytes = rlp_list(&[entry]);
        let res: Result<AuthorizationList, _> = AuthorizationList::decode_list_full(&bytes);
        assert!(res.is_err());
    }

    #[test]
    fn parses_eip7702_transfer_with_nonempty_lists() {
        let to = address!("0x1234567890abcdef1234567890abcdef12345678");

        // Access list with one address and one slot
        let al_item = AccessListItem {
            address: address!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            storage_keys: vec![b256!(
                "0x1111111111111111111111111111111111111111111111111111111111111111"
            )],
        };
        let access_list = AccessList(vec![al_item]);

        // Authorization list with two entries
        let auth0 = signed_auth_from_rs(
            1,
            address!("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            0,
            0,
            &[],
            &[0xEE; 32],
        );
        let auth1 = signed_auth_from_rs(
            1,
            address!("0xcccccccccccccccccccccccccccccccccccccccc"),
            1,
            1,
            &[0x01, 0x02],
            &[0x03, 0x04],
        );
        let auth_list = vec![auth0, auth1];

        let bytes = encode_eip7702_payload(
            10,      // chain_id
            99,      // nonce
            5,       // maxPriorityFeePerGas
            7,       // maxFeePerGas
            800_000, // gasLimit
            to,
            0, // value
            &[0xAA, 0xBB, 0xCC],
            access_list,
            auth_list,
        );

        let tx: EIP7702Tx = RlpListDecode::decode_list_full(&bytes).expect("parse should succeed");

        assert_eq!(tx.chain_id, 10);
        assert_eq!(tx.nonce, 99);
        assert_eq!(tx.gas_limit, 800_000);
        assert_eq!(tx.max_priority_fee_per_gas, RuintU256::from(5u128));
        assert_eq!(tx.max_fee_per_gas, RuintU256::from(7u128));
        assert_eq!(tx.to, to.as_slice());
        assert_eq!(tx.data, &[0xAA, 0xBB, 0xCC]);

        // Access list assertions
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

        // Authorization list assertions
        assert_eq!(tx.authorization_list.count, Some(2));
        let mut it = tx.authorization_list.iter();

        let a0 = it.next().unwrap();
        assert_eq!(
            a0.address,
            address!("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").as_slice()
        );
        assert_eq!(a0.y_parity, 0);
        assert_eq!(a0.r.len(), 0);
        assert_eq!(a0.s.len(), 32);

        let a1 = it.next().unwrap();
        assert_eq!(
            a1.address,
            address!("0xcccccccccccccccccccccccccccccccccccccccc").as_slice()
        );
        assert_eq!(a1.y_parity, 1);
        assert_eq!(&a1.r, &[0x01, 0x02]);
        assert_eq!(&a1.s, &[0x03, 0x04]);

        assert!(it.next().is_none());
    }

    #[test]
    fn eip7702_rejects_bad_to_length() {
        // 19-byte to address -> should fail. Must handcraft because Alloy won't create invalid Address.
        let to_bad = vec![0x11u8; 19];
        let access_list = encode(&AccessList::default());
        let auth_list = encode(Vec::<SignedAuthorization>::new());
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
            auth_list,
        ]);

        let res: Result<EIP7702Tx, _> = RlpListDecode::decode_list_full(&bytes);
        assert!(res.is_err());
    }
}
