use crate::k256::{
    elliptic_curve::{
        bigint::{CheckedAdd, U256},
        Curve, FieldBytesEncoding,
    },
    Secp256k1,
};

use super::{
    context::{
        ECMultContext, ECMULT_TABLE_SIZE_A, ECMULT_TABLE_SIZE_G, WINDOW_A, WINDOW_G, WNAF_BITS,
    },
    field::FieldElement,
    points::{Affine, AffineStorage, Jacobian},
    scalars::Scalar,
    Secp256k1Err,
};

#[cfg(feature = "secp256k1-static-context")]
pub fn recover(
    message: &crate::k256::Scalar,
    signature: &crate::k256::ecdsa::Signature,
    recovery_id: &crate::k256::ecdsa::RecoveryId,
) -> Result<Affine, Secp256k1Err> {
    use super::context::ECRECOVER_CONTEXT;

    recover_with_context(message, signature, recovery_id, &ECRECOVER_CONTEXT)
}

pub fn recover_with_context(
    message: &crate::k256::Scalar,
    signature: &crate::k256::ecdsa::Signature,
    recovery_id: &crate::k256::ecdsa::RecoveryId,
    context: &ECMultContext,
) -> Result<Affine, Secp256k1Err> {
    let (mut sigr, mut sigs) = Scalar::from_signature(signature);
    let message = Scalar::from_k256_scalar(*message);

    // We go through bytes because it's mod GROUP_ORDER and later we need mod BASE FIELD
    let mut brx = sigr.to_repr();

    if recovery_id.is_x_reduced() {
        match <U256 as FieldBytesEncoding<Secp256k1>>::decode_field_bytes(&brx)
            .checked_add(&Secp256k1::ORDER)
            .into_option()
        {
            Some(restored) => {
                brx = <U256 as FieldBytesEncoding<Secp256k1>>::encode_field_bytes(&restored);
            }
            None => return Err(Secp256k1Err::OperationOverflow),
        }
    }

    let is_odd = recovery_id.is_y_odd();
    let x = Affine::decompress(&brx, is_odd).ok_or(Secp256k1Err::InvalidParams)?;

    let xj = x.to_jacobian();

    sigr.invert_in_place();
    sigs *= sigr;

    sigr *= message;
    sigr.negate_in_place();

    let mut pk = ecmult(&xj, &sigs, &sigr, context).to_affine();
    pk.normalize_in_place();

    if pk.is_infinity() {
        return Err(Secp256k1Err::RecoveredInfinity);
    }

    Ok(pk)
}

