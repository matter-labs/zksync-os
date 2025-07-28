use zk_ee::{
    define_subsystem,
    system::{
        BalanceSubsystemError, CallModifier, DeconstructionSubsystemError, NonceSubsystemError,
    },
};

define_subsystem!(
    Evm,
    interface EvmInterfaceError {
        NoDeploymentScheme,
        UnknownDeploymentData,
        BytecodeNoPadding,
        UnexpectedModifier{ modifier: CallModifier },
    },
    cascade EvmCascadedError {
        Nonce(NonceSubsystemError),
        Balance(BalanceSubsystemError),
        Deconstruction(DeconstructionSubsystemError),
    }
);
