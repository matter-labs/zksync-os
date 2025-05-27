#![no_main]

use crypto::blake2s::Blake2s256;
use crypto::MiniDigest;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut hasher = Blake2s256::new();

    let mut d = *data.first().unwrap_or(&1);
    if d == 0 {
        d += 1
    }

    let chunk_size = std::cmp::max(data.len() / d as usize, 1);
    for chunk in data.chunks(chunk_size) {
        hasher.update(chunk);
    }

    let incremental_digest = hasher.finalize();

    let full_digest = Blake2s256::digest(data);

    assert_eq!(incremental_digest, full_digest, "Digest results mismatch!");
});
