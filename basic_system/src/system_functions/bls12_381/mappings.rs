use zk_ee::system::{Ergs, Resources, SystemFunction};

use super::*;

pub const BLS12_381_FIELD_TO_G1_GAS: u64 = 5500;
pub const BLS12_381_FIELD_EXT_TO_G2_GAS: u64 = 23800;

/// Evaluate a polynomial (given as a coefficient slice, constant term first)
/// at `x` using Horner's method. Allocation-free.
fn evaluate_polynomial<F: crypto::ark_ff::Field>(coeffs: &[F], x: &F) -> F {
    let mut result = F::ZERO;
    for c in coeffs.iter().rev() {
        result *= x;
        result += c;
    }
    result
}

/// Apply an isogeny map without allocation. Equivalent to arkworks'
/// `IsogenyMap::apply` but avoids `DensePolynomial` (which uses `Vec`).
fn apply_isogeny_map_no_alloc<
    Domain: crypto::ark_ec::models::short_weierstrass::SWCurveConfig,
    Codomain: crypto::ark_ec::models::short_weierstrass::SWCurveConfig<BaseField = Domain::BaseField>,
>(
    map: &crypto::ark_ec::hashing::curve_maps::wb::IsogenyMap<'_, Domain, Codomain>,
    domain_point: crypto::ark_ec::short_weierstrass::Affine<Domain>,
) -> Result<
    crypto::ark_ec::short_weierstrass::Affine<Codomain>,
    crypto::ark_ec::hashing::HashToCurveError,
> {
    use crypto::ark_ec::AffineRepr;
    match domain_point.xy() {
        Some((x, y)) => {
            let x_num = evaluate_polynomial(map.x_map_numerator, &x);
            let x_den = evaluate_polynomial(map.x_map_denominator, &x);
            let y_num = evaluate_polynomial(map.y_map_numerator, &x);
            let y_den = evaluate_polynomial(map.y_map_denominator, &x);

            // Montgomery's trick: compute both inverses with a single inversion.
            use crypto::ark_ff::AdditiveGroup;
            use crypto::ark_ff::Field;
            let zero = Domain::BaseField::ZERO;
            let prod = x_den * y_den;
            let (x_den_inv, y_den_inv) = if let Some(prod_inv) = prod.inverse() {
                (y_den * prod_inv, x_den * prod_inv)
            } else {
                // At least one denominator is zero — fall back to individual inversions.
                (
                    x_den.inverse().unwrap_or(zero),
                    y_den.inverse().unwrap_or(zero),
                )
            };
            let img_x = x_num * x_den_inv;
            let img_y = (y_num * y) * y_den_inv;
            Ok(crypto::ark_ec::short_weierstrass::Affine::<Codomain>::new_unchecked(img_x, img_y))
        }
        None => Ok(crypto::ark_ec::short_weierstrass::Affine::identity()),
    }
}

/// Map a field element to a G1 curve point using SWU + isogeny, without
/// global allocation. Replaces `WBMap::map_to_curve` which internally uses
/// `DensePolynomial` (allocates via global allocator).
fn map_to_g1_no_alloc(element: Fq) -> Result<G1Affine, crypto::ark_ec::hashing::HashToCurveError> {
    use crypto::ark_ec::hashing::curve_maps::swu::SWUMap;
    use crypto::ark_ec::hashing::curve_maps::wb::WBConfig;
    use crypto::ark_ec::hashing::map_to_curve_hasher::MapToCurve;
    use crypto::bls12_381::curves::g1;

    let point_on_iso_curve =
        SWUMap::<<g1::Config as WBConfig>::IsogenousCurve>::map_to_curve(element)?;
    apply_isogeny_map_no_alloc(&g1::Config::ISOGENY_MAP, point_on_iso_curve)
}

/// Same as `map_to_g1_no_alloc` but for G2 (Fp2 → G2).
fn map_to_g2_no_alloc(element: Fq2) -> Result<G2Affine, crypto::ark_ec::hashing::HashToCurveError> {
    use crypto::ark_ec::hashing::curve_maps::swu::SWUMap;
    use crypto::ark_ec::hashing::curve_maps::wb::WBConfig;
    use crypto::ark_ec::hashing::map_to_curve_hasher::MapToCurve;
    use crypto::bls12_381::curves::g2;

    let point_on_iso_curve =
        SWUMap::<<g2::Config as WBConfig>::IsogenousCurve>::map_to_curve(element)?;
    apply_isogeny_map_no_alloc(&g2::Config::ISOGENY_MAP, point_on_iso_curve)
}

