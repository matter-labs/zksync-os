use basic_system::system_functions::bn254_ecadd::Bn254AddImpl;
use basic_system::system_functions::bn254_ecmul::Bn254MulImpl;
use basic_system::system_functions::bn254_pairing_check::Bn254PairingCheckImpl;
use basic_system::system_functions::ecrecover::EcRecoverImpl;
use basic_system::system_functions::keccak256::Keccak256Impl;
use basic_system::system_functions::p256_verify::P256VerifyImpl;
use basic_system::system_functions::point_evaluation::PointEvaluationImpl;
use basic_system::system_functions::ripemd160::RipeMd160Impl;
use basic_system::system_functions::sha256::Sha256Impl;
use zk_ee::reference_implementations::BaseResources;
use zk_ee::reference_implementations::DecreasingNative;
use zk_ee::system::base_system_functions::{
    Bn254AddErrors, Bn254MulErrors, Bn254PairingCheckErrors, Keccak256Errors, P256VerifyErrors,
    PointEvaluationErrors, RipeMd160Errors, Secp256k1ECRecoverErrors, Sha256Errors,
};
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::Resource;
use zk_ee::system::logger::NullLogger;
use zk_ee::system::{SystemFunction, SystemFunctionExt};

/// The bn254 and point evaluation functions run without advice here, so the oracle is unused
struct DummyOracle;

impl zk_ee::oracle::IOOracle for DummyOracle {
    type RawIterator<'a> = Box<dyn ExactSizeIterator<Item = usize> + 'static>;

    fn raw_query<
        'a,
        I: zk_ee::oracle::usize_serialization::UsizeSerializable
            + zk_ee::oracle::usize_serialization::UsizeDeserializable,
    >(
        &'a mut self,
        _query_type: u32,
        _input: &I,
    ) -> Result<Self::RawIterator<'a>, zk_ee::system::errors::internal::InternalError> {
        unreachable!("the oracle is not consulted without advice");
    }
}

pub fn ecadd(src: &[u8], dst: &mut Vec<u8>) -> Result<(), SubsystemError<Bn254AddErrors>> {
    let allocator = std::alloc::Global;
    let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;
    Bn254AddImpl::<false>::execute(
        &src,
        dst,
        &mut resource,
        &mut DummyOracle,
        &mut NullLogger,
        allocator,
    )
}

pub fn sha256(src: &[u8], dst: &mut Vec<u8>) -> Result<(), SubsystemError<Sha256Errors>> {
    let allocator = std::alloc::Global;
    let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;
    Sha256Impl::execute(&src, dst, &mut resource, allocator)
}

pub fn keccak256(src: &[u8], dst: &mut Vec<u8>) -> Result<(), SubsystemError<Keccak256Errors>> {
    let allocator = std::alloc::Global;
    let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;
    Keccak256Impl::execute(&src, dst, &mut resource, allocator)
}

pub fn ripemd160(src: &[u8], dst: &mut Vec<u8>) -> Result<(), SubsystemError<RipeMd160Errors>> {
    let allocator = std::alloc::Global;
    let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;
    RipeMd160Impl::execute(&src, dst, &mut resource, allocator)
}

pub fn ecmul(src: &[u8], dst: &mut Vec<u8>) -> Result<(), SubsystemError<Bn254MulErrors>> {
    let allocator = std::alloc::Global;
    let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;
    Bn254MulImpl::<false>::execute(
        &src,
        dst,
        &mut resource,
        &mut DummyOracle,
        &mut NullLogger,
        allocator,
    )
}

pub fn p256_verify(src: &[u8], dst: &mut Vec<u8>) -> Result<(), SubsystemError<P256VerifyErrors>> {
    let allocator = std::alloc::Global;
    let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;
    P256VerifyImpl::execute(&src, dst, &mut resource, allocator)
}

pub fn pairing(
    src: &[u8],
    dst: &mut Vec<u8>,
) -> Result<(), SubsystemError<Bn254PairingCheckErrors>> {
    let allocator = std::alloc::Global;
    let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;
    Bn254PairingCheckImpl::<false>::execute(
        &src,
        dst,
        &mut resource,
        &mut DummyOracle,
        &mut NullLogger,
        allocator,
    )
}

pub fn kzg(src: &[u8], dst: &mut Vec<u8>) -> Result<(), SubsystemError<PointEvaluationErrors>> {
    let allocator = std::alloc::Global;
    let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;
    PointEvaluationImpl::<false>::execute(
        &src,
        dst,
        &mut resource,
        &mut DummyOracle,
        &mut NullLogger,
        allocator,
    )
}
