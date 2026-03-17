#![no_main]
#![feature(allocator_api)]

use std::alloc::Global;

use alloy::consensus::{SignableTransaction, TxEip1559, TxEip2930, TxEnvelope, TxLegacy};
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
    };

    let mut out = Vec::new();
    encode_envelope_2718(&envelope, &mut out);
    Some(out)
}

/// Encode a TxEnvelope in EIP-2718 format (matching the codebase's proven encoding approach).
fn encode_envelope_2718(env: &TxEnvelope, out: &mut Vec<u8>) {
    use alloy::rlp::Encodable;
    match env {
        TxEnvelope::Legacy(signed) => {
            signed.rlp_encode(out);
        }
        TxEnvelope::Eip2930(signed) => {
            out.push(0x01);
            signed.rlp_encode(out);
        }
        TxEnvelope::Eip1559(signed) => {
            out.push(0x02);
            signed.rlp_encode(out);
        }
        TxEnvelope::Eip4844(signed) => {
            out.push(0x03);
            signed.rlp_encode(out);
        }
        TxEnvelope::Eip7702(signed) => {
            out.push(0x04);
            signed.rlp_encode(out);
        }
    }
}

/// Apply byte-level mutations to encoded transaction bytes.
fn apply_mutations(data: &mut Vec<u8>, mutations: &[Mutation]) {
    for mutation in mutations {
        if data.is_empty() {
            break;
        }
        match mutation {
            Mutation::FlipBit { position, bit } => {
                let pos = *position as usize % data.len();
                data[pos] ^= 1 << (*bit % 8);
            }
            Mutation::InsertByte { position, byte } => {
                if data.len() < 65536 {
                    let pos = *position as usize % (data.len() + 1);
                    data.insert(pos, *byte);
                }
            }
            Mutation::DeleteByte { position } => {
                let pos = *position as usize % data.len();
                data.remove(pos);
            }
            Mutation::ReplaceByte { position, byte } => {
                let pos = *position as usize % data.len();
                data[pos] = *byte;
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
    let alloy_ok = alloy_result.is_ok() && alloy_cursor.is_empty();

    // If both parsers accept, the signing hash must agree.
    if let (Ok(our_tx), true) = (&our_result, alloy_ok) {
        let mut cursor2: &[u8] = &encoded;
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

fuzz_target!(|input: FuzzInput| {
    fuzz(input);
});
