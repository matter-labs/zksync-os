use zk_ee::{define_subsystem, system::CallModifier};

define_subsystem!(
    Evm,
    interface EvmInterfaceError {
        NoDeploymentScheme,
        UnknownDeploymentData,
        BytecodeNoPadding,
        UnexpectedModifier{ modifier: CallModifier },
    }
);
