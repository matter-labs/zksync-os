use crate::run::convert::FromInterface;
use crate::run::errors::ForwardSubsystemError;
use crate::run::fri_proof_sidecar::FromInterfaceSidecar;
use crate::run::output::BlockOutput;
use crate::run::output::TxResult;
use crate::run::tracing_impl::TracerWrapped;
use crate::run::validator_impl::ValidatorWrapped;
use crate::run::{
    run_block_with_chain_config, simulate_tx_with_chain_config, FriVerifierArtifacts,
};
use std::sync::Arc;
use zk_ee::system::metadata::chain_config::ChainConfig;
use zk_ee::system::metadata::zk_metadata::BlockMetadataFromOracle;
use zksync_os_interface::tracing::{AnyTracer, AnyTxValidator};
use zksync_os_interface::traits::{
    AnyBlockContext, EncodedTx, FriProofSidecarSource, PreimageSource, ReadStorage, RunBlock,
    SimulateTx, TxResultCallback, TxSource,
};

/// Forward-mode `RunBlock` impl.
#[derive(Default)]
pub struct RunBlockForward {
    pub fri_verifier_artifacts: Option<Arc<FriVerifierArtifacts>>,
}

impl RunBlock for RunBlockForward {
    type Config = ChainConfig;
    type Error = ForwardSubsystemError;
    type BlockOutput = BlockOutput;

    fn run_block<
        Storage: ReadStorage,
        PreimgSrc: PreimageSource,
        TrSrc: TxSource,
        FriSidecar: FriProofSidecarSource,
        TrCallback: TxResultCallback,
        Tracer: AnyTracer,
        Validator: AnyTxValidator,
        BlockContext: AnyBlockContext,
    >(
        &self,
        config: ChainConfig,
        block_context: BlockContext,
        storage: Storage,
        preimage_source: PreimgSrc,
        tx_source: TrSrc,
        fri_proof_sidecar: FriSidecar,
        tx_result_callback: TrCallback,
        tracer: &mut Tracer,
        validator: &mut Validator,
    ) -> Result<Self::BlockOutput, Self::Error> {
        let evm_tracer = tracer.as_evm().expect("only EVM tracers are supported");
        let evm_tx_validator = validator
            .as_evm()
            .expect("only EVM validators are supported");
        run_block_with_chain_config(
            config,
            BlockMetadataFromOracle::from_interface(block_context),
            storage,
            preimage_source,
            tx_source,
            FromInterfaceSidecar(fri_proof_sidecar),
            self.fri_verifier_artifacts.clone(),
            tx_result_callback,
            &mut TracerWrapped(evm_tracer),
            &mut ValidatorWrapped(evm_tx_validator),
        )
    }
}

impl SimulateTx for RunBlockForward {
    type Config = ChainConfig;
    type Error = ForwardSubsystemError;

    fn simulate_tx<
        Storage: ReadStorage,
        PreimgSrc: PreimageSource,
        Tracer: AnyTracer,
        Validator: AnyTxValidator,
        BlockContext: AnyBlockContext,
    >(
        &self,
        config: ChainConfig,
        transaction: EncodedTx,
        block_context: BlockContext,
        storage: Storage,
        preimage_source: PreimgSrc,
        tracer: &mut Tracer,
        validator: &mut Validator,
    ) -> Result<TxResult, Self::Error> {
        let evm_tracer = tracer.as_evm().expect("only EVM tracers are supported");
        let evm_tx_validator = validator
            .as_evm()
            .expect("only EVM validators are supported");
        simulate_tx_with_chain_config(
            config,
            transaction,
            BlockMetadataFromOracle::from_interface(block_context),
            storage,
            preimage_source,
            &mut TracerWrapped(evm_tracer),
            &mut ValidatorWrapped(evm_tx_validator),
        )
    }
}
