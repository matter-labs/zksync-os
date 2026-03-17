use core::alloc::Allocator;
use zk_ee::memory::MinimalByteAddressableSlice;
use zk_ee::system::{MissingSystemFunction, Resources, SystemFunctions, SystemFunctionsExt};

pub mod bn254_ecadd;
pub mod bn254_ecmul;
pub mod bn254_pairing_check;
pub mod ecrecover;
pub mod field_ops;
pub mod keccak256;
pub mod modexp;
pub mod p256_verify;
pub mod point_evaluation;
pub mod ripemd160;
pub mod sha256;

///
/// Internal utility function to reverse byte array
///
#[inline(always)]
fn bytereverse(input: &mut [u8]) {
    assert!(input.len().is_multiple_of(2));
    let len = input.len();
    for i in 0..len / 2 {
        input.swap(i, len - 1 - i);
    }
}

///
/// No std system functions implementations.
/// All of them are following EVM specs(for precompiles and keccak opcode).
/// USE_ADVICE const parameter affects only the forward run, as advice
/// is always used for proving one.
///
pub struct NoStdSystemFunctions<const USE_ADVICE: bool>;

impl<R: Resources, const USE_ADVICE: bool> SystemFunctions<R> for NoStdSystemFunctions<USE_ADVICE> {
    type Keccak256 = keccak256::Keccak256Impl;
    type Sha256 = sha256::Sha256Impl;
    type Secp256k1AddProjective = MissingSystemFunction;
    type Secp256k1MulProjective = MissingSystemFunction;
    type Secp256r1AddProjective = MissingSystemFunction;
    type Secp256r1MulProjective = MissingSystemFunction;
    type P256Verify = p256_verify::P256VerifyImpl;
    type Bn254Add = bn254_ecadd::Bn254AddImpl;
    type Bn254Mul = bn254_ecmul::Bn254MulImpl;
    type Bn254PairingCheck = bn254_pairing_check::Bn254PairingCheckImpl;
    type RipeMd160 = ripemd160::RipeMd160Impl;
    type PointEvaluation = point_evaluation::PointEvaluationImpl;
}

impl<R: Resources, const USE_ADVICE: bool> SystemFunctionsExt<R>
    for NoStdSystemFunctions<USE_ADVICE>
{
    type Secp256k1ECRecover = ecrecover::EcRecoverImpl<USE_ADVICE>;
    type ModExp = modexp::ModExpImpl<USE_ADVICE>;
}
