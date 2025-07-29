use super::transaction::ZkSyncTransaction;
use super::*;
use crate::bootloader::errors::TxError;
use crate::bootloader::supported_ees::SupportedEEVMState;
use constants::{PAYMASTER_VALIDATE_AND_PAY_SELECTOR, TX_CALLDATA_OFFSET};
use errors::{BootloaderSubsystemError, InvalidTransaction};
use system_hooks::addresses_constants::BOOTLOADER_FORMAL_ADDRESS;
use system_hooks::HooksStorage;
use zk_ee::internal_error;
use zk_ee::system::{EthereumLikeTypes, System};

// Helpers for paymaster flow.

impl<S: EthereumLikeTypes> BasicBootloader<S> {
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::type_complexity)]
    pub(crate) fn validate_and_pay_for_paymaster_transaction<'a>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        transaction: &mut ZkSyncTransaction,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        paymaster: B160,
        _caller_ee_type: ExecutionEnvironmentType,
        resources: &mut S::Resources,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ReturnValues<'a, S>, TxError>
    where
        S::IO: IOSubsystemExt,
    {
        let _ = system.get_logger().write_fmt(format_args!(
            "About to start call to validateAndPayForPaymasterTransaction\n"
        ));

        let CompletedExecution {
            resources_returned,
            reverted,
            return_values,
            ..
        } = BasicBootloader::call_account_method(
            system,
            system_functions,
            memories,
            transaction,
            tx_hash,
            suggested_signed_hash,
            paymaster,
            PAYMASTER_VALIDATE_AND_PAY_SELECTOR,
            resources,
            tracer,
        )
        .map_err(TxError::oon_as_validation)?;

        *resources = resources_returned;
        // Return memory isn't flushed, as it's read by
        // store_paymaster_context_and_check_magic
        if reverted {
            Err(TxError::Validation(InvalidTransaction::Revert {
                method: errors::AAMethod::PaymasterValidateAndPay,
                output: None, // TODO
            }))
        } else {
            Ok(return_values)
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::type_complexity)]
    pub(crate) fn store_paymaster_context_and_check_magic(
        _system: &mut System<S>,
        _pre_tx_buffer: &mut [u8],
        _return_values: &ReturnValues<S>,
    ) -> Result<(), TxError> {
        todo!();

        // // The paymaster validation step should return context of type "bytes context"
        // // This means that the returndata is encoded the following way:
        // // 0x20 || context_len || context_bytes...
        // let returndata_region = return_values.returndata;
        // let returndata_slice = &*returndata_region;
        // let returndata_len = returndata_slice.len();
        // // The minimal allowed returndatasize is 64: magicValue || offset
        // require!(
        //     returndata_len >= 64,
        //     AAValidationError(InvalidAA::PaymasterReturnDataTooShort),
        //     system
        // )?;
        // // Check magic
        // require!(
        //     &returndata_slice[..4] == PAYMASTER_VALIDATE_AND_PAY_SELECTOR,
        //     AAValidationError(InvalidAA::PaymasterInvalidMagic),
        //     system
        // )?;
        // let offset = U256::from_be_slice(&returndata_slice[32..64]);
        // // Ensuring that the returned offset is not greater than the returndata length
        // // Note, that we cannot use addition here to prevent an overflow
        // require!(
        //     offset <= U256::from(returndata_len),
        //     AAValidationError(InvalidAA::PaymasterContextInvalid),
        //     system
        // )?;
        // // Can not read the returned length.
        // // It is safe to add here due to the previous check.
        // require!(
        //     offset.overflowing_add(U256::from(32)).0 <= U256::from(returndata_len),
        //     AAValidationError(InvalidAA::PaymasterContextInvalid),
        //     system
        // )?;
        // let offset_u = u256_to_u64_saturated(&offset) as usize;
        // // Reading the length of the context
        // let context_len = U256::from_be_slice(&returndata_slice[offset_u..(offset_u + 32)]);
        // // Ensuring that context_len is not greater than the length of the paymaster context
        // // Note, that this check at the same time prevents an overflow in the future operations with
        // // context_len
        // require!(
        //     context_len <= U256::from(MAX_PAYMASTER_CONTEXT_LEN_BYTES),
        //     AAValidationError(InvalidAA::PaymasterReturnDataTooShort),
        //     system
        // )?;
        // let rounded_context_len = Self::length_rounded_by_words(context_len)
        //     .ok_or(internal_error!("rounding context length"))?;
        // require!(
        //     rounded_context_len <= U256::from(MAX_PAYMASTER_CONTEXT_LEN_BYTES),
        //     AAValidationError(InvalidAA::PaymasterReturnDataTooShort),
        //     system
        // )?;
        // require!(
        //     offset
        //         .overflowing_add(context_len)
        //         .0
        //         .overflowing_add(U256::from(32))
        //         .0
        //         <= U256::from(returndata_len),
        //     AAValidationError(InvalidAA::PaymasterContextInvalid),
        //     system
        // )?;
        // let size = u256_to_u64_saturated(&context_len) as usize + 32;
        // // Copy context into buffer
        // // We store it in the beginning of the buffer, then move it to the
        // // right location.
        // pre_tx_buffer[..size].copy_from_slice(&returndata_slice[offset_u..(offset_u + size)]);
        // // Pad with zeroes the context
        // let context_len_u = u256_to_u64_saturated(&context_len) as usize;
        // let rounded_context_len_u = u256_to_u64_saturated(&rounded_context_len) as usize;
        // for byte in &mut pre_tx_buffer[(context_len_u + 32)..(rounded_context_len_u + 32)] {
        //     *byte = 0;
        // }
        // // system.purge_return_memory();
        // Ok(())
    }

    #[allow(dead_code)]
    fn length_rounded_by_words(len: U256) -> Option<U256> {
        let c32 = U256::from(32);
        let needed_words = len.overflowing_add(U256::from(31)).0.wrapping_div(c32);
        needed_words.checked_mul(c32)
    }

    /// Returns if the transaction succeeded.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::type_complexity)]
    #[allow(dead_code)]
    pub(crate) fn paymaster_post_op(
        _system: &mut System<S>,
        _system_functions: &mut HooksStorage<S, S::Allocator>,
        _callstack: &mut SliceVec<SupportedEEVMState<S>>,
        _transaction: &mut ZkSyncTransaction,
        _tx_hash: Bytes32,
        _suggested_signed_hash: Bytes32,
        _success: bool,
        _max_refunded_gas: u64,
        _paymaster: B160,
        _gas_per_pubdata: U256,
        _validation_pubdata: u64,
        _resources: &mut S::Resources,
    ) -> Result<bool, BootloaderSubsystemError>
