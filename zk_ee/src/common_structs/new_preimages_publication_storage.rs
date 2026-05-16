use crate::utils::Bytes32;
use crate::{internal_error, system::errors::internal::InternalError};
use alloc::alloc::Global;
use core::alloc::Allocator;

use super::{
    cache_record::CacheRecord,
    history_map::{CacheSnapshotId, HistoryMap, HistoryMapItemRef},
};

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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
            Err(internal_error!("Overflow in num_uses"))
        }
    }
}

// we want to store new preimages for DA

pub struct NewPreimagesPublicationStorage<A: Allocator + Clone = Global> {
    cache: HistoryMap<Bytes32, CacheRecord<Elem, ()>, A, ()>,
}

impl<A: Allocator + Clone> NewPreimagesPublicationStorage<A> {
    pub fn new_from_parts(allocator: A) -> Self {
        Self {
            cache: HistoryMap::new(allocator.clone()),
        }
    }

    pub fn begin_new_tx(&mut self) {
        self.cache.commit();
    }

    pub fn finish_tx(&mut self) {}

    #[track_caller]
    pub fn start_frame(&mut self) -> CacheSnapshotId {
        self.cache.snapshot()
    }

    #[track_caller]
    #[must_use]
    pub fn finish_frame(
        &mut self,
        rollback_handle: Option<&CacheSnapshotId>,
    ) -> Result<(), InternalError> {
        if let Some(x) = rollback_handle {
            self.cache.rollback(*x)
        } else {
            Ok(())
        }
    }

    pub fn add_preimage(
        &mut self,
        hash: &Bytes32,
        preimage_publication_byte_len: usize,
        preimage_type: PreimageType,
    ) -> Result<(), InternalError> {
        let mut item = self.cache.get_or_insert(hash, || {
            let new = Elem {
                preimage_type,
                value: PreimagesPublicationStorageValue {
                    num_uses: 0,
                    publication_net_bytes: preimage_publication_byte_len,
                },
            };
            Ok((CacheRecord::new(new), ()))
        })?;

        item.update(|x| {
            x.update(|elem, _| {
                if elem.value.num_uses > 1 {
                    assert_eq!(
                        elem.value.publication_net_bytes,
                        preimage_publication_byte_len
                    );
                }
                elem.value.mark_use()
            })
        })?;

        Ok(())
    }

    pub fn net_diffs_iter(
        &'_ self,
    ) -> impl Iterator<Item = HistoryMapItemRef<'_, Bytes32, CacheRecord<Elem, ()>, A>> {
        self.cache.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calculate_total_pubdata_used(storage: &NewPreimagesPublicationStorage) -> usize {
        let mut pubdata_used = 0;
        storage
            .cache
            .apply_to_all_updated_elements::<_, ()>(|_, r, _| {
                pubdata_used += r.value().value.publication_net_bytes;
                Ok(())
            })
            .expect("We're returning ok.");

        pubdata_used
    }

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

        assert_eq!(calculate_total_pubdata_used(&storage), 100);

        storage
            .finish_frame(None)
            .expect("Correct finishing snapshot");

        assert_eq!(calculate_total_pubdata_used(&storage), 100);
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

        storage
            .finish_frame(None)
            .expect("Correct finishing snapshot");

        assert_eq!(calculate_total_pubdata_used(&storage), 200);
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

        storage
            .finish_frame(Some(&ss))
            .expect("Correct finishing snapshot");

        assert_eq!(calculate_total_pubdata_used(&storage), 0);
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

        storage
            .finish_frame(None)
            .expect("Correct finishing snapshot");

        assert_eq!(calculate_total_pubdata_used(&storage), 200);
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

        storage
            .finish_frame(Some(&ss))
            .expect("Correct finishing snapshot");

        assert_eq!(calculate_total_pubdata_used(&storage), 100);
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

        storage
            .finish_frame(None)
            .expect("Correct finishing snapshot");

        assert_eq!(calculate_total_pubdata_used(&storage), 100);
    }

    #[test]
    fn test_empty_transaction() {
        let mut storage = NewPreimagesPublicationStorage::<_>::new_from_parts(Global);

        storage.begin_new_tx();
        storage.start_frame();
        storage
            .finish_frame(None)
            .expect("Correct finishing snapshot");

        assert_eq!(calculate_total_pubdata_used(&storage), 000);
    }
}
