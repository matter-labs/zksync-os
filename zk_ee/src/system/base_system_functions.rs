use crate::{define_subsystem, internal_error, oracle::IOOracle, system::logger::Logger};

use super::{
    common_traits::TryExtend,
    errors::subsystem::{Subsystem, SubsystemError},
    Resources,
};

// Definitions of errors for all system functions
define_subsystem!(Keccak256);
define_subsystem!(Sha256);
define_subsystem!(Secp256k1ECRecover);
define_subsystem!(Secp256k1AddProjective);
define_subsystem!(Secp256k1MulProjective);
define_subsystem!(Secp256r1AddProjective);
define_subsystem!(Secp256r1MulProjective);
define_subsystem!(P256Verify);

define_subsystem!(Bn254Add,
                  interface Bn254AddInterfaceError
                  {
                      InvalidPoint
                  }
);

define_subsystem!(Bn254Mul,
                  interface Bn254MulInterfaceError
                  {
                      InvalidPoint
                  }
);
define_subsystem!(Bn254PairingCheck,
                  interface Bn254PairingCheckInterfaceError
                  {
                      InvalidPoint,
                      InvalidPairingSize
                  }
);

define_subsystem!(RipeMd160);

define_subsystem!(ModExp,
                  interface ModExpInterfaceError
                  {
                      InvalidInputLength,
                      InvalidModulus,
                      DivisionByZero,
                      InputLengthExceedsLimit
                  }
);

define_subsystem!(PointEvaluation,
                  interface PointEvaluationInterfaceError
                  {
                      InvalidPoint,
                      InvalidInputSize,
                      InvalidVersionedHash,
                      InvalidScalar,
                      PairingMismatch,
                  }
);

define_subsystem!(Bls12Precompile,
                  interface Bls12PrecompileInterfaceError
                  {
                      InvalidFieldElement,
                      InvalidG1Point,
                      InvalidG2Point,
                      InvalidInputSize,
                      PointNotInSubgroup,
                  }
);

define_subsystem!(Blake2FPrecompile,
                  interface Blake2FPrecompileInterfaceError
                  {
                      InvalidInputSize,
                      InvalidBooleanFlag,
                  }
);

define_subsystem!(MissingSystemFunction,
                    // Used only for tests
                  interface MockedSystemFunctionError
                  {
                      InvalidInputLength,
                  }
);

///
/// System function implementation.
///
pub trait SystemFunction<R: Resources, E: Subsystem> {
    /// Writes result to the `output` and returns actual output slice length that was used.
    /// Should return error on invalid inputs and if resources do not even cover basic parsing cost.
    /// In practice only pairing can have invalid input(size) on charging stage.
    fn execute<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<E>>;
}

///
/// Extended system function implementation for cases when IO oracle access is needed
///
pub trait SystemFunctionExt<R: Resources, E: Subsystem> {
    /// Writes result to the `output` and returns actual output slice length that was used.
    /// Should return error on invalid inputs and if resources do not even cover basic parsing cost.
    /// in practice only pairing can have invalid input(size) on charging stage.
    /// Callee is provided with access to oracle for it's work, and to logger if needed.
    fn execute<
        O: IOOracle,
        L: Logger,
        D: TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        oracle: &mut O,
        logger: &mut L,
        allocator: A,
    ) -> Result<(), SubsystemError<E>>;
}

pub struct MissingSystemFunction;

impl<R: Resources> SystemFunction<R, MissingSystemFunctionErrors> for MissingSystemFunction {
    fn execute<D: ?Sized + TryExtend<u8>, A: core::alloc::Allocator + Clone>(
        _: &[u8],
        _: &mut D,
        _: &mut R,
        _: A,
    ) -> Result<(), SubsystemError<MissingSystemFunctionErrors>> {
        Err(internal_error!("This system function is not defined for this system").into())
    }
}

// Additional implementations for missing projective curve operations
impl<R: Resources> SystemFunction<R, Secp256k1AddProjectiveErrors> for MissingSystemFunction {
    fn execute<D: ?Sized + TryExtend<u8>, A: core::alloc::Allocator + Clone>(
        _: &[u8],
        _: &mut D,
        _: &mut R,
        _: A,
    ) -> Result<(), SubsystemError<Secp256k1AddProjectiveErrors>> {
        Err(internal_error!("Secp256k1 add projective not implemented").into())
    }
}

