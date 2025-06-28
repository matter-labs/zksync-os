use crate::run::{NextTxResponse, PreimageSource, ReadStorageTree, TxSource};
use basic_system::system_implementation::flat_storage_model::*;
use serde::{Deserialize, Serialize};
use zk_ee::common_structs::derive_flat_storage_key;
use zk_ee::common_structs::BasicIOImplementerFSM;
use zk_ee::internal_error;
use zk_ee::kv_markers::StorageAddress;
use zk_ee::oracle::*;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::metadata::BlockMetadataFromOracle;
use zk_ee::system_io_oracle::dyn_usize_iterator::DynUsizeIterator;
use zk_ee::system_io_oracle::*;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::*;

use super::ReadStorage;

#[derive(Debug, Serialize, Deserialize)]
pub struct ForwardRunningOracleAux<T: ReadStorageTree, PS: PreimageSource, TS: TxSource> {
    pub storage_commitment: Option<FlatStorageCommitment<TREE_HEIGHT>>,
    pub block_metadata: BlockMetadataFromOracle,
    pub tree: T,
    pub tx_source: TS,
    pub preimage_source: PS,
    pub next_tx: Option<Vec<u8>>,
}

impl<T: ReadStorageTree + Clone, PS: PreimageSource + Clone, TS: TxSource + Clone> Clone
    for ForwardRunningOracleAux<T, PS, TS>
{
    fn clone(&self) -> Self {
        ForwardRunningOracleAux {
            storage_commitment: self.storage_commitment,
            block_metadata: self.block_metadata,
            tree: self.tree.clone(),
            tx_source: self.tx_source.clone(),
            preimage_source: self.preimage_source.clone(),
            next_tx: self.next_tx.clone(),
        }
    }
}

impl<T: ReadStorageTree, PS: PreimageSource, TS: TxSource> From<ForwardRunningOracle<T, PS, TS>>
    for ForwardRunningOracleAux<T, PS, TS>
{
    fn from(oracle: ForwardRunningOracle<T, PS, TS>) -> Self {
        ForwardRunningOracleAux {
            storage_commitment: oracle.io_implementer_init_data.map(|x| x.state_root_view),
            block_metadata: oracle.block_metadata,
            tree: oracle.tree,
            tx_source: oracle.tx_source,
            preimage_source: oracle.preimage_source,
            next_tx: oracle.next_tx,
        }
    }
}

impl<T: ReadStorageTree, PS: PreimageSource, TS: TxSource> From<ForwardRunningOracleAux<T, PS, TS>>
    for ForwardRunningOracle<T, PS, TS>
{
    fn from(oracle: ForwardRunningOracleAux<T, PS, TS>) -> Self {
        ForwardRunningOracle {
            io_implementer_init_data: Some(BasicIOImplementerFSM {
                state_root_view: match oracle.storage_commitment {
                    Some(storage_commitment) => storage_commitment,
                    None => FlatStorageCommitment {
                        root: Default::default(),
                        next_free_slot: 0,
                    },
                },
                pubdata_diffs_log_hash: Bytes32::ZERO,
                num_pubdata_diffs_logs: 0,
                block_functionality_is_completed: false,
            }),
            block_metadata: oracle.block_metadata,
            tree: oracle.tree,
            tx_source: oracle.tx_source,
            preimage_source: oracle.preimage_source,
            next_tx: oracle.next_tx,
        }
    }
}

#[derive(Debug)]
pub struct ForwardRunningOracle<T: ReadStorageTree, PS: PreimageSource, TS: TxSource> {
    pub io_implementer_init_data: Option<BasicIOImplementerFSM<FlatStorageCommitment<TREE_HEIGHT>>>,
    pub block_metadata: BlockMetadataFromOracle,
    pub tree: T,
    pub tx_source: TS,
    pub preimage_source: PS,
    pub next_tx: Option<Vec<u8>>,
}

impl<T: ReadStorageTree + Clone, PS: PreimageSource + Clone, TS: TxSource + Clone> Clone
    for ForwardRunningOracle<T, PS, TS>
{
    fn clone(&self) -> Self {
        ForwardRunningOracle {
            io_implementer_init_data: self.io_implementer_init_data,
            block_metadata: self.block_metadata,
            tree: self.tree.clone(),
            tx_source: self.tx_source.clone(),
            preimage_source: self.preimage_source.clone(),
            next_tx: self.next_tx.clone(),
        }
    }
}

