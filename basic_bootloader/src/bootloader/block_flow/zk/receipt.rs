//! The ZK receipt-hash leaf used to build the block header's `receipts_root`.

use crate::bootloader::rlp::{CachingRLPEncodable, ReceiptEncoder};
use crate::bootloader::transaction_flow::logs_bloom::LogsBloom;
use crypto::blake2s::Blake2s256;
use crypto::MiniDigest;
use zk_ee::common_structs::GenericEventContentRef;
use zk_ee::system::MAX_EVENT_TOPICS;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::Bytes32;

/// Computes the ZK receipt-hash leaf:
/// `blake2s(type? || rlp([status, cumulative_gas_used, logs_bloom, [logs...]]))`.
pub(crate) fn compute_receipt_hash<'events, I>(
    tx_type: u8,
    status: &bool,
    cumulative_gas_used: &u64,
    events: I,
) -> Bytes32
where
    I: Iterator<
            Item = GenericEventContentRef<'events, { MAX_EVENT_TOPICS }, EthereumIOTypesConfig>,
        > + Clone,
{
    // The ZK receipts root commits to a zero logs bloom: the bloom is an
    // Ethereum consensus field, and the ZK block header logs bloom is always
    // zero, so computing a real per-receipt bloom (a keccak256 over every log
    // address and topic) would be wasted prover work. The receipt still carries
    // the (zero) bloom field for Ethereum receipt layout compatibility. The
    // Ethereum block flow continues to compute the real bloom for its receipts.
    let bloom = LogsBloom::default();

    let mut receipt_encoder = ReceiptEncoder::new_from_fields(
        tx_type,
        status,
        cumulative_gas_used,
        bloom.as_bytes(),
        events,
    );
    let mut receipt_hasher = Blake2s256::new();
    receipt_encoder.encode_into(&mut receipt_hasher);
    Bytes32::from_array(receipt_hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use alloy::consensus::{Eip658Value, Receipt, ReceiptWithBloom};
    use alloy_primitives::{Address, Bloom, Bytes, Log, B256};
    use alloy_rlp::Encodable as _;
    use arrayvec::ArrayVec;
    use ruint::aliases::B160;
    use zk_ee::utils::Bytes32;

    /// blake2s over `type? || rlp([status, gas, bloom, [log]])`, the reference
    /// for a single-log ZK receipt hash.
    fn reference_receipt_hash(tx_type: u8, status: bool, gas: u64, bloom: Bloom) -> Bytes32 {
        let alloy_log = Log::new_unchecked(
            Address::from([0xaau8; 20]),
            alloc::vec![B256::from([0x11u8; 32]), B256::from([0x22u8; 32])],
            Bytes::copy_from_slice(&[0xde, 0xad, 0xbe, 0xef]),
        );
        let rwb = ReceiptWithBloom {
            receipt: Receipt {
                status: Eip658Value::Eip658(status),
                cumulative_gas_used: gas,
                logs: alloc::vec![alloy_log],
            },
            logs_bloom: bloom,
        };
        let mut rlp = Vec::new();
        if tx_type != 0 {
            rlp.push(tx_type);
        }
        rwb.encode(&mut rlp);
        let mut hasher = Blake2s256::new();
        hasher.update(&rlp);
        Bytes32::from_array(hasher.finalize())
    }

    #[test]
    fn zk_receipt_hash_uses_zero_bloom() {
        let addr = B160::from_be_bytes::<20>([0xaau8; 20]);
        let mut topics: ArrayVec<Bytes32, MAX_EVENT_TOPICS> = ArrayVec::new();
        topics.push(Bytes32::from_array([0x11u8; 32]));
        topics.push(Bytes32::from_array([0x22u8; 32]));
        let data = [0xde, 0xad, 0xbe, 0xef];
        let event: GenericEventContentRef<'_, MAX_EVENT_TOPICS, EthereumIOTypesConfig> =
            GenericEventContentRef {
                address: &addr,
                topics: &topics,
                data: &data,
            };

        let tx_type = 2u8;
        let status = true;
        let gas: u64 = 0x5208;

        let got = compute_receipt_hash(tx_type, &status, &gas, core::iter::once(event));

        // The ZK path must commit to a zero bloom...
        let zero_bloom = Bloom::from_slice(&[0u8; 256]);
        assert_eq!(
            got.as_u8_array_ref(),
            reference_receipt_hash(tx_type, status, gas, zero_bloom).as_u8_array_ref(),
        );

        // ...and NOT the real (non-zero) bloom those events would produce.
        let nonzero_bloom = Bloom::from_slice(&[0xffu8; 256]);
        assert_ne!(
            got.as_u8_array_ref(),
            reference_receipt_hash(tx_type, status, gas, nonzero_bloom).as_u8_array_ref(),
        );
    }
}
