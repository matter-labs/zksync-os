#![feature(generic_const_exprs)]
#![cfg_attr(target_arch = "wasm32", no_std)]

use alloc::vec::Vec;
use syslib::{contract, types::ints::U256};

#[derive(Default)]
struct Contract {
}

#[contract]
impl Contract {
    pub fn memory_alloc_heavy(&self, seed: &U256) -> Result<U256, &str> {

        // We need to allocate ~1MB of memory in small chunks.
        // Not using uintx so not to interfere with benchmarking.
        
        let bs = seed.as_bytes();

        let s1 = u64::from_le_bytes(bs[0..8].try_into().unwrap());
        let s2 = u64::from_le_bytes(bs[8..16].try_into().unwrap());
        let s3 = u64::from_le_bytes(bs[16..24].try_into().unwrap());
        let s4 = u64::from_le_bytes(bs[24..32].try_into().unwrap());

        let mut aggr = 1;

        for _ in 0 .. (1 << 5) {
            let mut v = Vec::new();

            for j in 0..(1 << 1) {
                v.push(j * s1);
            }

            for j in v.drain(..) {
                aggr += j;
            }
        }

        Ok(U256::from_usize(aggr as usize))
    }

    pub fn fibonacciish(&self, a: &U256, b: &U256, rounds: u32) -> Result<U256, &str> {
        let mut a = (**a).clone();
        let mut b = (**b).clone();

        for _ in 0 .. *rounds {
            let c = a.add(&b);

            a = b;
            b = c;
        }

        Ok(b)
    }
}
