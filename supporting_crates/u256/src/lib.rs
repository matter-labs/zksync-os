#![cfg_attr(not(test), no_std)]

// Custom types below are NOT Copy in Rust's sense, even though Clone internally would use copy

#[cfg(not(all(target_arch = "riscv32", feature = "delegation")))]
mod naive;

#[cfg(not(all(target_arch = "riscv32", feature = "delegation")))]
pub use self::naive::U256;

#[cfg(all(not(target_arch = "riscv32"), feature = "delegation"))]
const _: () = { compile_error!("`delegation` feature can only be used on RISC-V arch") };

// #[cfg(all(target_arch = "riscv32", feature = "delegation"))]
#[cfg(any(all(target_arch = "riscv32", feature = "delegation"), test))]
mod risc_v;

#[cfg(all(target_arch = "riscv32", feature = "delegation"))]
pub use self::risc_v::U256;

pub fn init() {
    #[cfg(any(all(target_arch = "riscv32", feature = "delegation"), test))]
    {
        crypto::bigint_riscv::init();
    }
}
