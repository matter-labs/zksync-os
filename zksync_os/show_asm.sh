#!/bin/sh
# cargo asm --rust --context=1 --bin zksync_os
# cargo asm --rust --context=0 --bin zksync_os "evm_interpreter::interpreter::<impl evm_interpreter::InterpreterFrame<S>>::step"
cargo asm -C target_feature=+zbb,+m --rust --context=2 --target=riscv32i-unknown-none-elf --bin zksync_os main 0
# cargo asm -C target_feature=+zbb,+m --rust --context=0 --target=riscv32i-unknown-none-elf --bin zksync_os "verifier::simulate_verify_main"
# cargo asm -C target_feature=+zbb,+m --rust --context=0 --target=riscv32i-unknown-none-elf --bin zksync_os "verifier::read_read_value::Leaf<A>::read_and_hash"