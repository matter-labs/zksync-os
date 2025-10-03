use crate::io_oracle::CsrBasedIOOracle;
use crate::skip_list_quasi_vec::num_elements_in_backing_node;
use crate::skip_list_quasi_vec::ListVec;
use crate::system::bootloader::BootloaderAllocator;
use alloc::alloc::Allocator;
use basic_bootloader::bootloader::transaction_flow::zk::ZkTransactionFlowOnlyEOA;
use basic_bootloader::bootloader::BasicBootloader;
use basic_system::system_functions::NoStdSystemFunctions;
use basic_system::system_implementation::system::EthereumLikeStorageAccessCostModel;
use basic_system::system_implementation::system::FullIO;
use stack_trait::StackCtor;
use stack_trait::StackCtorConst;
use zk_ee::memory::*;
use zk_ee::oracle::IOOracle;
use zk_ee::reference_implementations::BaseResources;
use zk_ee::system::{logger::Logger, EthereumLikeTypes, SystemTypes};
use zk_ee::types_config::EthereumIOTypesConfig;

pub mod bootloader;

pub struct LVStackCtor {}

impl StackCtor<LVStackCtor> for LVStackCtor {
    type Stack<T: Sized, const N: usize, A: Allocator + Clone> = ListVec<T, N, A>;

    fn new_in<T, A: Allocator + Clone>(
        alloc: A,
    ) -> Self::Stack<T, { <LVStackCtor>::extra_const_param::<T, A>() }, A>
    where
        [(); <LVStackCtor>::extra_const_param::<T, A>()]:,
    {
        Self::Stack::<T, { <LVStackCtor>::extra_const_param::<T, A>() }, A>::new_in(alloc)
    }
}

impl const StackCtorConst for LVStackCtor {
    fn extra_const_param<T, A: Allocator>() -> usize {
        num_elements_in_backing_node::<T, A>()
    }
}

pub struct ProofRunningSystemTypes<O, L>(O, L);

type Native = zk_ee::reference_implementations::DecreasingNative;

impl<O: IOOracle, L: Logger + Default> SystemTypes for ProofRunningSystemTypes<O, L> {
    type IOTypes = EthereumIOTypesConfig;
    type Resources = BaseResources<Native>;
    type IO = FullIO<
        Self::Allocator,
        Self::Resources,
        EthereumLikeStorageAccessCostModel,
        LVStackCtor,
        LVStackCtor,
        O,
        true,
    >;
    type SystemFunctions = NoStdSystemFunctions;
    type SystemFunctionsExt = NoStdSystemFunctions;
    type Allocator = BootloaderAllocator;
    type Logger = L;
    type Metadata = zk_ee::system::metadata::Metadata<Self::IOTypes>;
}

impl<O: IOOracle, L: Logger + Default> EthereumLikeTypes for ProofRunningSystemTypes<O, L> {}

pub type ProvingBootloader<O, L> =
    BasicBootloader<ProofRunningSystemTypes<O, L>, ZkTransactionFlowOnlyEOA>;
