use alloc::{alloc::Global, collections::BTreeMap};
use core::{alloc::Allocator, marker::PhantomData};
use storage_models::common_structs::{snapshottable_io::SnapshottableIo, PreimageCacheModel};
use zk_ee::common_structs::history_map::NopSnapshotId;
use zk_ee::{
    common_structs::PreimageType,
    execution_environment_type::ExecutionEnvironmentType,
    internal_error,
    system::{
        errors::{internal::InternalError, system::SystemError},
        Resources,
    },
    system_io_oracle::{IOOracle, SimpleOracleQuery, PREIMAGE_SUBSPACE_MASK},
    utils::{Bytes32, UsizeAlignedByteBox, USIZE_SIZE},
};

use super::super::cost_constants::PREIMAGE_CACHE_GET_NATIVE_COST;
use crate::system_functions::keccak256::keccak256_native_cost;
use crate::system_implementation::ethereum_storage_model::ETHEREUM_STORAGE_SUBSPACE_MASK;

pub struct PreimageLengthQuery;

pub const ETHEREUM_BYTECODE_LENGTH_FROM_PREIMAGE_QUERY_ID: u32 =
    PREIMAGE_SUBSPACE_MASK | ETHEREUM_STORAGE_SUBSPACE_MASK;
pub const ETHEREUM_BYTECODE_PREIMAGE_QUERY_ID: u32 =
    PREIMAGE_SUBSPACE_MASK | ETHEREUM_STORAGE_SUBSPACE_MASK | 0x01;

impl SimpleOracleQuery for PreimageLengthQuery {
    const QUERY_ID: u32 = ETHEREUM_BYTECODE_LENGTH_FROM_PREIMAGE_QUERY_ID;
    type Input = Bytes32;
    type Output = u32;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PreimageRequestForUnknownLength {
    pub hash: Bytes32,
    pub preimage_type: PreimageType,
}

pub struct BytecodeKeccakPreimagesStorage<R: Resources, A: Allocator + Clone = Global> {
    pub storage: BTreeMap<Bytes32, UsizeAlignedByteBox<A>, A>,
    pub(crate) allocator: A,
    _marker: PhantomData<R>,
}

impl<R: Resources, A: Allocator + Clone> BytecodeKeccakPreimagesStorage<R, A> {
    pub fn new_from_parts(allocator: A) -> Self {
        Self {
            storage: BTreeMap::new_in(allocator.clone()),
            allocator,
            _marker: PhantomData,
        }
    }

    #[must_use]
    fn expose_preimage<const PROOF_ENV: bool>(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        hash: &Bytes32,
        resources: &mut R,
        oracle: &mut impl IOOracle,
    ) -> Result<&'static [u8], SystemError> {
        use zk_ee::system::Computational;
        resources.charge(&R::from_native(R::Native::from_computational(
            PREIMAGE_CACHE_GET_NATIVE_COST,
        )))?;
        if let Some(cached) = self.storage.get(hash) {
            unsafe {
                let cached: &'static [u8] = core::mem::transmute(cached.as_slice());

                Ok(cached)
            }
        } else {
            // We do not charge for gas in this concrete implementation and
            // expect higher-level model todo so.
            // We charge for native.
            let expected_length_in_bytes =
                PreimageLengthQuery::get(oracle, hash).expect("must get preimage length") as usize;
            // NOTE: we leave some slack for 64/32 bit arch mismatches
            let buffer_size = expected_length_in_bytes.next_multiple_of(USIZE_SIZE) / USIZE_SIZE;
            let buffer_size = buffer_size.next_multiple_of(2);
            let mut buffered = UsizeAlignedByteBox::from_init_fn_in(
                buffer_size,
                |dst| {
                    oracle
                        .expose_preimage(ETHEREUM_BYTECODE_PREIMAGE_QUERY_ID, hash, dst)
                        .expect("must get preimage")
                },
                self.allocator.clone(),
            );
            // truncate
            buffered.truncated_to_byte_length(expected_length_in_bytes);

            let native_cost = keccak256_native_cost::<R>(expected_length_in_bytes);
            resources.charge(&R::from_native(native_cost))?;

            if PROOF_ENV {
                use crypto::sha3::Keccak256;
                use crypto::MiniDigest;
                let recomputed_hash = Bytes32::from_array(Keccak256::digest(buffered.as_slice()));

                if recomputed_hash != *hash {
                    return Err(internal_error!("Account hash mismatch").into());
                }
            } else {
                debug_assert!({
                    use crypto::sha3::Keccak256;
                    use crypto::MiniDigest;
                    let recomputed_hash =
                        Bytes32::from_array(Keccak256::digest(buffered.as_slice()));

                    recomputed_hash == *hash
                });
            }

            let inserted = self.storage.entry(*hash).or_insert(buffered);
            // Safety: IO implementer that will use it is expected to live beoynd any frame (as it's part of the OS),
            // so we can extend the lifetime
            unsafe {
                let cached: &'static [u8] = core::mem::transmute(inserted.as_slice());

                Ok(cached)
            }
        }
    }

    fn insert_verified_preimage(
        &mut self,
        hash: &Bytes32,
        preimage: UsizeAlignedByteBox<A>,
    ) -> Result<&'static [u8], SystemError> {
        let inserted = self.storage.entry(*hash).or_insert(preimage);

        unsafe {
            let cached: &'static [u8] = core::mem::transmute(inserted.as_slice());

            Ok(cached)
        }
    }
}

impl<R: Resources, A: Allocator + Clone> SnapshottableIo for BytecodeKeccakPreimagesStorage<R, A> {
    type StateSnapshot = NopSnapshotId;

    fn begin_new_tx(&mut self) {}

    fn start_frame(&mut self) -> Self::StateSnapshot {
        NopSnapshotId::new()
    }

    fn finish_frame(
        &mut self,
        _rollback_handle: Option<&Self::StateSnapshot>,
    ) -> Result<(), InternalError> {
        Ok(())
    }
}

impl<R: Resources, A: Allocator + Clone> PreimageCacheModel
    for BytecodeKeccakPreimagesStorage<R, A>
{
    type Resources = R;
    type PreimageRequest = PreimageRequestForUnknownLength;

    fn get_preimage<const PROOF_ENV: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        preimage_type: &Self::PreimageRequest,
        resources: &mut Self::Resources,
        oracle: &mut impl IOOracle,
    ) -> Result<&'static [u8], SystemError> {
        // we will NOT charge for preimages in here, but instead higher-level model should do it

        let PreimageRequestForUnknownLength { hash, .. } = preimage_type;

        // preimage type is not important in our case, we do not version them yet
        self.expose_preimage::<PROOF_ENV>(ee_type, hash, resources, oracle)
    }

    fn record_preimage<const PROOF_ENV: bool>(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        preimage_type: &Self::PreimageRequest,
        resources: &mut Self::Resources,
        preimage: &[&[u8]],
    ) -> Result<&'static [u8], SystemError> {
        use crate::system_implementation::flat_storage_model::cost_constants::PREIMAGE_CACHE_SET_NATIVE_COST;
        use zk_ee::system::Computational;
        // we will NOT charge ergs for preimages in here, but instead higher-level model should do it
        resources.charge(&R::from_native(R::Native::from_computational(
            PREIMAGE_CACHE_SET_NATIVE_COST,
        )))?;

        let PreimageRequestForUnknownLength { hash, .. } = preimage_type;

        let boxed_data = UsizeAlignedByteBox::from_slices_in(preimage, self.allocator.clone());

        self.insert_verified_preimage(hash, boxed_data)
    }
}
