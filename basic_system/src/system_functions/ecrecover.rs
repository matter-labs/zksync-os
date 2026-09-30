use super::*;
use crate::cost_constants::{ECRECOVER_COST_GAS, ECRECOVER_NATIVE_COST};
use field_ops::Secp256k1HooksWithOracle;
use zk_ee::common_traits::TryExtend;
use zk_ee::oracle::IOOracle;
use zk_ee::out_of_return_memory;
use zk_ee::system::base_system_functions::Secp256k1ECRecoverErrors;
use zk_ee::system::errors::{subsystem::SubsystemError, system::SystemError};
use zk_ee::system::SystemFunctionExt;

///
/// ecrecover system function implementation.
///
pub struct EcRecoverImpl<const USE_ADVICE: bool>;

impl<R: Resources, const USE_ADVICE: bool> SystemFunctionExt<R, Secp256k1ECRecoverErrors>
    for EcRecoverImpl<USE_ADVICE>
{
    /// If the input size is less than expected - it will be padded with zeroes.
    /// If the input size is greater - redundant bytes will be ignored.
    /// If the input is invalid(v != 27|28 or failed to recover signer) returns `Ok(0)`.
    ///
    /// Returns `OutOfGas` if not enough resources provided.
    fn execute<O: IOOracle, L, D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        oracle: &mut O,
        _logger: &mut L,
        _allocator: A,
    ) -> Result<(), SubsystemError<Secp256k1ECRecoverErrors>> {
        Ok(cycle_marker::wrap_with_resources!(
            "ecrecover",
            resources,
            {
                ecrecover_as_system_function_inner::<_, _, _, _, USE_ADVICE>(
                    input,
                    output,
                    resources,
                    Some(oracle),
                )
            }
        )?)
    }
}

// if the oracle is provided, it will be used for field operations
fn ecrecover_as_system_function_inner<
    O: IOOracle,
    S: ?Sized + MinimalByteAddressableSlice,
    D: ?Sized + TryExtend<u8>,
    R: Resources,
    const USE_ADVICE: bool,
>(
    src: &S,
    dst: &mut D,
    resources: &mut R,
    oracle: Option<&mut O>,
) -> Result<(), SystemError> {
    resources.charge_legacy_gas_and_native(ECRECOVER_COST_GAS, ECRECOVER_NATIVE_COST)?;
    // digest, v, r, s in ABI
    let mut buffer = WordAligned([0u8; 128]);
    for (dst, src) in buffer.0.iter_mut().zip(src.iter()) {
        *dst = *src;
    }

    // follow https://github.com/ethereum/go-ethereum/blob/aadcb886753079d419f966a3bc990f708f8d1c3b/core/vm/contracts.go#L188

    let ([digest, v, r, s], []) = buffer.0.as_chunks::<32>() else {
        unreachable!("128 bytes are 4 chunks of 32")
    };

    if v[..31].iter().all(|el| *el == 0) == false {
        return Ok(());
    }

    let rec_id = v[31].wrapping_sub(27);
    if (rec_id == 0 || rec_id == 1) == false {
        return Ok(());
    }

    let oracle = if USE_ADVICE { oracle } else { None };

    let Ok(public_key) = ecrecover_inner(digest, r, s, rec_id == 1, oracle) else {
        return Ok(());
    };

    let address_hash = super::keccak256::keccak256_digest(&public_key.0);

    dst.try_extend(core::iter::repeat_n(0, 12).chain(address_hash.into_iter().skip(12)))
        .map_err(|_| out_of_return_memory!())?;

    Ok(())
}

/// Bytes at an address aligned for words: the conversions between big-endian bytes and integers
/// work by words then (see `crypto::bigint_delegation::u256::from_be_bytes`)
#[repr(C, align(4))]
pub struct WordAligned<const N: usize>(pub [u8; N]);

