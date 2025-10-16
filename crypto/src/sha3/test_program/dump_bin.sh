#!/bin/bash

set -e

# cargo objdump --features keccak_f1600_test --release --target riscv32i-unknown-none-elf -v -- -d

rustup target add riscv32i-unknown-none-elf
rustup component add llvm-tools 

rustup component add llvm-tools-preview
cargo install cargo-binutils

# for compilation, target riscv32 is already set by .cargo folder
cargo update

cargo objcopy --release --features keccak_f1600_test -- -O binary app_keccak_simple.bin
cargo objcopy --release --features bad_keccak_f1600_test -- -O binary app_keccak_bad.bin
cargo objcopy --release --features hash_chain_test -- -O binary app_keccak_bench.bin
cargo objcopy --release --features mini_digest_test -- -O binary app_keccak_complex.bin

cargo objcopy --release --features keccak_f1600_test -- -O binary --only-section=.text app_keccak_simple.text
cargo objcopy --release --features bad_keccak_f1600_test -- -O binary --only-section=.text app_keccak_bad.text
cargo objcopy --release --features hash_chain_test -- -O binary --only-section=.text app_keccak_bench.text
cargo objcopy --release --features mini_digest_test -- -O binary --only-section=.text app_keccak_complex.text

cargo objdump --release --features keccak_f1600_test -- -d >app_keccak_simple.asm
cargo objdump --release --features bad_keccak_f1600_test -- -d >app_keccak_bad.asm
cargo objdump --release --features hash_chain_test -- -d >app_keccak_bench.asm
cargo objdump --release --features mini_digest_test -- -d >app_keccak_complex.asm