/// Compute na*a+ng*g where g is the generator.
/// Algorithm adapted from https://github.com/bitcoin-core/secp256k1/blob/master/src/ecmult_impl.h#L237
fn ecmult(a: &Jacobian, na: &Scalar, ng: &Scalar, context: &ECMultContext) -> Jacobian {
    let mut z = FieldElement::ONE;

    let mut prea: [Affine; ECMULT_TABLE_SIZE_A] = [Affine::DEFAULT; ECMULT_TABLE_SIZE_A];
    let mut aux: [FieldElement; ECMULT_TABLE_SIZE_A] = [FieldElement::ZERO; ECMULT_TABLE_SIZE_A];

    let mut bits_na_1 = 0;
    let mut bits_na_lam = 0;

    let mut bits_ng_1 = 0;
    let mut bits_ng_128 = 0;

    let mut wnaf_na_1 = [0i32; WNAF_BITS];
    let mut wnaf_na_lam = [0i32; WNAF_BITS];

    let mut wnaf_ng_1 = [0i32; WNAF_BITS];
    let mut wnaf_ng_128 = [0i32; WNAF_BITS];

    let mut bits = 0;

    if !na.is_zero() && !a.is_infinity() {
        // split na into 129 bit scalars
        // where na_1 + na_lam * lambda = na

        // NOTE: this action moves integer representation to the normal form
        let (na_1, na_lam) = na.decompose();

        // build wnaf representation
        bits_na_1 = wnaf(&mut wnaf_na_1, &na_1, WINDOW_A);
        bits_na_lam = wnaf(&mut wnaf_na_lam, &na_lam, WINDOW_A);

        debug_assert!(bits_na_1 <= WNAF_BITS as i32);
        debug_assert!(bits_na_lam <= WNAF_BITS as i32);

        if bits_na_1 > bits_na_lam {
            bits = bits_na_1;
        } else {
            bits = bits_na_lam;
        }

        // Calculate odd multiples of a.
        // All multiples are brought to the same `z` "denominator".
        // Due to secp256k1 we can pretend the z coordinate is 1 and use affine addition formulas,
        // and correct the result at the end
        odd_multiples_table_windowa(&mut prea, &mut aux, &mut z, a);
        table_set_globalz_windowa(&mut prea, &aux);

        for i in 0..ECMULT_TABLE_SIZE_A {
            aux[i] = FieldElement::BETA;
            aux[i] *= prea[i].x;
        }
    }

    if !ng.is_zero() {
        // TODO: use u128 instead

        // split ng into ~128 bit scalars.
        // NOTE: it's NOT endomorphism decomposition, so it's 128 bits exact decomposition
        // where ng_1 + ng_128*2^128 = ng
        let (ng_1, ng_128) = ng.decompose_128(); // NOTE: must return normal form

        // build wnaf representation
        bits_ng_1 = wnaf(&mut wnaf_ng_1, &ng_1, WINDOW_G);
        bits_ng_128 = wnaf(&mut wnaf_ng_128, &ng_128, WINDOW_G);

        if bits_ng_1 > bits {
            bits = bits_ng_1;
        }
        if bits_ng_128 > bits {
            bits = bits_ng_128;
        }
    }

    let mut r = Jacobian::INFINITY;

    for i in (0..bits).rev() {
        r.double_in_place(None);

        let n = wnaf_na_1[i as usize];
        if i < bits_na_1 && n != 0 {
            r.add_ge_in_place(table_get_ge(&prea, n, WINDOW_A), None);
        }

        let n = wnaf_na_lam[i as usize];
        if i < bits_na_lam && n != 0 {
            r.add_ge_in_place(table_get_ge_lambda(&prea, &aux, n, WINDOW_A), None);
        }

        let n = wnaf_ng_1[i as usize];
        if i < bits_ng_1 && n != 0 {
            r.add_zinv_in_place(table_get_ge_storage(&context.pre_g, n, WINDOW_G), &z);
        }

        let n = wnaf_ng_128[i as usize];
        if i < bits_ng_128 && n != 0 {
            r.add_zinv_in_place(table_get_ge_storage(&context.pre_g_128, n, WINDOW_G), &z);
        }
    }

    if !r.is_infinity() {
        r.z *= z
    }

    r
}

