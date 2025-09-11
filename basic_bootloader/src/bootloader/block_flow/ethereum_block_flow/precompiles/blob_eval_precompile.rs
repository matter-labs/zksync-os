use alloc::boxed::Box;
use core::fmt::Write;
use crypto::ark_ec::pairing::Pairing;
use crypto::ark_ec::AffineRepr;
use crypto::ark_ff::{Field, PrimeField};
use evm_interpreter::ERGS_PER_GAS;
use system_hooks::make_error_return_state;
use system_hooks::make_return_state_from_returndata_region;
use system_hooks::HooksStorage;
use system_hooks::{
    addresses_constants::POINT_EVAL_HOOK_ADDRESS_LOW, StatefulImmutableSystemHook,
    StatefulImmutableSystemHookImpl,
};
use zk_ee::define_subsystem;
use zk_ee::interface_error;
use zk_ee::internal_error;
use zk_ee::memory::slice_vec::SliceVec;
use zk_ee::system::errors::root_cause::GetRootCause;
use zk_ee::system::errors::root_cause::RootCause;
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::*;
use zk_ee::system::{errors::internal::InternalError, System, SystemTypes};
use zk_ee::utils::cheap_clone::CheapCloneRiscV;

pub const TRUSTED_SETUP_TAU_G2_BYTES: [u8; 96] = const {
    let Ok(res) = const_hex::const_decode_to_array(
        b"b5bfd7dd8cdeb128843bc287230af38926187075cbfbefa81009a2ce615ac53d2914e5870cb452d2afaaab24f3499f72185cbfee53492714734429b7b38608e23926c911cceceac9a36851477ba4c60b087041de621000edc98edada20c1def2"
    ) else {
        panic!()
    };

    res
};

pub const POINT_EVAL_PRECOMPILE_SUCCESS_RESPONSE: [u8; 64] = const {
    // u256_be(4096) || u256_be(BLS12-381 Fr characteristic)
    let Ok(res) = const_hex::const_decode_to_array(
        b"000000000000000000000000000000000000000000000000000000000000100073eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001"
    ) else {
        panic!()
    };

    res
};

pub const POINT_EVAL_PRECOMPILE_GAS: u64 = 50_000;
pub const KZG_VERSIONED_HASH_VERSION_BYTE: u8 = 0x01;

define_subsystem!(PointEvaluationPrecompile,
  interface PointEvaluationPrecompileInterfaceError
  {
      InvalidPoint,
      InvalidInputSize,
      InvalidVersionedHash,
      InvalidScalar,
      PairingMismatch,
  }
);

pub struct BlobEvaluationPrecompile {
    pub g2_by_tau_point: crypto::bls12_381::G2Affine,
    pub prepared_g2_generator:
        <crypto::bls12_381::curves::Bls12_381 as crypto::ark_ec::pairing::Pairing>::G2Prepared,
}

