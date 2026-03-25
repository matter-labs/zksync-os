/// Using this trait allows using Copy instead of Clone in the proving
/// environment. It is useful for types like errors which contain less fields
/// when compiled for proving.
pub trait CheapCloneProvingEnv {
    /// Clone in forward mode, Copy in proving environment.
    /// Used for performance on small types.
    fn clone_or_copy(&self) -> Self;
}

#[cfg(feature = "proving_env")]
impl<T> CheapCloneProvingEnv for T
where
    T: Copy,
{
    #[inline]
    fn clone_or_copy(&self) -> Self {
        *self
    }
}

#[cfg(not(feature = "proving_env"))]
impl<T> CheapCloneProvingEnv for T
where
    T: Clone,
{
    #[inline]
    fn clone_or_copy(&self) -> Self {
        self.clone()
    }
}