impl<R: Resources> SystemFunction<R, Secp256k1MulProjectiveErrors> for MissingSystemFunction {
    fn execute<D: ?Sized + TryExtend<u8>, A: core::alloc::Allocator + Clone>(
        _: &[u8],
        _: &mut D,
        _: &mut R,
        _: A,
    ) -> Result<(), SubsystemError<Secp256k1MulProjectiveErrors>> {
        Err(internal_error!("Secp256k1 mul projective not implemented").into())
    }
}

impl<R: Resources> SystemFunction<R, Secp256r1AddProjectiveErrors> for MissingSystemFunction {
    fn execute<D: ?Sized + TryExtend<u8>, A: core::alloc::Allocator + Clone>(
        _: &[u8],
        _: &mut D,
        _: &mut R,
        _: A,
    ) -> Result<(), SubsystemError<Secp256r1AddProjectiveErrors>> {
        Err(internal_error!("Secp256r1 add projective not implemented").into())
    }
}

impl<R: Resources> SystemFunction<R, Secp256r1MulProjectiveErrors> for MissingSystemFunction {
    fn execute<D: ?Sized + TryExtend<u8>, A: core::alloc::Allocator + Clone>(
        _: &[u8],
        _: &mut D,
        _: &mut R,
        _: A,
    ) -> Result<(), SubsystemError<Secp256r1MulProjectiveErrors>> {
        Err(internal_error!("Secp256r1 mul projective not implemented").into())
    }
}

pub trait SystemFunctions<R: Resources> {
    type Keccak256: SystemFunction<R, Keccak256Errors>;
    type Sha256: SystemFunction<R, Sha256Errors>;
    type Secp256k1AddProjective: SystemFunction<R, Secp256k1AddProjectiveErrors>;
    type Secp256k1MulProjective: SystemFunction<R, Secp256k1MulProjectiveErrors>;
    type Secp256r1AddProjective: SystemFunction<R, Secp256r1AddProjectiveErrors>;
    type Secp256r1MulProjective: SystemFunction<R, Secp256r1MulProjectiveErrors>;
    type P256Verify: SystemFunction<R, P256VerifyErrors>;
    type Bn254Add: SystemFunction<R, Bn254AddErrors>;
    type Bn254Mul: SystemFunction<R, Bn254MulErrors>;
    type Bn254PairingCheck: SystemFunction<R, Bn254PairingCheckErrors>;
    type RipeMd160: SystemFunction<R, RipeMd160Errors>;
    type PointEvaluation: SystemFunction<R, PointEvaluationErrors>;
    type Bls12G1Add: SystemFunction<R, Bls12PrecompileErrors>;
    type Bls12G2Add: SystemFunction<R, Bls12PrecompileErrors>;
    type Bls12G1Msm: SystemFunction<R, Bls12PrecompileErrors>;
    type Bls12G2Msm: SystemFunction<R, Bls12PrecompileErrors>;
    type Bls12PairingCheck: SystemFunction<R, Bls12PrecompileErrors>;
    type Bls12MapFpToG1: SystemFunction<R, Bls12PrecompileErrors>;
    type Bls12MapFp2ToG2: SystemFunction<R, Bls12PrecompileErrors>;
    type Blake2F: SystemFunction<R, Blake2FPrecompileErrors>;

    fn keccak256<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Keccak256Errors>> {
        Self::Keccak256::execute(input, output, resources, allocator)
    }

    fn sha256<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Sha256Errors>> {
        Self::Sha256::execute(input, output, resources, allocator)
    }

    fn secp256k1_add_projective<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Secp256k1AddProjectiveErrors>> {
        Self::Secp256k1AddProjective::execute(input, output, resources, allocator)
    }

    fn secp256k1_mul_projective<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Secp256k1MulProjectiveErrors>> {
        Self::Secp256k1MulProjective::execute(input, output, resources, allocator)
    }

    fn secp256r1_add_projective<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Secp256r1AddProjectiveErrors>> {
        Self::Secp256r1AddProjective::execute(input, output, resources, allocator)
    }

    fn secp256r1_mul_projective<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Secp256r1MulProjectiveErrors>> {
        Self::Secp256r1MulProjective::execute(input, output, resources, allocator)
    }

