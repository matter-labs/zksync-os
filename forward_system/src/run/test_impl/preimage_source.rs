use crate::run::PreimageSource;
use std::collections::HashMap;
use zk_ee::utils::Bytes32;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct InMemoryPreimageSource {
    pub inner: HashMap<Bytes32, Vec<u8>>,
}

impl PreimageSource for InMemoryPreimageSource {
    fn get_preimage(&mut self, hash: Bytes32) -> Option<Vec<u8>> {
        self.inner.get(&hash).cloned()
    }
}