where {
        todo!();

        // let _ = system
        //     .get_logger()
        //     .write_fmt(format_args!("About to start call to postTransaction\n"));

        // // The postOp method has the following signature:
        // // function postTransaction(
        // //     bytes calldata _context,
        // //     Transaction calldata _transaction,
        // //     bytes32 _txHash,
        // //     bytes32 _suggestedSignedHash,
        // //     ExecutionResult _txResult,
        // //     uint256 _maxRefundedGas
        // // ) external payable;
        // // The encoding is the following:
        // // 1. Offset to the _context's content. (32 bytes)
        // // 2. Offset to the _transaction's content. (32 bytes)
        // // 3. _txHash (32 bytes)
        // // 4. _suggestedSignedHash (32 bytes)
        // // 5. _txResult (32 bytes)
        // // 6. _maxRefundedGas (32 bytes)
        // // 7. _context (note, that the content must be padded to 32 bytes)
        // // 8. _transaction

        // let unpadded_context_length = U256::from_be_slice(&pre_tx_buffer[..32]);
        // let context_length = Self::length_rounded_by_words(unpadded_context_length)
        //     .ok_or(internal_error!("Rounding context length"))?;
        // let context_length_u = u256_to_u64_saturated(&context_length) as usize;
        // // Selector + Initial offsets + fixed sized fields
        // let header_length = 4 + U256::BYTES * 6;
        // let calldata_start = TX_OFFSET - context_length_u - header_length;
        // let calldata = MemoryRegion {
        //     region_type: MemoryRegionType::FirstFrameCalldata,
        //     description: MemoryRegionDescription {
        //         offset: calldata_start,
        //         len: header_length + context_length_u + tx_length,
        //     },
        // };

        // // First we copy the paymaster context to the right location.
        // let context_start = calldata_start + header_length;
        // pre_tx_buffer.copy_within(..context_length_u, context_start);

        // // Write selector
        // pre_tx_buffer[calldata_start..(calldata_start + 4)]
        //     .copy_from_slice(PAYMASTER_POST_TRANSACTION_SELECTOR);

        // // Write context offset
        // let context_offset_start = calldata_start + 4;
        // pre_tx_buffer[context_offset_start..(context_offset_start + U256::BYTES)].copy_from_slice(
        //     U256::to_be_bytes::<32>(&U256::from(TX_CALLDATA_OFFSET + tx_length)).as_ref(),
        // );

        // // Write tx offset
        // let tx_offset_start = context_offset_start + U256::BYTES;
        // pre_tx_buffer[tx_offset_start..(tx_offset_start + U256::BYTES)]
        //     .copy_from_slice(U256::to_be_bytes::<32>(&U256::from(TX_CALLDATA_OFFSET)).as_ref());

        // // Write tx_hash
        // let tx_hash_start = tx_offset_start + U256::BYTES;
        // pre_tx_buffer[tx_hash_start..(tx_hash_start + U256::BYTES)]
        //     .copy_from_slice(tx_hash.as_u8_ref());

        // // Write suggested_signed_hash
        // let signed_start = tx_hash_start + U256::BYTES;
        // pre_tx_buffer[signed_start..(signed_start + U256::BYTES)]
        //     .copy_from_slice(suggested_signed_hash.as_u8_ref());

        // // Write execution result
        // let execution_result = if success { U256::from(1) } else { U256::ZERO };
        // let result_start = signed_start + U256::BYTES;
        // pre_tx_buffer[result_start..(result_start + U256::BYTES)]
        //     .copy_from_slice(U256::to_be_bytes::<32>(&execution_result).as_ref());

        // // Write max refunded gas
        // let refund_start = result_start + U256::BYTES;
        // pre_tx_buffer[refund_start..(refund_start + U256::BYTES)]
        //     .copy_from_slice(U256::to_be_bytes::<32>(&U256::from(max_refunded_gas)).as_ref());

        // let resources_for_tx = resources.clone();

        // let CompletedExecution {
        //     resources_returned,
        //     reverted,
        //     return_values: _,
        // } = BasicBootloader::run_single_interaction::<_>(
        //     system,
        //     system_functions,
        //     callstack,
        //     calldata,
        //     &BOOTLOADER_FORMAL_ADDRESS,
        //     &paymaster,
        //     resources_for_tx,
        //     &U256::ZERO,
        //     true,
        //     true,
        // )?;

        // resources.spendable.ergs = resources_returned.spendable.ergs;

        // // Revert if there's not enough gas for pubdata.
        // let success = !reverted
        //     && check_enough_gas_for_pubdata(
        //         system,
        //         gas_per_pubdata,
        //         resources,
        //         Some(validation_pubdata),
        //     )?;

        // // TODO: when to purge?
        // // system.purge_return_memory();

        // Ok(success)
    }

    fn write_calldata_prefix(
        pre_tx_buffer: &mut [u8],
        calldata_start: usize,
        selector: &[u8],
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
    ) {
        // Write selector
        pre_tx_buffer[calldata_start..(calldata_start + 4)].copy_from_slice(selector);
        // Write tx_hash
        let tx_hash_start = calldata_start + 4;
        pre_tx_buffer[tx_hash_start..(tx_hash_start + U256::BYTES)]
            .copy_from_slice(tx_hash.as_u8_ref());
        // Write suggested_signed_hash
        let signed_start = tx_hash_start + U256::BYTES;
        pre_tx_buffer[signed_start..(signed_start + U256::BYTES)]
            .copy_from_slice(suggested_signed_hash.as_u8_ref());
        // Write offset
        let offset_start = signed_start + U256::BYTES;
        pre_tx_buffer[offset_start..(offset_start + U256::BYTES)]
            .copy_from_slice(U256::to_be_bytes::<32>(&U256::from(TX_CALLDATA_OFFSET)).as_ref());
    }

    /// Used to call a method with the following signature;
    /// someName(
    ///     bytes32 _txHash,
    ///     bytes32 _suggestedSignedHash,
    ///     Transaction calldata _transaction
    /// )
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::type_complexity)]
    pub fn call_account_method<'a>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        transaction: &mut ZkSyncTransaction,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        from: B160,
        selector: &[u8],
        resources: &mut S::Resources,
        tracer: &mut impl Tracer<S>,
    ) -> Result<CompletedExecution<'a, S>, BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        let header_length = 4 + U256::BYTES * 3;
        let calldata_start = TX_OFFSET - header_length;
        let calldata_end = calldata_start
            .checked_add(transaction.tx_body_length())
            .ok_or(internal_error!("overflow"))?;

        let pre_tx_buffer = transaction.pre_tx_buffer();
        Self::write_calldata_prefix(
            pre_tx_buffer,
            calldata_start,
            selector,
            tx_hash,
            suggested_signed_hash,
        );

        // we can now take and cast as transaction is static relative to EEs
        let calldata = &transaction.underlying_buffer()[calldata_start..calldata_end];

        let resources_for_tx = resources.clone();

        BasicBootloader::run_single_interaction(
            system,
            system_functions,
            memories,
            calldata,
            &BOOTLOADER_FORMAL_ADDRESS,
            &from,
            resources_for_tx,
            &U256::ZERO,
            true,
            tracer,
        )
    }
}
