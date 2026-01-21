use core::mem::MaybeUninit;

use zk_ee::out_of_return_memory;
use zk_ee::system::{Ergs, Resources, SystemFunction};

use super::mixing_function::*;
use super::*;

pub const GAS_PER_ROUND: u64 = 1;
pub const INPUT_LEN: usize = 213;

#[inline(always)]
fn parse_blake2_state(
    input: &[u8; INPUT_LEN - core::mem::size_of::<u32>()],
) -> (
    [u64; BLAKE2S_STATE_WIDTH_IN_U64_WORDS],
    [u64; BLAKE2S_BLOCK_SIZE_U64_WORDS],
    (u64, u64),
    u8,
) {
    // length is pre-checked, so we should just check that we are in good endianness, and
    // "read unaligned" almost everything

    #[cfg(target_endian = "big")]
    compile_error!("big endian archs are not supported");

    unsafe {
        let mut src_ptr = input.as_ptr();
        // assume init immedatelly, but use pointers later on to avoid huge stack to stack copy
        let state: [u64; BLAKE2S_STATE_WIDTH_IN_U64_WORDS] =
            core::ptr::read_unaligned(src_ptr.cast());
        src_ptr = src_ptr.add(64);

        let message_block: [u64; BLAKE2S_BLOCK_SIZE_U64_WORDS] =
            core::ptr::read_unaligned(src_ptr.cast());
        src_ptr = src_ptr.add(128);

        let t0: u64 = core::ptr::read_unaligned(src_ptr.cast());
        src_ptr = src_ptr.add(8);
        let t1: u64 = core::ptr::read_unaligned(src_ptr.cast());
        src_ptr = src_ptr.add(8);
        let finalization_flag = src_ptr.read();

        (state, message_block, (t0, t1), finalization_flag)
    }
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

        let mut extended_state = unsafe {
            let mut extended_state: MaybeUninit<[u64; BLAKE2S_EXTENDED_STATE_WIDTH_IN_U64_WORDS]> =
                MaybeUninit::uninit();
            extended_state
                .as_mut_ptr()
                .cast::<[u64; BLAKE2S_STATE_WIDTH_IN_U64_WORDS]>()
                .write(state);
            extended_state
                .as_mut_ptr()
                .cast::<[u64; BLAKE2S_STATE_WIDTH_IN_U64_WORDS]>()
                .add(1)
                .write(BLAKE2B_IV);

            extended_state.assume_init()
        };

        extended_state[12] ^= t0;
        extended_state[13] ^= t1;
        if finalization_flag {
            extended_state[14] = !extended_state[14];
        }

        round_function_for_num_rounds(&mut extended_state, &message_block, num_rounds as usize);

        for i in 0..BLAKE2S_STATE_WIDTH_IN_U64_WORDS {
            state[i] ^= extended_state[i] ^ extended_state[i + BLAKE2S_STATE_WIDTH_IN_U64_WORDS];
        }

        #[cfg(target_endian = "big")]
        compile_error!("big endian archs are not supported");

        // write back - no endianness changes
        unsafe {
            output
                .try_extend(
                    core::slice::from_raw_parts(
                        state.as_ptr().cast::<u8>(),
                        BLAKE2S_STATE_WIDTH_IN_U64_WORDS * core::mem::size_of::<u64>(),
                    )
                    .iter()
                    .copied(),
                )
                .map_err(|_| out_of_return_memory!())?;
        }

        Ok(())
    }
}
