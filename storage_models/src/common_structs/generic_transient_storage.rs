use alloc::alloc::Global;
use zk_ee::common_structs::history_map::HistoryMapItemRefMut;

use core::alloc::Allocator;
use core::marker::PhantomData;
use zk_ee::common_traits::key_like_with_bounds::KeyLikeWithBounds;
use zk_ee::system::errors::{internal::InternalError, system::SystemError};
use zk_ee::{
    common_structs::history_map::{CacheSnapshotId, HistoryMap},
    memory::stack_trait::{StackCtor, StackCtorConst},
};

pub type GenericTransientStorageStackCheck<SCC: const StackCtorConst, A: Allocator> =
    [(); SCC::extra_const_param::<usize, A>()];

pub struct GenericTransientStorage<
    K: KeyLikeWithBounds,
    V: Clone,
    SC: StackCtor<SCC>,
    SCC: const StackCtorConst,
    A: Allocator + Clone = Global,
> where
    GenericTransientStorageStackCheck<SCC, A>:,
{
    cache: HistoryMap<K, V, A>,
    pub(crate) current_tx_number: u32,
    phantom: PhantomData<(SC, SCC)>,
    alloc: A,
}

impl<
        K: KeyLikeWithBounds,
        V: Clone + Default,
        SC: StackCtor<SCC>,
        SCC: const StackCtorConst,
        A: Allocator + Clone,
    > GenericTransientStorage<K, V, SC, SCC, A>
where
    GenericTransientStorageStackCheck<SCC, A>:,
{
    pub fn new_from_parts(allocator: A) -> Self {
        Self {
            cache: HistoryMap::new(allocator.clone()),
            current_tx_number: 0,
            phantom: PhantomData,
            alloc: allocator.clone(),
        }
    }

    pub fn begin_new_tx(&mut self) {
        // Just discard old history
        // Note: it will reset snapshots counter, old snapshots handlers can't be used anymore
        // Note: We will reset it redundantly for first tx
        self.cache = HistoryMap::new(self.alloc.clone());
        self.current_tx_number += 1;
    }

    #[track_caller]
    pub fn start_frame(&mut self) -> CacheSnapshotId {
        self.cache.snapshot()
    }

    /// Read element and initialize it if needed
    fn materialize_element<'a>(
        cache: &'a mut HistoryMap<K, V, A>,
        key: &'a K,
    ) -> Result<HistoryMapItemRefMut<'a, K, V, A>, SystemError>
    where
        V: Default,
    {
        cache.get_or_insert(key, || Ok(V::default()))
    }

    pub fn apply_read(&mut self, key: &K, dst: &mut V) -> Result<(), SystemError>
    where
        V: Default,
    {
        let data = Self::materialize_element(&mut self.cache, key)?;
        *dst = data.current().clone();

        Ok(())
    }

    pub fn apply_write(&mut self, key: &K, value: &V) -> Result<(), SystemError>
    where
        V: Default,
    {
        let mut data = Self::materialize_element(&mut self.cache, key)?;
        data.update(|x| {
            *x = value.clone();
            Ok(())
        })
        .map_err(SystemError::LeafRuntime)
    }

    #[track_caller]
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
}
