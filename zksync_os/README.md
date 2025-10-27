# zksync_os crate

This crate contains the main zksync_os program. All the configuration and data is fed to it through the CRS register (see section below).


It is compiled into RISCV format - you should run "./dump_bin.sh" to create the bin and elf files, that can later be used by zksync_os_runner to execute programs using risc V simulator.


## Outputs

By convention, data that is stored in registers 10-17 after the execution is considered
the 'output' of this execution.


## Communication with oracles (non-determinism sources)

zkOS communicates with oracles via CSR (Control and Status register) `0x7c0`.
It will request data, by writing the payload to that register, and, afterwards, try to read the data from the register itself.

When running, this is handled by the risc_v_simulator - that sees the opcodes that
are writing to this register, and forwards them to the oracles.

This means that zksync_os code MUST be run within the risc_v_simulator environment.

## How to prove & verify

You'll have to use the tools from `zksync-airbender` repo. More instructions coming soon.
