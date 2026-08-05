use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use zk_ee::utils::Bytes32;

use super::*;

// We can consider to extend this trait to hasher support, but so far MPT is only defined
// for hashes with 32 bytes output

// we want some implementation that can give un preimages for keys that we need
pub trait PreimagesOracle {
    fn provide_preimage<'a, I: Interner<'a> + 'a>(
        &mut self,
        key: &[u8; 32],
        interner: &'_ mut I,
    ) -> Result<&'a [u8], ()>;
}

// we will make a simple one as example and for test purposes

impl PreimagesOracle for () {
    fn provide_preimage<'a, I: Interner<'a> + 'a>(
        &mut self,
        _key: &[u8; 32],
        _interner: &'_ mut I,
    ) -> Result<&'a [u8], ()> {
        Err(())
    }
}

impl<A: Allocator + Clone> PreimagesOracle for BTreeMap<Bytes32, Vec<u8, A>, A> {
    fn provide_preimage<'a, I: Interner<'a> + 'a>(
        &mut self,
        key: &[u8; 32],
        interner: &'_ mut I,
    ) -> Result<&'a [u8], ()> {
        // we do not benefit from word-level writes here
        let key = Bytes32::from_array(*key);
        if let Some(known) = self.get(&key) {
            let mut buffer = interner.get_buffer(known.len())?;
            buffer.write_slice(known);

            Ok(buffer.flush())
        } else {
            // panic!("Failed to get preimage for key {:?}", &key);
            Err(())
        }
    }
}
