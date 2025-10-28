#![no_std]
#![no_main]

use riscv_common::zksync_os_finish_success;

#[link_section = ".init.rust"]
#[export_name = "_start_rust"]
unsafe extern "C" fn start_rust() -> ! {
    main()
}

unsafe fn workload() -> ! {
    // just invoke blake
    use crypto::MiniDigest;
    let _output = core::hint::black_box(crypto::blake2s::Blake2s256::digest(&[1, 2, 3, 4, 5]));

    // and invoke some bigint via point multiplication by scalar

    use crypto::ark_ec::AffineRepr;
    use crypto::bn254::G1Affine;
    let _result = core::hint::black_box(G1Affine::generator().mul_bigint(&[123u64]));

    zksync_os_finish_success(&[1, 0, 0, 0, 0, 0, 0, 0]);
}

#[inline(never)]
fn main() -> ! {
    unsafe { workload() }
}
