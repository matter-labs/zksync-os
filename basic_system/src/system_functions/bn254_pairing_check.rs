use super::*;
use crate::cost_constants::{
    BN254_PAIRING_BASE_NATIVE_COST, BN254_PAIRING_COST_PER_PAIR_GAS,
    BN254_PAIRING_PER_PAIR_NATIVE_COST, BN254_PAIRING_STATIC_COST_GAS,
};
use crypto::precompiles::bn254_pairing_check;
use zk_ee::common_traits::TryExtend;
use zk_ee::oracle::IOOracle;
use zk_ee::system::base_system_functions::{
    Bn254PairingCheckErrors, Bn254PairingCheckInterfaceError, SystemFunctionExt,
};
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::logger::Logger;
use zk_ee::{interface_error, out_of_return_memory};

///
/// bn254 pairing check system function implementation.
/// With `USE_ADVICE`, the field inversion of the final exponentiation is taken from an oracle
/// hint.
///
pub struct Bn254PairingCheckImpl<const USE_ADVICE: bool>;

impl<R: Resources, const USE_ADVICE: bool> SystemFunctionExt<R, Bn254PairingCheckErrors>
    for Bn254PairingCheckImpl<USE_ADVICE>
{
    /// Returns `OutOfGas` if not enough resources provided.
    /// Returns `InvalidInput` error if the input size is not divisible by 192
    /// or failed to create affine points from inputs
    fn execute<
        O: IOOracle,
        L: Logger,
        D: TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        src: &[u8],
        dst: &mut D,
        resources: &mut R,
        oracle: &mut O,
        _logger: &mut L,
        allocator: A,
    ) -> Result<(), SubsystemError<Bn254PairingCheckErrors>> {
        cycle_marker::wrap_with_resources!("bn254_pairing", resources, {
            let oracle = if USE_ADVICE { Some(oracle) } else { None };
            let num_pairs = src.len() / 192;
            let gas_cost = BN254_PAIRING_STATIC_COST_GAS
                + BN254_PAIRING_COST_PER_PAIR_GAS * (num_pairs as u64);
            // Pairing has a large fixed cost (final exponentiation) charged once
            // when there is any pairing work, plus a per-pair Miller-loop cost.
            let native_cost = if num_pairs == 0 {
                0
            } else {
                BN254_PAIRING_BASE_NATIVE_COST
                    + (num_pairs as u64) * BN254_PAIRING_PER_PAIR_NATIVE_COST
            };

            resources.charge_legacy_gas_and_native(gas_cost, native_cost)?;

            if !src.len().is_multiple_of(192) {
                return Err(interface_error!(
                    Bn254PairingCheckInterfaceError::InvalidPairingSize
                ));
            }

            let success = if src.is_empty() {
                true
            } else {
                bn254_pairing_check::<A, O>(num_pairs, src, allocator, oracle)
                    .map_err(|_| interface_error!(Bn254PairingCheckInterfaceError::InvalidPoint))?
            };

            dst.try_extend(core::iter::repeat_n(0, 31).chain(core::iter::once(success as u8)))
                .map_err(|_| out_of_return_memory!())?;

            Ok(())
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use callable_oracles::field_hints::NativeFieldOpsQuery;
    use oracle_provider::ZkEENonDeterminismSource;
    use zk_ee::reference_implementations::BaseResources;
    use zk_ee::reference_implementations::DecreasingNative;
    use zk_ee::system::logger::NullLogger;
    use zk_ee::system::Resource;

    #[test]
    fn test_pairing_external() {
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

        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(NativeFieldOpsQuery);
        Bn254PairingCheckImpl::<false>::execute(
            src,
            &mut dst,
            &mut resource,
            &mut oracle,
            &mut NullLogger,
            allocator,
        )
        .unwrap();

        assert_eq!(expected, dst.as_slice());

        let mut dst = vec![];

        Bn254PairingCheckImpl::<true>::execute(
            src,
            &mut dst,
            &mut resource,
            &mut oracle,
            &mut NullLogger,
            allocator,
        )
        .unwrap();

        assert_eq!(expected, dst.as_slice());
    }
}
