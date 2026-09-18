use alloc::{alloc::Global, collections::BTreeMap};
use core::{alloc::Allocator, marker::PhantomData};
use storage_models::common_structs::{snapshottable_io::SnapshottableIo, PreimageCacheModel};
use zk_ee::common_structs::history_map::NopSnapshotId;
use zk_ee::oracle::query_ids::PREIMAGE_SUBSPACE_MASK;
use zk_ee::oracle::simple_oracle_query::SimpleOracleQuery;
use zk_ee::oracle::IOOracle;
use zk_ee::{
    common_structs::PreimageType,
    execution_environment_type::ExecutionEnvironmentType,
    internal_error,
    system::{
        errors::{internal::InternalError, system::SystemError},
        Resources,
    },
    utils::{Bytes32, UsizeAlignedByteBox, USIZE_SIZE},
};

use super::super::cost_constants::PREIMAGE_CACHE_GET_NATIVE_COST;
use crate::system_functions::keccak256::keccak256_native_cost;
use crate::system_implementation::ethereum_storage_model::ETHEREUM_STORAGE_SUBSPACE_MASK;
use crate::system_implementation::flat_storage_model::bytecode_padding_len;

pub struct PreimageLengthQuery;

#[allow(clippy::identity_op)]
pub const ETHEREUM_BYTECODE_LENGTH_FROM_PREIMAGE_QUERY_ID: u32 =
    PREIMAGE_SUBSPACE_MASK | ETHEREUM_STORAGE_SUBSPACE_MASK | 0x00;
pub const ETHEREUM_BYTECODE_PREIMAGE_QUERY_ID: u32 =
    PREIMAGE_SUBSPACE_MASK | ETHEREUM_STORAGE_SUBSPACE_MASK | 0x01;

impl SimpleOracleQuery for PreimageLengthQuery {
    const QUERY_ID: u32 = ETHEREUM_BYTECODE_LENGTH_FROM_PREIMAGE_QUERY_ID;
    type Input = Bytes32;
    type Output = u32;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PreimageRequestForUnknownLength {
    pub hash: Bytes32,
    pub preimage_type: PreimageType,
}

/// Bytecode with its jumpdest artifacts, in the layout that EVM interpreter takes for the code
/// with cached artifacts: `code || padding to the alignment || artifacts`.
///
/// The artifacts only depend on the code, and the same code is usually executed many times over
/// a block (every call frame needs the artifacts), so they are computed once, when the code gets
/// here, and live as long as the code does.
pub struct BytecodeWithArtifacts<A: Allocator> {
    buffer: UsizeAlignedByteBox<A>,
    code_len: usize,
    artifacts_len: usize,
}

impl<A: Allocator + Clone> BytecodeWithArtifacts<A> {
    fn new_in(code: &[&[u8]], allocator: A) -> Self {
        let code_len: usize = code.iter().map(|el| el.len()).sum();
        // artifacts are made over the contiguous code, so we need a copy of it if it is in pieces
        let contiguous;
        let code: &[u8] = match code {
            [single] => single,
            _ => {
                contiguous = UsizeAlignedByteBox::from_slices_in(code, allocator.clone());
                contiguous.as_slice()
            }
        };
        let artifacts = evm_interpreter::BytecodePreprocessingData::create_artifacts_inner(
            allocator.clone(),
            code,
        );
        let padding = [0u8; evm_interpreter::BYTECODE_ALIGNMENT - 1];
        let padding = &padding[..bytecode_padding_len(code_len)];
        let artifacts = artifacts.as_slice();
        let buffer = UsizeAlignedByteBox::from_slices_in(&[code, padding, artifacts], allocator);

        Self {
            buffer,
            code_len,
            artifacts_len: artifacts.len(),
        }
    }

    pub fn code(&self) -> &[u8] {
        &self.buffer.as_slice()[..self.code_len]
    }

    /// NOTE: it is the buffer on the heap that lives as long as the cache does, not `self`:
    /// the map is free to move its values around when other ones are inserted.
    ///
    /// Safety: IO implementer that will use it is expected to live beoynd any frame (as it's part
    /// of the OS), the entries are never removed from the cache, and their buffers are never
    /// reallocated, so we can extend the lifetime
    fn as_executable(&self) -> ExecutableBytecode {
        let bytecode: &'static [u8] = unsafe { core::mem::transmute(self.buffer.as_slice()) };

        ExecutableBytecode {
            bytecode,
            code_len: self.code_len as u32,
            artifacts_len: self.artifacts_len as u32,
        }
    }
}

/// Bytecode in the form the execution environment takes it
#[derive(Clone, Copy)]
pub struct ExecutableBytecode {
    /// `code || padding || artifacts`
    pub bytecode: &'static [u8],
    pub code_len: u32,
    pub artifacts_len: u32,
}

impl ExecutableBytecode {
    pub fn code(&self) -> &'static [u8] {
        &self.bytecode[..self.code_len as usize]
    }
}

pub struct BytecodeKeccakPreimagesStorage<R: Resources, A: Allocator + Clone = Global> {
    pub storage: BTreeMap<Bytes32, BytecodeWithArtifacts<A>, A>,
    pub(crate) allocator: A,
    /// Every preimage is hashed to be verified. The hasher (its state is big and aligned for
    /// the delegation) is kept here and reset after every use instead of being created, and then
    /// moved into `finalize`, every time.
    hasher: crypto::sha3::Keccak256,
    _marker: PhantomData<R>,
}