impl BlobEvaluationPrecompile {
    pub fn initialize_as_hook<S: SystemTypes>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, <S as SystemTypes>::Allocator>,
    ) -> Result<(), InternalError>
    where
        S::IO: IOSubsystemExt,
    {
        use crypto::ark_serialize::CanonicalDeserialize;
        let g2_by_tau_point = <crypto::bls12_381::curves::Bls12_381 as crypto::ark_ec::pairing::Pairing>::G2Affine::deserialize_compressed(&TRUSTED_SETUP_TAU_G2_BYTES[..]).expect("must decode from trusted setup");
        let prepared_g2_generator = crypto::bls12_381::G2Affine::generator().into();

        let new = Self {
            g2_by_tau_point,
            prepared_g2_generator,
        };

        system_functions.add_hook(
            POINT_EVAL_HOOK_ADDRESS_LOW,
            system_hooks::SystemHook::StatefulImmutable(Box::new_in(new, system.get_allocator())),
        );

        Ok(())
    }

    fn evaluate<'a, R: Resources>(
        &self,
        input: &[u8],
        output: &mut SliceVec<'a, u8>,
        resources: &mut R,
    ) -> Result<(), PointEvaluationPrecompileSubsystemError> {
        cycle_marker::wrap_with_resources!("point_eval_precompile", resources, {
            let cost_ergs = Ergs(POINT_EVAL_PRECOMPILE_GAS * ERGS_PER_GAS);
            let cost_native = 0;
            resources.charge(&R::from_ergs_and_native(
                cost_ergs,
                <R::Native as zk_ee::system::Computational>::from_computational(cost_native),
            ))?;

            if input.len() != 192 {
                return Err(PointEvaluationPrecompileSubsystemError::LeafUsage(
                    interface_error!(PointEvaluationPrecompileInterfaceError::InvalidInputSize),
                ));
            }

            fn versioned_hash_for_kzg(data: &[u8]) -> [u8; 32] {
                use crypto::sha256::Digest;
                let mut hash: [u8; 32] = crypto::sha256::Sha256::digest(data).into();
                hash[0] = KZG_VERSIONED_HASH_VERSION_BYTE;

                hash
            }

            // Each check without any parsing
            let versioned_hash = &input[..32];
            let commitment = &input[96..144];

            // so far it's just one version
            if versioned_hash_for_kzg(commitment) != versioned_hash {
                return Err(PointEvaluationPrecompileSubsystemError::LeafUsage(
                    interface_error!(PointEvaluationPrecompileInterfaceError::InvalidVersionedHash),
                ));
            }

            fn parse_g1_compressed(input: &[u8]) -> Result<crypto::bls12_381::G1Affine, ()> {
                // format coincides with one defined in ZCash/Arkworks
                use crypto::ark_serialize::CanonicalDeserialize;
                crypto::bls12_381::G1Affine::deserialize_compressed(input).map_err(|_| ())
            }

            // Parse the commitment and proof
            let Ok(commitment_point) = parse_g1_compressed(commitment) else {
                return Err(PointEvaluationPrecompileSubsystemError::LeafUsage(
                    interface_error!(PointEvaluationPrecompileInterfaceError::InvalidPoint),
                ));
            };
            let proof = &input[144..192];
            let Ok(proof) = parse_g1_compressed(proof) else {
                return Err(PointEvaluationPrecompileSubsystemError::LeafUsage(
                    interface_error!(PointEvaluationPrecompileInterfaceError::InvalidPoint),
                ));
            };

            // We do not need internal representation, just canonical scalar
            fn parse_scalar(
                input: &[u8; 32],
            ) -> Result<<crypto::bls12_381::Fr as PrimeField>::BigInt, ()> {
                // Arkworks has strange format for integer serialization, so we do manually
                let mut repr = [0u64; 4];
                for (dst, src) in repr.iter_mut().zip(input.as_rchunks::<8>().1.iter().rev()) {
                    *dst = u64::from_be_bytes(*src);
                }
                let repr = crypto::BigInt::new(repr);
                if repr >= crypto::bls12_381::Fr::MODULUS {
                    Err(())
                } else {
                    Ok(repr)
                }
            }

            let Ok(z) = parse_scalar(input[32..64].try_into().unwrap()) else {
                return Err(PointEvaluationPrecompileSubsystemError::LeafUsage(
                    interface_error!(PointEvaluationPrecompileInterfaceError::InvalidScalar),
                ));
            };

            let Ok(y) = parse_scalar(input[64..96].try_into().unwrap()) else {
                return Err(PointEvaluationPrecompileSubsystemError::LeafUsage(
                    interface_error!(PointEvaluationPrecompileInterfaceError::InvalidScalar),
                ));
            };

            // e(y - P, G₂) * e(proof, X - z) == 1
            let mut y_minus_p = crypto::bls12_381::G1Affine::generator().mul_bigint(&y);
            y_minus_p -= &commitment_point;

            let mut g2_el: crypto::bls12_381::G2Projective = self.g2_by_tau_point.into();
            let z_in_g2 = crypto::bls12_381::G2Affine::generator().mul_bigint(&z);
            g2_el -= z_in_g2;

            use crypto::ark_ec::CurveGroup;
            let y_minus_p_prepared: crypto::bls12_381::G1Affine = y_minus_p.into_affine();
            let g2_el: <crypto::bls12_381::curves::Bls12_381 as crypto::ark_ec::pairing::Pairing>::G2Prepared = g2_el.into_affine().into();

            let gt_el = crypto::bls12_381::curves::Bls12_381::multi_pairing(
                [y_minus_p_prepared, proof],
                [self.prepared_g2_generator.clone(), g2_el],
            );
            if gt_el.0 == <crypto::bls12_381::curves::Bls12_381 as crypto::ark_ec::pairing::Pairing>::TargetField::ONE {
                output.extend(POINT_EVAL_PRECOMPILE_SUCCESS_RESPONSE);
                Ok(())
            } else {
                Err(PointEvaluationPrecompileSubsystemError::LeafUsage(
                    interface_error!(PointEvaluationPrecompileInterfaceError::PairingMismatch),
                ))
            }
        })
    }
}

