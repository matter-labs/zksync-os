#![no_main]
#![feature(allocator_api)]

use std::alloc::Global;

use alloy::consensus::{
    SignableTransaction, TxEip1559, TxEip2930, TxEip4844, TxEip7702, TxEnvelope, TxLegacy,
};
use alloy::eips::eip2718::Encodable2718;
use alloy::network::TxSignerSync;
use alloy::primitives::{Address, Bytes, TxKind, U256};
use alloy::signers::local::PrivateKeySigner;
use alloy_rlp::Decodable;
use arbitrary::Arbitrary;
use basic_bootloader::bootloader::transaction::rlp_encoded::RlpEncodedTransaction;
use libfuzzer_sys::fuzz_target;
use once_cell::sync::Lazy;
use ruint::aliases::B160;
use zk_ee::utils::UsizeAlignedByteBox;

/// Cached signer used to produce validly-signed base transactions.
static SIGNER: Lazy<PrivateKeySigner> = Lazy::new(|| {
    let mut key = [0u8; 32];
    key[31] = 1;
    PrivateKeySigner::from_bytes(&alloy::primitives::B256::from(key)).unwrap()
});

/// Structured fuzz input: a valid transaction template plus byte-level mutations.
#[derive(Arbitrary, Debug)]
struct FuzzInput {
    tx: TxParams,
    mutations: Vec<Mutation>,
}

/// Transaction parameters used to build a valid signed transaction.
#[derive(Arbitrary, Debug)]
enum TxParams {
    Legacy {
        nonce: u64,
        gas_price: u64,
        gas_limit: u64,
        to: Option<[u8; 20]>,
        value: u64,
        data: Vec<u8>,
    },
    Eip2930 {
        nonce: u64,
        gas_price: u64,
        gas_limit: u64,
        to: Option<[u8; 20]>,
        value: u64,
        data: Vec<u8>,
    },
    Eip1559 {
        nonce: u64,
        max_fee: u64,
        max_priority_fee: u64,
        gas_limit: u64,
        to: Option<[u8; 20]>,
        value: u64,
        data: Vec<u8>,
    },
    Eip4844 {
        nonce: u64,
        max_fee: u64,
        max_priority_fee: u64,
        gas_limit: u64,
        to: [u8; 20],
        value: u64,
        data: Vec<u8>,
        max_fee_per_blob_gas: u64,
    },
    Eip7702 {
        nonce: u64,
        max_fee: u64,
        max_priority_fee: u64,
        gas_limit: u64,
        to: [u8; 20],
        value: u64,
        data: Vec<u8>,
    },
}

/// Byte-level mutations applied after encoding to explore edge cases.
#[derive(Arbitrary, Debug)]
enum Mutation {
    FlipBit { position: u16, bit: u8 },
    InsertByte { position: u16, byte: u8 },
    DeleteByte { position: u16 },
    ReplaceByte { position: u16, byte: u8 },
    Truncate { keep: u16 },
}

