use super::Transaction;
use crate::bootloader::errors::TxError;
use evm_interpreter::ERGS_PER_GAS;
use zk_ee::system::{Ergs, Resource, Resources};
use zk_ee::{
    execution_environment_type::ExecutionEnvironmentType,
    system::{EthereumLikeTypes, IOSubsystemExt, System},
    utils::Bytes32,
};

/// Parse and warm up accounts and storage slots from the access list.
///
/// Touches all accounts and storage keys in the access list so they are hot
/// before execution.
///
/// Returns Ok on success, or `TxError` if an IO operation fails.
pub fn parse_and_warm_up_access_list<S: EthereumLikeTypes>(
    system: &mut System<S>,
    resources: &mut S::Resources,
    transaction: &Transaction<S::Allocator>,
) -> Result<(), TxError>
where
    S::IO: IOSubsystemExt,
{
    use crate::bootloader::transaction::rlp_encoded::AccessListForAddress;
    if let Some(iter) = transaction.access_list_iter() {
        for AccessListForAddress {
            address,
            slots_list,
        } in iter
        {
            // per-address charge
            resources.charge(&S::Resources::from_ergs_and_native(
                Ergs(evm_interpreter::gas_constants::ACCESS_LIST_ADDRESS * ERGS_PER_GAS),
                    <<S::Resources as Resources>::Native as zk_ee::system::Computational>::from_computational(crate::bootloader::constants::PER_ADDRESS_ACCESS_LIST_NATIVE_COST)
                )
            )?;
            resources.with_infinite_ergs(|resources| {
                system
                    .io
                    .touch_account(ExecutionEnvironmentType::NoEE, resources, &address)
            })?;
            for key in slots_list.iter() {
                // per-slot charge
                resources.charge(&S::Resources::from_ergs_and_native(
                    Ergs(evm_interpreter::gas_constants::ACCESS_LIST_STORAGE_KEY * ERGS_PER_GAS),
                        <<S::Resources as Resources>::Native as zk_ee::system::Computational>::from_computational(crate::bootloader::constants::PER_SLOT_ACCESS_LIST_NATIVE_COST)
                    )
                )?;
                let key = key?;
                resources.with_infinite_ergs(|resources| {
                    system.io.storage_touch(
                        ExecutionEnvironmentType::NoEE,
                        resources,
                        &address,
                        &Bytes32::from_array(*key),
                    )
                })?;
            }
        }
    }

    Ok(())
}
