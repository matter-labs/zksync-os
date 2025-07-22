use super::*;
use crate::cost_constants::{
    BN254_PAIRING_BASE_NATIVE_COST, BN254_PAIRING_COST_PER_PAIR_ERGS,
    BN254_PAIRING_PER_PAIR_NATIVE_COST, BN254_PAIRING_STATIC_COST_ERGS,
};
use crate::system_functions::bytereverse;
use alloc::vec::Vec;
use crypto::ark_ec::AffineRepr;
use crypto::ark_ff::Zero;
use crypto::ark_serialize::{CanonicalDeserialize, Valid};
use zk_ee::interface_error;
use zk_ee::system::base_system_functions::{
    Bn254PairingCheckErrors, Bn254PairingCheckInterfaceError, SystemFunction,
};
use zk_ee::system::errors::subsystem::SubsystemError;

///
/// bn254 pairing check system function implementation.
///
pub struct Bn254PairingCheckImpl;

impl<R: Resources> SystemFunction<R, Bn254PairingCheckErrors> for Bn254PairingCheckImpl {
    /// Returns `OutOfGas` if not enough resources provided.
    /// Returns `InvalidInput` error if the input size is not divisible by 192
    /// or failed to create affine points from inputs
    fn execute<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        src: &[u8],
        dst: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Bn254PairingCheckErrors>> {
        cycle_marker::wrap_with_resources!("bn254_pairing", resources, {
            let num_pairs = src.len() / 192;
            let ergs_cost = BN254_PAIRING_STATIC_COST_ERGS
                + BN254_PAIRING_COST_PER_PAIR_ERGS.times(num_pairs as u64);
            let native_cost = (num_pairs as u64) * BN254_PAIRING_PER_PAIR_NATIVE_COST
                + BN254_PAIRING_BASE_NATIVE_COST;

            resources.charge(&R::from_ergs_and_native(
                ergs_cost,
                <R::Native as zk_ee::system::Computational>::from_computational(native_cost),
            ))?;

            if src.len() % 192 != 0 {
                return Err(SubsystemError::LeafUsage(interface_error!(
                    Bn254PairingCheckInterfaceError::InvalidPairingSize
                )));
            }

            let success = if src.is_empty() {
                true
            } else {
                bn254_pairing_check_inner::<A>(num_pairs, src, allocator).map_err(|_| {
                    SubsystemError::LeafUsage(interface_error!(
                        Bn254PairingCheckInterfaceError::InvalidPoint
                    ))
                })?
            };

            dst.extend(core::iter::repeat_n(0, 31).chain(core::iter::once(success as u8)));

            Ok(())
        })
    }
}

fn bn254_pairing_check_inner<A: Allocator>(
    num_pairs: usize,
    src: &[u8],
    allocator: A,
) -> Result<bool, ()> {
    use crypto::ark_ec::pairing::Pairing;
    use crypto::ark_ff::{One, PrimeField};
    use crypto::bn254::curves::{Bn254, G1Affine, G2Affine};
    use crypto::bn254::fields::{Fq, Fq2};

    if num_pairs == 0 {
        return Ok(true);
    }

    let mut pairs = Vec::with_capacity_in(num_pairs, allocator);
    let mut src_iter = src.iter();

    for _ in 0..num_pairs {
        let mut buffer = [0u8; 192];
        for (dst, src) in buffer.iter_mut().zip(&mut src_iter) {
            *dst = *src;
        }
        let mut it = buffer.array_chunks::<32>();
        unsafe {
            let mut g1_x = *it.next().unwrap_unchecked();
            let mut g1_y = *it.next().unwrap_unchecked();

            // NOTE: Ethereum serialization is strange
            let mut g2_x_c1 = *it.next().unwrap_unchecked();
            let mut g2_x_c0 = *it.next().unwrap_unchecked();
            let mut g2_y_c1 = *it.next().unwrap_unchecked();
            let mut g2_y_c0 = *it.next().unwrap_unchecked();

            bytereverse(&mut g1_x);
            bytereverse(&mut g1_y);

            let g1_x =
                <Fq as PrimeField>::BigInt::deserialize_uncompressed(&g1_x[..]).map_err(|_| ())?;
            let g1_y =
                <Fq as PrimeField>::BigInt::deserialize_uncompressed(&g1_y[..]).map_err(|_| ())?;
            let g1_x = Fq::from_bigint(g1_x).ok_or(())?;
            let g1_y = Fq::from_bigint(g1_y).ok_or(())?;

            let g1_point = if g1_x.is_zero() && g1_y.is_zero() {
                G1Affine::zero()
            } else {
                let g1_point = G1Affine::new_unchecked(g1_x, g1_y);
                g1_point.check().map_err(|_| ())?;
                g1_point
            };

            bytereverse(&mut g2_x_c0);
            bytereverse(&mut g2_x_c1);
            bytereverse(&mut g2_y_c0);
            bytereverse(&mut g2_y_c1);

            let g2_x_c0 = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&g2_x_c0[..])
                .map_err(|_| ())?;
            let g2_x_c1 = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&g2_x_c1[..])
                .map_err(|_| ())?;
            let g2_x_c0 = Fq::from_bigint(g2_x_c0).ok_or(())?;
            let g2_x_c1 = Fq::from_bigint(g2_x_c1).ok_or(())?;

            let g2_x = Fq2::new(g2_x_c0, g2_x_c1);

            let g2_y_c0 = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&g2_y_c0[..])
                .map_err(|_| ())?;
            let g2_y_c1 = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&g2_y_c1[..])
                .map_err(|_| ())?;
            let g2_y_c0 = Fq::from_bigint(g2_y_c0).ok_or(())?;
            let g2_y_c1 = Fq::from_bigint(g2_y_c1).ok_or(())?;

            let g2_y = Fq2::new(g2_y_c0, g2_y_c1);

            let g2_point = if g2_x.is_zero() && g2_y.is_zero() {
                G2Affine::zero()
            } else {
                let g2_point = G2Affine::new_unchecked(g2_x, g2_y);
                g2_point.check().map_err(|_| ())?;
                g2_point
            };

            pairs.push((g1_point, g2_point));
        }
    }

    let g1_iter = pairs.iter().map(|(g1, _)| g1);
    let g2_iter = pairs.iter().map(|(_, g2)| g2);
    let result = Bn254::multi_pairing(g1_iter, g2_iter);
    let success = result.0.is_one();
    Ok(success)
}

