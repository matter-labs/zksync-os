# AGENTS.md

## Purpose
This repository implements ZKsync OS, a Rust state transition function of a blockchain (ZK L2 rollup) designed to be proved using ZKsync Airbender prover.

The code is security-critical and consensus-sensitive. Prefer minimal, auditable changes.

### Dual Target Architecture

The ZKsync OS is used in two environments:
- Host/sequencer execution (forward mode, native architecture)
- RISC-V proving execution (proof mode, RISC-V architecture)

The system is implemented in Rust and compiled into a RISC-V binary for proving using `zksync-airbender`.

Forward mode also supports a special call-simulation configuration (behavior similar to `eth_call`) used to check potential results and gas usage for a call. This simulation path skips normal transaction validation and fee payment logic.

## High-Level Architecture
Core crates and responsibilities:
- `zk_ee/`: foundational interfaces/types for system, memory, oracle, errors, execution environments and common structs
- `zksync_os/`: RISC-V binary target (built separately, excluded from workspace)
- `zksync_os_runner/`: executes RISC-V binaries via simulator, returns program output
- `basic_system/`: system implementation (IO, storage, resources, system functions)
- `basic_bootloader/`: block/tx flow orchestration, main execution loop, main part of concrete state transition function implementation
- `evm_interpreter/`: EVM execution environment implementation
- `forward_system/`: sequencer/forward execution adapter
- `proof_running_system/`: proving-target system wiring and allocator setup
- `system_hooks/`: system hook dispatch + precompile/precompile-like behavior
- `crypto/`: cryptographic primitives and utilities used by system/interpreter/precompiles
- `api/`: API interfaces and abstractions used by the sequencer and other integrations
- `cycle_marker/`: performance and cycle tracking utilities
- `oracle_provider/`: implementation of non-determinism source and query processor routing for oracle IO used by system to get external data
- `callable_oracles/`: implementations of special callable oracles (arithmetic, blob KZG commitment, etc.)
- `storage_models/`: shared storage abstractions and common structs used by system implementations

Reference docs:
- `docs/overview.md`
- `docs/bootloader/bootloader.md`
- `docs/system/system.md`
- `docs/da_commitment_schemes.md`

## Build and Test
Prerequisites (one-time):
```bash
rustup target add riscv32i-unknown-none-elf
cargo install cargo-binutils
rustup component add llvm-tools-preview
```

Common commands:
```bash
cargo build --release
cargo test --release --workspace
cargo fmt
cargo clippy --all -- -D warnings
```

Note: `cargo test --workspace` does not include crates/directories excluded in root `Cargo.toml` (e.g. `zksync_os`, `tests/fuzzer`, `tests/evm_tester`, `tests/instances/eth_runner`).

RISC-V build (from repo root):
```bash
cd zksync_os && ./dump_bin.sh --type for-tests
```

### Testing Infrastructure

The project uses a custom testing rig located in `tests/rig/` with the main abstraction being the `Chain` struct for in-memory chain state testing. Tests are organized in `tests/instances/` and other directories and follow this pattern:
1. Set up initial chain state (predeployed contracts, balances)
2. Define transactions to execute
3. Call `run_block` to execute (typically runs both forward and proof systems, unless configured otherwise)

#### EVM tester

The project uses special EVM tester setup to run the Ethereum execution spec tests. It is used to check the EVM compatibility of ZKsync OS.

To run EVM tester:
```bash
cd tests/evm_tester && cargo run --bin evm-tester --release --features zksync_os_forward_system/no_print
```

## Agent Working Rules
- Treat behavior changes as protocol changes unless proven otherwise.
- Avoid refactors that alter serialization, hashing, state layout, gas/resource accounting, or oracle query semantics without explicit validation.
- Keep forward/proof behavior aligned; if one path changes, inspect the other.
- Prefer fixing root causes over adding panics or broad `unwrap`/`expect`.
- Add/adjust tests when touching:
  - tx validation/fee logic
  - block finalization/pubdata
  - storage model transitions
  - system hooks and event parsing

## Review Priorities
When reviewing, prioritize:
1. Consensus/state transition correctness.
2. Panic paths reachable from untrusted input.
3. Resource accounting regressions (ergs/native/pubdata).
4. Cross-mode divergence between forward and proving paths.

## Panic and Error Policy
- Do not introduce `todo!`, `unimplemented!`, or `unreachable!` on paths reachable from external input.
- Avoid `unwrap`/`expect` unless the invariant is locally guaranteed and documented.
- Prefer returning typed validation/internal errors and mapping them at boundaries.
- When touching error enums, verify all conversion/mapping code paths are exhaustively updated.

## Debug Tips
- Use `system_log!`-driven traces in bootloader/system paths to inspect control flow.
- Check `tests/rig/` for canonical in-memory execution harness usage.
- For proving-related workflows, build `zksync_os` binary first (`dump_bin.sh`) and reuse test fixtures where possible.

## Practical Navigation
- Workspace members are listed in root `Cargo.toml`.
- Integration test harness: `tests/rig/`.
- Test instances: `tests/instances/*`.
- Fuzzing: `tests/fuzzer/`.
