use core::mem::MaybeUninit;
use zk_ee::{
    oracle::{memory_io::OracleQuery, query_ids::HISTORICAL_BLOCK_HASH_QUERY_ID, IOOracle},
    system::{
        errors::internal::InternalError,
        metadata::dynamic_metadata_responder::{DynamicMetadataResponder, MetadataRequest},
    },
    utils::Bytes32,
};

pub struct BlockHashMetadataRequest;

impl MetadataRequest for BlockHashMetadataRequest {
    type Input = u8;
    type Output = Bytes32;
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockHashesCache {
    #[cfg_attr(feature = "serde", serde(with = "serde_big_array::BigArray"))]
    cache: [Bytes32; 256],
    deepest_accessed: u32,
}

impl Default for BlockHashesCache {
    fn default() -> Self {
        Self {
            cache: [Bytes32::ZERO; 256],
            deepest_accessed: u32::MAX,
        }
    }
}

/// The hashes of the 256 previous blocks, by depth (the parent first)
pub struct HistoricalHashesQuery;

impl OracleQuery for HistoricalHashesQuery {
    type Input = ();
    type Output = [Bytes32; 256];

    const QUERY_ID: u32 = HISTORICAL_BLOCK_HASH_QUERY_ID;
}

impl BlockHashesCache {
    pub fn from_oracle(oracle: &mut impl IOOracle) -> Result<Self, InternalError> {
        let mut new = MaybeUninit::<Self>::uninit();
        let this = new.as_mut_ptr();
        // SAFETY: the fields are disjoint places inside `new`, which they initialize, and
        // `MaybeUninit<T>` has the layout of `T`
        unsafe {
            (&raw mut (*this).deepest_accessed).write(u32::MAX);
            HistoricalHashesQuery::get_into(
                oracle,
                (),
                &mut *(&raw mut (*this).cache).cast::<MaybeUninit<[Bytes32; 256]>>(),
            )?;
            Ok(new.assume_init())
        }
    }

    pub fn cache_entry(&self, depth: usize) -> &Bytes32 {
        &self.cache[depth]
    }

    pub fn num_elements_to_verify(&self) -> usize {
        if self.deepest_accessed == u32::MAX {
            0
        } else {
            (self.deepest_accessed + 1) as usize
        }
    }
}

impl DynamicMetadataResponder for BlockHashesCache {
    #[inline(always)]
    fn can_respond<M: MetadataRequest>() -> bool {
        core::any::TypeId::of::<M>() == core::any::TypeId::of::<BlockHashMetadataRequest>()
    }

    fn get_metadata_with_bookkeeping<M: MetadataRequest>(&mut self, input: M::Input) -> M::Output {
        assert!(Self::can_respond::<M>());
        let input = Self::cast_input::<M, BlockHashMetadataRequest>(input);
        let input = input as u32;
        if self.deepest_accessed == u32::MAX {
            self.deepest_accessed = input;
        }
        self.deepest_accessed = core::cmp::max(self.deepest_accessed, input);

        Self::cast_output::<BlockHashMetadataRequest, M>(self.cache[input as usize])
    }
}
