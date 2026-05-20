use std::collections::BTreeMap;
use std::collections::HashMap;

use super::*;
use basic_system::system_implementation::ethereum_storage_model::caches::account_properties::EthereumAccountProperties;
use basic_system::system_implementation::ethereum_storage_model::ETHEREUM_ACCOUNT_INITIAL_STATE_QUERY_ID;
use basic_system::system_implementation::ethereum_storage_model::*;
use ruint::aliases::B160;
use std::alloc::Global;
use vec_trait::VecCtor;
use zk_ee::utils::Bytes32;

#[derive(Debug, Clone)]
pub struct InMemoryEthereumInitialAccountStateResponder {
    pub state_root: [u8; 32],
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

impl InMemoryEthereumInitialAccountStateResponder {
    const SUPPORTED_QUERY_IDS: &[u32] = &[ETHEREUM_ACCOUNT_INITIAL_STATE_QUERY_ID];

    pub fn new(
        state_root: [u8; 32],
        source: HashMap<B160, EthereumAccountProperties>,
        preimages_oracle: BTreeMap<Bytes32, Vec<u8>>,
    ) -> Self {
        use crypto::MiniDigest;
        Self {
            state_root,
            source,
            preimages_oracle,
            interner: BoxInterner::with_capacity_in(1 << 26, Global),
            hasher: crypto::sha3::Keccak256::new(),
        }
    }
}

impl OracleQueryProcessor for InMemoryEthereumInitialAccountStateResponder {
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

        let address: B160 = decode_input(input);

        let account = if let Some(data) = self.source.get(&address).copied() {
            data
        } else {
            use crypto::MiniDigest;
            let hash = crypto::sha3::Keccak256::digest(address.to_be_bytes::<20>());
            let digits = digits_from_key(&hash);
            let path = Path::new(&digits);
            self.interner.reset();
            let mut accounts_mpt: EthereumMPT<'_, Global, VecCtor, false> =
                EthereumMPT::new_in(self.state_root, &mut self.interner, Global).unwrap();
            if let Ok(encoding) = accounts_mpt.get(
                path,
                &mut self.preimages_oracle,
                &mut self.interner,
                &mut self.hasher,
            ) {
                if !encoding.is_empty() {
                    EthereumAccountProperties::parse_from_rlp_bytes(encoding)
                        .expect("must parse account data")
                } else {
                    EthereumAccountProperties::EMPTY_ACCOUNT
                }
            } else {
                EthereumAccountProperties::EMPTY_ACCOUNT
            }
        };

        let mut result = Vec::new();
        account.write_words(&mut |w| result.push(w));
        Ok(result)
    }
}
