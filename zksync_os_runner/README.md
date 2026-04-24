# zksync_os_runner

This crate is a thin facade over airbender-platform's `TranspilerRunner` for executing
a ZKsync OS RISC-V program.

It expects the program to be built with `cargo airbender build` into a distribution
directory laid out as `dist/<app>/app.{bin,elf,text}` plus `manifest.toml`. The path
to that directory is passed as an argument.

The main method (`lib.rs:run`) takes a slice of `u32` words as pre-recorded
non-determinism input (typically produced by a native proof input run) and runs zkOS for
a given number of cycles.
