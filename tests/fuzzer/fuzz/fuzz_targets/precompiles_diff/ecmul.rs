#![no_main]
#![feature(allocator_api)]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use fuzz_precompiles_forward::precompiles::ecmul as ecmul_forward;
use fuzz_precompiles_proving::precompiles::ecmul as ecmul_proving;
use revm_precompile::bn254;
use crate::common::{PointKind,CoordMut,build_point_bytes};

mod common;

const BN254_ECMUL_SRC_REQUIRED_LENGTH: usize = 96;
const BN254_ECMUL_DST_MIN_LENGTH: usize = 64;

#[derive(Arbitrary, Debug, Clone)]
struct Input {
    // Point generation class
    kind: PointKind,

    // Scalars for constructing valid point
    s_bytes: [u8; 32],

    // Scalar for multiplication
    k_bytes: [u8; 32],

    // Coordinate-level mutations
    m_x: CoordMut,
    m_y: CoordMut,

    // Decide which coordinate to mutate
    spice: u8,

    // Raw bytes
    raw_p: [u8; 64],

    // Mutate the input bytes by trimming length
    #[arbitrary(with = mutate_len)]
    max_len: u8,
}

fn mutate_len(u: &mut Unstructured<'_>) -> arbitrary::Result<u8> {
    let x: u8 = u.arbitrary()?;
    let len = match x % 8 {
        0 => 0,
        1 => 1,
        2 => BN254_ECMUL_DST_MIN_LENGTH as u8,
        3 => BN254_ECMUL_SRC_REQUIRED_LENGTH as u8 - 1,
        _ => BN254_ECMUL_SRC_REQUIRED_LENGTH as u8,
    };
    Ok(len)
}

fn build_input_bytes(i: &Input) -> [u8; BN254_ECMUL_SRC_REQUIRED_LENGTH] {
    let p = build_point_bytes(i.kind, i.s_bytes, i.m_x, i.m_y, i.spice, i.raw_p);
    let mut buf = [0u8; BN254_ECMUL_SRC_REQUIRED_LENGTH];
    buf[..64].copy_from_slice(&p);
    buf[64..].copy_from_slice(&i.k_bytes);
    buf
}

fn fuzz(data: &[u8]) {
    let mut u = Unstructured::new(data);
    let input = match Input::arbitrary(&mut u) {
        Ok(x) => x,
        Err(_) => return,
    };

    let in_bytes = build_input_bytes(&input);
    let n = usize::from(input.max_len);
    let input_slice = &in_bytes[..n.min(BN254_ECMUL_SRC_REQUIRED_LENGTH)];

    let mut dst1 = Vec::new();
    let mut dst2 = Vec::new();

    let r_reth = bn254::run_mul(input_slice, u64::MAX, u64::MAX);
    let reth_ok = r_reth.as_ref().is_ok_and(|x| !x.reverted);

    let r1 = ecmul_forward(input_slice, &mut dst1);
    let r1_ok = r1.is_ok();

    // Skip if both RETH and Forward run failed
    if !reth_ok && !r1_ok {
        return;
    }

    let r2 = ecmul_proving(input_slice, &mut dst2);
    let r2_ok = r2.is_ok();

    if reth_ok || r1_ok || r2_ok {
        assert!(reth_ok,  "reth reverted but others accepted");
        assert!(r1_ok,   "forward run rejected but reth accepted");
        assert!(r2_ok,   "proving run rejected but reth accepted");

        let reth_out = r_reth.unwrap().bytes.to_vec();

        assert_eq!(reth_out.len(), BN254_ECMUL_DST_MIN_LENGTH, "reth output length mismatch");
        assert_eq!(dst1.len(), BN254_ECMUL_DST_MIN_LENGTH, "forward output length mismatch");
        assert_eq!(dst2.len(), BN254_ECMUL_DST_MIN_LENGTH, "proving output length mismatch");

        assert_eq!(dst1, reth_out, "forward <> reth bytes mismatch");
        assert_eq!(dst2, reth_out, "proving <> reth bytes mismatch");
    }
}

fuzz_target!(|data: &[u8]| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(data);
});
