use basic_system::system_functions::bn254_ecadd::Bn254AddImpl;
use basic_system::system_functions::sha256::Sha256Impl;
use basic_system::system_functions::keccak256::Keccak256Impl;
use basic_system::system_functions::ripemd160::RipeMd160Impl;
use basic_system::system_functions::bn254_ecmul::Bn254MulImpl;
use basic_system::system_functions::modexp::ModExpImpl;
use zk_ee::reference_implementations::BaseResources;
use zk_ee::system::{SystemFunction,SystemFunctionExt};
use zk_ee::system::Resource;
use zk_ee::reference_implementations::DecreasingNative;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::base_system_functions::{Bn254AddErrors,Sha256Errors,RipeMd160Errors,Keccak256Errors,
Bn254MulErrors,ModExpErrors};
use core::slice::SlicePattern;

pub fn ecadd(src: &[u8], dst: &mut Vec<u8>) -> Result<(), SubsystemError<Bn254AddErrors>> {
    let allocator = std::alloc::Global;
    let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;
    Bn254AddImpl::execute(&src.as_slice(), dst, &mut resource, allocator)
}

pub fn sha256(src: &[u8], dst: &mut Vec<u8>) -> Result<(), SubsystemError<Sha256Errors>> {
    let allocator = std::alloc::Global;
    let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;
    Sha256Impl::execute(&src.as_slice(), dst, &mut resource, allocator)
}

pub fn keccak256(src: &[u8], dst: &mut Vec<u8>) -> Result<(), SubsystemError<Keccak256Errors>> {
    let allocator = std::alloc::Global;
    let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;
    Keccak256Impl::execute(&src.as_slice(), dst, &mut resource, allocator)
}

pub fn ripemd160(src: &[u8], dst: &mut Vec<u8>) -> Result<(), SubsystemError<RipeMd160Errors>> {
    let allocator = std::alloc::Global;
    let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;
    RipeMd160Impl::execute(&src.as_slice(), dst, &mut resource, allocator)
}

pub fn ecmul(src: &[u8], dst: &mut Vec<u8>) -> Result<(), SubsystemError<Bn254MulErrors>> {
    let allocator = std::alloc::Global;
    let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;
    Bn254MulImpl::execute(&src.as_slice(), dst, &mut resource, allocator)
}

pub fn modexp(src: &[u8], dst: &mut Vec<u8>) -> Result<(), SubsystemError<ModExpErrors>> {
    let allocator = std::alloc::Global;
    let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;
    ModExpImpl::execute(
        &src.as_slice(),
        dst,
        &mut resource,
        // We're in x86 target, so oracle and logger aren't going to be used.
        &mut DummyOracle {},
        &mut zk_ee::system::NullLogger,
        allocator,
    )
}

struct DummyOracle {}

impl zk_ee::oracle::IOOracle for DummyOracle {
    type RawIterator<'a> = Box<dyn ExactSizeIterator<Item = usize> + 'static>;

    fn raw_query<'a, I: zk_ee::oracle::usize_serialization::UsizeSerializable + zk_ee::oracle::usize_serialization::UsizeDeserializable>(
        &'a mut self,
        _query_type: u32,
        _input: &I,
    ) -> Result<Self::RawIterator<'a>, zk_ee::system::errors::internal::InternalError> {
        unreachable!("oracle should not be consulted on native targets");
    }
}