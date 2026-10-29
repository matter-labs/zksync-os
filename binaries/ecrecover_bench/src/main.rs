//! A cycle benchmark of `ecrecover` as the airbender-crypto code computes it:
//! one hardcoded signature (geth's `ecrecover` test vector) is recovered
//! `repeats` times, each recovery (the secp256k1 recovery and the keccak of
//! the public key, as the precompile does) between two cycle markers, so the
//! host reads the cycles and the delegation calls of every recovery.
//!
//! The host drives the run over the CSR channel: it sends the mode and the
//! number of repeats, then serves the hints of the crate's own CSR hooks
//! (`crypto::secp256k1::oracle::CsrHooks`, the ZKsync OS oracle protocol: a
//! square root, an inverse, a quotient or the witness of the recovery comes
//! from the host and is checked). Whether the key is witnessed is the host's
//! choice: it answers the witness request, or declines it. Without the hints
//! the crate's own field algorithms run (`DefaultSecp256k1Hooks`). The program
//! returns a fold of the recovered addresses, which the host checks.
//!
//! The variants are the crate's features: `bigint` (the u256 delegation for
//! the field arithmetic, whose elements cross the channel as their Montgomery
//! words) and `keccak` (the keccak-f[1600] delegation).

#![no_std]
#![no_main]

use airbender::rt::sys::{emit_cycle_marker, read_word};
use crypto::secp256k1::field::FieldElement;
use crypto::secp256k1::hooks::{DefaultSecp256k1Hooks, Secp256k1Hooks};
use crypto::secp256k1::oracle::{CsrChannel, OracleHooks};
use crypto::secp256k1::recover_from_bytes_with_hooks;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;

/// geth's `ecrecover` test vector: the digest, `r`, `s` of a signature with
/// `v = 27` (an even `y`), recovering `0xceaccac640adf55b2028469bd36ba501f28b699d`
const DIGEST: [u8; 32] = [
    0x38, 0xd1, 0x8a, 0xcb, 0x67, 0xd2, 0x5c, 0x8b, 0xb9, 0x94, 0x27, 0x64, 0xb6, 0x2f, 0x18, 0xe1,
    0x70, 0x54, 0xf6, 0x6a, 0x81, 0x7b, 0xd4, 0x29, 0x54, 0x23, 0xad, 0xf9, 0xed, 0x98, 0x87, 0x3e,
];
const R: [u8; 32] = [
    0x38, 0xd1, 0x8a, 0xcb, 0x67, 0xd2, 0x5c, 0x8b, 0xb9, 0x94, 0x27, 0x64, 0xb6, 0x2f, 0x18, 0xe1,
    0x70, 0x54, 0xf6, 0x6a, 0x81, 0x7b, 0xd4, 0x29, 0x54, 0x23, 0xad, 0xf9, 0xed, 0x98, 0x87, 0x3e,
];
const S: [u8; 32] = [
    0x78, 0x9d, 0x1d, 0xd4, 0x23, 0xd2, 0x5f, 0x07, 0x72, 0xd2, 0x74, 0x8d, 0x60, 0xf7, 0xe4, 0xb8,
    0x1b, 0xb1, 0x4d, 0x08, 0x6e, 0xba, 0x8e, 0x8e, 0x8e, 0xfb, 0x6d, 0xcf, 0xf8, 0xa4, 0xae, 0x02,
];
const Y_IS_ODD: bool = false;

/// The address of the signer: keccak of the recovered public key, as the
/// precompile computes it
fn recover_address<H: Secp256k1Hooks>(hooks: &mut H) -> [u8; 32] {
    let point = recover_from_bytes_with_hooks(&DIGEST, &R, &S, Y_IS_ODD, hooks)
        .expect("the vector recovers a key");
    let mut key = [0u8; 64];
    let (x, y) = key.split_at_mut(32);
    point.write_coordinates(
        x.try_into().expect("32 bytes"),
        y.try_into().expect("32 bytes"),
    );
    let mut hasher = Keccak256::new();
    hasher.update(&key);
    let mut hash = [0u8; 32];
    hasher.finalize_reset_with_closure(|digest| hash = *digest.as_array());
    hash[..12].fill(0);
    hash
}

/// The field operations one by one, `MICRO_OPS` of each between two cycle
/// markers, in this order: multiplication, squaring, addition, subtraction,
/// normalization, zero test, encoding to bytes, decoding from bytes, and for
/// the software field the magnitude-tracked subtraction and the weak
/// normalization (two empty marker pairs on the delegated field). The host
/// divides the cycles of each pair by `MICRO_OPS`.
const MICRO_OPS: u32 = 256;

fn field_micro() -> [u32; 8] {
    let a = FieldElement::from_bytes(&DIGEST).expect("below the modulus");
    let b = FieldElement::from_bytes(&S).expect("below the modulus");
    let mut x = a;
    let mut sink = 0u32;
    let mut block = |f: &mut dyn FnMut()| {
        emit_cycle_marker();
        for _ in 0..MICRO_OPS {
            f();
        }
        emit_cycle_marker();
    };
    block(&mut || x.mul_in_place(&b));
    block(&mut || x.square_in_place());
    block(&mut || x.add_in_place(&b));
    block(&mut || x.sub_in_place(&b));
    block(&mut || x.normalize_in_place());
    block(&mut || sink ^= u32::from(x.is_zero()));
    block(&mut || sink ^= x.to_bytes()[31] as u32);
    block(&mut || x = FieldElement::from_bytes(&a.to_bytes().into()).expect("an element"));
    #[cfg(not(feature = "bigint"))]
    {
        block(&mut || x.sub_in_place_magnitude(&b, 1));
        block(&mut || x.normalize_weak_in_place());
    }
    #[cfg(feature = "bigint")]
    {
        block(&mut || {});
        block(&mut || {});
    }
    let bytes: [u8; 32] = x.to_bytes().into();
    let mut out = [0u32; 8];
    for (word, chunk) in out.iter_mut().zip(bytes.as_chunks::<4>().0) {
        *word = u32::from_le_bytes(*chunk);
    }
    out[0] ^= sink;
    out
}

#[airbender::main]
fn main() -> [u32; 8] {
    // 0: recoveries without hints, 1: with the hints of the host (which witnesses the key or
    // not), 2: the field operations one by one
    let mode = read_word();
    if mode == 2 {
        return field_micro();
    }
    let repeats = read_word();
    let mut folded = [0u32; 8];
    for i in 0..repeats {
        emit_cycle_marker();
        let address = match mode {
            0 => recover_address(&mut DefaultSecp256k1Hooks),
            _ => recover_address(&mut OracleHooks::new(CsrChannel)),
        };
        emit_cycle_marker();
        // a fold the host recomputes: every repeat must recover the address
        for (word, chunk) in folded.iter_mut().zip(address.as_chunks::<4>().0) {
            *word ^= u32::from_le_bytes(*chunk).rotate_left(i % 32);
        }
    }
    folded
}
