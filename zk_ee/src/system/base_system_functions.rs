use super::Resources;
use crate::{internal_error, system::errors::SystemFunctionError};

///
/// System function implementation.
///
pub trait SystemFunction<R: Resources> {
    /// Writes result to the `output` and returns actual output slice length that was used.
    /// Should return error on invalid inputs and if resources do not even cover basic parsing cost.
    /// in practice only pairing can have invalid input(size) on charging stage.
    fn execute<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SystemFunctionError>;
}

pub struct MissingSystemFunction;
impl<R: Resources> SystemFunction<R> for MissingSystemFunction {
    fn execute<D: ?Sized + Extend<u8>, A: core::alloc::Allocator + Clone>(
        _: &[u8],
        _: &mut D,
        _: &mut R,
        _: A,
    ) -> Result<(), SystemFunctionError> {
        Err(internal_error!("This system function is not defined for this system").into())
    }
}

pub trait SystemFunctions<R: Resources> {
    type Keccak256: SystemFunction<R>;
    type Sha256: SystemFunction<R>;
    type Secp256k1ECRecover: SystemFunction<R>;
    type Secp256k1AddProjective: SystemFunction<R>;
    type Secp256k1MulProjective: SystemFunction<R>;
    type Secp256r1AddProjective: SystemFunction<R>;
    type Secp256r1MulProjective: SystemFunction<R>;
    type P256Verify: SystemFunction<R>;
    type Bn254Add: SystemFunction<R>;
    type Bn254Mul: SystemFunction<R>;
    type Bn254PairingCheck: SystemFunction<R>;
    type RipeMd160: SystemFunction<R>;
    type ModExp: SystemFunction<R>;

    fn keccak256<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SystemFunctionError> {
        Self::Keccak256::execute(input, output, resources, allocator)
    }

    fn sha256<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SystemFunctionError> {
        Self::Sha256::execute(input, output, resources, allocator)
    }

    fn secp256k1_ec_recover<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SystemFunctionError> {
        Self::Secp256k1ECRecover::execute(input, output, resources, allocator)
    }

    fn secp256k1_add_projective<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SystemFunctionError> {
        Self::Secp256k1AddProjective::execute(input, output, resources, allocator)
    }

    fn secp256k1_mul_projective<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SystemFunctionError> {
        Self::Secp256k1MulProjective::execute(input, output, resources, allocator)
    }

    fn secp256r1_add_projective<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SystemFunctionError> {
        Self::Secp256r1AddProjective::execute(input, output, resources, allocator)
    }

    fn secp256r1_mul_projective<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SystemFunctionError> {
        Self::Secp256r1MulProjective::execute(input, output, resources, allocator)
    }

    fn p256_verify<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SystemFunctionError> {
        Self::P256Verify::execute(input, output, resources, allocator)
    }

    fn bn254_add<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SystemFunctionError> {
        Self::Bn254Add::execute(input, output, resources, allocator)
    }

    fn bn254_mul<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SystemFunctionError> {
        Self::Bn254Mul::execute(input, output, resources, allocator)
    }

    fn bn254_pairing_check<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SystemFunctionError> {
        Self::Bn254PairingCheck::execute(input, output, resources, allocator)
    }

    fn ripemd160<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SystemFunctionError> {
        Self::RipeMd160::execute(input, output, resources, allocator)
    }

    fn mod_exp<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SystemFunctionError> {
        Self::ModExp::execute(input, output, resources, allocator)
    }
}
