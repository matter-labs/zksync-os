use crate::common_traits::usize_serializable_dynamic::UsizeSerializableDynamic;
use crate::kv_markers::UsizeSerializable;
use alloc::alloc::Global;
use alloc::boxed::Box;
use core::pin::Pin;

// This is self-ref
// TODO: more comments
pub struct DynUsizeIterator<I> {
    item: Pin<Box<I>>,
    iterator: Option<Box<dyn ExactSizeIterator<Item = usize> + 'static>>,
}

impl<I: 'static> DynUsizeIterator<I> {
    #[allow(dead_code)]
    fn get_inner_static_ref(&'_ self) -> &'static I {
        unsafe { core::mem::transmute(self.item.as_ref().get_ref()) }
    }

    pub fn from_constructor<
        IT: ExactSizeIterator<Item = usize> + 'static,
        FN: FnOnce(&'static I) -> IT,
    >(
        item: I,
        closure: FN,
    ) -> Self {
        let item = Box::pin(item);
        let static_ref: &'static I = unsafe { core::mem::transmute(item.as_ref().get_ref()) };
        let iterator = (closure)(static_ref);

        Self {
            item,
            iterator: Some(Box::new(iterator)),
        }
    }
}

impl<I: UsizeSerializable + 'static> DynUsizeIterator<I> {
    pub fn from_owned(item: I) -> Self {
        let item = Box::pin(item);
        let static_ref: &'static I = unsafe { core::mem::transmute(item.as_ref().get_ref()) };
        let iterator = I::iter(static_ref);

        Self {
            item,
            iterator: Some(Box::new(iterator)),
        }
    }
}

impl<I: UsizeSerializableDynamic<Global> + 'static> DynUsizeIterator<I> {
    pub fn from_owned_dynamic(item: I) -> Self {
        let item = Box::pin(item);
        let static_ref: &'static I = unsafe { core::mem::transmute(item.as_ref().get_ref()) };
        let iterator = I::iter(static_ref, Global);

        Self {
            item,
            iterator: Some(Box::new(iterator)),
        }
    }
}

impl<I> Iterator for DynUsizeIterator<I> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let mut should_drop = false;
        let Some(it) = self.iterator.as_mut() else {
            // related access
            return None;
        };
        let result = it.next();
        if ExactSizeIterator::len(it) == 0 {
            should_drop = true;
        }
        if should_drop {
            // cleanup
            drop(self.iterator.take().unwrap());
        }

        result
    }
}

impl<I> ExactSizeIterator for DynUsizeIterator<I> {
    fn len(&self) -> usize {
        self.iterator.as_ref().map(|it| it.len()).unwrap_or(0)
    }
}

impl<I> Drop for DynUsizeIterator<I> {
    fn drop(&mut self) {
        drop(self.iterator.take());
    }
}
