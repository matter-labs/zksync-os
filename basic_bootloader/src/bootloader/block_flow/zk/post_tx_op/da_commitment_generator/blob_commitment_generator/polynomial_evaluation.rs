use super::brp_roots_of_unity::BRP_ROOTS_OF_UNITY;
use super::*;
use arrayvec::ArrayVec;

/// Evaluate blob polynomial in the given point.
///
/// `data` is not the blob itself, but the bytes we encode into the blob. We
/// chunk `data` by 31 bytes and interpret each chunk as a BE blob element.
/// Trailing positions [K, N) are implicit zeros and are skipped:
/// they contribute nothing to the barycentric sum, so we never compute them.
///
/// Correctness when `x` happens to equal a root of unity `ω_j`:
/// - `j < K`: the K-element batch-inverse accumulator is zero; the fast-path
///   loop returns `poly[j]` directly.
/// - `j ≥ K`: the K-element accumulator is non-zero (none of
///   `(x − ω_0), …, (x − ω_{K-1})` is zero), so the regular path runs. The
///   trailing `(x^N − 1)` factor is then zero (because `x` is itself an
///   N-th root of unity), zeroing the result — which matches the correct
///   value `P(ω_j) = 0` since `poly[j] = 0` by construction.
pub fn evaluate_blob_polynomial(data: &[u8], x: &crypto::bls12_381::Fr) -> crypto::bls12_381::Fr {
    debug_assert!(data.len() <= ENCODABLE_BYTES_PER_BLOB);

    let mut poly: ArrayVec<crypto::bls12_381::Fr, ELEMENTS_PER_4844_BLOB> = ArrayVec::new();
    let chunks = data.as_chunks::<BLOB_CHUNK_SIZE>();
    let remainder = chunks.1;
    for chunk in chunks.0 {
        poly.push(crypto::bls12_381::Fr::from_bigint(parse_u256_be(chunk)).unwrap());
    }
    if !remainder.is_empty() {
        let mut last_chunk = [0u8; BLOB_CHUNK_SIZE];
        last_chunk[..remainder.len()].copy_from_slice(remainder);
        poly.push(crypto::bls12_381::Fr::from_bigint(parse_u256_be(&last_chunk)).unwrap());
    }
    let k = poly.len();

    // Barycentric Lagrange interpolation over the K non-zero coefficients.
    // Based on https://github.com/ethereum/c-kzg-4844/blob/8b59c2922d78ae792889452ece33a4054c60aab1/src/eip4844/eip4844.c#L192

    let mut inverses_in: ArrayVec<crypto::bls12_381::Fr, ELEMENTS_PER_4844_BLOB> = ArrayVec::new();
    for &root in &BRP_ROOTS_OF_UNITY[..k] {
        inverses_in.push(*x - root);
    }

    // Batch-invert the K denominators.
    let mut accumulator = crypto::bls12_381::Fr::one();
    let mut inverses: ArrayVec<crypto::bls12_381::Fr, ELEMENTS_PER_4844_BLOB> = ArrayVec::new();
    for i in 0..k {
        inverses.push(accumulator);
        accumulator *= inverses_in[i];
    }

    if accumulator.inverse_in_place().is_none() {
        // `x` coincides with one of ω_0, …, ω_{K-1}; return P(ω_i) directly.
        for i in 0..k {
            if *x == BRP_ROOTS_OF_UNITY[i] {
                return poly[i];
            }
        }
        unreachable!()
    }

    for i in (0..k).rev() {
        inverses[i] *= accumulator;
        accumulator *= inverses_in[i];
    }

    let mut out = crypto::bls12_381::Fr::zero();
    let mut tmp: crypto::bls12_381::Fr;
    for i in 0..k {
        tmp = inverses[i] * BRP_ROOTS_OF_UNITY[i];
        tmp *= poly[i];
        out += tmp;
    }

    tmp = crypto::bls12_381::Fr::from_bigint(parse_u256_be(
        &(ELEMENTS_PER_4844_BLOB as u64).to_be_bytes(),
    ))
    .unwrap();
    out /= tmp;
    tmp = x.pow([ELEMENTS_PER_4844_BLOB as u64]);
    tmp -= crypto::bls12_381::Fr::one();
    out *= tmp;

    out
}
