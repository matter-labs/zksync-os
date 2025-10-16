#[cfg(not(target_arch = "riscv32"))]
compile_error!("invalid arch - should only be compiled for RISC-V");

use super::AlignedState;
use seq_macro::seq;

// TODO: eventually Rust assembly will interpolate CSR indexes

use common_constants::{keccak_special5_invoke, keccak_special5_load_initial_control};

pub(crate) fn keccak_f1600(state: &mut AlignedState) {
    unsafe {
        let state_ptr = state.0.as_mut_ptr();
        
        // start by setting initial control
        keccak_special5_load_initial_control!();

        // then run 24 rounds
        seq!(round in 0..24 {
            // iota-theta-rho-chi-nopi: 5 iota_columnxor + 2 columnmix + 5 theta + 5 rho + 5*2 chi
            // control flow is guarded by circuit itself
            seq!(i in 0..27 {
                keccak_special5_invoke!(state_ptr);
            });
        });

        // then add +1 for the final iota
        keccak_special5_invoke!(state_ptr);
    }
}
