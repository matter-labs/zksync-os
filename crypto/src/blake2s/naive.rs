#[derive(Clone, Debug)]
pub struct Blake2s256 {
    inner: blake2::Blake2s256,
}

use core::convert::AsRef;

impl Blake2s256 {
    fn new_impl() -> Self {
        use blake2::Digest;
        Self {
            inner: blake2::Blake2s256::new(),
        }
    }

    #[allow(dead_code)]
    fn digest_impl(mut self, data: impl AsRef<[u8]>) -> [u8; 32] {
        self.update_impl(data);
        self.finalize_impl()
    }

    #[allow(dead_code)]
    fn update_impl(&mut self, data: impl AsRef<[u8]>) {
        use blake2::Digest;
        self.inner.update(data);
    }

    #[allow(dead_code)]
    fn finalize_impl(self) -> [u8; 32] {
        use blake2::Digest;
        self.inner.finalize().into()
    }
}

impl crate::MiniDigest for Blake2s256 {
    type HashOutput = [u8; 32];

    #[inline(always)]
    fn new() -> Self {
        Self::new_impl()
    }

    #[inline(always)]
    fn digest(input: impl AsRef<[u8]>) -> Self::HashOutput {
        <blake2::Blake2s256 as blake2::Digest>::digest(input).into()
    }

    #[inline(always)]
    fn update(&mut self, input: impl AsRef<[u8]>) {
        use blake2::Digest;
        self.inner.update(input);
    }

    #[inline(always)]
    fn finalize(self) -> Self::HashOutput {
        use blake2::Digest;
        self.inner.finalize().into()
    }

    #[inline(always)]
    fn finalize_reset(&mut self) -> Self::HashOutput {
        use blake2::Digest;
        self.inner.finalize_reset().into()
    }
}
