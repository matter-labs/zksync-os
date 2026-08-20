# Repository Structure

Main runtime crates:

- [`zk_ee`](../zk_ee/) - core interfaces/types for system, oracle, memory, and execution environments.
- [`basic_system`](../basic_system/) - concrete system implementation (IO, storage, resources, system functions).
- [`basic_bootloader`](../basic_bootloader/) - transaction/block flow orchestration and finalization.
- [`evm_interpreter`](../evm_interpreter/) - EVM execution environment implementation.
- [`system_hooks`](../system_hooks/) - system-level hooks and precompile/system-call behavior.
- [`forward_system`](../forward_system/) - forward (sequencer) execution wiring and output conversion.
- [`proof_running_system`](../proof_running_system/) - proving-target wiring and allocator/runtime setup.
- [`oracle_provider`](../oracle_provider/) - non-determinism/oracle source and query routing.
- [`callable_oracles`](../callable_oracles/) - callable oracle implementations (e.g. arithmetic/KZG helpers).
- [`crypto`](../crypto/) - cryptographic primitives.
- [`storage_models`](../storage_models/) - shared storage abstractions and common structs.
- [`zksync_os_runner`](../zksync_os_runner/) - RISC-V transpiler runner.

RISC-V program crate:

- [`zksync_os`](../zksync_os/) - main RISC-V binary that is executed/proved.
  - Note: `zksync_os` is intentionally excluded from the root workspace and is built from its own directory.
