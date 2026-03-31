# zksync_os crate

This crate contains the main zksync_os program. All the configuration and data is fed to it through the CSR register (see section below).

It is compiled into RISC-V format — run `./dump_bin.sh` to create the `.bin`, `.text`, and `.elf` files. The `.bin` and `.text` files are used by `zksync_os_runner` to simulate execution via the `riscv_transpiler` VM.

## Outputs

By convention, data that is stored in registers 10-17 after the execution is considered
the 'output' of this execution.

## Communication with oracles (non-determinism sources)

zkOS communicates with oracles via CSR (Control and Status Register) `0x7c0`.
It will request data by writing the payload to that register, and afterwards try to read the data from the register itself.

During simulation, this is handled by the `riscv_transpiler` VM — it intercepts the
opcodes writing to this register and forwards them to the `NonDeterminismCSRSource`
implementation provided by the runner.

This means that zksync_os code MUST be run within the transpiler VM environment.

## How to prove & verify

You'll have to use the tools from the `zksync-airbender` repo. See [Proving tests with CLI](../docs/proving_tests_with_cli.md) for instructions.
