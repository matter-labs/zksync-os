pub mod aligned_buffer;
pub mod aligned_vector;
pub mod bytes32;
pub mod cheap_clone;
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
