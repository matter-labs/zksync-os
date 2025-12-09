use crate::bootloader::errors::InvalidTransaction;
use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::{Rlp, RlpListDecode};
use crate::bootloader::transaction::rlp_encoded::transaction_types::EthereumTxType;
use ruint::aliases::B160;
use system_hooks::addresses_constants::L2_INTEROP_ROOT_STORAGE_ADDRESS;

/// ZKsync OS service (type 0x7d) transaction .
/// Used for system operations, such as importing interop roots.
/// Can only be executed in service blocks, i.e. blocks with only service
/// transactions.
/// They have no signature, as they are added directly by the operator.
///
#[derive(Clone, Copy, Debug)]
pub(crate) struct ServiceTx<'a> {
    pub(crate) nonce: u64,
    pub(crate) gas_limit: u64,
    pub(crate) to: &'a [u8; 20], // NOTE: has to be one of the addresses in SERVICE_DESTINATION_WHITELIST
    pub(crate) data: &'a [u8],
}

const SERVICE_DESTINATION_WHITELIST: &[B160] = &[L2_INTEROP_ROOT_STORAGE_ADDRESS];

pub const SERVICE_TX_TYPE: u8 = 0x7d;

impl<'a> EthereumTxType for ServiceTx<'a> {
    const TX_TYPE: u8 = SERVICE_TX_TYPE;
}

impl<'a> RlpListDecode<'a> for ServiceTx<'a> {
    /// Decode the 4-field list body:
    /// [nonce, gas_limit, destination, data]
    fn decode_list_body(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        let nonce = r.u64()?;
        let gas_limit = r.u64()?;

        let to_slice = r.bytes()?;
        if to_slice.len() != 20 {
            return Err(InvalidTransaction::InvalidStructure);
        }
        let to: &'a [u8; 20] = to_slice
            .try_into()
            .map_err(|_| InvalidTransaction::InvalidStructure)?;

        let to_b160 = B160::from_be_bytes(*to);

        // Validate whitelist
        if !SERVICE_DESTINATION_WHITELIST.contains(&to_b160) {
            return Err(InvalidTransaction::InvalidStructure);
        }

        let data = r.bytes()?;
        Ok(Self {
            nonce,
            gas_limit,
            to,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::RlpListDecode;
    use alloy_rlp::Encodable;

    /// Helper to RLP-encode the 4-field ServiceTx body:
    /// [nonce, gas_limit, destination, data]
    fn encode_service_tx(nonce: u64, gas_limit: u64, to: &[u8], data: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();

        // Temporary placeholder for the list header; we’ll fix it once we know the payload length.
        buf.push(0xc0); // dummy

        let start = buf.len();

        nonce.encode(&mut buf);
        gas_limit.encode(&mut buf);
        to.encode(&mut buf);
        data.encode(&mut buf);

        let payload_len = buf.len() - start;

        // Short list form is enough for these tiny tests.
        assert!(payload_len < 56, "test list unexpectedly large");
        buf[0] = 0xc0 + payload_len as u8;
        buf
    }

    #[test]
    fn empty_to_fails() {
        let nonce = 0;
        let gas_limit = 21_000;
        let to: &[u8] = &[]; // RLP empty string -> len() == 0
        let data: &[u8] = &[0x01, 0x02];

        let bytes = encode_service_tx(nonce, gas_limit, to, data);

        let res = ServiceTx::decode_list_full(&bytes);
        assert!(matches!(res, Err(InvalidTransaction::InvalidStructure)));
    }

    #[test]
    fn to_outside_whitelist_fails() {
        let nonce = 1;
        let gas_limit = 50_000;

        // Some arbitrary 20-byte address that is not in the whitelist.
        let to_bytes: [u8; 20] = [0x11u8; 20];

        let data: &[u8] = &[];

        let bytes = encode_service_tx(nonce, gas_limit, &to_bytes, data);

        let res = ServiceTx::decode_list_full(&bytes);
        assert!(matches!(res, Err(InvalidTransaction::InvalidStructure)));
    }

    #[test]
    fn to_in_whitelist_parses() {
        let nonce = 7;
        let gas_limit = 42_000;

        let to_bytes: [u8; 20] = L2_INTEROP_ROOT_STORAGE_ADDRESS.to_be_bytes();
        let data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef];

        let bytes = encode_service_tx(nonce, gas_limit, &to_bytes, &data);

        let tx: ServiceTx<'_> =
            ServiceTx::decode_list_full(&bytes).expect("whitelisted address must decode");

        assert_eq!(tx.nonce, nonce);
        assert_eq!(tx.gas_limit, gas_limit);
        assert_eq!(tx.to, to_bytes.as_slice());
        assert_eq!(tx.data, data.as_slice());
    }
}
