use crate::common_structs::state_root_view::StateRootView;
use crate::common_structs::ProofData;
use crate::oracle::memory_io::OracleQuery;
use crate::oracle::query_ids::{
    DISCONNECT_ORACLE_QUERY_ID, INITIAL_STORAGE_SLOT_VALUE_QUERY_ID, ZK_PROOF_DATA_INIT_QUERY_ID,
};
use crate::oracle::simple_oracle_query::SimpleOracleQuery;
use crate::storage_types::InitialStorageSlotData;
use crate::types_config::{EthereumIOTypesConfig, SystemIOTypesConfig};
use crate::utils::Bytes32;
use ruint::aliases::B160;

pub struct InitialStorageSlotQuery<IOTypes: SystemIOTypesConfig> {
    _marker: core::marker::PhantomData<IOTypes>,
}

impl OracleQuery for InitialStorageSlotQuery<EthereumIOTypesConfig> {
    const QUERY_ID: u32 = INITIAL_STORAGE_SLOT_VALUE_QUERY_ID;
    /// `(address, key)`
    type Input = (B160, Bytes32);
    type Output = InitialStorageSlotData<EthereumIOTypesConfig>;
}

pub struct DisconnectOracleQuery;

impl SimpleOracleQuery for DisconnectOracleQuery {
    const QUERY_ID: u32 = DISCONNECT_ORACLE_QUERY_ID;
    type Input = ();
    type Output = ();
}

pub struct ZKProofDataQuery<IOTypes: SystemIOTypesConfig, SR: StateRootView<IOTypes>> {
    _marker: core::marker::PhantomData<(IOTypes, SR)>,
}

impl<SR: StateRootView<EthereumIOTypesConfig>> SimpleOracleQuery
    for ZKProofDataQuery<EthereumIOTypesConfig, SR>
{
    const QUERY_ID: u32 = ZK_PROOF_DATA_INIT_QUERY_ID;
    type Input = ();
    type Output = ProofData<SR>;
}
