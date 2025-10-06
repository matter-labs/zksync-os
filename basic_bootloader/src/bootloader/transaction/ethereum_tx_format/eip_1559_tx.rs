use crate::bootloader::transaction::ethereum_tx_format::eip_2930_tx::AccessList;
use crate::bootloader::transaction::ethereum_tx_format::minimal_rlp_parser::{Parser, RLPParsable};
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

impl<'a> RLPParsable<'a> for EIP1559Tx<'a> {
    fn try_parse(parser: &mut Parser<'a>) -> Result<Self, ()> {
        let chain_id = RLPParsable::try_parse(parser)?;
        let nonce = RLPParsable::try_parse(parser)?;
        let max_priority_fee_per_gas = RLPParsable::try_parse(parser)?;
        let max_fee_per_gas = RLPParsable::try_parse(parser)?;
        let gas_limit = RLPParsable::try_parse(parser)?;
        let to: &'a [u8] = RLPParsable::try_parse(parser)?;
        if !(to.len() == 0 || to.len() == 20) {
            return Err(());
        }
        let value = RLPParsable::try_parse(parser)?;
        let data = RLPParsable::try_parse(parser)?;
        let access_list = RLPParsable::try_parse(parser)?;

        let new = Self {
            chain_id,
            nonce,
            max_priority_fee_per_gas,
            max_fee_per_gas,
            gas_limit,
            to,
            value,
            data,
            access_list,
        };

        Ok(new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootloader::transaction::ethereum_tx_format::minimal_rlp_parser::{
        Parser, RLPParsable,
    };

    // Alloy imports
    use alloy::consensus::TxEip1559;
    use alloy::eips::eip2930::AccessList;
    use alloy_primitives::{address, Address, Bytes, TxKind, U256};
    use alloy_rlp::Encodable;

    use ruint::aliases::U256 as RuintU256;

    fn alloy_eip1559_signing_payload_transfer(
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

    fn alloy_eip1559_signing_payload_create(
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

        let bytes = alloy_eip1559_signing_payload_transfer(
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

        let mut p = Parser::new(&bytes);
        let tx: EIP1559Tx = RLPParsable::try_parse(&mut p).expect("parse should succeed");

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

        let bytes = alloy_eip1559_signing_payload_create(
            1,             // chain_id
            0,             // nonce
            1_000_000_000, // max_priority_fee_per_gas
            2_000_000_000, // max_fee_per_gas
            1_000_000,     // gas_limit
            0,             // value
            initcode.clone(),
        );

        let mut p = Parser::new(&bytes);
        let tx: EIP1559Tx = RLPParsable::try_parse(&mut p).expect("parse should succeed");

        assert_eq!(tx.to.len(), 0);

        assert_eq!(tx.data, &*initcode);
    }
}