    fn p256_verify<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<P256VerifyErrors>> {
        Self::P256Verify::execute(input, output, resources, allocator)
    }

    fn bn254_add<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Bn254AddErrors>> {
        Self::Bn254Add::execute(input, output, resources, allocator)
    }

    fn bn254_mul<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Bn254MulErrors>> {
        Self::Bn254Mul::execute(input, output, resources, allocator)
    }

    fn bn254_pairing_check<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Bn254PairingCheckErrors>> {
        Self::Bn254PairingCheck::execute(input, output, resources, allocator)
    }

    fn ripemd160<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<RipeMd160Errors>> {
        Self::RipeMd160::execute(input, output, resources, allocator)
    }

    fn point_evaluation<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<PointEvaluationErrors>> {
        Self::PointEvaluation::execute(input, output, resources, allocator)
    }

    fn bls12_g1_add<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Bls12PrecompileErrors>> {
        Self::Bls12G1Add::execute(input, output, resources, allocator)
    }

    fn bls12_g2_add<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Bls12PrecompileErrors>> {
        Self::Bls12G2Add::execute(input, output, resources, allocator)
    }

    fn bls12_g1_msm<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Bls12PrecompileErrors>> {
        Self::Bls12G1Msm::execute(input, output, resources, allocator)
    }

    fn bls12_g2_msm<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Bls12PrecompileErrors>> {
        Self::Bls12G2Msm::execute(input, output, resources, allocator)
    }

    fn bls12_pairing_check<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Bls12PrecompileErrors>> {
        Self::Bls12PairingCheck::execute(input, output, resources, allocator)
    }

    fn bls12_map_fp_to_g1<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Bls12PrecompileErrors>> {
        Self::Bls12MapFpToG1::execute(input, output, resources, allocator)
    }

    fn bls12_map_fp2_to_g2<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Bls12PrecompileErrors>> {
        Self::Bls12MapFp2ToG2::execute(input, output, resources, allocator)
    }

    fn blake2f<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Blake2FPrecompileErrors>> {
        Self::Blake2F::execute(input, output, resources, allocator)
    }
}

pub trait SystemFunctionsExt<R: Resources> {
    type Secp256k1ECRecover: SystemFunctionExt<R, Secp256k1ECRecoverErrors>;
    type ModExp: SystemFunctionExt<R, ModExpErrors>;
    type DivRem: DivRemExt;
    type WideDivRem: WideDivRemExt;

    fn secp256k1_ec_recover<
        O: IOOracle,
        L: Logger,
        D: TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        oracle: &mut O,
        logger: &mut L,
        allocator: A,
    ) -> Result<(), SubsystemError<Secp256k1ECRecoverErrors>> {
        Self::Secp256k1ECRecover::execute(input, output, resources, oracle, logger, allocator)
    }

    fn mod_exp<
        O: IOOracle,
        L: Logger,
        D: TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        oracle: &mut O,
        logger: &mut L,
        allocator: A,
    ) -> Result<(), SubsystemError<ModExpErrors>> {
        Self::ModExp::execute(input, output, resources, oracle, logger, allocator)
    }

    fn u256_div_rem<O: IOOracle>(
        dividend_or_quotient: &mut u256::U256,
        divisor_or_remainder: &mut u256::U256,
        oracle: &mut O,
    ) {
        Self::DivRem::execute(dividend_or_quotient, divisor_or_remainder, oracle)
    }

    fn u256_wide_div_rem<O: IOOracle>(
        dividend_lo: &mut u256::U256,
        dividend_hi: &mut u256::U256,
        divisor: &mut u256::U256,
        oracle: &mut O,
    ) {
        Self::WideDivRem::execute(dividend_lo, dividend_hi, divisor, oracle)
    }
}

pub trait DivRemExt {
    fn execute<O: IOOracle>(
        dividend_or_quotient: &mut u256::U256,
        divisor_or_remainder: &mut u256::U256,
        oracle: &mut O,
    );
}

pub trait WideDivRemExt {
    fn execute<O: IOOracle>(
        dividend_lo: &mut u256::U256,
        dividend_hi: &mut u256::U256,
        divisor: &mut u256::U256,
        oracle: &mut O,
    );
}
