use zk_ee::utils::Bytes32;

pub trait PreimageSource: 'static {
    fn get_preimage(&mut self, hash: Bytes32) -> Option<Vec<u8>>;
    // fn get_preimage(&mut self, preimage_type: PreimageType, hash: Bytes32) -> Option<Vec<u8>>;
}
