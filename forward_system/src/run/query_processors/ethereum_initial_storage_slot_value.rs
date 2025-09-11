use std::collections::HashMap;

use super::*;
use basic_system::system_implementation::ethereum_storage_model::digits_from_key;
use basic_system::system_implementation::ethereum_storage_model::BoxInterner;
use basic_system::system_implementation::ethereum_storage_model::EthereumMPT;
use basic_system::system_implementation::ethereum_storage_model::Path;
use basic_system::system_implementation::ethereum_storage_model::RLPSlice;
use basic_system::system_implementation::ethereum_storage_model::{
    caches::account_properties::{bytes32_from_rlp_slice, EthereumAccountProperties},
    EMPTY_ROOT_HASH,
};
use ruint::aliases::B160;
use std::alloc::Global;
use std::collections::BTreeMap;
use zk_ee::memory::vec_trait::VecCtor;
use zk_ee::{
    kv_markers::{InitialStorageSlotData, StorageAddress},
    system_io_oracle::{dyn_usize_iterator::DynUsizeIterator, INITIAL_STORAGE_SLOT_VALUE_QUERY_ID},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InMemoryEthereumInitialStorageSlotValueResponder {
    pub source: HashMap<B160, EthereumAccountProperties>,
    pub preimages_oracle: BTreeMap<Bytes32, Vec<u8>>,
}

impl InMemoryEthereumInitialStorageSlotValueResponder {
    const SUPPORTED_QUERY_IDS: &[u32] = &[INITIAL_STORAGE_SLOT_VALUE_QUERY_ID];
}

impl<M: MemorySource> OracleQueryProcessor<M> for InMemoryEthereumInitialStorageSlotValueResponder {
    fn supported_query_ids(&self) -> Vec<u32> {
        Self::SUPPORTED_QUERY_IDS.to_vec()
    }

    fn supports_query_id(&self, query_id: u32) -> bool {
        Self::SUPPORTED_QUERY_IDS.contains(&query_id)
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        _memory: &M,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static> {
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

        let address = StorageAddress::<EthereumIOTypesConfig>::from_iter(&mut query.into_iter())
            .expect("must deserialize hash value");

        let data = self
            .source
            .get(&address.address)
            .copied()
            .unwrap_or(EthereumAccountProperties::EMPTY_ACCOUNT);
        let initial_root = data.storage_root;
        let mut value = Bytes32::ZERO;
        if !data.is_empty() && initial_root != EMPTY_ROOT_HASH {
            use crypto::MiniDigest;
            let hash = crypto::sha3::Keccak256::digest(address.key.as_u8_array_ref());
            let digits = digits_from_key(&hash);
            let path = Path::new(&digits);
            // make MPT...
            let mut interner = BoxInterner::with_capacity_in(1 << 26, Global);
            let mut hasher = crypto::sha3::Keccak256::new();
            let mut accounts_mpt: EthereumMPT<'_, Global, VecCtor> =
                EthereumMPT::new_in(initial_root.as_u8_array(), &mut interner, Global).unwrap();
            let Ok(encoding) =
                accounts_mpt.get(path, &mut self.preimages_oracle, &mut interner, &mut hasher)
            else {
                panic!(
                    "Failed to get initial storage slot value for address 0x{:040x} and key {:?}",
                    address.address.as_uint(),
                    address.key,
                );
            };
            if !encoding.is_empty() {
                // strip one more RLP
                let rlp_slice = RLPSlice::from_slice(encoding).unwrap();
                value = bytes32_from_rlp_slice(&rlp_slice).unwrap();
            }
        };
        let is_new = value.is_zero();
        let initial_value = InitialStorageSlotData::<EthereumIOTypesConfig> {
            is_new_storage_slot: is_new,
            initial_value: value,
        };

        DynUsizeIterator::from_constructor(initial_value, |inner_ref| {
            UsizeSerializable::iter(inner_ref)
        })
    }
}
