use super::{errors::{InternalError, NoErrors, SubsystemError, SubsystemErrorTypes}, Resources};


macro_rules! declare_system_function_no_errors {
    ($name:ident) => {

        paste::paste! {
            #[derive(Debug, PartialEq, Eq, Clone, Copy)]
            pub struct [<$name Errors>];

            impl SubsystemErrorTypes for [<$name Errors>] {
                type Interface = NoErrors;
                type Wrapped = NoErrors;
            }
        }
    };
}

macro_rules! declare_system_function_errors {
    ($name:ident, { $($variant:ident),* }) => {
        paste::paste! {
            #[derive(Debug, PartialEq, Eq, Clone, Copy)]
            pub enum [<$name InterfaceError>] {
                $($variant,)*
            }

            #[derive(Clone, Debug, Eq, PartialEq)]
            pub struct [<$name Errors>];

            impl SubsystemErrorTypes for [<$name Errors>] {
                type Interface = [<$name InterfaceError>];
                type Wrapped = NoErrors;
            }
        }
    };
}


// Definitions of errors for all system functions
declare_system_function_no_errors!(Keccak256);
declare_system_function_no_errors!(Sha256);
declare_system_function_no_errors!(Secp256k1ECRecover);
declare_system_function_no_errors!(Secp256k1AddProjective);
declare_system_function_no_errors!(Secp256k1MulProjective);
declare_system_function_no_errors!(Secp256r1AddProjective);
declare_system_function_no_errors!(Secp256r1MulProjective);
declare_system_function_errors!(P256Verify,
                      {
                          InvalidInputLength
                      }
);

declare_system_function_errors!(Bn254Add,
                      {
                          InvalidPoint
                      }
);

declare_system_function_errors!(Bn254Mul,
                      {
                          InvalidPoint
                      }
);
declare_system_function_errors!(Bn254PairingCheck,
                                {
                                    InvalidPoint,
                                    InvalidPairingSize
                                }
);

declare_system_function_no_errors!(RipeMd160);

declare_system_function_errors!(ModExp,
                                {
                                    InvalidInputLength,
                                    InvalidModulus,
                                    DivisionByZero
                                }
);

declare_system_function_no_errors!(MissingSystemFunction);


///
/// System function implementation.
///
pub trait SystemFunction<R: Resources, E: SubsystemErrorTypes> {
    /// Writes result to the `output` and returns actual output slice length that was used.
    /// Should return error on invalid inputs and if resources do not even cover basic parsing cost.
    /// in practice only pairing can have invalid input(size) on charging stage.
    fn execute<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<E>>;
}

pub struct MissingSystemFunction;

impl<R: Resources> SystemFunction<R, MissingSystemFunctionErrors> for MissingSystemFunction {
    fn execute<D: ?Sized + Extend<u8>, A: core::alloc::Allocator + Clone>(
        _: &[u8],
        _: &mut D,
        _: &mut R,
        _: A,
    ) -> Result<(), SubsystemError<MissingSystemFunctionErrors>> {
        Err(InternalError("This system function is not defined for this system").into())
    }
}

// Additional implementations for missing projective curve operations
impl<R: Resources> SystemFunction<R, Secp256k1AddProjectiveErrors> for MissingSystemFunction {
    fn execute<D: ?Sized + Extend<u8>, A: core::alloc::Allocator + Clone>(
        _: &[u8],
        _: &mut D,
        _: &mut R,
        _: A,
    ) -> Result<(), SubsystemError<Secp256k1AddProjectiveErrors>> {
        Err(InternalError("Secp256k1 add projective not implemented").into())
    }
}

impl<R: Resources> SystemFunction<R, Secp256k1MulProjectiveErrors> for MissingSystemFunction {
    fn execute<D: ?Sized + Extend<u8>, A: core::alloc::Allocator + Clone>(
        _: &[u8],
        _: &mut D,
        _: &mut R,
        _: A,
    ) -> Result<(), SubsystemError<Secp256k1MulProjectiveErrors>> {
        Err(InternalError("Secp256k1 mul projective not implemented").into())
    }
}

