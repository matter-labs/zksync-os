use crate::bootloader::runner::{run_till_completion, RunnerMemoryBuffers};
use system_hooks::HooksStorage;
use zk_ee::system::errors::{FatalError, InternalError, SystemError, UpdateQueryError};
use zk_ee::system::CallModifier;
use zk_ee::system::{EthereumLikeTypes, System};

use super::*;

impl<S: EthereumLikeTypes> BasicBootloader<S> {
    ///
    /// Mints [value] to address [to].
    ///
    pub fn mint_token(
        system: &mut System<S>,
        nominal_token_value: &U256,
        to: &B160,
        resources: &mut S::Resources,
    ) -> Result<(), SystemError>
    where
        S::IO: IOSubsystemExt,
    {
        // TODO: debug implementation for ruint types uses global alloc, which panics in ZKsync OS
        #[cfg(not(target_arch = "riscv32"))]
        let _ = system.get_logger().write_fmt(format_args!(
            "Minting {:?} tokens to {:?}\n",
            nominal_token_value, to
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
            .map_err(|e| match e {
                UpdateQueryError::NumericBoundsError => {
                    InternalError("Insufficient balance while minting").into()
                }
                UpdateQueryError::System(e) => e,
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
    ) -> Result<CompletedExecution<'a, S>, FatalError>
    where
        S::IO: IOSubsystemExt,
    {
        if DEBUG_OUTPUT {
            let _ = system
                .get_logger()
                .write_fmt(format_args!("`caller` = {:?}\n", caller));
            let _ = system
                .get_logger()
                .write_fmt(format_args!("`callee` = {:?}\n", callee));
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
                .map_err(|e| match e {
                    SystemError::OutOfErgs => unreachable!("OOG on infinite resources"),
                    SystemError::OutOfNativeResources => FatalError::OutOfNativeResources,
                    SystemError::Internal(e) => FatalError::Internal(e),
                })?
                .ee_version
                .0
        };

        // start execution
        let rollback_handle = should_make_frame
            .then(|| {
                system
                    .start_global_frame()
                    .map_err(|_| InternalError("must start a frame before execution"))
            })
            .transpose()?;

        let ee_type = ExecutionEnvironmentType::parse_ee_version_byte(ee_version)?;

        let initial_request =
            ExecutionEnvironmentSpawnRequest::RequestedExternalCall(ExternalCallRequest {
                available_resources: resources.clone(),
                ergs_to_pass: Ergs(0),      // Doesn't matter in this case
                callers_caller: B160::ZERO, // Fine to use placeholder
                caller: *caller,
                callee: *callee,
                modifier: CallModifier::NoModifier,
                calldata,
                call_scratch_space: None,
                nominal_token_value: *nominal_token_value,
            });

        let final_state =
            run_till_completion(memories, system, system_functions, ee_type, initial_request)?;

        let TransactionEndPoint::CompletedExecution(CompletedExecution {
            return_values,
            resources_returned,
            reverted,
        }) = final_state
        else {
            return Err(InternalError("attempt to run ended up in invalid state").into());
        };

        if let Some(ref rollback_handle) = rollback_handle {
            system
                .finish_global_frame(reverted.then_some(rollback_handle))
                .map_err(|_| InternalError("must finish execution frame"))?;
        }
        Ok(CompletedExecution {
            return_values,
            resources_returned,
            reverted,
        })
    }
}
