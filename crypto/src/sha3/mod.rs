pub use sha3::*;

impl crate::MiniDigest for Keccak256 {
    type HashOutput = [u8; 32];

    #[inline(always)]
    fn new() -> Self {
        <Keccak256 as Digest>::new()
    }

    #[inline(always)]
    fn digest(input: impl AsRef<[u8]>) -> Self::HashOutput {
        let mut hasher = <Keccak256 as Digest>::new();
        <Keccak256 as Digest>::update(&mut hasher, input);
        let digest = <Keccak256 as Digest>::finalize(hasher);
        let mut result = [0u8; 32];
        #[allow(deprecated)]
        result.copy_from_slice(digest.as_slice());
        result
    }

    #[inline(always)]
    fn update(&mut self, input: impl AsRef<[u8]>) {
        <Keccak256 as Digest>::update(self, input);
    }

    #[inline(always)]
    fn finalize(self) -> Self::HashOutput {
        <Keccak256 as Digest>::finalize(self).into()
    }

    #[inline(always)]
    fn finalize_reset(&mut self) -> Self::HashOutput {
        <Keccak256 as Digest>::finalize_reset(self).into()
    }
}
