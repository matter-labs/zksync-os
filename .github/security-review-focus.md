Applies to the production ZKsync OS state transition function (STF): code built into the proving binary or into `forward_system` with the `production` feature set.

# ZKsync OS Threat Model

ZKsync OS is the STF of a ZK rollup. The same code runs in forward mode (sequencer, `forward_system`) and proving mode (RISC-V, `proof_running_system` / `zksync_os`, proved by Airbender). **Proving mode is ground truth**: only what the proving run computes is committed to L1.

## Actors and trust

- **Untrusted:** L2 tx senders (tx bytes, signatures, calldata, deployed bytecode); L1 priority tx requesters (content is user-chosen; L1 only validates and charges it).
- **Operator / sequencer:** trusted for liveness, ordering, inclusion and L1 tx filtering decisions; **never trusted for validity**. In proving, every oracle answer is operator-supplied and adversarial: storage reads, tree proofs, preimages, block metadata, blob/KZG data, `L1TxOperatorDecision`.
- **Trusted:** governance and upgrade txs, L1 contracts and verifier, `ChainConfig` (committed via `chain_config_hash`).

## Assets

- Balances, nonces, storage and code in the state tree (`state_root`, `next_free_slot`).
- Base-token backing: L2 supply must stay backed by funds locked in the L1 bridge.
- Everything L1 acts on: chain state commitment, `priority_operations_hash`, `l2_logs_tree_root` (user messages, withdrawals, L1 tx results), `upgrade_tx_hash`, pubdata / DA commitment, `chain_config_hash`.
- Provability: every batch committed to L1 must be provable with the current verification key. Batches are FRI-proven before the L1 commit, so an unprovable batch can be recovered by re-sequencing (splitting a block, dropping a tx). This is the planned recovery path, not routine operation; it has never been needed. It means a reorg: bad, but far less severe than a verification-key change. Re-sequencing cannot fix a failure in processing that every block must run (see the High chain-halt impact).

## Impacts

- **Critical, direct and publicly triggerable loss of funds:** a regular L2 or L1 tx steals balances, credits unbacked base token, or gets an L2→L1 log or message committed without the state change it attests to (withdrawal without burn, double withdrawal, wrong L1 tx result).
- **High, invalid executions provable:** the proving run relies on an oracle answer it does not constrain (e.g. an unchecked tree proof, preimage or crypto-primitive hint), or lets the operator select an outcome the STF does not explicitly allow.
- **High, chain halt that re-sequencing cannot fix:** proving fails in processing the operator cannot leave out, so no choice of transactions (dropping, force-reverting, splitting the block) lets the chain progress, and recovery needs a new verification key. Examples: a bug in the part of the bootloader that runs before the first tx, or a bug in the hashing of an L1 tx that makes it impossible to force-revert.

  This is **not** the usual DoS triggered by a transaction. If dropping or force-reverting the offending tx avoids the failure, the finding is the Medium impact below, however cheap or repeatable the attack is. No finding of this High kind has been confirmed so far. Rate one High only with an argument for why re-sequencing cannot avoid the failure.
- **Medium, valid executions unprovable:** attacker-controlled content makes proving fail (a panic on the proving path, forward/proving divergence, or proving cycles or memory not bounded by charged resources). The operator recovers by re-sequencing (dropping or force-reverting the tx, or splitting the block), so the impact is a forced reorg, not a halted chain or a verification-key change.
- **Medium, undocumented deviation from EVM (Osaka) behavior,** excluding divergences documented in `docs/execution_environments/evm.md` and `docs/not-a-bug.md`. A deviation that arises only from operator-controlled inputs (e.g. an operator-supplied coinbase or other `BlockContext` value) is not a finding.

## Invariants

1. Forward (64-bit host) and proving (32-bit RISC-V) runs are identical: same oracle queries in the same order, same results, resource charges, outputs and commitments. `usize` width, oracle word packing (`UsizeSerializable`), pointer sizes and `as` casts must not change results.
2. Oracle data is untrusted: the operator can answer anything. Its correctness may be assumed only in liveness arguments, since a lying oracle can at most stop the operator proving its own block. Wherever safety depends on an answer (storage reads and tree proofs, preimages, arithmetic and field hints for crypto primitives, blob KZG commitments), the program itself must constrain it, by recomputing it or checking a hash, proof or hint relation, so that a lie cannot yield a valid proof. Operator choices the STF explicitly permits (e.g. `L1TxOperatorDecision`) need only be constrained to the permitted set.
3. A panic is a liveness issue, so it matters only when an attacker can trigger it. No panic (`unwrap`, `expect`, `unreachable!`, indexing) may be reachable from attacker-controlled content: tx bytes, calldata, bytecode, L1 tx data, or user-written state, even when that content reaches the STF through the oracle. A panic that needs abnormal values in operator-only inputs (e.g. `BlockContext` fields such as the base fee) is operator self-DoS, not a finding.
4. Proving cost is bounded by charged gas and native resources. Charging uses only block-start facts, rollback-aware cache metadata and cache values, never cache-entry presence.
5. No path credits base token except the documented ones (L1 tx value is taken from the treasury). L2→L1 logs and messages are emitted only together with the state change they attest to. A revert rolls back state, transient storage, events and L2→L1 logs together.
6. Serialization, hashing, state layout, resource accounting and oracle-query semantics are protocol: changing them is a protocol change.
7. EVM equivalence: EVM bytecode behaves as on Ethereum Osaka (opcodes, gas costs, precompiles, call/create/revert semantics, state changes). The only exceptions are the divergences documented in `docs/execution_environments/evm.md` and `docs/not-a-bug.md`, extra charges for pubdata and native resources (`docs/double_resource_accounting.md`), and deviations that arise only from operator-controlled inputs.

## L1 (priority) transactions

`l1_tx_filtering_enabled` is on for every production chain. A priority tx that cannot be executed or proven can be dropped from the block or force-failed (force-reverted) by the operator instead of halting the chain, so an "unprovable L1 tx" finding is a forced reorg like any other unprovable execution (Medium). A failure in the processing that a force-reverted priority tx still goes through cannot be avoided this way: that is the High chain-halt impact. The filtering paths themselves must hold:

- **Force-revert path:** runs for every priority tx, whatever its body: decoding, value mint, full-gas-limit charge, refund of the remainder, failure result log. It must be panic-free and provable. The result log must be correct, because L1 relies on it to release failed deposits.
- **`RejectedAfterExecution`:** rolls the body back completely.
- **Decision replay:** proving replays the decision from the oracle, and it must apply at exactly the same point as in sequencing.
- **Upgrade txs:** never filtered, so an unprovable upgrade tx still halts the chain.
- **Priority operations hash:** every processed priority tx, force-reverted ones included, enters `priority_operations_hash` exactly once, in processing order.

## Out of scope

- Ethereum STF / runner paths (`basic_bootloader/src/bootloader/transaction_flow/ethereum/`, `basic_bootloader/src/bootloader/block_flow/ethereum/`, `basic_system/src/system_implementation/ethereum_storage_model/`), and anything reachable only without the `production` feature set (`for_tests`, `eth_runner`, `evm_tester`).
- Operator discretion: censorship (including force-reverting priority txs), ordering, inclusion, liveness. Paths reachable only by privileged or governance addresses.
- Incorrect third-party oracle data, economic or governance attacks, Sybil attacks.
- Timing side channels and constant-time issues: the STF processes only public data and holds no secrets.
- Prover internals (witness generation, GPU code) beyond the ZKsync OS program itself.
- Behavior documented in `docs/not-a-bug.md` and the divergences listed in `docs/execution_environments/evm.md`.