/// Fill `pre_a` with odd multiples of a.
/// Although pre_a is an array of affine points, it actually represents elements in jacobian coordinates
/// with their z coordinate omitted. The omitted z-coordinate can be recovered with `zr` and `z`.
/// Using `b.z` to denote the omitted z-coordinate of b:
/// - `pre_a[n-1].z = z`
/// - `pre_a[i-1].z = pre_a[i].z / zr[i]` for `n > i > 0`
///
/// Lastly, `zr[0]` is set so that `a.z = pre_a[0].z / zr[0]`
/// Based on https://github.com/bitcoin-core/secp256k1/blob/master/src/ecmult_impl.h#L73
fn odd_multiples_table_windowa(
    pre_a: &mut [Affine; ECMULT_TABLE_SIZE_A],
    zr: &mut [FieldElement; ECMULT_TABLE_SIZE_A],
    z: &mut FieldElement,
    a: &Jacobian,
) {
    debug_assert!(!a.is_infinity());

    let mut d = *a;
    d.double_in_place(None);

    // we perform additions using an isomorphic curve Y^2 = X^3 + 7*C^6 where  C := d.z
    // The isomorphism, phi, is given by (x,y) -> (x*C^2, y*C^3).
    // In Jacobian coordinates, phi is given by (x, y, z) -> (x*C^2, y*C^3, z) = (x, y, z/C)
    // So
    //      d_ge = phi(d) = (d.x, d.y, 1)
    //      ai = phi(a) = (a.x*C^2, a.y*C^3, a.z)
    // This lets us use the faster add_ge_var

    let d_ge = Affine {
        x: d.x,
        y: d.y,
        infinity: false,
    };

    pre_a[0].set_gej_zinv(a, &d.z);

    let mut ai = Jacobian {
        x: pre_a[0].x,
        y: pre_a[0].y,
        z: a.z,
    };

    // pre_a[0] is the point (a.x*C^2, a.y*C^3, a.z*C) which is equivalent to a.
    // Set zr[0] to C, which is the ratio between the omitted z(pre_a[0]) value and a.z.
    zr[0] = d.z;

    for i in 1..ECMULT_TABLE_SIZE_A {
        ai.add_ge_in_place(d_ge, Some(&mut zr[i]));
        pre_a[i] = Affine {
            x: ai.x,
            y: ai.y,
            infinity: false,
        };
    }

    // Multiply the last z-coordinate by C to undo the isomorphism.
    // Since the z-coordinates of the pre_a values are implied by the zr array of z-coordinate ratios,
    // undoing the isomorphism here undoes the isomorphism for all pre_a values.
    *z = ai.z;
    *z *= d.z;
}

fn table_set_globalz_windowa(
    pre_a: &mut [Affine; ECMULT_TABLE_SIZE_A],
    zr: &[FieldElement; ECMULT_TABLE_SIZE_A],
) {
    let mut i = ECMULT_TABLE_SIZE_A - 1;

    pre_a[i].y.normalize_in_place();

    let mut zs = zr[i];

    i -= 1;

    let mut ai = pre_a[i];
    pre_a[i].set_ge_zinv(&ai, &zs);

    while i > 0 {
        zs *= zr[i];
        i -= 1;

        ai = pre_a[i];
        pre_a[i].set_ge_zinv(&ai, &zs);
    }
}

/// Convert a scalar to a wnaf representation,
/// i.e. `a=sum(2^i * wnaf[i])`, with te following guarantees:
///     - each `wnaf[i]` is either 0, or an odd integer between `-(1<<(w-1) - 1)` and `1<<(w-1) - 1`
///     - two non-zero entries are separated by at least `w-1` zeros
///     - the number of set values in wnaf is returned
///
/// NOTE: the function assumes that `wnaf` is zeroed
fn wnaf(wnaf: &mut [i32], s: &Scalar, w: usize) -> i32 {
    debug_assert!(wnaf.len() <= 256);
    debug_assert!((2..=31).contains(&w));
    debug_assert!(wnaf.iter().all(|&x| x == 0));

    let mut s = *s;

    let mut last_set_bit: i32 = -1;
    let mut bit = 0;
    let mut sign = 1;
    let mut carry = 0;

    if s.bits(255, 1) > 0 {
        // Negation is the same in any form
        s.negate_in_place();
        sign = -1;
    }

    while bit < wnaf.len() {
        if s.bits(bit, 1) == carry as u32 {
            bit += 1;
            continue;
        }

        let mut now = w;
        if now > wnaf.len() - bit {
            now = wnaf.len() - bit;
        }

        let mut word = (s.bits_var(bit, now) as i32) + carry;

        carry = (word >> (w - 1)) & 1;
        word -= carry << w;

        wnaf[bit] = sign * word;
        last_set_bit = bit as i32;

        bit += now;
    }
    debug_assert_eq!(carry, 0);
    debug_assert!({
        let mut t = true;
        while bit < 256 {
            t = t && (s.bits(bit, 1) == 0);
            bit += 1;
        }
        t
    });

    last_set_bit + 1
}

