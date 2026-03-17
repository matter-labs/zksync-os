#![no_main]
#![feature(allocator_api)]

use std::alloc::Global;

use alloy::consensus::{SignableTransaction, TxEnvelope};
use alloy_rlp::Decodable;
use basic_bootloader::bootloader::transaction::rlp_encoded::RlpEncodedTransaction;
use libfuzzer_sys::fuzz_target;
use ruint::aliases::B160;
use zk_ee::utils::UsizeAlignedByteBox;

fn fuzz(data: &[u8]) {
    // Try parsing with our implementation using chain_id=1 (mainnet).
    let buffer = UsizeAlignedByteBox::<Global>::from_slice_in(data, Global);
    let our_result = RlpEncodedTransaction::parse_from_buffer(buffer, 1, B160::ZERO);

    // Try parsing with the Alloy reference implementation.
    let mut alloy_cursor: &[u8] = data;
    let alloy_result: Result<TxEnvelope, _> = TxEnvelope::decode(&mut alloy_cursor);
    // Alloy must also consume the full input to count as accepting.
    let alloy_ok = alloy_result.is_ok() && alloy_cursor.is_empty();

    // If both parsers accept, the signing hash must agree.
    if let (Ok(our_tx), true) = (&our_result, alloy_ok) {
        let mut cursor2: &[u8] = data;
        let env = TxEnvelope::decode(&mut cursor2).unwrap();
        let alloy_hash = match env {
            TxEnvelope::Legacy(signed) => signed.tx().signature_hash(),
            TxEnvelope::Eip2930(signed) => signed.tx().signature_hash(),
            TxEnvelope::Eip1559(signed) => signed.tx().signature_hash(),
            TxEnvelope::Eip4844(signed) => signed.tx().signature_hash(),
            TxEnvelope::Eip7702(signed) => signed.tx().signature_hash(),
        };
        assert_eq!(
            our_tx.hash_for_signature_verification().as_u8_array(),
            alloy_hash.0,
            "Signing hash mismatch between our parser and Alloy"
        );
    }
}

fuzz_target!(|data: &[u8]| {
    fuzz(data);
});
