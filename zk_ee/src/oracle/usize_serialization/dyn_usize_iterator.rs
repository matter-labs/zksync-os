use alloc::boxed::Box;

/// Type-erased iterator that owns its data and provides dynamic dispatch.
///
/// This struct enables returning `UsizeSerializable` iterators as boxed trait objects
/// while maintaining ownership of the underlying data. It uses unsafe lifetime extension
/// to create stable references for iterator construction, then manages cleanup automatically.
pub struct DynUsizeIterator<I: 'static, IT: ExactSizeIterator<Item = usize> + 'static> {
    item: I,
    iterator: Option<IT>,
}

impl<I: 'static, IT: ExactSizeIterator<Item = usize> + 'static> DynUsizeIterator<I, IT> {
    pub fn from_constructor<FN: FnOnce(&'static I) -> IT>(
        item: I,
        closure: FN,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static> {
        // TODO: eventually we will get in-place constructors
        unsafe {
            let mut item = Box::new(Self {
                item,
                iterator: None,
            });
            // now with location being stable, we can life-extend it and take reference
            let static_ref: &'static I = core::mem::transmute(&item.as_ref().item);
            let iterator = (closure)(static_ref);
            item.as_mut().iterator = Some(iterator);

            item as Box<dyn ExactSizeIterator<Item = usize> + 'static>
        }
    }
}

impl<I: 'static, IT: ExactSizeIterator<Item = usize> + 'static> Iterator
    for DynUsizeIterator<I, IT>
{
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        // Safety: we do not move out of item itself, but we modify iterator (also do not move unless drop)

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

impl<I: 'static, IT: ExactSizeIterator<Item = usize> + 'static> ExactSizeIterator
    for DynUsizeIterator<I, IT>
{
    fn len(&self) -> usize {
        self.iterator.as_ref().map(|it| it.len()).unwrap_or(0)
    }
}

impl<I: 'static, IT: ExactSizeIterator<Item = usize> + 'static> Drop for DynUsizeIterator<I, IT> {
    fn drop(&mut self) {
        // we do not move, so iterating is ok
        drop(self.iterator.take());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dyn_usize_iterator_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let mut iter = DynUsizeIterator::from_constructor(data, |data| data.iter().copied());

        assert_eq!(iter.len(), 5);
        assert_eq!(iter.next(), Some(1));
        assert_eq!(iter.len(), 4);
        assert_eq!(iter.next(), Some(2));
        assert_eq!(iter.next(), Some(3));
        assert_eq!(iter.len(), 2);
    }

    #[test]
    fn test_dyn_usize_iterator_empty() {
        let data = vec![];
        let mut iter = DynUsizeIterator::from_constructor(data, |data| data.iter().copied());

        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_dyn_usize_iterator_full_consumption() {
        let data = vec![10, 20, 30];
        let iter = DynUsizeIterator::from_constructor(data, |data| data.iter().copied());

        let collected: Vec<_> = iter.collect();
        assert_eq!(collected, vec![10, 20, 30]);
    }

    #[test]
    fn test_dyn_usize_iterator_length_tracking() {
        let data = vec![1, 2];
        let mut iter = DynUsizeIterator::from_constructor(data, |data| data.iter().copied());

        assert_eq!(iter.len(), 2);
        assert_eq!(iter.next(), Some(1));
        assert_eq!(iter.len(), 1);
        assert_eq!(iter.next(), Some(2));
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.len(), 0);
    }

    #[test]
    fn test_dyn_usize_iterator_with_array() {
        let data = [42, 100, 255];
        let mut iter = DynUsizeIterator::from_constructor(data, |data| data.iter().copied());

        assert_eq!(iter.len(), 3);
        assert_eq!(iter.next(), Some(42));
        assert_eq!(iter.next(), Some(100));
        assert_eq!(iter.next(), Some(255));
        assert_eq!(iter.next(), None);
    }
}
