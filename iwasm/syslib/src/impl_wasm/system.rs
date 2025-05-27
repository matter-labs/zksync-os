use super::*;

pub fn terminate_execution(reason: &str) -> ! {
    unsafe {
        short_host_op(
            ShortHostOp::Revert,
            0,
            reason.as_ptr() as usize as u32,
            reason.len() as u32,
        );

        core::hint::unreachable_unchecked();
    }
}

pub mod calldata {
    use iwasm_specification::host_ops::ShortHostOp;

    use crate::{handle_host_call, long_host_op, short_host_op};

    pub fn selector() -> u32 {
        handle_host_call(|| unsafe { short_host_op(ShortHostOp::CalldataSelector, 0, 0, 0).into() })
            as u32
    }

    pub fn size() -> u32 {
        handle_host_call(|| unsafe { short_host_op(ShortHostOp::CalldataSize, 0, 0, 0).into() })
            as u32
    }

    pub fn read_into(tgt: &mut [core::mem::MaybeUninit<u8>]) {
        handle_host_call(|| unsafe {
            short_host_op(
                ShortHostOp::CalldataReadInto,
                0,
                tgt.as_ptr() as usize as u32,
                0,
            )
            .into()
        });
    }
}

pub mod msg {
    use iwasm_specification::host_ops::ShortHostOp;

    use super::{handle_host_call, short_host_op, types::{Address}};

    pub fn sender() -> Address {
        let mut dst = Address::new();
        handle_host_call(|| unsafe { 
            short_host_op(
                ShortHostOp::MessageData, 
                1,
                dst.as_mut_ptr() as usize as u32,
                0
            )
            .into()
        });

        dst
    }
}

pub mod storage {
    use core::mem::MaybeUninit;

    use iwasm_specification::host_ops::ShortHostOp;

    use crate::{
        handle_host_call, short_host_op,
        types::ints::{U256, U256BE},
    };

    pub fn read_s(ix: &U256BE) -> U256BE {
        let mut dst = MaybeUninit::uninit();

        handle_host_call(|| unsafe {
            short_host_op(
                ShortHostOp::StorageRead,
                0,
                ix as *const _ as usize as u32,
                dst.as_mut_ptr() as usize as u32,
            )
            .into()
        });

        // Safety: host will either write the data or terminate.
        unsafe { dst.assume_init() }
    }

    pub fn write_s(ix: &U256BE, value: &U256BE) {
        handle_host_call(|| unsafe {
            short_host_op(
                ShortHostOp::StorageWrite,
                0,
                ix as *const _ as usize as u32,
                value as *const _ as usize as u32,
            )
            .into()
        });
    }
}

pub mod slice {
    use core::mem::MaybeUninit;

    use super::{handle_host_call, short_host_op, types::ints::U256BE};

    pub fn hash_keccak256(input: &[u8]) -> U256BE {
        let mut dst = MaybeUninit::uninit();

        handle_host_call(|| unsafe {
            short_host_op(
                iwasm_specification::host_ops::ShortHostOp::HashKeccak256, 
                input.len() as u64, 
                input.as_ptr() as usize as u32, 
                dst.as_mut_ptr() as usize as u32,
            ).into()
        });

        unsafe { dst.assume_init() }
    }
}
