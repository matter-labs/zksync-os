use oracle_provider::MemorySource;
use oracle_provider::OracleQueryProcessor;
use serde::{Deserialize, Serialize};
use zk_ee::oracle::usize_serialization::dyn_usize_iterator::DynUsizeIterator;
use zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};

// Oracle query processors for the forward running system.
// Each processor handles specific types of oracle queries.

mod block_metadata;
mod da_commitment_scheme;
mod ethereum_cl;
mod ethereum_header;
mod ethereum_initial_account_state;
mod ethereum_initial_storage_slot_value;
mod generic_preimage;
mod read_storage;
mod read_tree;
mod tx_data;
mod uart_print;
mod zk_proof_data;

pub use self::block_metadata::BlockMetadataResponder;
pub use self::da_commitment_scheme::DACommitmentSchemeResponder;
pub use self::ethereum_cl::EthereumCLResponder;
pub use self::ethereum_header::EthereumTargetBlockHeaderResponder;
pub use self::ethereum_initial_account_state::InMemoryEthereumInitialAccountStateResponder;
pub use self::ethereum_initial_storage_slot_value::InMemoryEthereumInitialStorageSlotValueResponder;
pub use self::generic_preimage::GenericPreimageResponder;
pub use self::read_storage::ReadStorageResponder;
pub use self::read_tree::ReadTreeResponder;
pub use self::tx_data::TxDataResponder;
pub use self::uart_print::UARTPrintResponder;
pub use self::zk_proof_data::ZKProofDataResponder;

use crate::run::*;

/// A collection of oracle query processors for forward running execution with oracle dump.
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct ForwardRunningOracleDump<
    T: ReadStorageTree + Clone,
    PS: PreimageSource + Clone,
    TS: TxSource + Clone,
> {
    pub zk_proof_data_responder: ZKProofDataResponder,
    pub da_commitment_scheme_responder: DACommitmentSchemeResponder,
    pub block_metadata_responder: BlockMetadataResponder,
    /// Handles storage tree read operations and Merkle proofs
    pub tree_responder: ReadTreeResponder<T>,
    /// Handles transaction data queries (next tx size, tx content)
    pub tx_data_responder: TxDataResponder<TS>,
    /// Handles generic preimage resolution for hashes
    pub preimage_responder: GenericPreimageResponder<PS>,
}
