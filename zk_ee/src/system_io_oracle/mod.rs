pub mod dyn_usize_iterator;
///
/// Oracle is an abstraction boundary on how OS (System trait) gets IO information and eventually
/// updates state and/or sends messages to one more layer above
///
/// NOTE: this trait is about pure oracle work,
/// so e.g. if one asks for preimage it gives SOME data, but validity of this data
/// versus image (that depends on which hash is used) it beyond the scope of this trait
///
use core::num::NonZeroU32;

use super::kv_markers::{ExactSizeChain, StorageAddress, UsizeDeserializable, UsizeSerializable};
use super::types_config::SystemIOTypesConfig;
use crate::system::errors::internal::InternalError;
use crate::utils::Bytes32;

///
/// Oracle iterator type marker, used to specify oracle query type.
///
pub trait OracleIteratorTypeMarker: 'static + Sized {
    const ID: u32;
    type Params: UsizeSerializable + UsizeDeserializable;
}

///
/// Next transaction size query type.
///
/// Note: `0` size means that there are no more transactions to process.
///
pub struct NextTxSize;
impl OracleIteratorTypeMarker for NextTxSize {
    const ID: u32 = 0;
    type Params = ();
}

///
/// New transaction content query type.
///
pub struct NewTxContentIterator;
impl OracleIteratorTypeMarker for NewTxContentIterator {
    const ID: u32 = 1;
    type Params = ();
}

///
/// IO Implementer initial data query type.
///
pub struct InitializeIOImplementerIterator;
impl OracleIteratorTypeMarker for InitializeIOImplementerIterator {
    const ID: u32 = 2;
    type Params = ();
}

///
/// Block level metadata query type.
///
pub struct BlockLevelMetadataIterator;

impl OracleIteratorTypeMarker for BlockLevelMetadataIterator {
    const ID: u32 = 3;
    type Params = ();
}

///
/// Initial storage slot data query type.
///
pub struct InitialStorageSlotDataIterator<IOTypes: SystemIOTypesConfig> {
    _marker: core::marker::PhantomData<IOTypes>,
}

impl<IOTypes: SystemIOTypesConfig> Default for InitialStorageSlotDataIterator<IOTypes> {
    fn default() -> Self {
        Self {
            _marker: core::marker::PhantomData,
        }
    }
}

impl<IOTypes: SystemIOTypesConfig> OracleIteratorTypeMarker
    for InitialStorageSlotDataIterator<IOTypes>
{
    const ID: u32 = 4;
    type Params = StorageAddress<IOTypes>;
}

///
/// Preimage content query type.
///
pub struct PreimageContentWordsIterator;

impl OracleIteratorTypeMarker for PreimageContentWordsIterator {
    const ID: u32 = 5;
    type Params = Bytes32;
}

///
/// Disconnect from oracle query type.
///
pub struct DisconnectOracleFormalIterator;

impl OracleIteratorTypeMarker for DisconnectOracleFormalIterator {
    const ID: u32 = 6;
    type Params = ();
}

// Next 3 queries used for proving tree update.

///
/// Proof for index query type.
///
pub struct ProofForIndexIterator;

impl OracleIteratorTypeMarker for ProofForIndexIterator {
    const ID: u32 = 7;
    type Params = u64;
}

///
/// Previous index query type.
///
pub struct PrevIndexIterator;

impl OracleIteratorTypeMarker for PrevIndexIterator {
    const ID: u32 = 8;
    type Params = Bytes32;
}

///
/// Exact index query type.
///
pub struct ExactIndexIterator;

impl OracleIteratorTypeMarker for ExactIndexIterator {
    const ID: u32 = 9;
    type Params = Bytes32;
}

///
/// UART access query type.
///
/// This type is not called on the oracle directly.
///
pub struct UARTAccessMarker;

impl OracleIteratorTypeMarker for UARTAccessMarker {
    const ID: u32 = u32::MAX;
    type Params = ();
}

#[derive(Clone, Copy, Debug)]
pub struct InitialStorageSlotData<IOTypes: SystemIOTypesConfig> {
    // we need to know what was a value of the storage slot,
    // and whether it existed in the state or has to be created
    // (so additional information is needed to reconstruct creation location)
    pub is_new_storage_slot: bool,
    pub initial_value: IOTypes::StorageValue,
}

impl<IOTypes: SystemIOTypesConfig> Default for InitialStorageSlotData<IOTypes> {
    fn default() -> Self {
        Self {
            is_new_storage_slot: false,
            initial_value: IOTypes::StorageValue::default(),
        }
    }
}

impl<IOTypes: SystemIOTypesConfig> UsizeSerializable for InitialStorageSlotData<IOTypes> {
    const USIZE_LEN: usize = <bool as UsizeSerializable>::USIZE_LEN
        + <u8 as UsizeSerializable>::USIZE_LEN
        + <IOTypes::StorageValue as UsizeSerializable>::USIZE_LEN;
    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            UsizeSerializable::iter(&self.is_new_storage_slot),
            UsizeSerializable::iter(&self.initial_value),
        )
    }
}

impl<IOTypes: SystemIOTypesConfig> UsizeDeserializable for InitialStorageSlotData<IOTypes> {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let is_new_storage_slot = UsizeDeserializable::from_iter(src)?;
        let initial_value = UsizeDeserializable::from_iter(src)?;

        let new = Self {
            is_new_storage_slot,
            initial_value,
        };

        Ok(new)
    }
}

///
/// Oracle interface
///
pub trait IOOracle: 'static + Sized {
    /// Iterator type that oracle returns.
    type MarkerTiedIterator<'a>: ExactSizeIterator<Item = usize>;

    ///
    /// Main oracle method.
    /// Returns iterator for generic type marker.
    ///
    fn create_oracle_access_iterator<'a, M: OracleIteratorTypeMarker>(
        &'a mut self,
        init_value: M::Params,
    ) -> Result<Self::MarkerTiedIterator<'a>, InternalError>;

    // Few wrappers that return output in convenient types
    ///
    /// Returns the byte length of the next transaction.
    ///
    /// If there are no more transactions returns `None`.
    /// Note: length can't be 0, as 0 interpreted as no more transactions.
    ///
    fn try_begin_next_tx(&mut self) -> Option<NonZeroU32> {
        // go via query
        let mut it = self
            .create_oracle_access_iterator::<NextTxSize>(())
            .expect("must make an iterator");
        let size: u32 = UsizeDeserializable::from_iter(&mut it).expect("must initialize");
        assert!(it.next().is_none());

        NonZeroU32::new(size)
    }

    fn get_block_level_metadata<T: UsizeDeserializable>(&mut self) -> T {
        // go via query
        let mut it = self
            .create_oracle_access_iterator::<BlockLevelMetadataIterator>(())
            .expect("must make an iterator");
        let result: T = UsizeDeserializable::from_iter(&mut it).expect("must initialize");
        assert!(it.next().is_none());

        result
    }
}
