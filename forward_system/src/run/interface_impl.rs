use crate::run::convert::FromInterface;
use crate::run::errors::ForwardSubsystemError;
use crate::run::output::TxResult;
use crate::run::tracing_impl::TracerWrapped;
use crate::run::{run_block, simulate_tx};
use zk_ee::system::metadata::zk_metadata::BlockMetadataFromOracle;
use zksync_os_interface::tracing::AnyTracer;
use zksync_os_interface::traits::{
    EncodedTx, PreimageSource, ReadStorage, RunBlock, SimulateTx, TxResultCallback, TxSource,
};
use zksync_os_interface::types::BlockContext;
use zksync_os_interface::types::BlockOutput;

pub struct RunBlockForward {
    // Empty struct for now, but it can contain some configuration in the future.
    // For example, a flag to enable/disable specific behavior for subversions of the system.
    // These flags can then be used inside `run_block`/`simulate_tx` below to control the execution flow.
}

impl RunBlock for RunBlockForward {
    type Config = ();
    type Error = ForwardSubsystemError;

    fn run_block<
        Storage: ReadStorage,
        PreimgSrc: PreimageSource,
        TrSrc: TxSource,
        TrCallback: TxResultCallback,
        Tracer: AnyTracer,
    >(
        &self,
        _config: (),
        block_context: BlockContext,
        storage: Storage,
        preimage_source: PreimgSrc,
        tx_source: TrSrc,
        tx_result_callback: TrCallback,
        tracer: &mut Tracer,
    ) -> Result<BlockOutput, Self::Error> {
        let evm_tracer = tracer.as_evm().expect("only EVM tracers are supported");
        run_block(
            BlockMetadataFromOracle::from_interface(block_context),
            storage,
            preimage_source,
            tx_source,
            tx_result_callback,
            &mut TracerWrapped(evm_tracer),
        )
    }
}

impl SimulateTx for RunBlockForward {
    type Config = ();
    type Error = ForwardSubsystemError;

    fn simulate_tx<Storage: ReadStorage, PreimgSrc: PreimageSource, Tracer: AnyTracer>(
        &self,
        _config: (),
        transaction: EncodedTx,
        block_context: BlockContext,
        storage: Storage,
        preimage_source: PreimgSrc,
        tracer: &mut Tracer,
    ) -> Result<TxResult, Self::Error> {
        let evm_tracer = tracer.as_evm().expect("only EVM tracers are supported");
        simulate_tx(
            transaction,
            BlockMetadataFromOracle::from_interface(block_context),
            storage,
            preimage_source,
            &mut TracerWrapped(evm_tracer),
        )
    }
}
