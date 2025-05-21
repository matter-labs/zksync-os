
use core::alloc::Allocator;
use alloc::vec::Vec;

pub use u256::U256;

mod mpnat;
mod arith;
mod double;

pub use mpnat::MPNatU256;
use zk_ee::{system::logger::Logger, system_io_oracle::IOOracle};

pub(super) fn modexp<O: IOOracle, L: Logger, A: Allocator + Clone>(
    base: &[u8],
    exp: &[u8],
    modulus: &[u8],
    oracle: &mut O,
    logger: &mut L,
    allocator: A,
) -> Vec<u8, A> {

    let m = MPNatU256::from_big_endian(&modulus, allocator.clone());
    let output = if m.digits.len() == 1 && m.digits[0] == u256::U256::ZERO {
        Vec::new_in(allocator)
    } else {
        let mut x = MPNatU256::from_big_endian(&base, allocator.clone());
        let mut x = x.modpow(&exp, &m, oracle, logger, allocator.clone());
        x.trim();
        let r = x.to_big_endian(allocator);
        r
    };

    output
}
