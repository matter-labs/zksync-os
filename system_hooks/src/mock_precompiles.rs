//! Mocked precompiles needed to pass some tests in the EVM test suite.
//! Not to be used in production.
#[allow(clippy::module_inception)]
#[cfg(feature = "mock-unsupported-precompiles")]
pub(crate) mod mock_precompiles {
    use zk_ee::system::{errors::SystemFunctionError, Resources, SystemFunction};

    pub struct Blake;
    impl<R: Resources> SystemFunction<R> for Blake {
        fn execute<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
            input: &[u8],
            _output: &mut D,
            _resources: &mut R,
            _allocator: A,
        ) -> Result<(), SystemFunctionError> {
            if input.len() != 213 {
                return Err(SystemFunctionError::InvalidInput);
            }
            Ok(())
        }
    }

    pub struct PointEval;
    impl<R: Resources> SystemFunction<R> for PointEval {
        fn execute<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
            input: &[u8],
            _output: &mut D,
            _resources: &mut R,
            _allocator: A,
        ) -> Result<(), SystemFunctionError> {
            if input.len() != 193 {
                return Err(SystemFunctionError::InvalidInput);
            }
            Ok(())
        }
    }
}
