#![cfg_attr(not(test), no_std)]

// Custom types below are NOT Copy in Rust's sense, even though Clone internally would use copy

#[cfg(not(feature = "delegation"))]
// #[cfg(not(all(target_arch = "riscv32", feature = "delegation")))]
// #[cfg(not(any(all(target_arch = "riscv32", feature = "delegation"), test)))]
mod naive;

#[cfg(not(feature = "delegation"))]
// #[cfg(not(all(target_arch = "riscv32", feature = "delegation")))]
// #[cfg(not(any(all(target_arch = "riscv32", feature = "delegation"), test)))]
pub use self::naive::U256;

// #[cfg(all(not(target_arch = "riscv32"), feature = "delegation"))]
// const _: () = { compile_error!("`delegation` feature can only be used on RISC-V arch") };

#[cfg(feature = "delegation")]
// #[cfg(all(target_arch = "riscv32", feature = "delegation"))]
// #[cfg(any(all(target_arch = "riscv32", feature = "delegation"), test))]
mod risc_v;

#[cfg(feature = "delegation")]
// #[cfg(all(target_arch = "riscv32", feature = "delegation"))]
// #[cfg(any(all(target_arch = "riscv32", feature = "delegation"), test))]
pub use self::risc_v::U256;

#[derive(Debug)]
pub struct BitIteratorBE<Slice: AsRef<[u64]>> {
    s: Slice,
    n: usize,
}

impl<Slice: AsRef<[u64]>> BitIteratorBE<Slice> {
    pub fn new_without_leading_zeros(s: Slice) -> Self {
        let slice: &[u64] = s.as_ref();
        let mut n = slice.len() * 64;
        for word in slice.iter().rev() {
            if *word != 0 {
                n -= word.leading_zeros() as usize;
                break;
            } else {
                n -= 64;
            }
        }
        BitIteratorBE { s, n }
    }
}

impl<Slice: AsRef<[u64]>> Iterator for BitIteratorBE<Slice> {
    type Item = bool;

    fn next(&mut self) -> Option<bool> {
        if self.n == 0 {
            None
        } else {
            self.n -= 1;
            let part = self.n / 64;
            let bit = self.n - (64 * part);

            Some(self.s.as_ref()[part] & (1 << bit) > 0)
        }
    }
}
