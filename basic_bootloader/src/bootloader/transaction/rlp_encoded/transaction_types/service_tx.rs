use crate::bootloader::errors::InvalidTransaction;
use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::{Rlp, RlpListDecode};
use crate::bootloader::transaction::rlp_encoded::transaction_types::EthereumTxType;
use ruint::aliases::B160;
use system_hooks::addresses_constants::{
    L2_INTEROP_CENTER_ADDRESS, L2_INTEROP_ROOT_STORAGE_ADDRESS, SYSTEM_CONTEXT_ADDRESS,
};

/// ZKsync OS service (type 0x7d) transaction .
/// Used for system operations, such as importing interop roots.
/// Can only be executed in service blocks, i.e. blocks with only service
/// transactions.
/// They have no signature, as they are added directly by the operator.
///
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct ServiceTx<'a> {
    pub(crate) to: &'a [u8; 20], // NOTE: has to be one of the addresses in SERVICE_DESTINATION_WHITELIST
    pub(crate) data: &'a [u8], // NOTE: has to start with one of the selectors in SERVICE_DESTINATION_WHITELIST
    salt: u64, // Some salt used by the server to identify service transactions. Ignored by ZKsync OS.
}

/// Selector for
/// addInteropRootsInBatch((uint256,uint256,bytes32[])[])
/// -> 0xcca2f7bc
pub const ADD_INTEROP_ROOTS_IN_BATCH_SELECTOR: [u8; 4] = [0xcc, 0xa2, 0xf7, 0xbc];

/// Selector for
/// setSettlementLayerChainId(uint256)
/// -> 0x040203e6
pub const SET_SL_CHAIN_ID_SELECTOR: [u8; 4] = [0x04, 0x02, 0x03, 0xe6];

/// Selector for
/// setInteropFee(uint256)
/// -> 0x08273d8a
pub const SET_INTEROP_FEE_SELECTOR: [u8; 4] = [0x08, 0x27, 0x3d, 0x8a];

/// Pairs (destination, selector) that service transactions are allowed
/// to interact with.
const SERVICE_DESTINATION_WHITELIST: &[(B160, [u8; 4])] = &[
    (
        L2_INTEROP_ROOT_STORAGE_ADDRESS,
        ADD_INTEROP_ROOTS_IN_BATCH_SELECTOR,
    ),
    (SYSTEM_CONTEXT_ADDRESS, SET_SL_CHAIN_ID_SELECTOR),
    (L2_INTEROP_CENTER_ADDRESS, SET_INTEROP_FEE_SELECTOR),
];

fn whitelisted(to: B160, data: &[u8]) -> bool {
    let selector: [u8; 4] = match data.get(..4).and_then(|bytes| bytes.try_into().ok()) {
        Some(selector) => selector,
        None => return false,
    };
    SERVICE_DESTINATION_WHITELIST.contains(&(to, selector))
}

pub const SERVICE_TX_TYPE: u8 = 0x7d;

impl<'a> EthereumTxType for ServiceTx<'a> {
    const TX_TYPE: u8 = SERVICE_TX_TYPE;
}

impl<'a> RlpListDecode<'a> for ServiceTx<'a> {
    /// Decode the 2-field list body:
    /// [destination, data]
    fn decode_list_body(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        let to_slice = r.bytes()?;
        if to_slice.len() != 20 {
            return Err(InvalidTransaction::InvalidStructure);
        }
        let to: &'a [u8; 20] = to_slice
            .try_into()
            .map_err(|_| InvalidTransaction::InvalidStructure)?;

        let to_b160 = B160::from_be_bytes(*to);

        let data = r.bytes()?;
        // Validate whitelist
        if !whitelisted(to_b160, data) {
            return Err(InvalidTransaction::InvalidStructure);
        }
        let salt = r.u64()?;
        Ok(Self { to, data, salt })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::RlpListDecode;
    use alloy_rlp::Encodable;

    /// Helper to RLP-encode the 3-field ServiceTx body:
    /// [destination, data, salt]
    fn encode_service_tx(to: &[u8], data: &[u8], salt: u64) -> Vec<u8> {
        let mut buf = Vec::new();

        // Temporary placeholder for the list header; we’ll fix it once we know the payload length.
        buf.push(0xc0); // dummy

        let start = buf.len();

        to.encode(&mut buf);
        data.encode(&mut buf);
        salt.encode(&mut buf);

        let payload_len = buf.len() - start;

        // Short list form is enough for these tiny tests.
        assert!(payload_len < 56, "test list unexpectedly large");
        buf[0] = 0xc0 + payload_len as u8;
        buf
    }

    #[test]
    fn empty_to_fails() {
        let to: &[u8] = &[]; // RLP empty string -> len() == 0
        let data: &[u8] = &[0x01, 0x02];

        let bytes = encode_service_tx(to, data, 0);

        let res = ServiceTx::decode_list_full(&bytes);
        assert!(matches!(res, Err(InvalidTransaction::InvalidStructure)));
    }

    #[test]
    fn to_outside_whitelist_fails() {
        // Some arbitrary 20-byte address that is not in the whitelist.
        let to_bytes: [u8; 20] = [0x11u8; 20];

        let data: &[u8] = &[];

        let bytes = encode_service_tx(&to_bytes, data, 0);

        let res = ServiceTx::decode_list_full(&bytes);
        assert!(matches!(res, Err(InvalidTransaction::InvalidStructure)));
    }

    #[test]
    fn to_in_whitelist_parses() {
        let to_bytes: [u8; 20] = L2_INTEROP_ROOT_STORAGE_ADDRESS.to_be_bytes();
        let data: Vec<u8> = ADD_INTEROP_ROOTS_IN_BATCH_SELECTOR.to_vec();

        let bytes = encode_service_tx(&to_bytes, &data, 0);

        let tx: ServiceTx<'_> =
            ServiceTx::decode_list_full(&bytes).expect("whitelisted address must decode");

        assert_eq!(tx.to, to_bytes.as_slice());
        assert_eq!(tx.data, data.as_slice());
    }
}
