//! Isolates transaction validation hints in prover-input length comparisons.
use std::alloc::Global;

use alloy::consensus::TxEnvelope;
use basic_system::system_functions::NoStdSystemFunctions;
use oracle_provider::{ReadWitnessSource, RunMode, ZkEENonDeterminismSource};
use zk_ee::reference_implementations::{BaseResources, DecreasingNative};
use zk_ee::system::{logger::NullLogger, Resource, SystemFunctionsExt};
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

/// Records the exact RISC-V tape words consumed by production sender recovery for
/// these signed legacy transactions. Recovery hint counts depend on the signature,
/// so comparisons of otherwise equivalent KZG calls must remove this contribution.
/// Rejects other transaction kinds rather than guessing which validations their
/// bootloader paths perform.
pub fn legacy_sender_recovery_tape_len(txs: &[ZKsyncTxEnvelope]) -> usize {
    let mut oracle = ZkEENonDeterminismSource::new(RunMode::NativeRunSavingForRiscV);
    oracle.add_external_processor(callable_oracles::field_hints::NativeFieldOpsQuery);
    let mut oracle = ReadWitnessSource::new(oracle);
    let words = oracle.get_read_items();
    for tx in txs {
        let ZKsyncTxEnvelope::Ethereum(TxEnvelope::Legacy(signed), sender) = tx else {
            panic!("sender recovery tape helper requires signed legacy transactions");
        };
        let signature = signed.signature();
        let mut input = [0u8; 128];
        input[..32].copy_from_slice(signed.signature_hash().as_slice());
        input[63] = 27 + u8::from(signature.v());
        input[64..96].copy_from_slice(&signature.r().to_be_bytes::<32>());
        input[96..].copy_from_slice(&signature.s().to_be_bytes::<32>());
        let mut output = Vec::new();
        let mut resources = BaseResources::<DecreasingNative>::FORMAL_INFINITE;
        <NoStdSystemFunctions<true> as SystemFunctionsExt<BaseResources<DecreasingNative>>>::secp256k1_ec_recover(
            &input, &mut output, &mut resources, &mut oracle, &mut NullLogger, Global,
        ).expect("production sender recovery succeeds");
        assert_eq!(output.len(), 32);
        assert_eq!(&output[12..], sender.as_slice());
    }
    let len = words.borrow().len();
    len
}
