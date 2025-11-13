use super::brp_roots_of_unity::BRP_ROOTS_OF_UNITY;
use super::*;
use core::mem::MaybeUninit;

///
/// Evaluate blob polynomial in the given point.
///
/// Please note, that `data` is not the blob itself, but data we encode into the blob.
/// For encoding, we chunk `data` by 31 bytes and interpret each chunk as BE blob element.
///
pub fn evaluate_blob_polynomial(data: &[u8], x: &crypto::bls12_381::Fr) -> crypto::bls12_381::Fr {
    debug_assert!(data.len() <= ENCODABLE_BYTES_PER_BLOB);

    // encode data into polynomial (in evaluated form)
    let mut poly: [MaybeUninit<crypto::bls12_381::Fr>; ELEMENTS_PER_4844_BLOB] =
        [MaybeUninit::uninit(); ELEMENTS_PER_4844_BLOB];
    let mut poly_iter = poly.iter_mut();

    let chunks = data.array_chunks::<BLOB_CHUNK_SIZE>();
    let remainder = chunks.remainder();
    for chunk in chunks {
        poly_iter
            .next()
            .unwrap()
            .write(crypto::bls12_381::Fr::from_bigint(parse_u256_be(chunk)).unwrap());
    }
    if let Some(el) = poly_iter.next() {
        let mut last_chunk = [0u8; 31];
        last_chunk[..remainder.len()].copy_from_slice(remainder);
        el.write(crypto::bls12_381::Fr::from_bigint(parse_u256_be(&last_chunk)).unwrap());
    }
    for el in poly_iter {
        el.write(crypto::bls12_381::Fr::zero());
    }
    let poly = unsafe { MaybeUninit::array_assume_init(poly) };

    // barycentric Lagrange interpolation evaluation
    // based on https://github.com/ethereum/c-kzg-4844/blob/8b59c2922d78ae792889452ece33a4054c60aab1/src/eip4844/eip4844.c#L192

    let inverses_in: [crypto::bls12_381::Fr; ELEMENTS_PER_4844_BLOB] =
        core::array::from_fn(|i| *x - BRP_ROOTS_OF_UNITY[i]);

    // batch inverse

    let mut accumulator = crypto::bls12_381::Fr::one();
    let mut inverses: [crypto::bls12_381::Fr; ELEMENTS_PER_4844_BLOB] = core::array::from_fn(|i| {
        let inverse = accumulator;
        accumulator *= inverses_in[i];
        inverse
    });

    if accumulator.inverse_in_place().is_none() {
        // It's `None` when accumulator equals to zero, it means that point to evaluate at is
        // one of the evaluation points by which the polynomial is given, we can just return
        // the result directly.
        for i in 0..ELEMENTS_PER_4844_BLOB {
            if *x == BRP_ROOTS_OF_UNITY[i] {
                return poly[i];
            }
        }
        unreachable!()
    }

    for i in (0..ELEMENTS_PER_4844_BLOB).rev() {
        inverses[i] *= accumulator;
        accumulator *= inverses_in[i];
    }

    let mut out = crypto::bls12_381::Fr::zero();
    let mut tmp: crypto::bls12_381::Fr;
    for i in 0..ELEMENTS_PER_4844_BLOB {
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