/// The public key (its coordinates `x` and `y`, big-endian) of the signature `(r, s)` of
/// `digest`, for the point of the signature with the coordinate `x = r` and the `y` of the
/// given parity. Fails if `r` or `s` is not in `[1, order - 1]`, or there is no such point or
/// key.
pub fn ecrecover_inner<O: IOOracle>(
    digest: &[u8; 32],
    r: &[u8; 32],
    s: &[u8; 32],
    y_is_odd: bool,
    oracle: Option<&mut O>,
) -> Result<WordAligned<64>, ()> {
    use crypto::secp256k1::{hooks::DefaultSecp256k1Hooks, recover_from_bytes_with_hooks};

    let public_key = match oracle {
        Some(oracle) => recover_from_bytes_with_hooks(
            digest,
            r,
            s,
            y_is_odd,
            &mut Secp256k1HooksWithOracle::new(oracle),
        ),
        None => recover_from_bytes_with_hooks(digest, r, s, y_is_odd, &mut DefaultSecp256k1Hooks),
    }
    .map_err(|_| ())?;

    // represent as bytes, and we do not need compression
    let mut encoded = WordAligned([0u8; 64]);
    let ([x, y], []) = encoded.0.as_chunks_mut::<32>() else {
        unreachable!("64 bytes are 2 chunks of 32")
    };
    public_key.write_coordinates(x, y);

    Ok(encoded)
}

#[cfg(test)]
mod test {
    use super::*;
    use zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
    use zk_ee::oracle::IOOracle;
    use zk_ee::system::errors::internal::InternalError;

    /// The paths without advice never query the oracle.
    enum NoOracle {}

    impl zk_ee::oracle::memory_io::MemoryOracle for NoOracle {}

    impl IOOracle for NoOracle {
        type RawIterator<'a> = core::iter::Empty<usize>;

        fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
            &'a mut self,
            _query_type: u32,
            _input: &I,
        ) -> Result<Self::RawIterator<'a>, InternalError> {
            match *self {}
        }
    }
    use hex;
    use zk_ee::reference_implementations::BaseResources;
    use zk_ee::reference_implementations::DecreasingNative;
    use zk_ee::system::Resource;

    #[test]
    fn test_geth_ecrecover() {
        let input: [u8; 128] =
            hex::decode("38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e000000000000000000000000000000000000000000000000000000000000001b38d18acb67d25c8bb9942764b62f18e17054f66a817bd4295423adf9ed98873e789d1dd423d25f0772d2748d60f7e4b81bb14d086eba8e8e8efb6dcff8a4ae02")
                .expect("should decode hex")
                .try_into()
                .unwrap();

        let expected_pubkey: [u8; 32] =
            hex::decode("000000000000000000000000ceaccac640adf55b2028469bd36ba501f28b699d")
                .expect("should decode pubkey")
                .try_into()
                .unwrap();

        let mut pubkey = vec![];

        let mut resources = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

        ecrecover_as_system_function_inner::<NoOracle, _, _, _, false>(
            input.as_slice(),
            &mut pubkey,
            &mut resources,
            None,
        )
        .expect("ecrecover");
        assert_eq!(pubkey.len(), 32, "Size should be 32");
        assert_eq!(
            pubkey, expected_pubkey,
            "pubkey should be equal to reference"
        )
    }

    #[test]
    fn test_empty_input() {
        let input = [0u8; 128];
        let mut pubkey = vec![];

        let mut resources = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

        ecrecover_as_system_function_inner::<NoOracle, _, _, _, false>(
            input.as_slice(),
            &mut pubkey,
            &mut resources,
            None,
        )
        .expect("ecrecover");
        assert_eq!(pubkey.len(), 0, "Size should be 0");
    }

    #[test]
    fn test_point_of_infinity_in_result() {
        let input: [u8; 128] =
            hex::decode("6b8d2c81b11b2d699528dde488dbdf2f94293d0d33c32e347f255fa4a6c1f0a9000000000000000000000000000000000000000000000000000000000000001b79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f817986b8d2c81b11b2d699528dde488dbdf2f94293d0d33c32e347f255fa4a6c1f0a9")
                .expect("should decode hex")
                .try_into()
                .unwrap();

        let mut pubkey = vec![];

        let mut resources = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

        ecrecover_as_system_function_inner::<NoOracle, _, _, _, false>(
            input.as_slice(),
            &mut pubkey,
            &mut resources,
            None,
        )
        .expect("ecrecover");
        assert_eq!(pubkey.len(), 0, "Size should be 0 in case of error");
    }

    #[test]
    fn test_affine_point_decompression_regression() {
        let input: [u8; 128] =
            hex::decode("00c547e4f7b0f325ad1e56f57e26c745b09a3e503d86e00e5255ff7f715d3d1c000000000000000000000000000000000000000000000000000000000000001c00b1693892219d736caba55bdb67216e485557ea6b6af75f37096c9aa6a5a75f00b940b1d03b21e36b0e47e79769f095fe2ab855bd91e3a38756b7d75a9c4549")
                .expect("should decode hex")
                .try_into()
                .unwrap();

        let mut pubkey = vec![];

        let mut resources = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

        ecrecover_as_system_function_inner::<NoOracle, _, _, _, false>(
            input.as_slice(),
            &mut pubkey,
            &mut resources,
            None,
        )
        .expect("ecrecover");
        assert_eq!(pubkey.len(), 0, "Size should be 0 in case of error");
    }

    #[test]
    fn test_regressions() {
        let input: [u8; 128] = [
            34, 189, 7, 49, 212, 191, 250, 136, 64, 38, 37, 181, 186, 57, 224, 78, 233, 173, 214,
            83, 76, 49, 218, 108, 17, 157, 130, 90, 57, 130, 43, 41, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 28, 102, 102, 116, 99,
            212, 10, 196, 65, 102, 33, 136, 237, 62, 102, 50, 156, 33, 172, 161, 101, 19, 51, 146,
            204, 26, 20, 184, 68, 133, 96, 10, 135, 80, 135, 255, 193, 105, 5, 204, 108, 234, 239,
            23, 70, 48, 206, 157, 208, 196, 11, 63, 78, 148, 255, 0, 238, 54, 88, 166, 166, 127,
            236, 38, 19,
        ];
        let mut pubkey = vec![];
        let mut resources = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

        let expected_pubkey: [u8; 32] = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 99, 249, 114, 95, 16, 115, 88, 201, 17, 91, 201,
            216, 108, 114, 221, 88, 35, 233, 177, 230,
        ];
        ecrecover_as_system_function_inner::<NoOracle, _, _, _, false>(
            input.as_slice(),
            &mut pubkey,
            &mut resources,
            None,
        )
        .expect("ecrecover");
        assert_eq!(pubkey.len(), 32, "Size should be 32");
        assert_eq!(
            pubkey, expected_pubkey,
            "pubkey should be equal to reference"
        )
    }

    /// With the advice of the oracle (affine arithmetic with hinted divisions) the result is the
    /// one without it, for signatures that recover a key and for the inputs that do not
    #[test]
    fn test_advice_matches_no_advice() {
        use callable_oracles::field_hints::NativeFieldOpsQuery;
        use oracle_provider::ZkEENonDeterminismSource;
        use proptest::{prop_assert, prop_assert_eq, proptest};

        fn run(input: &[u8; 128], advice: bool) -> Vec<u8> {
            let mut output = vec![];
            let mut resources = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;
            if advice {
                let mut oracle = ZkEENonDeterminismSource::default();
                oracle.add_external_processor(NativeFieldOpsQuery);
                ecrecover_as_system_function_inner::<_, _, _, _, true>(
                    input.as_slice(),
                    &mut output,
                    &mut resources,
                    Some(&mut oracle),
                )
            } else {
                ecrecover_as_system_function_inner::<NoOracle, _, _, _, false>(
                    input.as_slice(),
                    &mut output,
                    &mut resources,
                    None,
                )
            }
            .expect("ecrecover");
            output
        }

        proptest!(|(digest: [u8; 32], key: [u8; 32], r: [u8; 32], s: [u8; 32], odd: bool)| {
            let mut input = [0u8; 128];
            input[..32].copy_from_slice(&digest);

            // any bytes: mostly no point of the curve, or a key nobody signed with
            input[63] = 27 + u8::from(odd);
            input[64..96].copy_from_slice(&r);
            input[96..].copy_from_slice(&s);
            prop_assert_eq!(run(&input, true), run(&input, false));

            if let Ok(key) = crypto::k256::ecdsa::SigningKey::from_bytes(&key.into()) {
                let (signature, recovery_id) = key.sign_prehash_recoverable(&digest).unwrap();
                if !recovery_id.is_x_reduced() {
                    input[63] = 27 + u8::from(recovery_id.is_y_odd());
                    input[64..].copy_from_slice(&signature.to_bytes());
                    let recovered = run(&input, true);
                    prop_assert_eq!(&recovered, &run(&input, false));

                    let public_key = key.verifying_key().to_encoded_point(false);
                    let address = super::super::keccak256::keccak256_digest(&public_key.as_bytes()[1..]);
                    prop_assert!(recovered[..12].iter().all(|byte| *byte == 0));
                    prop_assert_eq!(&recovered[12..], &address[12..]);
                }
            }
        });
    }
}
