# zksync_os_runner

This crate is responsible for running a program in ZKsync OS on riscV simulator.

It assumes that zksync_os binary is already compiled into riscV binary, and located
in "../zksync_os/app.bin". 

The main method (lib.rs:run) - takes as input the NonDeterminismCSRSource (a trait that will simulate/provide all the read & writes) - and then runs zkOS for a given number of cycles.