/// Build a valid signed transaction from structured parameters, then encode it.
fn build_and_encode(params: &TxParams) -> Option<Vec<u8>> {
    let envelope = match params {
        TxParams::Legacy {
            nonce,
            gas_price,
            gas_limit,
            to,
            value,
            data,
        } => {
            let mut tx = TxLegacy {
                chain_id: Some(1),
                nonce: *nonce,
                gas_price: *gas_price as u128,
                gas_limit: *gas_limit,
                to: match to {
                    Some(a) => TxKind::Call(Address::from(*a)),
                    None => TxKind::Create,
                },
                value: U256::from(*value),
                input: Bytes::from(data.to_vec()),
            };
            let sig = SIGNER.sign_transaction_sync(&mut tx).ok()?;
            TxEnvelope::Legacy(tx.into_signed(sig))
        }
        TxParams::Eip2930 {
            nonce,
            gas_price,
            gas_limit,
            to,
            value,
            data,
        } => {
            let mut tx = TxEip2930 {
                chain_id: 1,
                nonce: *nonce,
                gas_price: *gas_price as u128,
                gas_limit: *gas_limit,
                to: match to {
                    Some(a) => TxKind::Call(Address::from(*a)),
                    None => TxKind::Create,
                },
                value: U256::from(*value),
                input: Bytes::from(data.to_vec()),
                access_list: Default::default(),
            };
            let sig = SIGNER.sign_transaction_sync(&mut tx).ok()?;
            TxEnvelope::Eip2930(tx.into_signed(sig))
        }
        TxParams::Eip1559 {
            nonce,
            max_fee,
            max_priority_fee,
            gas_limit,
            to,
            value,
            data,
        } => {
            let mut tx = TxEip1559 {
                chain_id: 1,
                nonce: *nonce,
                max_fee_per_gas: *max_fee as u128,
                max_priority_fee_per_gas: *max_priority_fee as u128,
                gas_limit: *gas_limit,
                to: match to {
                    Some(a) => TxKind::Call(Address::from(*a)),
                    None => TxKind::Create,
                },
                value: U256::from(*value),
                input: Bytes::from(data.to_vec()),
                access_list: Default::default(),
            };
            let sig = SIGNER.sign_transaction_sync(&mut tx).ok()?;
            TxEnvelope::Eip1559(tx.into_signed(sig))
        }
        TxParams::Eip4844 {
            nonce,
            max_fee,
            max_priority_fee,
            gas_limit,
            to,
            value,
            data,
            max_fee_per_blob_gas,
        } => {
            let mut tx = TxEip4844 {
                chain_id: 1,
                nonce: *nonce,
                max_fee_per_gas: *max_fee as u128,
                max_priority_fee_per_gas: *max_priority_fee as u128,
                gas_limit: *gas_limit,
                to: Address::from(*to),
                value: U256::from(*value),
                input: Bytes::from(data.to_vec()),
                access_list: Default::default(),
                max_fee_per_blob_gas: *max_fee_per_blob_gas as u128,
                blob_versioned_hashes: Vec::new(),
            };
            let sig = SIGNER.sign_transaction_sync(&mut tx).ok()?;
            TxEnvelope::Eip4844(tx.into_signed(sig).into())
        }
        TxParams::Eip7702 {
            nonce,
            max_fee,
            max_priority_fee,
            gas_limit,
            to,
            value,
            data,
        } => {
            let mut tx = TxEip7702 {
                chain_id: 1,
                nonce: *nonce,
                max_fee_per_gas: *max_fee as u128,
                max_priority_fee_per_gas: *max_priority_fee as u128,
                gas_limit: *gas_limit,
                to: Address::from(*to),
                value: U256::from(*value),
                input: Bytes::from(data.to_vec()),
                access_list: Default::default(),
                authorization_list: Vec::new(),
            };
            let sig = SIGNER.sign_transaction_sync(&mut tx).ok()?;
            TxEnvelope::Eip7702(tx.into_signed(sig))
        }
    };

    Some(envelope.encoded_2718())
}

/// Apply byte-level mutations to encoded transaction bytes.
fn apply_mutations(data: &mut Vec<u8>, mutations: &[Mutation]) {
    for mutation in mutations {
        match mutation {
            Mutation::FlipBit { position, bit } => {
                if !data.is_empty() {
                    let pos = *position as usize % data.len();
                    data[pos] ^= 1 << (*bit % 8);
                }
            }
            Mutation::InsertByte { position, byte } => {
                if data.len() < 65536 {
                    let pos = *position as usize % (data.len() + 1);
                    data.insert(pos, *byte);
                }
            }
            Mutation::DeleteByte { position } => {
                if !data.is_empty() {
                    let pos = *position as usize % data.len();
                    data.remove(pos);
                }
            }
            Mutation::ReplaceByte { position, byte } => {
                if !data.is_empty() {
                    let pos = *position as usize % data.len();
                    data[pos] = *byte;
                }
            }
            Mutation::Truncate { keep } => {
                let keep = (*keep as usize).min(data.len());
                data.truncate(keep);
            }
        }
    }
}

fn fuzz(input: FuzzInput) {
    let Some(mut encoded) = build_and_encode(&input.tx) else {
        return;
    };

    // Apply byte-level mutations to the encoded transaction.
    apply_mutations(&mut encoded, &input.mutations);

    // Try parsing with our implementation (chain_id=1).
    let buffer = UsizeAlignedByteBox::<Global>::from_slice_in(&encoded, Global);
    let our_result = RlpEncodedTransaction::parse_from_buffer(buffer, 1, B160::ZERO);

    // Try parsing with the Alloy reference implementation.
    let mut alloy_cursor: &[u8] = &encoded;
    let alloy_result: Result<TxEnvelope, _> = TxEnvelope::decode(&mut alloy_cursor);
    let alloy_fully_consumed = alloy_cursor.is_empty();

    let alloy_ok = alloy_result.is_ok() && alloy_fully_consumed;
    let our_ok = our_result.is_ok();

    // Divergence: one side accepts while the other rejects.
    assert!(
        !(our_ok && !alloy_ok),
        "Our parser accepted but Alloy rejected (or had trailing bytes)"
    );
    assert!(
        !(!our_ok && alloy_ok),
        "Alloy accepted but our parser rejected"
    );

    // Both accepted — signing hashes must agree.
    if let (Ok(our_tx), Ok(ref env)) = (&our_result, &alloy_result) {
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

fuzz_target!(|input: FuzzInput| {
    fuzz(input);
});
