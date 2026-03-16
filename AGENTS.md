# AGENTS.md

## Purpose
ZKsync OS is a security-critical state transition function for a ZK rollup. It is supposed to be proved using ZKsync Airbender prover.
Changes are security-sensitive: prefer minimal, auditable changes.

## Runtime Model
The ZKsync OS is used in two environments:
- Forward mode: host/sequencer execution (`forward_system`).
- Proving mode: RISC-V execution for proof generation (`proof_running_system`, `zksync_os`).

Call simulation exists in forward mode (`eth_call` and `estimate_gas`-like path) and should skip normal tx validation/fee logic.

## High-Level Architecture
Core crates and responsibilities:
- `zk_ee/`: core system interfaces/types (system, oracle, memory, errors, shared structs)
- `basic_system/`: concrete system implementation (IO, storage, resources, system functions)
- `basic_bootloader/`: main part of concrete state transition function implementation. Block/tx flow orchestration, main execution loop
- `evm_interpreter/`: EVM execution environment implementation
- `system_hooks/`: system hook dispatch + precompile/precompile-like behavior
- `storage_models/`: shared storage abstractions and common structs used by system implementations
- `crypto/`: cryptographic primitives and utilities used by system/interpreter/precompiles
- `oracle_provider/`: implementation of non-determinism source and query processor routing for oracle IO used by system to get external data
- `callable_oracles/`: implementations of special callable oracles (arithmetic, blob KZG commitment, etc.)
- `forward_system/`: sequencer/forward execution adapter, outputs, simulation paths
- `proof_running_system/`: proving-target system wiring and allocator setup
- `api/`: API interfaces and abstractions used by the sequencer and other integrations
- `cycle_marker/`: performance and cycle tracking utilities
- `zksync_os/`: RISC-V binary target (built separately, excluded from workspace)
- `zksync_os_runner/`: executes RISC-V binaries via simulator, returns program output

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
# Workspace build/test
cargo build
cargo test --workspace

# Per-crate
cargo build -p <crate-name>
cargo test -p <crate-name>

# Quality checks
cargo fmt -p <crate-name>
cargo fmt
cargo clippy --all -- -D warnings

# Auto-fix some clippy issues
cargo clippy --fix -p <crate-name>
```

Note: `cargo test --workspace` does not include crates/directories excluded in root `Cargo.toml` (e.g. `zksync_os`, `tests/fuzzer`, `tests/evm_tester`, `tests/instances/eth_runner`).

RISC-V build (from repo root):
```bash
cd zksync_os && ./dump_bin.sh --type for-tests
```

### Testing Infrastructure

#### Integration tests
The project uses a custom testing rig located in `tests/rig/` with the main abstraction being the `TestingFramework` struct. Tests are organized in `tests/instances/` and follow this pattern:
1. Set up initial chain state (predeployed contracts, balances)
2. Define transactions to execute
3. Call `execute_block` to execute (typically runs both forward and proof systems, unless configured otherwise)

When adding or modifying tests under `tests/instances/`, read `tests/instances/TESTING.md` first and follow its conventions.

Avoid direct `Chain` usage in test instances unless a low-level path is explicitly needed.

To run rig-based tests without detailed executions logs use:
```bash
cargo test -p <crate-name> --features rig/no_print
```

By default, local rig runs skip RISC-V simulation unless `ZKSYNC_RISC_V_RUN` or `CI` is set. To force proving-mode simulation locally use:
```bash
ZKSYNC_RISC_V_RUN=true cargo test -p <crate-name>
```

#### EVM tester
The EVM tester is used to run the Ethereum execution spec tests to check the EVM compatibility of ZKsync OS.

To run EVM tester:
```bash
# prerequisite,  download fixtures if they are not downloaded yet
cd tests/evm_tester && ./download_ethereum_fixtures.sh

cd tests/evm_tester && cargo run --bin evm-tester --release --features zksync_os_forward_system/no_print
```

#### Unit tests
Unit tests should be specified in a tests module defined within the module file being tested.
The tests module should be defined at the bottom after all the existing code to test.

## Agent Working Rules

### General rules
- Security of this project is critical. Choose correctness over convenience.
- Prioritize safety, correctness and efficiency.
- Write readable and well-documented code that follows Rust style conventions.
- Treat behavior changes as protocol changes unless proven otherwise.
- Avoid changing serialization/hashing/state-layout/resource-accounting/oracle-query semantics without explicit validation.
- Keep forward/proof behavior aligned; if one path changes, inspect the other, especially important for crypto primitives.
- Add/adjust tests when corresponding logic is changed.
- Never include credentials, keys, or tokens in code.
- When possible, make match statements exhaustive and avoid wildcard arms.
- Pay attention to what facilities already exist for testing, and aim to reuse them.
- Do not introduce `std` or global allocator dependencies in no-std runtime paths targeting proving RISC-V binaries.

### Pull Request conventions
- When opening a PR, always use `.github/pull_request_template.md` and fill all sections concisely.
- PR titles must follow semantic prefixes:
  - `feat(<scope>): <description>` (or `feat: <description>`)
  - `fix(<scope>): <description>` (or `fix: <description>`)
  - `test(<scope>): <description>` (or `test: <description>`)
  - `chore(<scope>): <description>` (or `chore: <description>`)

### Review Priorities
When reviewing, prioritize:
1. State transition correctness. Code changes should not lead to security issues or break invariants.
2. Panic paths reachable from untrusted input. ZKsync OS should not panic in production.
3. Edge cases reachable from malformed/external input.
4. Cross-mode divergence (forward vs proving).

### Panic and Error Policy
- Do not introduce `todo!`, `unimplemented!`, or `unreachable!` on paths reachable from external input.
- Avoid `unwrap`/`expect` unless the invariant is locally guaranteed and documented.
- Prefer fixing root causes over adding panics or broad `unwrap`/`expect`.
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