impl<R: Resources> SystemFunction<R, Secp256r1AddProjectiveErrors> for MissingSystemFunction {
    fn execute<D: ?Sized + Extend<u8>, A: core::alloc::Allocator + Clone>(
        _: &[u8],
        _: &mut D,
        _: &mut R,
        _: A,
    ) -> Result<(), SubsystemError<Secp256r1AddProjectiveErrors>> {
        Err(InternalError("Secp256r1 add projective not implemented").into())
    }
}

impl<R: Resources> SystemFunction<R, Secp256r1MulProjectiveErrors> for MissingSystemFunction {
    fn execute<D: ?Sized + Extend<u8>, A: core::alloc::Allocator + Clone>(
        _: &[u8],
        _: &mut D,
        _: &mut R,
        _: A,
    ) -> Result<(), SubsystemError<Secp256r1MulProjectiveErrors>> {
        Err(InternalError("Secp256r1 mul projective not implemented").into())
    }
}

pub trait SystemFunctions<R: Resources> {
    type Keccak256: SystemFunction<R, Keccak256Errors>;
    type Sha256: SystemFunction<R, Sha256Errors>;
    type Secp256k1ECRecover: SystemFunction<R, Secp256k1ECRecoverErrors>;
    type Secp256k1AddProjective: SystemFunction<R, Secp256k1AddProjectiveErrors>;
    type Secp256k1MulProjective: SystemFunction<R, Secp256k1MulProjectiveErrors>;
    type Secp256r1AddProjective: SystemFunction<R, Secp256r1AddProjectiveErrors>;
    type Secp256r1MulProjective: SystemFunction<R, Secp256r1MulProjectiveErrors>;
    type P256Verify: SystemFunction<R, P256VerifyErrors>;
    type Bn254Add: SystemFunction<R, Bn254AddErrors>;
    type Bn254Mul: SystemFunction<R, Bn254MulErrors>;
    type Bn254PairingCheck: SystemFunction<R, Bn254PairingCheckErrors>;
    type RipeMd160: SystemFunction<R, RipeMd160Errors>;
    type ModExp: SystemFunction<R, ModExpErrors>;

    fn keccak256<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Keccak256Errors>> {
        Self::Keccak256::execute(input, output, resources, allocator)
    }

    fn sha256<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Sha256Errors>> {
        Self::Sha256::execute(input, output, resources, allocator)
    }

    fn secp256k1_ec_recover<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Secp256k1ECRecoverErrors>> {
        Self::Secp256k1ECRecover::execute(input, output, resources, allocator)
    }

    fn secp256k1_add_projective<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Secp256k1AddProjectiveErrors>> {
        Self::Secp256k1AddProjective::execute(input, output, resources, allocator)
    }

    fn secp256k1_mul_projective<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Secp256k1MulProjectiveErrors>> {
        Self::Secp256k1MulProjective::execute(input, output, resources, allocator)
    }

    fn secp256r1_add_projective<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Secp256r1AddProjectiveErrors>> {
        Self::Secp256r1AddProjective::execute(input, output, resources, allocator)
    }

    fn secp256r1_mul_projective<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Secp256r1MulProjectiveErrors>> {
        Self::Secp256r1MulProjective::execute(input, output, resources, allocator)
    }

    fn p256_verify<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<P256VerifyErrors>> {
        Self::P256Verify::execute(input, output, resources, allocator)
    }

    fn bn254_add<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Bn254AddErrors>> {
        Self::Bn254Add::execute(input, output, resources, allocator)
    }

    fn bn254_mul<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Bn254MulErrors>> {
        Self::Bn254Mul::execute(input, output, resources, allocator)
    }

    fn bn254_pairing_check<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Bn254PairingCheckErrors>> {
        Self::Bn254PairingCheck::execute(input, output, resources, allocator)
    }

    fn ripemd160<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<RipeMd160Errors>> {
        Self::RipeMd160::execute(input, output, resources, allocator)
    }

    fn mod_exp<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<ModExpErrors>> {
        Self::ModExp::execute(input, output, resources, allocator)
    }
}
