#![no_main]
#![feature(allocator_api)]

use std::path::Path;
use arbitrary::{Arbitrary,Unstructured};
use libfuzzer_sys::fuzz_target;
use revm_precompile::kzg_point_evaluation;
use fuzz_precompiles_forward::precompiles::kzg as kzg_forward;
use fuzz_precompiles_proving::precompiles::kzg as kzg_proving;
use c_kzg::{Blob,Bytes32,KzgCommitment,KzgProof,KzgSettings,
ethereum_kzg_settings,BYTES_PER_BLOB};
use once_cell::sync::Lazy;
use revm::primitives::keccak256;
use sha2::{Digest, Sha256};

const VH_LEN: usize = 32; // Vershioned hash 
const C_LEN: usize = 48;  // Commitment
const P_LEN: usize = 48;  // Proof
const F_LEN: usize = 32;  // Field element
const IN_LEN: usize = VH_LEN + C_LEN + F_LEN + F_LEN + P_LEN;

#[derive(Arbitrary, Debug, Clone, Copy)]
enum FieldSel { Commit, Z, Y, Proof }

#[derive(Arbitrary, Debug, Clone, Copy)]
enum Mutation {
    None,
    Flip(FieldSel),
    Zero(FieldSel),
    ZeroAll,
}

#[derive(Arbitrary, Debug, Clone)]
enum Case {
    Random,
    Valid { mut_after: Mutation, bit_idx: u8 },
}

#[derive(Arbitrary, Debug, Clone)]
struct Input {
    case: Case,
    trunc: Option<u16>,
}

static KS: Lazy<&'static KzgSettings> = Lazy::new(|| ethereum_kzg_settings(0));

fn gen_valid(u: &mut Unstructured<'_>) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; BYTES_PER_BLOB as usize];
    for fe in buf.chunks_mut(32) {
        let _ = u.fill_buffer(&mut fe[..16]);
        fe[16..].fill(0);
    }

    let blob = Blob::from_bytes(&buf).ok()?;

    let mut z_raw = [0u8; 32];
    let _ = u.fill_buffer(&mut z_raw[..16]);
    let z = Bytes32::from(z_raw);

    let commitment = KS.blob_to_kzg_commitment(&blob).ok()?;
    let (proof, y) = KS.compute_kzg_proof(&blob, &z).ok()?;

    let vh = commitment_versioned_hash(&commitment);

    let mut out = vec![0u8; IN_LEN];
    let mut off = 0;

    out[off..off+VH_LEN].copy_from_slice(&vh);
    off += VH_LEN;

    out[off..off+F_LEN].copy_from_slice(z.as_ref());
    off += F_LEN;

    out[off..off+F_LEN].copy_from_slice(y.as_ref());
    off += F_LEN;

    out[off..off+C_LEN].copy_from_slice(commitment.to_bytes().as_ref());
    off += C_LEN;

    out[off..off+P_LEN].copy_from_slice(proof.to_bytes().as_ref());

    Some(out)
}

#[inline]
fn commitment_versioned_hash(commitment: &KzgCommitment) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(commitment.to_bytes().as_ref());
    let k = hasher.finalize();
    let mut out = [0u8; 32];
    out[0] = 0x01;
    out[1..].copy_from_slice(&k[1..]);
    out
}

fn field_range(sel: FieldSel) -> (usize, usize) {
    match sel {
        FieldSel::Commit => (VH_LEN + 2*F_LEN, VH_LEN + 2*F_LEN + C_LEN),
        FieldSel::Z      => (VH_LEN, VH_LEN + F_LEN),
        FieldSel::Y      => (VH_LEN + F_LEN, VH_LEN + 2*F_LEN),
        FieldSel::Proof  => (VH_LEN + 2*F_LEN + C_LEN, IN_LEN),
    }
}

fn apply_mutation(buf: &mut Vec<u8>, m: Mutation, bit_idx: u8) {
    match m {
        Mutation::None => {}
        Mutation::ZeroAll => {
            buf.fill(0);
        }
        Mutation::Flip(sel) => {
            let (a, b) = field_range(sel);
            let n = b - a;
            let i = a + ((bit_idx as usize) % n);
            buf[i] ^= 1;
        }
        Mutation::Zero(sel) => {
            let (a, b) = field_range(sel);
            buf[a..b].fill(0);
        }
    }
}

fn build_input(u: &mut Unstructured<'_>, i: &Input) -> Vec<u8> {
    let mut out = match &i.case {
        Case::Random => {
            let mut v = vec![0u8; IN_LEN];
            u.fill_buffer(&mut v);
            v
        }
        Case::Valid { mut_after, bit_idx } => {
            let mut v = gen_valid(u).unwrap_or_else(|| {
                let mut r = vec![0u8; IN_LEN];
                let _ = u.fill_buffer(&mut r);
                r
            });
            apply_mutation(&mut v, *mut_after, *bit_idx);
            v
        }
    };

    if let Some(t) = i.trunc {
        let t = t as usize;
        if t < out.len() { out.truncate(t); }
    }

    out
}

fn fuzz(data: &[u8]) {
    let mut u = Unstructured::new(data);
    let input = match Input::arbitrary(&mut u) {
        Ok(x) => x,
        Err(_) => return,
    };

    let in_bytes = build_input(&mut u, &input);

    let mut out_forward = Vec::new();
    let mut out_proving = Vec::new();

    let r_reth = kzg_point_evaluation::run(in_bytes.as_slice(), u64::MAX);
    let reth_ok = r_reth.as_ref().is_ok_and(|x| !x.reverted);

    let r1 = kzg_forward(in_bytes.as_slice(), &mut out_forward);
    let fwd_ok = r1.is_ok();
    let r2 = kzg_proving(in_bytes.as_slice(), &mut out_proving);
    let prv_ok = r2.is_ok();

    match (reth_ok, fwd_ok, prv_ok) {
        (true, true, true) => {
            let reth_bytes = r_reth.unwrap().bytes.to_vec();
            assert_eq!(out_forward, reth_bytes, "forward <> reth bytes mismatch");
            assert_eq!(out_proving, reth_bytes, "proving <> reth bytes mismatch");
        }
        (false, false, false) => { }
        _ => {
            panic!(
                "status mismatch: reth_ok={reth_ok} fwd_ok={fwd_ok} prv_ok={prv_ok}, in_len={}",
                in_bytes.len()
            );
        }
    }
}

fuzz_target!(|data: &[u8]| {
    fuzz(data);
});