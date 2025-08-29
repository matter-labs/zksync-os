use crate::utils::USIZE_SIZE;

pub trait UsizeReadable {
    ///
    /// TODO: add doc
    ///
    /// # Safety
    ///
    unsafe fn read_usize(&mut self) -> usize;
}

pub trait SafeUsizeReadable: UsizeReadable {
    fn len(&self) -> usize;
    fn try_read(&mut self) -> Result<usize, ()> {
        unsafe { Ok(UsizeReadable::read_usize(self)) }
    }
}

pub struct ReadIterWrapper<T: 'static + Clone + Copy, I: Iterator<Item = T>> {
    inner: I,
}

impl<T: 'static + Clone + Copy, I: Iterator<Item = T>> From<I> for ReadIterWrapper<T, I> {
    fn from(value: I) -> Self {
        ReadIterWrapper::<T, I> { inner: value }
    }
}

impl<I: Iterator<Item = u8>> UsizeReadable for ReadIterWrapper<u8, I> {
    unsafe fn read_usize(&mut self) -> usize {
        let mut dst = 0usize.to_ne_bytes();
        for (dst, src) in dst.iter_mut().zip(&mut self.inner) {
            *dst = src;
        }
        usize::from_ne_bytes(dst)
    }
}

impl<I: Iterator<Item = usize>> UsizeReadable for ReadIterWrapper<usize, I> {
    unsafe fn read_usize(&mut self) -> usize {
        self.inner.next().unwrap_unchecked()
    }
}

impl<I: ExactSizeIterator<Item = u8>> Iterator for ReadIterWrapper<u8, I> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        if self.inner.len() == 0 {
            return None;
        }
        let mut dst = 0usize.to_ne_bytes();
        for (dst, src) in dst.iter_mut().zip(&mut self.inner) {
            *dst = src;
        }
        Some(usize::from_ne_bytes(dst))
    }
}

impl<I: ExactSizeIterator<Item = u8>> ExactSizeIterator for ReadIterWrapper<u8, I> {
    fn len(&self) -> usize {
        self.inner.len().next_multiple_of(USIZE_SIZE) / USIZE_SIZE
    }
}

impl<I: ExactSizeIterator<Item = usize>> Iterator for ReadIterWrapper<usize, I> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        if self.inner.len() == 0 {
            return None;
        }
        self.inner.next()
    }
}

impl<I: ExactSizeIterator<Item = usize>> ExactSizeIterator for ReadIterWrapper<usize, I> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

#[derive(Clone)]
pub struct ExactSizeIterReadWrapper<I: ExactSizeIterator<Item = usize>> {
    inner: I,
}

impl<I: ExactSizeIterator<Item = usize>> From<I> for ExactSizeIterReadWrapper<I> {
    fn from(value: I) -> Self {
        ExactSizeIterReadWrapper::<I> { inner: value }
    }
}

impl<I: ExactSizeIterator<Item = usize>> UsizeReadable for ExactSizeIterReadWrapper<I> {
    unsafe fn read_usize(&mut self) -> usize {
        self.inner.next().unwrap_unchecked()
    }
}

impl<I: ExactSizeIterator<Item = usize>> SafeUsizeReadable for ExactSizeIterReadWrapper<I> {
    fn len(&self) -> usize {
        self.inner.len()
    }
    fn try_read(&mut self) -> Result<usize, ()> {
        self.inner.next().ok_or(())
    }
}

pub trait UsizeWriteable {
    ///
    /// TODO: add doc
    ///
    /// # Safety
    ///
    unsafe fn write_usize(&mut self, value: usize);
}

pub trait SafeUsizeWritable: UsizeWriteable {
    fn len(&self) -> usize;
    fn try_write(&mut self, value: usize) -> Result<(), ()> {
        unsafe {
            UsizeWriteable::write_usize(self, value);
        }
        Ok(())
    }
}

pub struct WriteIterWrapper<'a, T: 'static + Clone + Copy, I: Iterator<Item = &'a mut T>> {
    inner: I,
}

impl<'a, T: 'static + Clone + Copy, I: ExactSizeIterator<Item = &'a mut T>>
    WriteIterWrapper<'a, T, I>
{
    pub fn usize_len(&self) -> usize {
        self.inner.len().next_multiple_of(USIZE_SIZE) / USIZE_SIZE
    }
}

impl<'a, T: 'static + Clone + Copy, I: Iterator<Item = &'a mut T>> From<I>
    for WriteIterWrapper<'a, T, I>
{
    fn from(value: I) -> Self {
        WriteIterWrapper::<T, I> { inner: value }
    }
}

// TODO: specialize in case of aligned iterator

impl<'a, I: Iterator<Item = &'a mut u8>> UsizeWriteable for WriteIterWrapper<'a, u8, I> {
    unsafe fn write_usize(&mut self, value: usize) {
        let le_bytes = value.to_ne_bytes();
        for (src, dst) in le_bytes.into_iter().zip(&mut self.inner) {
            *dst = src;
        }
    }
}

impl<'a, I: Iterator<Item = &'a mut usize>> UsizeWriteable for WriteIterWrapper<'a, usize, I> {
    unsafe fn write_usize(&mut self, value: usize) {
        *self.inner.next().unwrap_unchecked() = value;
    }
}

impl<'a, I: ExactSizeIterator<Item = &'a mut u8>> SafeUsizeWritable
    for WriteIterWrapper<'a, u8, I>
{
    fn len(&self) -> usize {
        self.usize_len()
    }
    fn try_write(&mut self, value: usize) -> Result<(), ()> {
        let le_bytes = value.to_ne_bytes();
        for byte in le_bytes.into_iter() {
            *self.inner.next().ok_or(())? = byte;
        }
        Ok(())
    }
}
