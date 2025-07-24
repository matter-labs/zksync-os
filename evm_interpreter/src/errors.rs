use zk_ee::{
    define_subsystem,
    system::{CallModifier, NonceSubsystemError},
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
    }
);
