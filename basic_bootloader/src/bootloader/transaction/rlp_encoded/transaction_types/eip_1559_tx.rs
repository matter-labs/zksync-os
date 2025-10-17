use crate::bootloader::errors::InvalidTransaction;
use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::{Rlp, RlpListDecode};
use crate::bootloader::transaction::rlp_encoded::transaction_types::eip_2930_tx::AccessList;
use crate::bootloader::transaction::rlp_encoded::transaction_types::EthereumTxType;
use ruint::aliases::U256;

/// EIP-1559 (type 0x02) transaction payload (unsigned part).
///
/// This mirrors the RLP list layout defined by EIP-1559:
/// `[chainId, nonce, maxPriorityFeePerGas, maxFeePerGas, gasLimit, to, value, data, accessList]`.
///
#[derive(Clone, Copy, Debug)]
pub(crate) struct EIP1559Tx<'a> {
    pub(crate) chain_id: u64,
    pub(crate) nonce: u64,
    pub(crate) max_priority_fee_per_gas: U256,
    pub(crate) max_fee_per_gas: U256,
    pub(crate) gas_limit: u64,
    pub(crate) to: &'a [u8], // NOTE: it may be empty for deployments
    pub(crate) value: U256,
    pub(crate) data: &'a [u8],
    pub(crate) access_list: AccessList<'a>,
}

impl<'a> EthereumTxType for EIP1559Tx<'a> {
    const TX_TYPE: u8 = 2;
}

