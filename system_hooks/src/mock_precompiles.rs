//! Mocked precompiles needed to pass some tests in the EVM test suite.
//! Not to be used in production.
#[allow(clippy::module_inception)]
#[cfg(feature = "mock-unsupported-precompiles")]
pub(crate) mod mock_precompiles {
    use zk_ee::{
        common_traits::TryExtend,
        interface_error,
        system::{
            base_system_functions::MissingSystemFunctionErrors, errors::subsystem::SubsystemError,
            MockedSystemFunctionError, Resources, SystemFunction,
        },
    };

    pub struct Blake2f;
    impl<R: Resources> SystemFunction<R, MissingSystemFunctionErrors> for Blake2f {
        fn execute<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
            input: &[u8],
            _output: &mut D,
            _resources: &mut R,
            _allocator: A,
        ) -> Result<(), SubsystemError<MissingSystemFunctionErrors>> {
            if input.len() != 213 {
                return Err(interface_error!(MockedSystemFunctionError::InvalidInputLength).into());
            }
            Ok(())
        }
    }

    #[cfg(not(feature = "point_eval_precompile"))]
    pub struct PointEvaluation;
    #[cfg(not(feature = "point_eval_precompile"))]
    impl<R: Resources> SystemFunction<R, MissingSystemFunctionErrors> for PointEvaluation {
        fn execute<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
            input: &[u8],
            _output: &mut D,
            _resources: &mut R,
            _allocator: A,
        ) -> Result<(), SubsystemError<MissingSystemFunctionErrors>> {
            if input.len() != 192 {
                return Err(interface_error!(MockedSystemFunctionError::InvalidInputLength).into());
            }
            Ok(())
        }
    }
}
