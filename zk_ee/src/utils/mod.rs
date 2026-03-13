pub mod aligned_vector;
pub mod bytes32;
pub mod cheap_clone;
pub mod convenience;
pub mod integer_utils;
pub mod stack_linked_list;
pub mod type_assert;
pub mod usize_rw;
pub mod write_bytes;

pub const USIZE_ALIGNMENT: usize = core::mem::align_of::<usize>();
pub const USIZE_SIZE: usize = core::mem::size_of::<usize>();
pub const U64_SIZE: usize = core::mem::size_of::<u64>();
pub const U64_ALIGNMENT: usize = core::mem::align_of::<u64>();

const _: () = const {
    assert!(U64_ALIGNMENT >= USIZE_ALIGNMENT);
    assert!(U64_SIZE >= USIZE_SIZE);
};

use crypto::MiniDigest;

pub use self::aligned_vector::*;
pub use self::bytes32::*;
pub use self::convenience::*;
pub use self::integer_utils::*;
pub use self::type_assert::*;

pub struct NopHasher;

impl MiniDigest for NopHasher {
    type HashOutput = ();

    fn new() -> Self {
        Self
    }
    fn digest(_input: impl AsRef<[u8]>) -> Self::HashOutput {}
    fn update(&mut self, _input: impl AsRef<[u8]>) {}
    fn finalize(self) -> Self::HashOutput {}
    fn finalize_reset(&mut self) -> Self::HashOutput {}
}