impl<R: Resources, A: Allocator + Clone> BytecodeKeccakPreimagesStorage<R, A> {
    pub fn new_from_parts(allocator: A) -> Self {
        use crypto::MiniDigest;

        Self {
            storage: BTreeMap::new_in(allocator.clone()),
            allocator,
            hasher: crypto::sha3::Keccak256::new(),
            _marker: PhantomData,
        }
    }

    fn keccak256(&mut self, input: &[u8]) -> Bytes32 {
        use crypto::MiniDigest;

        self.hasher.update(input);
        Bytes32::from_array(self.hasher.finalize_reset())
    }

    /// Same as `get_preimage`, but gives the code together with its artifacts
    pub fn get_executable_bytecode<const PROOF_ENV: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        hash: &Bytes32,
        resources: &mut R,
        oracle: &mut impl IOOracle,
    ) -> Result<ExecutableBytecode, SystemError> {
        self.expose_preimage::<PROOF_ENV>(ee_type, hash, resources, oracle)
    }

    #[must_use]
    fn expose_preimage<const PROOF_ENV: bool>(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        hash: &Bytes32,
        resources: &mut R,
        oracle: &mut impl IOOracle,
    ) -> Result<ExecutableBytecode, SystemError> {
        use zk_ee::system::Computational;
        resources.charge(&R::from_native(R::Native::from_computational(
            PREIMAGE_CACHE_GET_NATIVE_COST,
        )))?;
        if let Some(cached) = self.storage.get(hash) {
            Ok(cached.as_executable())
        } else {
            // We do not charge for gas in this concrete implementation and
            // expect higher-level model todo so.
            // We charge for native.
            let expected_length_in_bytes =
                PreimageLengthQuery::get(oracle, hash).expect("must get preimage length") as usize;
            // NOTE: we leave some slack for 64/32 bit arch mismatches
            let buffer_size = expected_length_in_bytes.next_multiple_of(USIZE_SIZE) / USIZE_SIZE;
            let buffer_size = buffer_size.next_multiple_of(2);
            // The code is read right into the buffer that has the place for its artifacts
            // after it, and they are computed there
            let code_buffer_size = buffer_size;
            let artifacts_len = evm_interpreter::artifacts_byte_len(expected_length_in_bytes);
            debug_assert_eq!(artifacts_len % USIZE_SIZE, 0);
            let artifacts_buffer_size = artifacts_len / USIZE_SIZE;
            let buffer = UsizeAlignedByteBox::from_init_fn_in(
                code_buffer_size + artifacts_buffer_size,
                |dst| {
                    let (code_dst, artifacts_dst) = dst.split_at_mut(code_buffer_size);
                    let written = oracle
                        .expose_preimage(ETHEREUM_BYTECODE_PREIMAGE_QUERY_ID, hash, code_dst)
                        .expect("must get preimage");
                    for word in code_dst.iter_mut().skip(written) {
                        word.write(0);
                    }
                    for word in artifacts_dst.iter_mut() {
                        word.write(0);
                    }
                    // SAFETY: all of `dst` is initialized above, and the code is the beginning
                    // of the first part (the length fits by the computation of `buffer_size`)
                    unsafe {
                        // whatever comes after the code in its last words is the padding
                        let code = core::slice::from_raw_parts(
                            code_dst.as_ptr().cast::<u8>(),
                            expected_length_in_bytes,
                        );
                        let artifacts = core::slice::from_raw_parts_mut(
                            artifacts_dst.as_mut_ptr().cast::<usize>(),
                            artifacts_buffer_size,
                        );
                        evm_interpreter::analyze_into(code, artifacts);
                    }

                    code_buffer_size + artifacts_buffer_size
                },
                self.allocator.clone(),
            );
            let buffered = BytecodeWithArtifacts {
                buffer,
                code_len: expected_length_in_bytes,
                artifacts_len,
            };

            let native_cost = keccak256_native_cost::<R>(expected_length_in_bytes);
            resources.charge(&R::from_native(native_cost))?;

            if PROOF_ENV {
                let recomputed_hash = self.keccak256(buffered.code());

                if recomputed_hash != *hash {
                    return Err(internal_error!("Account hash mismatch").into());
                }
            } else {
                debug_assert!(self.keccak256(buffered.code()) == *hash);
            }

            Ok(self
                .storage
                .entry(*hash)
                .or_insert(buffered)
                .as_executable())
        }
    }

    fn insert_verified_preimage(
        &mut self,
        hash: &Bytes32,
        preimage: &[&[u8]],
    ) -> Result<&'static [u8], SystemError> {
        let allocator = self.allocator.clone();
        let inserted = self
            .storage
            .entry(*hash)
            .or_insert_with(|| BytecodeWithArtifacts::new_in(preimage, allocator));

        Ok(inserted.as_executable().code())
    }
}

impl<R: Resources, A: Allocator + Clone> SnapshottableIo for BytecodeKeccakPreimagesStorage<R, A> {
    type StateSnapshot = NopSnapshotId;

    fn begin_new_tx(&mut self) {}

    fn finish_tx(&mut self) -> Result<(), InternalError> {
        Ok(())
    }

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
        let cached = self.expose_preimage::<PROOF_ENV>(ee_type, hash, resources, oracle)?;

        Ok(cached.code())
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

        self.insert_verified_preimage(hash, preimage)
    }
}
