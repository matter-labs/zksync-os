// This is generic trait that ensures that for whatever structure representing the "current block",
// it can also self-verify that it's a "chain" if some sort. Structure is itself responsible to parse
// the format of "previous" items

use crate::bootloader::errors::BootloaderSubsystemError;
use core::alloc::Allocator;
use zk_ee::oracle::IOOracle;

pub trait ChainChecker {
    type ExtraData;
    type Output;

    fn verify_chain<A: Allocator + Clone>(
        &self,
        current_block_number: u64,
        verification_depth: usize,
        oracle: &mut impl IOOracle,
        extra_data: &Self::ExtraData,
        allocator: A,
    ) -> Result<Self::Output, BootloaderSubsystemError>;
}