impl<'a, S: SystemTypes> StatefulImmutableSystemHookImpl<'a, S> for BlobEvaluationPrecompile
where
    S::IO: IOSubsystemExt,
{
    fn invoke(
        &'_ self,
        request: ExternalCallRequest<'_, S>,
        _caller_ee: u8,
        system: &'_ mut System<S>,
        return_memory: &'a mut [core::mem::MaybeUninit<u8>],
    ) -> Result<
        (
            CompletedExecution<'a, S>,
            &'a mut [core::mem::MaybeUninit<u8>],
        ),
        zk_ee::system::errors::system::SystemError,
    > {
        let ExternalCallRequest {
            available_resources,
            calldata,
            modifier,
            ..
        } = request;

        // We allow static calls as we are "pure" hook
        if modifier == CallModifier::Constructor {
            return Err(internal_error!("precompile called with constructor modifier").into());
        }

        let mut resources = available_resources;
        let mut return_vec = SliceVec::new(return_memory);

        let result = self.evaluate::<S::Resources>(calldata, &mut return_vec, &mut resources);

        match result {
            Ok(()) => {
                let (returndata, rest) = return_vec.destruct();
                Ok((
                    make_return_state_from_returndata_region(resources, returndata),
                    rest,
                ))
            }
            Err(e) => match e.root_cause() {
                RootCause::Runtime(RuntimeError::OutOfErgs(_))
                | RootCause::Internal(_)
                | RootCause::Usage(_) => {
                    let _ = system
                        .get_logger()
                        .write_fmt(format_args!("Out of gas during system hook\nError:{e:?}\n"));
                    resources.exhaust_ergs();
                    let (_, rest) = return_vec.destruct();
                    Ok((make_error_return_state(resources), rest))
                }
                RootCause::Runtime(e @ RuntimeError::OutOfNativeResources(_)) => {
                    Err(Into::<SystemError>::into(e.clone_or_copy()))
                }
            },
        }
    }
}

impl<S: SystemTypes> StatefulImmutableSystemHook<S> for BlobEvaluationPrecompile
where
    S::IO: IOSubsystemExt,
{
    fn invoke<'a>(
        &'_ self,
        request: ExternalCallRequest<'_, S>,
        caller_ee: u8,
        system: &'_ mut System<S>,
        return_memory: &'a mut [core::mem::MaybeUninit<u8>],
    ) -> Result<
        (
            CompletedExecution<'a, S>,
            &'a mut [core::mem::MaybeUninit<u8>],
        ),
        zk_ee::system::errors::system::SystemError,
    >
    where
        System<S>: 'a,
    {
        StatefulImmutableSystemHookImpl::invoke(self, request, caller_ee, system, return_memory)
    }
}
