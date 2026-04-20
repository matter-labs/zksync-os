/// FRI verifier function pointer, registered at startup by `zksync_os/src/main.rs`.
///
/// On riscv32 the real `full_statement_verifier` is linked only into the `zksync_os` binary
/// crate (which is excluded from the workspace build). `basic_bootloader` does NOT depend on
/// `full_statement_verifier` directly; instead it calls through this pointer so that the
/// workspace build can compile without pulling in the verifier's std-requiring transitive deps.
///
/// On non-riscv32 (host / forward execution), the FRI proof is verified inline in the bootloader
/// using `verify_fri_statement_host` — this pointer is never called in that path.

use core::sync::atomic::{AtomicPtr, Ordering};

static FRI_VERIFIER_FN: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());

/// Register the FRI verifier function. Must be called before any FRI proof transaction is
/// processed. Called unconditionally from `zksync_os/src/main.rs` at startup.
pub fn register_fri_verifier(f: fn() -> [u32; 16]) {
    FRI_VERIFIER_FN.store(f as *mut (), Ordering::Relaxed);
}

/// Run the registered FRI verifier. Panics if no verifier has been registered.
pub fn run_fri_verifier() -> [u32; 16] {
    let ptr = FRI_VERIFIER_FN.load(Ordering::Relaxed);
    assert!(!ptr.is_null(), "FRI verifier not registered");
    let f: fn() -> [u32; 16] = unsafe { core::mem::transmute(ptr) };
    f()
}
