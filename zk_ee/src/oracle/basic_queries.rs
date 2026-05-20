use crate::common_structs::state_root_view::StateRootView;
use crate::common_structs::ProofData;
use crate::oracle::query_ids::{
    DISCONNECT_ORACLE_QUERY_ID, INITIAL_STORAGE_SLOT_VALUE_QUERY_ID, ZK_PROOF_DATA_INIT_QUERY_ID,
};
use crate::oracle::simple_oracle_query::SimpleOracleQuery;
use crate::oracle::word_layout::WordLayout;
use crate::storage_types::{InitialStorageSlotData, StorageAddress};
use crate::types_config::{EthereumIOTypesConfig, SystemIOTypesConfig};

pub struct InitialStorageSlotQuery<IOTypes: SystemIOTypesConfig> {
    _marker: core::marker::PhantomData<IOTypes>,
}

impl<IOTypes: SystemIOTypesConfig> SimpleOracleQuery for InitialStorageSlotQuery<IOTypes>
where
    StorageAddress<IOTypes>: WordLayout,
    InitialStorageSlotData<IOTypes>: WordLayout,
{
    const QUERY_ID: u32 = INITIAL_STORAGE_SLOT_VALUE_QUERY_ID;
    type Input = StorageAddress<IOTypes>;
    type Output = InitialStorageSlotData<IOTypes>;
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

impl<SR: StateRootView<EthereumIOTypesConfig> + WordLayout> SimpleOracleQuery
    for ZKProofDataQuery<EthereumIOTypesConfig, SR>
where
    ProofData<SR>: WordLayout,
{
    const QUERY_ID: u32 = ZK_PROOF_DATA_INIT_QUERY_ID;
    type Input = ();
    type Output = ProofData<SR>;
}
