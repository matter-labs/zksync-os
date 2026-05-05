#![no_std]
#![no_main]

#[airbender::main]
fn main() -> bool {
    crypto::blake2s::blake2s_tests::run_tests();
    true
}