impl<'a> RlpListDecode<'a> for EIP1559Tx<'a> {
    /// Decode the 9-field EIP-1559 list body:
    /// [chainId, nonce, maxPriorityFeePerGas, maxFeePerGas, gas, to, value, data, accessList]
    fn decode_list_body(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        let chain_id = r.u64()?;
        let nonce = r.u64()?;
        let max_priority_fee_per_gas = r.u256()?;
        let max_fee_per_gas = r.u256()?;
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
            max_priority_fee_per_gas,
            max_fee_per_gas,
            gas_limit,
            to,
            value,
            data,
            access_list,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::RlpListDecode;

    // Alloy imports
    use alloy::consensus::TxEip1559;
    use alloy::eips::eip2930::AccessList;
    use alloy_primitives::{address, Address, Bytes, TxKind, U256};
    use alloy_rlp::Encodable;

    use ruint::aliases::U256 as RuintU256;

    fn alloy_eip1559_payload_transfer(
        chain_id: u64,
        nonce: u64,
        max_priority: u128,
        max_fee: u128,
        gas_limit: u64,
        to_addr: Address,
        value: u128,
        data: Bytes,
    ) -> Vec<u8> {
        let tx = TxEip1559 {
            chain_id,
            nonce,
            gas_limit,
            max_fee_per_gas: max_fee,
            max_priority_fee_per_gas: max_priority,
            to: TxKind::Call(to_addr),
            value: U256::from(value),
            access_list: AccessList::default(),
            input: data,
        };
        let mut out: Vec<u8> = vec![];
        tx.encode(&mut out);
        out
    }

    fn alloy_eip1559_payload_create(
        chain_id: u64,
        nonce: u64,
        max_priority: u128,
        max_fee: u128,
        gas_limit: u64,
        value: u128,
        initcode: Bytes,
    ) -> Vec<u8> {
        let tx = TxEip1559 {
            chain_id,
            nonce,
            gas_limit,
            max_fee_per_gas: max_fee,
            max_priority_fee_per_gas: max_priority,
            to: TxKind::Create,
            value: U256::from(value),
            access_list: AccessList::default(),
            input: initcode,
        };
        let mut out: Vec<u8> = vec![];
        tx.encode(&mut out);
        out
    }

    #[test]
    fn parses_eip1559_transfer_from_alloy_payload() {
        let to = address!("0x1111111111111111111111111111111111111111");
        let value = 12345u128;
        let data = Bytes::new();

        let bytes = alloy_eip1559_payload_transfer(
            1,             // chain_id
            7,             // nonce
            1_500_000_000, // max_priority_fee_per_gas
            2_000_000_000, // max_fee_per_gas
            21_000,        // gas_limit
            to,
            value,
            data.clone(),
        );

        println!("bytes ={}", hex::encode(&bytes));

        let tx: EIP1559Tx = RlpListDecode::decode_list_full(&bytes).expect("parse should succeed");

        assert_eq!(tx.chain_id, 1);
        assert_eq!(tx.nonce, 7);
        assert_eq!(tx.gas_limit, 21_000);

        assert_eq!(
            tx.max_priority_fee_per_gas,
            RuintU256::from(1_500_000_000u128)
        );
        assert_eq!(tx.max_fee_per_gas, RuintU256::from(2_000_000_000u128));

        assert_eq!(tx.to.len(), 20);
        assert_eq!(tx.to, to.as_slice());

        assert_eq!(tx.value, RuintU256::from(value));
        assert_eq!(tx.data, &*data);
    }

    #[test]
    fn parses_eip1559_create_from_alloy_payload() {
        let initcode = Bytes::from(vec![0x60, 0x60, 0x60, 0x40, 0x52]);

        let bytes = alloy_eip1559_payload_create(
            1,             // chain_id
            0,             // nonce
            1_000_000_000, // max_priority_fee_per_gas
            2_000_000_000, // max_fee_per_gas
            1_000_000,     // gas_limit
            0,             // value
            initcode.clone(),
        );

        let tx: EIP1559Tx = RlpListDecode::decode_list_full(&bytes).expect("parse should succeed");

        assert_eq!(tx.to.len(), 0);

        assert_eq!(tx.data, &*initcode);
    }

    /// Attack vector example
    #[test]
    fn handles_invalid_data_encoding() {
        use alloy_rlp::{BufMut, Encodable};

        fn build_tx_manually(data: Vec<u8>) -> Vec<u8> {
            // Manually construct an EIP1559 transaction with invalid data encoding
            let mut buf = Vec::new();

            // EIP1559 transaction list with 9 fields:
            // [chainId, nonce, maxPriorityFeePerGas, maxFeePerGas, gasLimit, to, value, data, accessList]

            // Start RLP list with 9 items
            let list_header = 0xc0 + 9; // Short list with 9 items
            buf.put_u8(list_header);

            // chainId: 1
            1u64.encode(&mut buf);

            // nonce: 0
            0u64.encode(&mut buf);

            // maxPriorityFeePerGas: 1000000000
            1_000_000_000u128.encode(&mut buf);

            // maxFeePerGas: 2000000000
            2_000_000_000u128.encode(&mut buf);

            // gasLimit: 21000
            21_000u64.encode(&mut buf);

            // to: empty (contract creation)
            buf.put_u8(0x80); // empty string

            // value: 0
            0u128.encode(&mut buf);

            buf.extend_from_slice(&data);

            // accessList: empty
            let empty_access_list: Vec<u8> = Vec::new();
            empty_access_list.encode(&mut buf);

            // Now recalculate the correct list header length
            let payload_len = buf.len() - 1; // subtract the temporary header
            buf[0] = if payload_len < 56 {
                0xc0 + payload_len as u8
            } else {
                // For longer lists, we'd need more complex encoding
                panic!("Payload too long for this test");
            };

            buf
        }

        let test_cases = [
            // Single byte encoded as short string (non-minimal)
            vec![0x81, 0x00], // Should be just [0x00]
            vec![0x81, 0x01], // Should be just [0x01]
            vec![0x81, 0x7f], // Should be just [0x7f]
            // Short string that could be single byte
            vec![0x81, 0x80], // This is actually correct encoding for byte 0x80
            // Zero-length long string (should use short form 0x80)
            vec![0xb8, 0x00], // Should be just [0x80]
            // One-byte string with long encoding (should use short form)
            vec![0xb8, 0x01, 0x41], // Should be [0x81, 0x41]
            vec![0x80],             // empty string,
        ];

        for case in test_cases {
            // data: INVALID encoding - non-minimal encodings
            let buf = build_tx_manually(case);

            // Test our parser
            let our_result = EIP1559Tx::decode_list_full(&buf);

            // Test Alloy's parser
            let alloy_result: Result<alloy::consensus::TxEip1559, alloy_rlp::Error> =
                alloy_rlp::Decodable::decode(&mut &buf[..]);

            // Both parsers should handle the invalid data encoding consistently

            match (our_result, alloy_result) {
                (Ok(_), Ok(_)) => {}
                (Err(_), Err(_)) => {}
                (our, alloy) => {
                    panic!("Divergence. Alloy: {:?}, Our: {:?}", alloy, our);
                }
            }
        }
    }
}
