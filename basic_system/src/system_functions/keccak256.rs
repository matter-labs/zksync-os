use zk_ee::system::base_system_functions::{Keccak256Errors, SystemFunction};
use zk_ee::system::errors::subsystem::SubsystemError;

use super::*;

use crate::cost_constants::{
    KECCAK256_BASE_NATIVE_COST, KECCAK256_CHUNK_SIZE, KECCAK256_PER_WORD_COST_ERGS,
    KECCAK256_ROUND_NATIVE_COST, KECCAK256_STATIC_COST_ERGS,
};

///
/// keccak256 system function implementation.
///
pub struct Keccak256Impl;

impl<R: Resources> SystemFunction<R, Keccak256Errors> for Keccak256Impl {
    /// Returns `OutOfGas` if not enough resources provided.
    fn execute<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        _allocator: A,
    ) -> Result<(), SubsystemError<Keccak256Errors>> {
        cycle_marker::wrap_with_resources!("keccak", resources, {
            keccak256_as_system_function_inner(input, output, resources)
        })
    }
}

pub fn keccak256_native_cost_u64(len: usize) -> u64 {
    let rounds = core::cmp::max(1, len.div_ceil(KECCAK256_CHUNK_SIZE));
    (rounds as u64) * KECCAK256_ROUND_NATIVE_COST + KECCAK256_BASE_NATIVE_COST
}

pub fn keccak256_native_cost<R: Resources>(len: usize) -> R::Native {
    use zk_ee::system::Computational;
    R::Native::from_computational(keccak256_native_cost_u64(len))
}

fn keccak256_as_system_function_inner<D: ?Sized + Extend<u8>, R: Resources>(
    src: &[u8],
    dst: &mut D,
    resources: &mut R,
) -> Result<(), SubsystemError<Keccak256Errors>> {
    let words = src.len().div_ceil(32);
    let ergs_cost = KECCAK256_STATIC_COST_ERGS + KECCAK256_PER_WORD_COST_ERGS.times(words as u64);
    let native_cost = keccak256_native_cost::<R>(src.len());
    resources.charge(&R::from_ergs_and_native(ergs_cost, native_cost))?;

    use crypto::sha3::*;
    let mut hasher = Keccak256::new();
    hasher.update(src);
    let hash = hasher.finalize();

    dst.extend(hash);

    Ok(())
}

#[cfg(test)]
mod test {

    use super::*;
    use hex;
    use zk_ee::reference_implementations::BaseResources;
    use zk_ee::reference_implementations::DecreasingNative;
    use zk_ee::system::Resource;

    #[test]
    fn test_keccak_as_system_function_inner_empty() {
        let src: &[u8] = &[];
        let mut dst = vec![];

        let mut resources = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

        keccak256_as_system_function_inner(src, &mut dst, &mut resources).expect("hashing");
        assert_eq!(dst.len(), 32, "Size should be 32");
        // Keccak256 for empty input
        let reference: [u8; 32] =
            hex::decode("c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470")
                .expect("should decode hex")
                .try_into()
                .unwrap();
        assert_eq!(dst, reference, "hash should be equal to reference")
    }

    #[test]
    fn test_keccak_as_system_function_inner_one_round_exact() {
        // Input vector:
        let src: &[u8] = &hex::decode("aa51048a26605ae06c17d2e43b5a86f6905d97a43692ce89523b9359f91dd79db184b8707af8f6af6c0c52254a9056aaed669c8b08323d1e36d08f36389631afa978198c67a624587e1827a5324fd18fa4d97680e776806b5634790525fc78209ca06dc614a99672adddb7d93464edd4a8c3849b10421cb266f245d8f1aeba9e4e3951fd23275c84").expect("should decode hex");
        let mut dst = vec![];

        let mut resources = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

        keccak256_as_system_function_inner(src, &mut dst, &mut resources).expect("hashing");
        assert_eq!(dst.len(), 32, "Size should be 32");
        // Keccak256 of input vector
        let reference: [u8; 32] =
            hex::decode("b7ab06b9d3572d5a186dba7a2d85dfb741c0b8e9230484e5b78b22c44b4b8484")
                .expect("should decode hex")
                .try_into()
                .unwrap();
        assert_eq!(dst, reference, "hash should be equal to reference")
    }

    #[test]
    fn test_keccak_as_system_function_inner_multiple_rounds() {
        // Input vector:
        let src: &[u8] = &hex::decode("e61f6836aae8963f41ef33890d5cbcc9acef3a60493e8444a2b338632e2fc1bf106b656e1c6f8a80c9387f439422cc9fd0c3ecd47d9ea557ece661233232308f017b17acd1a4b37d8e34356d77ceb0ba470eba36de4620366bba52e010124fc854bbfb9e419b49155ffa777858c6358efc84ef4ba57965855410429bd087d30a46c8f120386796a154b6166a3c852c20472ef0c29063103da01b29ea1eee4d6cf4a42c55eba8d0c08889be165b6df012d0357cad83316ca997e5fd33e58e6b48a80c2bb1c73b12b97d9453d965a408aef7137eb2da4aaed6878ac698f57ecd920462b6252408523033c640b7830b27b3ce8c24289c47bbb8ee1a4a950d9088b47208a6615917f43d05258378374a6ba6a0d04e0fc6975ba319c599c0187acc438b7edc4eaa97a9d20e7e71ea").expect("should decode hex");
        let mut dst = vec![];

        let mut resources = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

        keccak256_as_system_function_inner(src, &mut dst, &mut resources).expect("hashing");
        assert_eq!(dst.len(), 32, "Size should be 32");
        // Keccak256 of src
        let reference: [u8; 32] =
            hex::decode("128893d959a098e876f38fe4c73b2f8d098a8f7bb9657d0a304f847c3ee086bb")
                .expect("should decode hex")
                .try_into()
                .unwrap();
        assert_eq!(dst, reference, "hash should be equal to reference")
    }
}
