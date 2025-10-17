pub(crate) use super::ErrorContext;

pub trait Contextualized<E>: Sized {
    #[inline(always)]
    fn context(self, context: ErrorContext) -> E {
        self.with_context(|| context)
    }

    #[inline(always)]
    #[cfg_attr(target_arch = "riscv32", allow(unused))]
    fn with_context<F>(self, f: F) -> E
    where
        F: FnOnce() -> ErrorContext,
    {
        #[cfg(target_arch = "riscv32")]
        {
            self.with_context_inner(|| ErrorContext::default())
        }
        #[cfg(not(target_arch = "riscv32"))]
        {
            self.with_context_inner(f)
        }
    }

    /// Has to be implemented for error types.
    fn with_context_inner<F>(self, f: F) -> E
    where
        F: FnOnce() -> ErrorContext;
}

/// This helper allows to add context to an error inside `Result`
impl<T, E> Contextualized<Result<T, E>> for Result<T, E>
where
    E: Contextualized<E>,
{
    #[inline(always)]
    fn with_context_inner<F>(self, f: F) -> Result<T, E>
    where
        F: FnOnce() -> ErrorContext,
    {
        self.map_err(|e| e.with_context(f))
    }
}
