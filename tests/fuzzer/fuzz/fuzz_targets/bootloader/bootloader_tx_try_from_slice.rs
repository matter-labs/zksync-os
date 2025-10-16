#![no_main]

use basic_bootloader::bootloader::transaction::AbiEncodedTransaction;
use libfuzzer_sys::fuzz_target;

fn fuzz(data: &[u8]) {
    let mut data = data.to_owned();
    let _ = AbiEncodedTransaction::try_from_slice(&mut data);
}

fuzz_target!(|data: &[u8]| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(data);
});
