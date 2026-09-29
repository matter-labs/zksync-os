use super::*;

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
#[test]
fn test_geth_ecrecover() {
    let input = valid_input();

    let expected_pubkey: [u8; 32] =
        hex::decode("000000000000000000000000ceaccac640adf55b2028469bd36ba501f28b699d")
            .expect("should decode pubkey")
            .try_into()
            .unwrap();

    let (result, pubkey, _, _) = call::<false, _>(
        &input,
        TestResources::FORMAL_INFINITE,
        &mut honest_oracle(),
        None,
    );
    assert!(result.is_ok());
    assert_eq!(pubkey.len(), 32, "Size should be 32");
    assert_eq!(
        pubkey, expected_pubkey,
        "pubkey should be equal to reference"
    )
}

#[test]
fn test_empty_input() {
    let input = [0u8; 128];
    let (result, pubkey, _, _) = call::<false, _>(
        &input,
        TestResources::FORMAL_INFINITE,
        &mut honest_oracle(),
        None,
    );
    assert!(result.is_ok());
    assert_eq!(pubkey.len(), 0, "Size should be 0");
}

#[test]
fn test_point_of_infinity_in_result() {
    let input = infinity_input();

    let (result, pubkey, _, _) = call::<false, _>(
        &input,
        TestResources::FORMAL_INFINITE,
        &mut honest_oracle(),
        None,
    );
    assert!(result.is_ok());
    assert_eq!(pubkey.len(), 0, "Size should be 0 in case of error");
}

#[test]
fn test_affine_point_decompression_regression() {
    let input: [u8; 128] =
        hex::decode("00c547e4f7b0f325ad1e56f57e26c745b09a3e503d86e00e5255ff7f715d3d1c000000000000000000000000000000000000000000000000000000000000001c00b1693892219d736caba55bdb67216e485557ea6b6af75f37096c9aa6a5a75f00b940b1d03b21e36b0e47e79769f095fe2ab855bd91e3a38756b7d75a9c4549")
            .expect("should decode hex")
            .try_into()
            .unwrap();

    let (result, pubkey, _, _) = call::<false, _>(
        &input,
        TestResources::FORMAL_INFINITE,
        &mut honest_oracle(),
        None,
    );
    assert!(result.is_ok());
    assert_eq!(pubkey.len(), 0, "Size should be 0 in case of error");
}

#[test]
fn test_regressions() {
    let input: [u8; 128] = [
        34, 189, 7, 49, 212, 191, 250, 136, 64, 38, 37, 181, 186, 57, 224, 78, 233, 173, 214, 83,
        76, 49, 218, 108, 17, 157, 130, 90, 57, 130, 43, 41, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 28, 102, 102, 116, 99, 212, 10, 196,
        65, 102, 33, 136, 237, 62, 102, 50, 156, 33, 172, 161, 101, 19, 51, 146, 204, 26, 20, 184,
        68, 133, 96, 10, 135, 80, 135, 255, 193, 105, 5, 204, 108, 234, 239, 23, 70, 48, 206, 157,
        208, 196, 11, 63, 78, 148, 255, 0, 238, 54, 88, 166, 166, 127, 236, 38, 19,
    ];
    let expected_pubkey: [u8; 32] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 99, 249, 114, 95, 16, 115, 88, 201, 17, 91, 201, 216,
        108, 114, 221, 88, 35, 233, 177, 230,
    ];
    let (result, pubkey, _, _) = call::<false, _>(
        &input,
        TestResources::FORMAL_INFINITE,
        &mut honest_oracle(),
        None,
    );
    assert!(result.is_ok());
    assert_eq!(pubkey.len(), 32, "Size should be 32");
    assert_eq!(
        pubkey, expected_pubkey,
        "pubkey should be equal to reference"
    )
}

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
                None,
            )
        } else {
            ecrecover_as_system_function_inner::<NoOracle, _, _, _, false>(
                input.as_slice(),
                &mut output,
                &mut resources,
                None,
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
                let address = crate::system_functions::keccak256::keccak256_digest(&public_key.as_bytes()[1..]);
                prop_assert!(recovered[..12].iter().all(|byte| *byte == 0));
                prop_assert_eq!(&recovered[12..], &address[12..]);
            }
        }
    });
}