fn table_get_ge(pre: &[Affine], n: i32, w: usize) -> Affine {
    debug_assert!(table_verify(n, w));

    if n > 0 {
        pre[(n - 1) as usize / 2]
    } else {
        let mut r = pre[(-n - 1) as usize / 2];
        r.y.negate_in_place(1);
        r
    }
}

fn table_get_ge_lambda(pre: &[Affine], aux: &[FieldElement], n: i32, w: usize) -> Affine {
    debug_assert!(table_verify(n, w));

    if n > 0 {
        Affine {
            x: aux[(n - 1) as usize / 2],
            y: pre[(n - 1) as usize / 2].y,
            infinity: false,
        }
    } else {
        let mut y = pre[(-n - 1) as usize / 2].y;
        y.negate_in_place(1);

        Affine {
            x: aux[(-n - 1) as usize / 2],
            y,
            infinity: false,
        }
    }
}

fn table_get_ge_storage(pre: &[AffineStorage; ECMULT_TABLE_SIZE_G], n: i32, w: usize) -> Affine {
    debug_assert!(table_verify(n, w));

    if n > 0 {
        pre[(n - 1) as usize / 2].to_affine()
    } else {
        let mut r = pre[(-n - 1) as usize / 2].to_affine();
        r.y.negate_in_place(1);
        r
    }
}

fn table_verify(n: i32, w: usize) -> bool {
    let n = n as usize;

    ((n & 1) == 1) || (n >= !(1 << ((w - 1) - 1))) || (n <= (1 << ((w - 1) - 1)))
}

#[cfg(test)]
mod tests {
    use super::ecmult;
    use crate::secp256k1::scalars::Scalar;
    use crate::secp256k1::{context::ECRECOVER_CONTEXT, test_vectors::MUL_TEST_VECTORS};
    use crate::secp256k1::{
        field::FieldElement,
        points::{Affine, Jacobian},
    };

    use proptest::{prop_assert_eq, proptest};

    #[cfg(feature = "secp256k1-static-context")]
    #[test]
    fn ecmult_0I_0G() {
        #[cfg(feature = "bigint_ops")]
        crate::secp256k1::init();

        assert_eq!(ECRECOVER_CONTEXT.pre_g[0].to_affine(), Affine::GENERATOR);

        // 0*infinity + 0*G = 0
        let res = ecmult(
            &Jacobian::INFINITY,
            &Scalar::ZERO,
            &Scalar::ZERO,
            &ECRECOVER_CONTEXT,
        )
        .to_affine();

        assert!(res.is_infinity());
    }

    #[cfg(feature = "secp256k1-static-context")]
    #[test]
    fn ecmult_0I_1G() {
        #[cfg(feature = "bigint_ops")]
        crate::secp256k1::init();

        // 0*infinity + 1*G = G
        let mut res = ecmult(
            &Jacobian::INFINITY,
            &Scalar::ZERO,
            &Scalar::ONE,
            &ECRECOVER_CONTEXT,
        )
        .to_affine();

        res.normalize_in_place();

        assert_eq!(res, Affine::GENERATOR);
    }

    #[cfg(feature = "secp256k1-static-context")]
    #[test]
    fn ecmult_0I_3G() {
        #[cfg(feature = "bigint_ops")]
        crate::secp256k1::init();

        // 0*infinity + 3*G = 3*G
        let res = ecmult(
            &Jacobian::INFINITY,
            &Scalar::ZERO,
            &Scalar::from_u128(3),
            &ECRECOVER_CONTEXT,
        )
        .to_affine();

        assert_eq!(res, ECRECOVER_CONTEXT.pre_g[1].to_affine())
    }

    #[cfg(feature = "secp256k1-static-context")]
    #[test]
    fn ecmult_0I_5G() {
        #[cfg(feature = "bigint_ops")]
        crate::secp256k1::init();

        let res = ecmult(
            &Jacobian::INFINITY,
            &Scalar::ZERO,
            &Scalar::from_u128(5),
            &ECRECOVER_CONTEXT,
        )
        .to_affine();

        assert_eq!(res, ECRECOVER_CONTEXT.pre_g[2].to_affine());
    }

