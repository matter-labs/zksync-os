use crypto::MiniDigest;

pub mod flat_storage_model;
pub mod memory;
pub mod system;

struct NopHasher;

impl MiniDigest for NopHasher {
    type HashOutput = ();

    fn new() -> Self {
        Self
    }
    fn digest(_input: impl AsRef<[u8]>) -> Self::HashOutput {}
    fn update(&mut self, _input: impl AsRef<[u8]>) {}
    fn finalize(self) -> Self::HashOutput {}
}
