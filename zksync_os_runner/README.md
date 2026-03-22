# zksync_os_runner

This crate is responsible for running a program in ZKsync OS on the RISC-V transpiler.

It assumes that the zksync_os binary is already compiled into RISC-V format, producing
both a `.bin` (ROM image) and a `.text` (instruction section) file. The path to the
`.bin` file is passed as an argument; the `.text` file is expected at the same path
with a `.text` extension.

The main method (`lib.rs:run`) takes as input a `NonDeterminismCSRSource` (a trait that
provides all CSR read/write handling for oracles) and then runs zkOS for a given number
of cycles using the `riscv_transpiler` VM.