impl<T: ReadStorageTree, PS: PreimageSource, TS: TxSource> ForwardRunningOracle<T, PS, TS> {
    pub fn make_iter_dyn<'a, M: OracleIteratorTypeMarker>(
        &'a mut self,
        init_value: M::Params,
    ) -> Result<Box<dyn ExactSizeIterator<Item = usize> + 'static>, InternalError> {
        match core::any::TypeId::of::<M>() {
            a if a == core::any::TypeId::of::<NextTxSize>() => {
                let len = match &self.next_tx {
                    Some(next_tx) => next_tx.len(),
                    None => {
                        match self.tx_source.get_next_tx() {
                            NextTxResponse::SealBatch => 0,
                            NextTxResponse::Tx(next_tx) => {
                                let next_tx_len = next_tx.len();
                                // `0` interpreted as seal batch
                                assert_ne!(next_tx_len, 0);
                                self.next_tx = Some(next_tx);
                                next_tx_len
                            }
                        }
                    }
                } as u32;

                let iterator = DynUsizeIterator::from_owned(len);

                Ok(Box::new(iterator))
            }
            a if a == core::any::TypeId::of::<NewTxContentIterator>() => {
                let Some(tx) = self.next_tx.take() else {
                    return Err(internal_error!(
                        "trying to read next tx content before size query or after seal response",
                    ));
                };

                let iterator = DynUsizeIterator::from_constructor(tx, |inner_ref| {
                    ReadIterWrapper::from(inner_ref.iter().copied())
                });

                Ok(Box::new(iterator))
            }
            a if a == core::any::TypeId::of::<InitializeIOImplementerIterator>() => {
                let iterator = DynUsizeIterator::from_owned(
                    self.io_implementer_init_data
                        .take()
                        .expect("io implementer data is none (second read or not set initially)"),
                );

                Ok(Box::new(iterator))
            }
            a if a == core::any::TypeId::of::<BlockLevelMetadataIterator>() => {
                unsafe {
                    *(&init_value as *const M::Params)
                        .cast::<<BlockLevelMetadataIterator as OracleIteratorTypeMarker>::Params>()
                };
                // we do not use it for anything
                let iterator = DynUsizeIterator::from_owned(self.block_metadata);

                Ok(Box::new(iterator))
            }
            a if a
                == core::any::TypeId::of::<InitialStorageSlotDataIterator<EthereumIOTypesConfig>>(
                ) =>
            {
                let StorageAddress { address, key } = unsafe {
                    *(&init_value as *const M::Params).cast::<<InitialStorageSlotDataIterator<
                        EthereumIOTypesConfig,
                    > as OracleIteratorTypeMarker>::Params>(
                    )
                };
                let flat_key = derive_flat_storage_key(&address, &key);
                let slot_data: InitialStorageSlotData<EthereumIOTypesConfig> =
                    if let Some(cold) = self.tree.read(flat_key) {
                        InitialStorageSlotData {
                            initial_value: cold,
                            is_new_storage_slot: false,
                        }
                    } else {
                        // default value, but it's potentially new storage slot in state!
                        InitialStorageSlotData {
                            initial_value: Bytes32::ZERO,
                            is_new_storage_slot: true,
                        }
                    };
                let iterator = DynUsizeIterator::from_owned(slot_data);

                Ok(Box::new(iterator))
            }
            a if a == core::any::TypeId::of::<PreimageContentWordsIterator>() => {
                let hash = unsafe {
                    *(&init_value as *const M::Params).cast::<<PreimageContentWordsIterator as OracleIteratorTypeMarker>::Params>()
                };
                let preimage = self
                    .preimage_source
                    .get_preimage(hash)
                    .ok_or(internal_error!("must know a preimage for hash"))?;

                let iterator = DynUsizeIterator::from_constructor(preimage, |inner_ref| {
                    ReadIterWrapper::from(inner_ref.iter().copied())
                });

                Ok(Box::new(iterator))
            }
            a if a == core::any::TypeId::of::<ExactIndexIterator>() => {
                let flat_key = unsafe {
                    *(&init_value as *const M::Params)
                        .cast::<<ExactIndexIterator as OracleIteratorTypeMarker>::Params>()
                };
                let existing = self
                    .tree
                    .tree_index(flat_key)
                    .expect("Reading index for key that is not in the tree");

                let iterator = DynUsizeIterator::from_owned(existing);

                Ok(Box::new(iterator))
            }
            a if a == core::any::TypeId::of::<ProofForIndexIterator>() => {
                let index = unsafe {
                    *(&init_value as *const M::Params)
                        .cast::<<ProofForIndexIterator as OracleIteratorTypeMarker>::Params>()
                };
                let existing = self.tree.merkle_proof(index);
                let proof = ValueAtIndexProof {
                    proof: ExistingReadProof { existing },
                };

                let iterator = DynUsizeIterator::from_owned(proof);

                Ok(Box::new(iterator))
            }
            a if a == core::any::TypeId::of::<PrevIndexIterator>() => {
                let flat_key = unsafe {
                    *(&init_value as *const M::Params)
                        .cast::<<PrevIndexIterator as OracleIteratorTypeMarker>::Params>()
                };
                let prev_index = self.tree.prev_tree_index(flat_key);
                let iterator = DynUsizeIterator::from_owned(prev_index);
                Ok(Box::new(iterator))
            }
            _ => Err(internal_error!("Invalid marker")),
        }
    }
}

impl<T: ReadStorageTree, PS: PreimageSource, TS: TxSource> IOOracle
    for ForwardRunningOracle<T, PS, TS>
{
    type MarkerTiedIterator<'a> = Box<dyn ExactSizeIterator<Item = usize> + 'static>;

    fn create_oracle_access_iterator<M: OracleIteratorTypeMarker>(
        &mut self,
        init_value: M::Params,
    ) -> Result<Self::MarkerTiedIterator<'_>, InternalError> {
        self.make_iter_dyn::<M>(init_value)
    }
}