    #[cfg(feature = "secp256k1-static-context")]
    #[test]
    fn ecmult_0I_8G() {
        #[cfg(feature = "bigint_ops")]
        crate::secp256k1::init();

        // t = 5G + 3G
        let mut t = ECRECOVER_CONTEXT.pre_g[2].to_affine().to_jacobian();
        t.add_ge_in_place(ECRECOVER_CONTEXT.pre_g[1].to_affine(), None);

        let t = t.to_affine();

        // 0*infinity + 8*G = 8*G
        let res = ecmult(
            &Jacobian::INFINITY,
            &Scalar::ZERO,
            &Scalar::from_u128(8),
            &ECRECOVER_CONTEXT,
        )
        .to_affine();

        assert_eq!(res, t);
    }

    #[cfg(feature = "secp256k1-static-context")]
    #[test]
    fn ecmult_1G_0G() {
        #[cfg(feature = "bigint_ops")]
        crate::secp256k1::init();

        let res = ecmult(
            &Affine::GENERATOR.to_jacobian(),
            &Scalar::ONE,
            &Scalar::ZERO,
            &ECRECOVER_CONTEXT,
        )
        .to_affine();

        assert_eq!(res, Affine::GENERATOR);
    }

    #[cfg(feature = "secp256k1-static-context")]
    #[test]
    fn ecmult_1G_1G() {
        #[cfg(feature = "bigint_ops")]
        crate::secp256k1::init();

        let res = ecmult(
            &Affine::GENERATOR.to_jacobian(),
            &Scalar::ONE,
            &Scalar::ONE,
            &ECRECOVER_CONTEXT,
        )
        .to_affine();

        let mut expected = Affine::GENERATOR.to_jacobian();
        expected.double_in_place(None);

        assert_eq!(res, expected.to_affine())
    }

    #[cfg(feature = "secp256k1-static-context")]
    #[test]
    fn ecmult_1G_2G() {
        #[cfg(feature = "bigint_ops")]
        crate::secp256k1::init();

        let res = ecmult(
            &Affine::GENERATOR.to_jacobian(),
            &Scalar::ONE,
            &Scalar::from_u128(2),
            &ECRECOVER_CONTEXT,
        )
        .to_affine();

        assert_eq!(res, ECRECOVER_CONTEXT.pre_g[1].to_affine());
    }

    #[cfg(feature = "secp256k1-static-context")]
    #[test]
    fn ecmult_2G_1G() {
        #[cfg(feature = "bigint_ops")]
        crate::secp256k1::init();

        let mut res = ecmult(
            &Affine::GENERATOR.to_jacobian(),
            &Scalar::from_u128(2),
            &Scalar::ONE,
            &ECRECOVER_CONTEXT,
        )
        .to_affine();

        res.normalize_in_place();

        assert_eq!(res, ECRECOVER_CONTEXT.pre_g[1].to_affine());
    }

    #[cfg(feature = "secp256k1-static-context")]
    #[test]
    fn compare_ecmult() {
        proptest!(|(k: Scalar)| {
            let res1 = ecmult(
                &Jacobian::INFINITY,
                &Scalar::ZERO,
                &k,
                &ECRECOVER_CONTEXT
            ).to_affine();

            let res2 = ecmult(
                &Affine::GENERATOR.to_jacobian(),
                &k,
                &Scalar::ZERO,
                &ECRECOVER_CONTEXT
            ).to_affine();

            prop_assert_eq!(res1, res2);
        })
    }

    #[cfg(feature = "secp256k1-static-context")]
    #[test]
    fn test_generator_multipules() {
        for (k, x, y) in MUL_TEST_VECTORS {
            let k = Scalar::from_repr(k.clone().into());

            let computed_ctx =
                ecmult(&Jacobian::INFINITY, &Scalar::ZERO, &k, &ECRECOVER_CONTEXT).to_affine();

            let computed = ecmult(
                &Affine::GENERATOR.to_jacobian(),
                &k,
                &Scalar::ZERO,
                &ECRECOVER_CONTEXT,
            )
            .to_affine();

            let expected = Affine {
                x: FieldElement::from_bytes_unchecked(x),
                y: FieldElement::from_bytes_unchecked(y),
                infinity: false,
            };

            assert_eq!(computed_ctx, computed);
            assert_eq!(computed_ctx, expected);
        }
    }

