use crate::common_structs::state_root_view::StateRootView;
use crate::common_structs::ProofData;
use crate::oracle::query_ids::{INITIAL_STORAGE_SLOT_VALUE_QUERY_ID, ZK_PROOF_DATA_INIT_QUERY_ID};
use crate::oracle::simple_oracle_query::SimpleOracleQuery;
use crate::storage_types::{InitialStorageSlotData, StorageAddress};
use crate::types_config::{EthereumIOTypesConfig, SystemIOTypesConfig};

pub struct InitialStorageSlotQuery<IOTypes: SystemIOTypesConfig> {
    _marker: core::marker::PhantomData<IOTypes>,
}

impl<IOTypes: SystemIOTypesConfig> SimpleOracleQuery for InitialStorageSlotQuery<IOTypes> {
    const QUERY_ID: u32 = INITIAL_STORAGE_SLOT_VALUE_QUERY_ID;
    type Input = StorageAddress<IOTypes>;
    type Output = InitialStorageSlotData<IOTypes>;
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
