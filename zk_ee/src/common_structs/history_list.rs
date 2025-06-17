use alloc::alloc::{Allocator, Global};
use core::marker::PhantomData;

use crate::memory::stack_trait::{Stack, StackCtor, StackCtorConst};

pub struct HistoryList<
    V,
    M: Clone,
    SC: StackCtor<SCC>,
    SCC: const StackCtorConst,
    A: Allocator + Clone = Global,
> where
    [(); SCC::extra_const_param::<(V, M), A>()]:,
{
    list: SC::Stack<(V, M), { SCC::extra_const_param::<(V, M), A>() }, A>,
    _phantom: PhantomData<A>,
}

impl<V, M: Clone, SC: StackCtor<SCC>, SCC: const StackCtorConst, A: Allocator + Clone>
    HistoryList<V, M, SC, SCC, A>
where
    [(); SCC::extra_const_param::<(V, M), A>()]:,
{
    pub fn new(alloc: A) -> Self {
        Self {
            list: SC::Stack::new_in(alloc),
            _phantom: PhantomData,
        }
    }

    pub fn snapshot(&mut self) -> usize {
        self.list.len()
    }

    pub fn rollback(&mut self, snapshot: usize) {
        self.list.truncate(snapshot);
    }

    pub fn push(&mut self, value: V, md: M) {
        self.list.push((value, md));
    }

    pub fn top(&self) -> Option<(&V, &M)> {
        self.list.top().map(|(v, m)| (v, m))
    }

    pub fn top_mut(&mut self) -> Option<(&mut V, &mut M)> {
        self.list.top_mut().map(|(v, m)| (v, m))
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &V> {
        self.list.iter().map(|(v, _)| v)
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }
}
