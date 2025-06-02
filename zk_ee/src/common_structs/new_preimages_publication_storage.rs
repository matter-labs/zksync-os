use crate::common_structs::history_map::Appearance;
use crate::system::errors::InternalError;
use crate::utils::Bytes32;
use alloc::alloc::Global;
use alloc::collections::BTreeMap;
use core::alloc::Allocator;

use super::history_map::{CacheSnapshotId, HistoryMap, HistoryMapItemRef, TransactionId};

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
pub enum PreimageType {
    Bytecode = 0,
    AccountData = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct PreimagesPublicationStorageValue {
    pub num_uses: usize,
    pub publication_net_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct Elem {
    pub preimage_type: PreimageType,
    pub value: PreimagesPublicationStorageValue,
}

impl PreimagesPublicationStorageValue {
    pub fn mark_use(&mut self) -> Result<(), InternalError> {
        if let Some(num_uses) = self.num_uses.checked_add(1) {
            self.num_uses = num_uses;
            Ok(())
        } else {
            Err(InternalError("Overflow in num_uses"))
        }
    }
}

// we want to store new preimages for DA

pub struct NewPreimagesPublicationStorage<A: Allocator + Clone = Global> {
    cache: HistoryMap<Bytes32, Elem, (), A>,
    current_tx_number: u32,
    pub inner: BTreeMap<Bytes32, (PreimagesPublicationStorageValue, PreimageType), A>,
}

impl<A: Allocator + Clone> NewPreimagesPublicationStorage<A> {
    pub fn new_from_parts(allocator: A) -> Self {
        Self {
            cache: HistoryMap::new(allocator.clone()),
            current_tx_number: 0,
            inner: BTreeMap::new_in(allocator.clone()),
        }
    }

    pub fn begin_new_tx(&mut self) {
        self.cache.commit();

        self.current_tx_number += 1;
    }

    #[track_caller]
    pub fn start_frame(&mut self) -> CacheSnapshotId {
        self.cache
            .snapshot(TransactionId(self.current_tx_number as u64))
    }

    #[track_caller]
    pub fn finish_frame(&mut self, rollback_handle: Option<&CacheSnapshotId>) {
        if let Some(x) = rollback_handle {
            self.cache.rollback(*x);
        }
    }

    pub fn add_preimage(
        &mut self,
        hash: &Bytes32,
        preimage_publication_byte_len: usize,
        preimage_type: PreimageType,
    ) -> Result<(), InternalError> {
        let mut item = self.cache.get_or_insert(&mut (), hash, |_| {
            let new = Elem {
                preimage_type,
                value: PreimagesPublicationStorageValue {
                    num_uses: 0,
                    publication_net_bytes: preimage_publication_byte_len,
                },
            };
            Ok((new, Appearance::Unset))
        })?;

        item.update(|x, _| {
            if x.value.num_uses > 1 {
                assert_eq!(x.value.publication_net_bytes, preimage_publication_byte_len);
            }
            x.value.mark_use()?;

            Ok(())
        })?;

        Ok(())
    }

    pub fn net_pubdata_used(&self) -> u64 {
        let mut size = 0;
        self.cache
            .for_total_diff_operands::<_, ()>(|_, r, _| {
                match r.appearance {
                    Appearance::Unset | Appearance::Retrieved => {}
                    Appearance::Deconstructed | Appearance::Updated => {
                        size += r.value.value.publication_net_bytes
                    }
                };
                Ok(())
            })
            .expect("We're returning ok.");

        size as u64
    }

    pub fn net_diffs_iter(&self) -> impl Iterator<Item = HistoryMapItemRef<Bytes32, Elem, (), A>> {
        self.cache.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_tx_single_frame_ok() {
        let mut storage = NewPreimagesPublicationStorage::<_>::new_from_parts(Global);

        storage.begin_new_tx();
        storage.start_frame();

        let hash = Bytes32::default();
        let preimage_publication_byte_len = 100;
        storage
            .add_preimage(&hash, preimage_publication_byte_len, PreimageType::Bytecode)
            .expect("add_preimage should succeed");

        assert_eq!(storage.net_pubdata_used(), 100);

        storage.finish_frame(None);

        assert_eq!(storage.net_pubdata_used(), 100);
    }

    #[test]
    fn single_tx_nested_frames_ok() {
        let mut storage = NewPreimagesPublicationStorage::<_>::new_from_parts(Global);

        storage.begin_new_tx();
        storage.start_frame();

        let preimage_publication_byte_len = 100;
        storage
            .add_preimage(
                &Bytes32::from([1u8; 32]),
                preimage_publication_byte_len,
                PreimageType::Bytecode,
            )
            .expect("add_preimage should succeed");

        storage.start_frame();

        storage
            .add_preimage(
                &Bytes32::from([2u8; 32]),
                preimage_publication_byte_len,
                PreimageType::Bytecode,
            )
            .expect("add_preimage should succeed");

        storage.finish_frame(None);

        assert_eq!(storage.net_pubdata_used(), 200);
    }

    #[test]
    fn single_tx_single_frame_revert() {
        let mut storage = NewPreimagesPublicationStorage::<_>::new_from_parts(Global);

        storage.begin_new_tx();
        let ss = storage.start_frame();

        let preimage_publication_byte_len = 100;
        storage
            .add_preimage(
                &Bytes32::from([1u8; 32]),
                preimage_publication_byte_len,
                PreimageType::Bytecode,
            )
            .expect("add_preimage should succeed");

        storage.finish_frame(Some(&ss));

        assert_eq!(storage.net_pubdata_used(), 0);
    }

    #[test]
    fn single_tx_single_frame_mul_imgs_ok() {
        let mut storage = NewPreimagesPublicationStorage::<_>::new_from_parts(Global);

        storage.begin_new_tx();
        storage.start_frame();

        let preimage_publication_byte_len = 100;
        storage
            .add_preimage(
                &Bytes32::from([1u8; 32]),
                preimage_publication_byte_len,
                PreimageType::Bytecode,
            )
            .expect("add_preimage should succeed");
        storage
            .add_preimage(
                &Bytes32::from([2u8; 32]),
                preimage_publication_byte_len,
                PreimageType::Bytecode,
            )
            .expect("add_preimage should succeed");

        storage.finish_frame(None);

        assert_eq!(storage.net_pubdata_used(), 200);
    }

    #[test]
    fn single_tx_nested_frames_mul_imgs_revert() {
        let mut storage = NewPreimagesPublicationStorage::<_>::new_from_parts(Global);

        storage.begin_new_tx();
        storage.start_frame();

        let preimage_publication_byte_len = 100;
        storage
            .add_preimage(
                &Bytes32::from([1u8; 32]),
                preimage_publication_byte_len,
                PreimageType::Bytecode,
            )
            .expect("add_preimage should succeed");

        let ss = storage.start_frame();

        storage
            .add_preimage(
                &Bytes32::from([2u8; 32]),
                preimage_publication_byte_len,
                PreimageType::Bytecode,
            )
            .expect("add_preimage should succeed");

        storage.finish_frame(Some(&ss));

        assert_eq!(storage.net_pubdata_used(), 100);
    }

    #[test]
    fn test_reuse_preimage() {
        let mut storage = NewPreimagesPublicationStorage::<_>::new_from_parts(Global);

        storage.begin_new_tx();
        storage.start_frame();

        let preimage_publication_byte_len = 100;
        storage
            .add_preimage(
                &Bytes32::from([1u8; 32]),
                preimage_publication_byte_len,
                PreimageType::Bytecode,
            )
            .expect("add_preimage should succeed");
        storage
            .add_preimage(
                &Bytes32::from([1u8; 32]),
                preimage_publication_byte_len,
                PreimageType::Bytecode,
            )
            .expect("add_preimage should succeed");

        storage.finish_frame(None);

        assert_eq!(storage.net_pubdata_used(), 100);
    }

    #[test]
    fn test_empty_transaction() {
        let mut storage = NewPreimagesPublicationStorage::<_>::new_from_parts(Global);

        storage.begin_new_tx();
        storage.start_frame();
        storage.finish_frame(None);

        assert_eq!(storage.net_pubdata_used(), 000);
    }
}
