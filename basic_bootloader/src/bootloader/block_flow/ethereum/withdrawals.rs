use basic_system::system_implementation::ethereum_storage_model::vec_trait::VecLikeCtor;
use basic_system::system_implementation::ethereum_storage_model::BoxInterner;
use basic_system::system_implementation::ethereum_storage_model::EthereumMPT;
use basic_system::system_implementation::ethereum_storage_model::LeafValue;
use basic_system::system_implementation::ethereum_storage_model::Path;
use core::fmt::Write;
use crypto::MiniDigest;
use ruint::aliases::B160;
use ruint::aliases::U256;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::Computational;
use zk_ee::system::EthereumLikeTypes;
use zk_ee::system::IOSubsystemExt;
use zk_ee::system::Resources;
use zk_ee::system::System;
use zk_ee::system_log;
use zk_ee::utils::Bytes32;

use crate::bootloader::block_flow::ethereum::rlp_ordering_and_key_for_index;
use crate::bootloader::block_flow::ethereum::short_digits_from_key;

use super::transaction::rlp_encoded::rlp::minimal_rlp_parser::HomList;
use super::transaction::rlp_encoded::rlp::minimal_rlp_parser::Rlp;
use super::transaction::rlp_encoded::rlp::minimal_rlp_parser::RlpListDecode;
use super::InvalidTransaction;

pub type WithdrawalsList<'a> = HomList<'a, WithdrawalRequest<'a>, true>;

#[derive(Clone, Copy, Debug)]
pub struct WithdrawalRequest<'a> {
    pub encoding: &'a [u8],
    pub index: u64,
    pub validator_index: u64,
    pub address: B160,
    pub value_in_gwei: u64,
}

impl<'a> RlpListDecode<'a> for WithdrawalRequest<'a> {
    // We need to capture the list header for the encoding (so that it's) hashed
    // in the tree, so we directly implement the method decode_list_from to capture
    // the header.
    fn decode_list_body(_r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        panic!("Withdrawals should be parsed using decode_list_from")
    }

    fn decode_list_from(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        let mark = r.mark();
        let mut inner = r.list()?;
        let index = inner.u64()?;
        let validator_index = inner.u64()?;
        let address = inner.b160()?;
        let value_in_gwei = inner.u64()?;
        if !inner.is_empty() {
            return Err(InvalidTransaction::InvalidStructure);
        }
        let encoding = r.consumed_since(mark);

        Ok(Self {
            encoding,
            index,
            validator_index,
            address,
            value_in_gwei,
        })
    }
}

pub(crate) fn process_withdrawals_list<'a, S: EthereumLikeTypes, VC: VecLikeCtor>(
    system: &mut System<S>,
    list: WithdrawalsList<'a>,
) -> Result<Bytes32, InternalError>
where
    S::IO: IOSubsystemExt,
{
    use basic_system::system_implementation::ethereum_storage_model::MPTInternalCapacities;
    let num_items = list.count.expect("must be prevalidated and have a count");

    let allocator = system.get_allocator();
    let mut interner = BoxInterner::with_capacity_in(1 << 20, allocator.clone());
    let mut hasher = crypto::sha3::Keccak256::new();
    let mpt_capacity =
        MPTInternalCapacities::<S::Allocator, VC>::with_capacity_in(num_items, allocator.clone());
    let mut mpt = EthereumMPT::<_, _, true>::empty_with_preallocated_capacities(
        mpt_capacity,
        allocator.clone(),
    );

    let mut resources = S::Resources::from_native(
        <S::Resources as Resources>::Native::from_computational(u64::MAX),
    );

    for (index, el) in list.iter().enumerate() {
        // meaningful work
        {
            system_log!(system, "Applying withdrawal towards 0x{:040x} for {} GWei, at index {} and validator index {}\n", el.address.as_uint(), el.value_in_gwei, el.index, el.validator_index);

            let amount = U256::from(1_000_000_000u64) * U256::from(el.value_in_gwei);

            resources
                .with_infinite_ergs(|resources| {
                    system.io.update_account_nominal_token_balance(
                        ExecutionEnvironmentType::NoEE, // out of scope of other interactions
                        resources,
                        &el.address,
                        &amount,
                        false,
                        false,
                    )
                })
                .expect("must not fail");

            let (_, index_rlp) = rlp_ordering_and_key_for_index(index as u32);
            let (buffer, len) = index_rlp;
            let digits = short_digits_from_key(&buffer);
            let path = Path::new(&digits[..(len * 2)]);
            let value = LeafValue::Slice {
                value_without_rlp_envelope: el.encoding,
                cached_encoding_len: 0,
            };
            mpt.insert_lazy_value(path, value, &mut (), &mut interner, &mut hasher)
                .expect("must insert into MPT");
        }
    }

    mpt.recompute(&mut (), &mut interner, &mut hasher)
        .expect("must rebuild MPT");
    let root = Bytes32::from_array(mpt.root(&mut hasher));

    Ok(root)
}