#[cfg(test)]
mod test {
    use super::*;
    use zk_ee::reference_implementations::BaseResources;
    use zk_ee::reference_implementations::DecreasingNative;
    use zk_ee::system::Resource;

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_pairing_inner() {
        let allocator = std::alloc::Global;
        crypto::init_lib();

        let src = hex::decode(
            "\
            1c76476f4def4bb94541d57ebba1193381ffa7aa76ada664dd31c16024c43f59\
            3034dd2920f673e204fee2811c678745fc819b55d3e9d294e45c9b03a76aef41\
            209dd15ebff5d46c4bd888e51a93cf99a7329636c63514396b4a452003a35bf7\
            04bf11ca01483bfa8b34b43561848d28905960114c8ac04049af4b6315a41678\
            2bb8324af6cfc93537a2ad1a445cfd0ca2a71acd7ac41fadbf933c2a51be344d\
            120a2a4cf30c1bf9845f20c6fe39e07ea2cce61f0c9bb048165fe5e4de877550\
            111e129f1cf1097710d41c4ac70fcdfa5ba2023c6ff1cbeac322de49d1b6df7c\
            2032c61a830e3c17286de9462bf242fca2883585b93870a73853face6a6bf411\
            198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2\
            1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed\
            090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b\
            12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa",
        )
        .unwrap();

        assert!(bn254_pairing_check_inner(2, src.as_slice(), allocator).unwrap());
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_pairing_external() {
        crypto::init_lib();
        let allocator = std::alloc::Global;

        let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

        let src: &[u8] = &hex::decode(
            "\
            1c76476f4def4bb94541d57ebba1193381ffa7aa76ada664dd31c16024c43f59\
            3034dd2920f673e204fee2811c678745fc819b55d3e9d294e45c9b03a76aef41\
            209dd15ebff5d46c4bd888e51a93cf99a7329636c63514396b4a452003a35bf7\
            04bf11ca01483bfa8b34b43561848d28905960114c8ac04049af4b6315a41678\
            2bb8324af6cfc93537a2ad1a445cfd0ca2a71acd7ac41fadbf933c2a51be344d\
            120a2a4cf30c1bf9845f20c6fe39e07ea2cce61f0c9bb048165fe5e4de877550\
            111e129f1cf1097710d41c4ac70fcdfa5ba2023c6ff1cbeac322de49d1b6df7c\
            2032c61a830e3c17286de9462bf242fca2883585b93870a73853face6a6bf411\
            198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2\
            1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed\
            090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b\
            12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa",
        )
        .unwrap();

        let expected =
            hex::decode("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap();
        let mut dst = vec![];

        let _ = Bn254PairingCheckImpl::execute(src, &mut dst, &mut resource, allocator).unwrap();

        assert_eq!(expected, dst.as_slice());

        let expected =
            hex::decode("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap();
        let mut dst = vec![];

        let _ = Bn254PairingCheckImpl::execute(src, &mut dst, &mut resource, allocator).unwrap();

        assert_eq!(expected, dst.as_slice());
    }
}
