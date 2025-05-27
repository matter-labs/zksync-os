#[allow(dead_code)]
pub(crate) trait UnsafeCellEx<T> {
    /// Dereferences the contained value. Synonym for `&*self.get()`. Same safety rules apply.
    unsafe fn u_deref(&self) -> &T;

    /// Mutably dereferences the contained value. Synonym for `&mut *self.get()`. Same safety rules
    /// apply.
    #[allow(clippy::mut_from_ref)]
    unsafe fn u_deref_mut(&self) -> &mut T;
}

impl<T> UnsafeCellEx<T> for core::cell::UnsafeCell<T> {
    unsafe fn u_deref(&self) -> &T {
        &*self.get()
    }

    unsafe fn u_deref_mut(&self) -> &mut T {
        &mut *self.get()
    }
}

pub(crate) trait PipeOp<T> {
    fn to<F, U>(self, f: F) -> U
    where
        F: FnOnce(T) -> U;

    #[allow(dead_code)]
    fn op<F>(self, f: F) -> T
    where
        F: FnOnce(&mut T);
}

impl<T> PipeOp<T> for T {
    fn to<F, U>(self, f: F) -> U
    where
        F: FnOnce(T) -> U,
    {
        f(self)
    }

    fn op<F>(mut self, f: F) -> T
    where
        F: FnOnce(&mut T),
    {
        f(&mut self);
        self
    }
}
