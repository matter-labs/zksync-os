use std::collections::HashMap;

use super::*;
use basic_system::system_implementation::ethereum_storage_model::digits_from_key;
use basic_system::system_implementation::ethereum_storage_model::vec_trait::VecCtor;
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
use zk_ee::oracle::query_ids::INITIAL_STORAGE_SLOT_VALUE_QUERY_ID;
use zk_ee::storage_types::InitialStorageSlotData;
use zk_ee::storage_types::StorageAddress;
use zk_ee::utils::Bytes32;

#[derive(Debug, Clone)]
pub struct InMemoryEthereumInitialStorageSlotValueResponder {
    pub source: HashMap<B160, EthereumAccountProperties>,
    pub preimages_oracle: BTreeMap<Bytes32, Vec<u8>>,
    interner: BoxInterner<Global>,
    hasher: crypto::sha3::Keccak256,
}

/// Decode WordLayout-encoded u32 words back into a typed value.
fn decode_input<T: WordLayout>(input: &[u32]) -> T {
    let mut cursor = 0;
    T::read_words(&mut || {
        let w = input.get(cursor).copied().unwrap_or(0);
        cursor += 1;
        w
    })
}

impl InMemoryEthereumInitialStorageSlotValueResponder {
    const SUPPORTED_QUERY_IDS: &[u32] = &[INITIAL_STORAGE_SLOT_VALUE_QUERY_ID];

    pub fn new(
        source: HashMap<B160, EthereumAccountProperties>,
        preimages_oracle: BTreeMap<Bytes32, Vec<u8>>,
    ) -> Self {
        use crypto::MiniDigest;
        Self {
            source,
            preimages_oracle,
            interner: BoxInterner::with_capacity_in(1 << 26, Global),
            hasher: crypto::sha3::Keccak256::new(),
        }
    }
}

impl OracleQueryProcessor for InMemoryEthereumInitialStorageSlotValueResponder {
    fn supported_query_ids(&self) -> Vec<u32> {
        Self::SUPPORTED_QUERY_IDS.to_vec()
    }

    fn supports_query_id(&self, query_id: u32) -> bool {
        Self::SUPPORTED_QUERY_IDS.contains(&query_id)
    }

    fn process(
        &mut self,
        query_id: u32,
        input: &[u32],
        _memory: &dyn oracle_provider::RamPeek,
    ) -> Result<Vec<u32>, InternalError> {
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

        let address: StorageAddress<EthereumIOTypesConfig> = decode_input(input);

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
            self.interner.reset();
            let mut accounts_mpt: EthereumMPT<'_, Global, VecCtor, false> =
                EthereumMPT::new_in(initial_root.as_u8_array(), &mut self.interner, Global)
                    .unwrap();
            let Ok(encoding) = accounts_mpt.get(
                path,
                &mut self.preimages_oracle,
                &mut self.interner,
                &mut self.hasher,
            ) else {
                panic!(
                    "Failed to get initial storage slot value for address 0x{:040x} and key {:?}",
                    address.address.as_uint(),
                    address.key,
                );
            };
            if !encoding.is_empty() {
                let rlp_slice = RLPSlice::from_slice(encoding).unwrap();
                value = bytes32_from_rlp_slice(&rlp_slice).unwrap();
            }
        };
        let is_new = value.is_zero();
        let initial_value = InitialStorageSlotData::<EthereumIOTypesConfig> {
            is_new_storage_slot: is_new,
            initial_value: value,
        };

        let mut result = Vec::new();
        initial_value.write_words(&mut |w| result.push(w));
        Ok(result)
    }
}
