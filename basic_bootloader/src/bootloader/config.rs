pub trait BasicBootloaderExecutionConfig: 'static + Clone + Copy + core::fmt::Debug {
    /// Flag to disable EOA signature validation.
    /// It can be used to optimize forward run.
    const VALIDATE_EOA_SIGNATURE: bool;
    /// Simulation flag(used for `eth_call` and `estimate_gas`)
    const SIMULATION: bool;
    /// Flag that enables the `FRI_PROOF_QUERY_ID` oracle query for
    /// `FriProofTx` transactions.
    ///
    /// Set `true` only on the proving config, which covers two uses:
    ///   - The RISC-V guest binary that runs the in-circuit verifier.
    ///   - The host-mode prover-input recording pass, whose sole job
    ///     is to drive the oracle so `ReadWitnessSource` captures the
    ///     proof stream the guest later replays over CSR.
    ///
    /// Forward-mode paths used for user-facing block building
    /// (`ForwardSimulationConfig`), `eth_call` (`CallSimulationConfig`),
    /// and ETH-replay (`ForwardETHLikeConfig`) set this `false`.
    /// Those paths trust the admission layer's FRI check and populate
    /// `TxLevelMetadata.verified_fri_statements` directly from the
    /// tx body's claimed list. The in-circuit verifier is the final
    /// authority — a mismatch there fails the block proof, same as
    /// any other bad witness.
    ///
    /// Mirrors the pattern `VALIDATE_EOA_SIGNATURE` uses for signature
    /// checks: admission-layer work that the bootloader deliberately
    /// skips during sequencing.
    const VERIFY_FRI_PROOFS: bool;
}

#[derive(Clone, Copy, Debug)]
pub struct BasicBootloaderProvingExecutionConfig;

impl BasicBootloaderExecutionConfig for BasicBootloaderProvingExecutionConfig {
    const SIMULATION: bool = false;
    const VALIDATE_EOA_SIGNATURE: bool = true;
    const VERIFY_FRI_PROOFS: bool = true;
}

#[derive(Clone, Copy, Debug)]
pub struct BasicBootloaderForwardSimulationConfig;

impl BasicBootloaderExecutionConfig for BasicBootloaderForwardSimulationConfig {
    const VALIDATE_EOA_SIGNATURE: bool = false;
    const SIMULATION: bool = false;
    const VERIFY_FRI_PROOFS: bool = false;
}

#[derive(Clone, Copy, Debug)]
pub struct BasicBootloaderCallSimulationConfig;

impl BasicBootloaderExecutionConfig for BasicBootloaderCallSimulationConfig {
    // doesn't really matter, as `SIMULATION` disables signature validation anyway
    const VALIDATE_EOA_SIGNATURE: bool = true;
    const SIMULATION: bool = true;
    const VERIFY_FRI_PROOFS: bool = false;
}

#[derive(Clone, Copy, Debug)]
pub struct BasicBootloaderForwardETHLikeConfig;

impl BasicBootloaderExecutionConfig for BasicBootloaderForwardETHLikeConfig {
    const VALIDATE_EOA_SIGNATURE: bool = true;
    const SIMULATION: bool = false;
    const VERIFY_FRI_PROOFS: bool = false;
}
