use alloc::alloc::Global;
use zk_ee::common_structs::history_map::HistoryMapItemRefMut;

use core::alloc::Allocator;
use core::marker::PhantomData;
use ruint::aliases::B160;
use zk_ee::common_traits::key_like_with_bounds::{KeyLikeWithBounds, TyEq};
use zk_ee::system::errors::SystemError;
use zk_ee::{
    common_structs::history_map::{Appearance, CacheSnapshotId, HistoryMap, TransactionId},
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
    cache: HistoryMap<K, V, (), A>,
    pub(crate) current_tx_number: u32,
    phantom: PhantomData<(SC, SCC)>,
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

    fn materialize_element<'a>(
        cache: &'a mut HistoryMap<K, V, (), A>,
        key: &'a K,
    ) -> Result<HistoryMapItemRefMut<'a, K, V, (), A>, SystemError>
    where
        V: Default,
    {
        cache.get_or_insert(&mut (), key, |_| {
            let new_value = V::default();
            Ok((new_value, Appearance::Unset))
        })
    }

    pub fn apply_read(&mut self, key: &K, dst: &mut V) -> Result<(), SystemError>
    where
        V: Default,
    {
        let data = Self::materialize_element(&mut self.cache, key)?;
        *dst = data.current().value.clone();

        Ok(())
    }

    pub fn apply_write(&mut self, key: &K, value: &V) -> Result<(), SystemError>
    where
        V: Default,
    {
        let mut data = Self::materialize_element(&mut self.cache, key)?;
        data.update(|x, _| {
            *x = value.clone();
            Ok(())
        })
        .map_err(SystemError::Internal)
    }

    #[track_caller]
    pub fn finish_frame(&mut self, rollback_handle: Option<&CacheSnapshotId>) {
        if let Some(x) = rollback_handle {
            self.cache.rollback(*x);
        }
    }

    pub fn clear_state(&mut self, address: &B160) -> Result<(), SystemError>
    where
        K::Subspace: TyEq<B160>,
    {
        use core::ops::Bound::Included;
        let lower_bound = K::lower_bound(TyEq::rwi(*address));
        let upper_bound = K::upper_bound(TyEq::rwi(*address));
        self.cache
            .for_each_range((Included(&lower_bound), Included(&upper_bound)), |mut x| {
                x.unset()
            })?;

        Ok(())
    }
}
