use zk_ee::out_of_return_memory;
use zk_ee::system::{Ergs, Resources, SystemFunction};

use super::mixing_function::*;
use super::*;

pub const GAS_PER_ROUND: u64 = 1;
pub const INPUT_LEN: usize = 213;

#[cfg(target_endian = "big")]
compile_error!("big endian archs are not supported");

/// Parse Blake2B compression state from the raw EIP-152 input (after the 4-byte round count).
///
/// Layout (all little-endian on the wire per Blake2B spec):
///   bytes [0..64)    — h[0..8]: 8 × u64 state words
///   bytes [64..192)  — m[0..16]: 16 × u64 message block words
///   bytes [192..200) — t0: offset counter low
///   bytes [200..208) — t1: offset counter high
///   byte  [208]      — finalization flag
#[inline(always)]
fn parse_blake2_state(
    input: &[u8; INPUT_LEN - core::mem::size_of::<u32>()],
) -> (
    [u64; BLAKE2B_STATE_WIDTH_IN_U64_WORDS],
    [u64; BLAKE2B_BLOCK_SIZE_U64_WORDS],
    (u64, u64),
    u8,
) {
    #[inline(always)]
    fn read_u64_le(bytes: &[u8], offset: usize) -> u64 {
        let chunk: [u8; 8] = bytes[offset..offset + 8]
            .try_into()
            // SAFETY-NOTE: statically guaranteed — caller passes a fixed-size array and
            // all offsets are compile-time constants within bounds.
            .expect("slice is exactly 8 bytes");
        u64::from_le_bytes(chunk)
    }

    let mut state = [0u64; BLAKE2B_STATE_WIDTH_IN_U64_WORDS];
    for i in 0..BLAKE2B_STATE_WIDTH_IN_U64_WORDS {
        state[i] = read_u64_le(input, i * 8);
    }

    let mut message_block = [0u64; BLAKE2B_BLOCK_SIZE_U64_WORDS];
    for i in 0..BLAKE2B_BLOCK_SIZE_U64_WORDS {
        message_block[i] = read_u64_le(input, 64 + i * 8);
    }

    let t0 = read_u64_le(input, 192);
    let t1 = read_u64_le(input, 200);
    let finalization_flag = input[208];

    (state, message_block, (t0, t1), finalization_flag)
}

pub struct Blake2FPrecompile;

impl<R: Resources> SystemFunction<R, Blake2FPrecompileErrors> for Blake2FPrecompile {
    fn execute<
        D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        _allocator: A,
    ) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Blake2FPrecompileErrors>> {
        if input.len() != INPUT_LEN {
            return Err(interface_error!(
                Blake2FPrecompileInterfaceError::InvalidInputSize
            ));
        }
        // we will very quickly parse number of round
        let num_rounds = u32::from_be_bytes(input.as_chunks::<4>().0[0]);
        let cost_ergs = Ergs(((num_rounds as u64) * GAS_PER_ROUND) * ERGS_PER_GAS);
        // TODO(EVM-1237): add native model
        let cost_native = 0;
        resources.charge(&R::from_ergs_and_native(
            cost_ergs,
            <R::Native as zk_ee::system::Computational>::from_computational(cost_native),
        ))?;

        let (mut state, message_block, (t0, t1), finalization_flag) =
            parse_blake2_state(input[4..].try_into().unwrap());

        let finalization_flag = match finalization_flag {
            0 => false,
            1 => true,
            _ => {
                return Err(interface_error!(
                    Blake2FPrecompileInterfaceError::InvalidBooleanFlag
                ));
            }
        };

        let mut extended_state = [0u64; BLAKE2B_EXTENDED_STATE_WIDTH_IN_U64_WORDS];
        extended_state[..BLAKE2B_STATE_WIDTH_IN_U64_WORDS].copy_from_slice(&state);
        extended_state[BLAKE2B_STATE_WIDTH_IN_U64_WORDS..].copy_from_slice(&BLAKE2B_IV);

        extended_state[12] ^= t0;
        extended_state[13] ^= t1;
        if finalization_flag {
            extended_state[14] = !extended_state[14];
        }

        round_function_for_num_rounds(&mut extended_state, &message_block, num_rounds as usize);

        for i in 0..BLAKE2B_STATE_WIDTH_IN_U64_WORDS {
            state[i] ^= extended_state[i] ^ extended_state[i + BLAKE2B_STATE_WIDTH_IN_U64_WORDS];
        }

        // Serialize state back to little-endian bytes (matches Blake2B wire format on LE targets).
        let mut result_bytes = [0u8; BLAKE2B_STATE_WIDTH_IN_U64_WORDS * core::mem::size_of::<u64>()];
        for (i, word) in state.iter().enumerate() {
            result_bytes[i * 8..(i + 1) * 8].copy_from_slice(&word.to_le_bytes());
        }
        output
            .try_extend(result_bytes)
            .map_err(|_| out_of_return_memory!())?;

        Ok(())
    }
}
