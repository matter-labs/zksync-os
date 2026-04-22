# zksync_os crate

This crate contains the main zksync_os program. All the configuration and data is fed to it through the CSR register (see section below).

It is compiled into RISC-V format via `cargo airbender build` (see `./dump_bin.sh`), which produces `dist/<app>/app.{bin,elf,text}` plus a `manifest.toml`. Those artifacts are consumed by `zksync_os_runner` to simulate execution via the airbender-platform transpiler.

## Outputs

By convention, data that is stored in registers 10-17 after the execution is considered
the 'output' of this execution.

## Communication with oracles (non-determinism sources)

zkOS communicates with oracles via CSR (Control and Status Register) `0x7c0`.
It will request data by writing the payload to that register, and afterwards try to read the data from the register itself.

During simulation, the airbender-platform transpiler intercepts the opcodes writing to
this register and serves pre-recorded non-determinism words passed to
`zksync_os_runner::Runner::run`.

This means that zksync_os code MUST be run within the transpiler VM environment.

## How to prove & verify

Use the `airbender-host` `CpuProver` / `GpuProver` APIs (used by `tests/rig` and
`tests/instances/eth_runner` when the `e2e_proving` / `proving` / `gpu` features are
enabled). See [Proving tests with CLI](../docs/proving_tests_with_cli.md) for instructions.
