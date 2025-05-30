pub mod aligned_buffer;
pub mod aligned_vector;
pub mod bytes32;
pub mod convenience;
pub mod integer_utils;
pub mod stack_linked_list;
pub mod type_assert;

use crypto::MiniDigest;

pub use self::aligned_buffer::*;
pub use self::aligned_vector::*;
pub use self::bytes32::*;
pub use self::convenience::*;
pub use self::integer_utils::*;
pub use self::type_assert::*;

pub static mut GLOBAL_ALLOC_ALLOWED: bool = false;

pub fn with_global_allocator<T>(closure: impl FnOnce() -> T) -> T {
    #[cfg(target_arch = "riscv32")]
    unsafe {
        GLOBAL_ALLOC_ALLOWED = true;
    }
    let result = (closure)();
    #[cfg(target_arch = "riscv32")]
    unsafe {
        GLOBAL_ALLOC_ALLOWED = false;
    }

    result
}

pub struct NopHasher;

impl MiniDigest for NopHasher {
    type HashOutput = ();

    fn new() -> Self {
        Self
    }
    fn digest(_input: impl AsRef<[u8]>) -> Self::HashOutput {}
    fn update(&mut self, _input: impl AsRef<[u8]>) {}
    fn finalize(self) -> Self::HashOutput {}
}
