#![no_std]
#![no_main]

#[airbender::main]
fn main() -> [u32; 8] {
    crypto::blake2s::blake2s_tests::run_tests();
    [1, 0, 0, 0, 0, 0, 0, 0]
}