#[derive(Clone, Debug)]
pub struct CallSimulationOracle<S: ReadStorage, PS: PreimageSource, TS: TxSource> {
    pub io_implementer_init_data:
        Option<BasicIOImplementerFSM<FlatStorageCommitment<TESTING_TREE_HEIGHT>>>,
    pub block_metadata: BlockMetadataFromOracle,
    pub storage: S,
    pub tx_source: TS,
    pub preimage_source: PS,
    pub next_tx: Option<Vec<u8>>,
}

impl<S: ReadStorage, PS: PreimageSource, TS: TxSource> CallSimulationOracle<S, PS, TS> {
    pub fn make_iter_dyn<'a, M: OracleIteratorTypeMarker>(
        &'a mut self,
        init_value: M::Params,
    ) -> Result<Box<dyn ExactSizeIterator<Item = usize> + 'static>, InternalError> {
        match core::any::TypeId::of::<M>() {
            a if a == core::any::TypeId::of::<NextTxSize>() => {
                let len = match &self.next_tx {
                    Some(next_tx) => next_tx.len(),
                    None => {
                        match self.tx_source.get_next_tx() {
                            NextTxResponse::SealBatch => 0,
                            NextTxResponse::Tx(next_tx) => {
                                let next_tx_len = next_tx.len();
                                // `0` interpreted as seal batch
                                assert_ne!(next_tx_len, 0);
                                self.next_tx = Some(next_tx);
                                next_tx_len
                            }
                        }
                    }
                } as u32;

                let iterator = DynUsizeIterator::from_owned(len);

                Ok(Box::new(iterator))
            }
            a if a == core::any::TypeId::of::<NewTxContentIterator>() => {
                let Some(tx) = self.next_tx.take() else {
                    return Err(internal_error!(
                        "trying to read next tx content before size query or after seal response",
                    ));
                };

                let iterator = DynUsizeIterator::from_constructor(tx, |inner_ref| {
                    ReadIterWrapper::from(inner_ref.iter().copied())
                });

                Ok(Box::new(iterator))
            }
            a if a == core::any::TypeId::of::<InitializeIOImplementerIterator>() => {
                let iterator = DynUsizeIterator::from_owned(
                    self.io_implementer_init_data
                        .take()
                        .expect("reading io implementer init data twice"),
                );

                Ok(Box::new(iterator))
            }
            a if a == core::any::TypeId::of::<BlockLevelMetadataIterator>() => {
                unsafe {
                    *(&init_value as *const M::Params)
                        .cast::<<BlockLevelMetadataIterator as OracleIteratorTypeMarker>::Params>()
                };
                // we do not use it for anything
                let iterator = DynUsizeIterator::from_owned(self.block_metadata);

                Ok(Box::new(iterator))
            }
            a if a
                == core::any::TypeId::of::<InitialStorageSlotDataIterator<EthereumIOTypesConfig>>(
                ) =>
            {
                let StorageAddress { address, key } = unsafe {
                    *(&init_value as *const M::Params).cast::<<InitialStorageSlotDataIterator<
                        EthereumIOTypesConfig,
                    > as OracleIteratorTypeMarker>::Params>(
                    )
                };
                let flat_key = derive_flat_storage_key(&address, &key);
                let slot_data: InitialStorageSlotData<EthereumIOTypesConfig> =
                    if let Some(cold) = self.storage.read(flat_key) {
                        InitialStorageSlotData {
                            initial_value: cold,
                            is_new_storage_slot: false,
                        }
                    } else {
                        // default value, but it's potentially new storage slot in state!
                        InitialStorageSlotData {
                            initial_value: Bytes32::ZERO,
                            is_new_storage_slot: true,
                        }
                    };
                let iterator = DynUsizeIterator::from_owned(slot_data);

                Ok(Box::new(iterator))
            }
            a if a == core::any::TypeId::of::<PreimageContentWordsIterator>() => {
                let hash = unsafe {
                    *(&init_value as *const M::Params).cast::<<PreimageContentWordsIterator as OracleIteratorTypeMarker>::Params>()
                };
                let preimage = self
                    .preimage_source
                    .get_preimage(hash)
                    .ok_or(internal_error!("must know a preimage for hash"))?;

                let iterator = DynUsizeIterator::from_constructor(preimage, |inner_ref| {
                    ReadIterWrapper::from(inner_ref.iter().copied())
                });

                Ok(Box::new(iterator))
            }
            _ => Err(internal_error!("Invalid marker")),
        }
    }
}

impl<S: ReadStorage, PS: PreimageSource, TS: TxSource> IOOracle
    for CallSimulationOracle<S, PS, TS>
{
    type MarkerTiedIterator<'a> = Box<dyn ExactSizeIterator<Item = usize> + 'static>;

    fn create_oracle_access_iterator<M: OracleIteratorTypeMarker>(
        &mut self,
        init_value: M::Params,
    ) -> Result<Self::MarkerTiedIterator<'_>, InternalError> {
        self.make_iter_dyn::<M>(init_value)
    }
}
