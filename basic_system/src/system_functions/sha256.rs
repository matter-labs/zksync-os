use crate::cost_constants::{
    SHA256_BASE_NATIVE_COST, SHA256_CHUNK_SIZE, SHA256_PER_WORD_COST_ERGS,
    SHA256_ROUND_NATIVE_COST, SHA256_STATIC_COST_ERGS,
};
use zk_ee::system::base_system_functions::{Sha256Errors, SystemFunction};
use zk_ee::system::errors::{subsystem::SubsystemError, system::SystemError};
use zk_ee::system::{Computational, Resources};

///
/// SHA-256 system function implementation.
///
pub struct Sha256Impl;

impl<R: Resources> SystemFunction<R, Sha256Errors> for Sha256Impl {
    /// If output len less than needed(32) returns `InternalError`.
    /// Returns `OutOfGas` if not enough resources provided.
    fn execute<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        src: &[u8],
        dst: &mut D,
        resources: &mut R,
        _: A,
    ) -> Result<(), SubsystemError<Sha256Errors>> {
        Ok(cycle_marker::wrap_with_resources!("sha256", resources, {
            sha256_as_system_function_inner(src, dst, resources)
        })?)
    }
}

fn nb_rounds(len: usize) -> u64 {
    let full_chunks = len / SHA256_CHUNK_SIZE;
    let tail = len % SHA256_CHUNK_SIZE;
    let num_rounds: u64 = full_chunks as u64;
    if tail <= 55 {
        num_rounds + 1
    } else {
        num_rounds + 2
    }
}

fn sha256_as_system_function_inner<D: ?Sized + Extend<u8>, R: Resources>(
    src: &[u8],
    dst: &mut D,
    resources: &mut R,
) -> Result<(), SystemError> {
    let word_size = src.len().div_ceil(32);
    let ergs_cost = SHA256_STATIC_COST_ERGS + SHA256_PER_WORD_COST_ERGS.times(word_size as u64);
    let native_cost = SHA256_BASE_NATIVE_COST + nb_rounds(src.len()) * SHA256_ROUND_NATIVE_COST;
    resources.charge(&R::from_ergs_and_native(
        ergs_cost,
        <R::Native as Computational>::from_computational(native_cost),
    ))?;

    use crypto::sha256::*;
    let mut hasher = Sha256::new();
    hasher.update(src);
    let hash = hasher.finalize();

    dst.extend(hash);

    Ok(())
}

#[cfg(test)]
mod test {

    use zk_ee::reference_implementations::BaseResources;
    use zk_ee::reference_implementations::DecreasingNative;
    use zk_ee::system::Resource;

    use super::*;

    #[test]
    fn test_sha256_as_system_function_inner_empty() {
        let src: &[u8] = &[];
        let mut dst = vec![];

        let mut resources = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

        sha256_as_system_function_inner(src, &mut dst, &mut resources).expect("hashing");
        assert_eq!(dst.len(), 32, "Size should be 32");
        // SHA256 for empty input
        let reference: [u8; 32] =
            hex::decode("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
                .expect("should decode hex")
                .try_into()
                .unwrap();
        assert_eq!(dst, reference, "hash should be equal to reference")
    }

    #[test]
    fn test_sha256_as_system_function_inner_one_round_exact() {
        // Reference vector:
        let src: &[u8] = &hex::decode("27e28f952e6dc6a200f40a885f27fbec03a0c715451a3af7c8ff371f88116828512f2e9ad07ca116702cb974dcef733a837b221b3c7651ce39fd40d99b7305bf").expect("should decode hex");
        let mut dst = vec![];
        let mut resources = BaseResources::<DecreasingNative>::FORMAL_INFINITE;

        sha256_as_system_function_inner(src, &mut dst, &mut resources).expect("hashing");
        assert_eq!(dst.len(), 32, "Size should be 32");
        // SHA256 of src
        let reference: [u8; 32] =
            hex::decode("6563a499c6c08bda61b76cc82de1d5af1b9bdc315bd21d60564b562e4a4a86fe")
                .expect("should decode hex")
                .try_into()
                .unwrap();
        assert_eq!(dst, reference, "hash should be equal to reference")
    }

    #[test]
    fn test_sha256_as_system_function_inner_multiple_rounds() {
        // Input vector:
        let src: &[u8] = &hex::decode("e61f6836aae8963f41ef33890d5cbcc9acef3a60493e8444a2b338632e2fc1bf106b656e1c6f8a80c9387f439422cc9fd0c3ecd47d9ea557ece661233232308f017b17acd1a4b37d8e34356d77ceb0ba470eba36de4620366bba52e010124fc854bbfb9e419b49155ffa777858c6358efc84ef4ba57965855410429bd087d30a46c8f120386796a154b6166a3c852c20472ef0c29063103da01b29ea1eee4d6cf4a42c55eba8d0c08889be165b6df012d0357cad83316ca997e5fd33e58e6b48a80c2bb1c73b12b97d9453d965a408aef7137eb2da4aaed6878ac698f57ecd920462b6252408523033c640b7830b27b3ce8c24289c47bbb8ee1a4a950d9088b47208a6615917f43d05258378374a6ba6a0d04e0fc6975ba319c599c0187acc438b7edc4eaa97a9d20e7e71ea").expect("should decode hex");
        let mut dst = vec![];

        let mut resources = BaseResources::<DecreasingNative>::FORMAL_INFINITE;

        sha256_as_system_function_inner(src, &mut dst, &mut resources).expect("hashing");
        assert_eq!(dst.len(), 32, "Size should be 32");
        // SHA256 of src
        let reference: [u8; 32] =
            hex::decode("47b651ba0453fda4b108f038c834773db1ff76f9636669862056c70c2f509cd6")
                .expect("should decode hex")
                .try_into()
                .unwrap();
        assert_eq!(dst, reference, "hash should be equal to reference")
    }
}