    #[cfg(feature = "secp256k1-static-context")]
    #[test]
    fn test_regressions() {
        #[cfg(feature = "bigint_ops")]
        crate::secp256k1::init();

        // recover point at infinity
        assert_eq!(
            recover_from_digest(
                [
                    107, 141, 44, 129, 177, 27, 45, 105, 149, 40, 221, 228, 136, 219, 223, 47, 148,
                    41, 61, 13, 51, 195, 46, 52, 127, 37, 95, 164, 166, 193, 240, 169
                ],
                [
                    121, 190, 102, 126, 249, 220, 187, 172, 85, 160, 98, 149, 206, 135, 11, 7, 2,
                    155, 252, 219, 45, 206, 40, 217, 89, 242, 129, 91, 22, 248, 23, 152
                ],
                [
                    107, 141, 44, 129, 177, 27, 45, 105, 149, 40, 221, 228, 136, 219, 223, 47, 148,
                    41, 61, 13, 51, 195, 46, 52, 127, 37, 95, 164, 166, 193, 240, 169
                ],
                0
            )
            .unwrap_err(),
            super::Secp256k1Err::RecoveredInfinity
        );

        // geth
        assert_eq!(
            recover_from_digest(
                [
                    56, 209, 138, 203, 103, 210, 92, 139, 185, 148, 39, 100, 182, 47, 24, 225, 112,
                    84, 246, 106, 129, 123, 212, 41, 84, 35, 173, 249, 237, 152, 135, 62
                ],
                [
                    56, 209, 138, 203, 103, 210, 92, 139, 185, 148, 39, 100, 182, 47, 24, 225, 112,
                    84, 246, 106, 129, 123, 212, 41, 84, 35, 173, 249, 237, 152, 135, 62
                ],
                [
                    120, 157, 29, 212, 35, 210, 95, 7, 114, 210, 116, 141, 96, 247, 228, 184, 27,
                    177, 77, 8, 110, 186, 142, 142, 142, 251, 109, 207, 248, 164, 174, 2
                ],
                0
            ),
            Ok(Affine {
                x: FieldElement::from_bytes_unchecked(&[
                    134, 18, 84, 164, 207, 141, 253, 45, 96, 226, 163, 62, 49, 67, 234, 198, 40,
                    88, 134, 240, 174, 217, 23, 17, 171, 126, 44, 1, 63, 38, 92, 85
                ]),
                y: FieldElement::from_bytes_unchecked(&[
                    253, 69, 102, 103, 243, 176, 134, 87, 121, 95, 230, 117, 75, 111, 188, 17, 24,
                    103, 20, 196, 228, 244, 141, 91, 133, 104, 0, 227, 232, 28, 48, 200
                ]),
                infinity: false
            })
        )
    }

    #[cfg(feature = "secp256k1-static-context")]
    fn recover_from_digest(
        digest: [u8; 32],
        r: [u8; 32],
        s: [u8; 32],
        rec_id: u8,
    ) -> Result<Affine, super::Secp256k1Err> {
        use k256::ecdsa::{RecoveryId, Signature};
        use {
            k256::elliptic_curve::ops::Reduce,
            k256::{ecdsa::hazmat::bits2field, Scalar},
        };

        let signature = Signature::from_scalars(r, s).unwrap();
        let recovery_id = RecoveryId::try_from(rec_id).unwrap();

        let message = <Scalar as Reduce<k256::U256>>::reduce_bytes(
            &bits2field::<k256::Secp256k1>(&digest).unwrap(),
        );

        super::recover(&message, &signature, &recovery_id)
    }
}
