use std::collections::BTreeMap;
use std::collections::HashMap;

use super::*;
use basic_system::system_implementation::ethereum_storage_model::caches::account_properties::EthereumAccountProperties;
use basic_system::system_implementation::ethereum_storage_model::ETHEREUM_ACCOUNT_INITIAL_STATE_QUERY_ID;
use basic_system::system_implementation::ethereum_storage_model::*;
use ruint::aliases::B160;
use std::alloc::Global;
use zk_ee::memory::vec_trait::VecCtor;
use zk_ee::system_io_oracle::dyn_usize_iterator::DynUsizeIterator;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InMemoryEthereumInitialAccountStateResponder {
    pub state_root: [u8; 32],
    pub source: HashMap<B160, EthereumAccountProperties>,
    pub preimages_oracle: BTreeMap<Bytes32, Vec<u8>>,
}

impl InMemoryEthereumInitialAccountStateResponder {
    const SUPPORTED_QUERY_IDS: &[u32] = &[ETHEREUM_ACCOUNT_INITIAL_STATE_QUERY_ID];
}

impl<M: MemorySource> OracleQueryProcessor<M> for InMemoryEthereumInitialAccountStateResponder {
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

        let address = B160::from_iter(&mut query.into_iter()).expect("must deserialize hash value");

        let account = if let Some(data) = self.source.get(&address).copied() {
            if format!("0x{:040x}", address.as_uint())
                == "0x9313b45f7aec45415e786e0abedc0f9ca5be71df"
            {
                panic!();
            }
            data
        } else {
            // there are some values that didn't show up in witness,
            // but we still have to check them (namely - constructed addresses)
            use crypto::MiniDigest;
            let hash = crypto::sha3::Keccak256::digest(address.to_be_bytes::<20>());
            let digits = digits_from_key(&hash);
            let path = Path::new(&digits);
            // make MPT...
            let mut interner = BoxInterner::with_capacity_in(1 << 26, Global);
            let mut hasher = crypto::sha3::Keccak256::new();
            let mut accounts_mpt: EthereumMPT<'_, Global, VecCtor> =
                EthereumMPT::new_in(self.state_root, &mut interner, Global).unwrap();
            if let Ok(encoding) =
                accounts_mpt.get(path, &mut self.preimages_oracle, &mut interner, &mut hasher)
            {
                if !encoding.is_empty() {
                    EthereumAccountProperties::parse_from_rlp_bytes(encoding)
                        .expect("must parse account data")
                } else {
                    EthereumAccountProperties::EMPTY_ACCOUNT
                }
            } else {
                // we can provide random garbage, if it was unobserved at the end
                EthereumAccountProperties::EMPTY_ACCOUNT
            }
        };

        DynUsizeIterator::from_constructor(account, UsizeSerializable::iter)
    }
}