pub struct Bls12381G1MappingPrecompile;

impl<R: Resources> SystemFunction<R, Bls12PrecompileErrors> for Bls12381G1MappingPrecompile {
    fn execute<
        D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        _allocator: A,
    ) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
        cycle_marker::wrap_with_resources!("bls12_381_map_fp_to_g1", resources, {
            bls12_381_map_fp_to_g1_as_system_function_inner(input, output, resources)
        })
    }
}

fn bls12_381_map_fp_to_g1_as_system_function_inner<
    D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
    R: Resources,
>(
    input: &[u8],
    output: &mut D,
    resources: &mut R,
) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
    if input.len() == 0 {
        return Err(interface_error!(
            Bls12PrecompileInterfaceError::InvalidInputSize
        ));
    }
    let cost_ergs = Ergs(BLS12_381_FIELD_TO_G1_GAS * ERGS_PER_GAS);
    let cost_native = crate::cost_constants::BLS12_381_MAP_FP_TO_G1_NATIVE_COST;
    resources.charge(&R::from_ergs_and_native(
        cost_ergs,
        <R::Native as zk_ee::system::Computational>::from_computational(cost_native),
    ))?;
    if input.len() != FIELD_ELEMENT_SERIALIZATION_LEN {
        return Err(interface_error!(
            Bls12PrecompileInterfaceError::InvalidInputSize
        ));
    }

    let field_element = crypto::bls12_381::eip2537::parse_fq_bytes(input.try_into().unwrap())
        .ok_or_else(|| interface_error!(Bls12PrecompileInterfaceError::InvalidFieldElement))?;
    let Ok(result) = map_to_g1_no_alloc(field_element) else {
        return Err(interface_error!(
            Bls12PrecompileInterfaceError::InvalidFieldElement
        ));
    };
    let result: G1Affine = result;
    let result = result.clear_cofactor();

    write_g1(result, output)?;

    Ok(())
}

pub struct Bls12381G2MappingPrecompile;

impl<R: Resources> SystemFunction<R, Bls12PrecompileErrors> for Bls12381G2MappingPrecompile {
    fn execute<
        D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        _allocator: A,
    ) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
        cycle_marker::wrap_with_resources!("bls12_381_map_fp2_to_g2", resources, {
            bls12_381_map_fp2_to_g2_as_system_function_inner(input, output, resources)
        })
    }
}

fn bls12_381_map_fp2_to_g2_as_system_function_inner<
    D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
    R: Resources,
>(
    input: &[u8],
    output: &mut D,
    resources: &mut R,
) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
    if input.len() == 0 {
        return Err(interface_error!(
            Bls12PrecompileInterfaceError::InvalidInputSize
        ));
    }
    let cost_ergs = Ergs(BLS12_381_FIELD_EXT_TO_G2_GAS * ERGS_PER_GAS);
    let cost_native = crate::cost_constants::BLS12_381_MAP_FP2_TO_G2_NATIVE_COST;
    resources.charge(&R::from_ergs_and_native(
        cost_ergs,
        <R::Native as zk_ee::system::Computational>::from_computational(cost_native),
    ))?;
    if input.len() != FIELD_EXT_ELEMENT_SERIALIZATION_LEN {
        return Err(interface_error!(
            Bls12PrecompileInterfaceError::InvalidInputSize
        ));
    }

    let field_element = crypto::bls12_381::eip2537::parse_fq2_bytes(input.try_into().unwrap())
        .ok_or_else(|| interface_error!(Bls12PrecompileInterfaceError::InvalidFieldElement))?;

    let Ok(result) = map_to_g2_no_alloc(field_element) else {
        return Err(interface_error!(
            Bls12PrecompileInterfaceError::InvalidFieldElement
        ));
    };
    let result: G2Affine = result;
    let result = result.clear_cofactor();

    write_g2(result, output)?;

    Ok(())
}
