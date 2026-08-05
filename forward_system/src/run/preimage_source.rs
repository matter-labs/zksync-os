use zk_ee::utils::Bytes32;

pub trait PreimageSource: 'static {
    fn get_preimage(&mut self, hash: Bytes32) -> Option<Vec<u8>>;
}

impl<T: zksync_os_interface::traits::PreimageSource> PreimageSource for T {
    fn get_preimage(&mut self, hash: Bytes32) -> Option<Vec<u8>> {
        self.get_preimage(hash.as_u8_array().into())
    }
}
