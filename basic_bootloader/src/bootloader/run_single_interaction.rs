use crate::bootloader::errors::BootloaderInterfaceError;
use crate::bootloader::runner::{run_till_completion, RunnerMemoryBuffers};
use errors::BootloaderSubsystemError;
use system_hooks::HooksStorage;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::errors::{runtime::RuntimeError, system::SystemError};
use zk_ee::system::CallModifier;
use zk_ee::system::{EthereumLikeTypes, System};
use zk_ee::{interface_error, internal_error, wrap_error};

use super::*;

impl<S: EthereumLikeTypes, F: BasicTransactionFlow<S>> BasicBootloader<S, F>
where
    S::IO: IOSubsystemExt,
{
    ///
    /// Mints [value] to address [to].
    ///
    pub fn mint_token(
        system: &mut System<S>,
        nominal_token_value: &U256,
        to: &B160,
        resources: &mut S::Resources,
    ) -> Result<(), BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        // TODO: debug implementation for ruint types uses global alloc, which panics in ZKsync OS
        #[cfg(not(target_arch = "riscv32"))]
        let _ = system.get_logger().write_fmt(format_args!(
            "Minting {nominal_token_value:?} tokens to {to:?}\n"
        ));

        let _old_balance = system
            .io
            .update_account_nominal_token_balance(
                ExecutionEnvironmentType::EVM,
                resources,
                to,
                nominal_token_value,
                false,
            )
            .map_err(|e| -> BootloaderSubsystemError {
                match e {
                    SubsystemError::LeafUsage(balance_error) => {
                        let _ = system
                            .get_logger()
                            .write_fmt(format_args!("Error while minting: {balance_error:?}"));
                        interface_error!(BootloaderInterfaceError::MintingBalanceOverflow)
                    }
                    _ => wrap_error!(e),
                }
            })?;

        Ok(())
    }

    ///
    /// Pre-condition: if [nominal_token_value] is not 0, this function
    /// assumes the caller's balance has been validated. It returns an
    /// internal error in case of balance underflow.
    ///
    pub fn run_single_interaction<'a>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        calldata: &[u8],
        caller: &B160,
        callee: &B160,
        mut resources: S::Resources,
        nominal_token_value: &U256,
        should_make_frame: bool,
        tracer: &mut impl Tracer<S>,
    ) -> Result<CompletedExecution<'a, S>, BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        if DEBUG_OUTPUT {
            let _ = system
                .get_logger()
                .write_fmt(format_args!("`caller` = {caller:?}\n"));
            let _ = system
                .get_logger()
                .write_fmt(format_args!("`callee` = {callee:?}\n"));
        }

        let ee_version = {
            resources
                .with_infinite_ergs(|inf_resources| {
                    system.io.read_account_properties(
                        ExecutionEnvironmentType::NoEE,
                        inf_resources,
                        caller,
                        AccountDataRequest::empty().with_ee_version(),
                    )
                })
                .map_err(|e| -> BootloaderSubsystemError {
                    match e {
                        SystemError::LeafRuntime(RuntimeError::OutOfErgs(_)) => {
                            unreachable!("OOG on infinite resources")
                        }
                        e @ SystemError::LeafRuntime(RuntimeError::FatalRuntimeError(_)) => {
                            e.into()
                        }
                        SystemError::LeafDefect(e) => e.into(),
                    }
                })?
                .ee_version
                .0
        };

        // start execution
        let rollback_handle = should_make_frame
            .then(|| {
                system
                    .start_global_frame()
                    .map_err(|_| internal_error!("must start a frame before execution"))
            })
            .transpose()?;

        let ee_type = ExecutionEnvironmentType::parse_ee_version_byte(ee_version)?;

        let initial_request = ExternalCallRequest {
            available_resources: resources.clone(),
            ergs_to_pass: resources.ergs(),
            callers_caller: B160::ZERO, // Fine to use placeholder
            caller: *caller,
            callee: *callee,
            modifier: CallModifier::NoModifier,
            input: calldata,
            call_scratch_space: None,
            nominal_token_value: *nominal_token_value,
        };

        let final_state = run_till_completion(
            memories,
            system,
            system_functions,
            ee_type,
            initial_request,
            tracer,
        )?;

        let CompletedExecution {
            resources_returned,
            result,
        } = final_state;

        if let Some(ref rollback_handle) = rollback_handle {
            system
                .finish_global_frame(result.failed().then_some(rollback_handle))
                .map_err(|_| internal_error!("must finish execution frame"))?;
        }
        Ok(CompletedExecution {
            resources_returned,
            result,
        })
    }
}
