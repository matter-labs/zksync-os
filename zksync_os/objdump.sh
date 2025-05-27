#!/bin/sh
cargo objdump --features proving --release --target riscv32i-unknown-none-elf -v -- -d
# cargo objdump --target riscv32i-unknown-none-elf -v -- -d