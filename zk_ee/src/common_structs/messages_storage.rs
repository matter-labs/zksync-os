use crate::{
    memory::stack_trait::{StackCtor, StackCtorConst},
    system::errors::SystemError,
    types_config::{EthereumIOTypesConfig, SystemIOTypesConfig},
    utils::{Bytes32, UsizeAlignedByteBox},
};
use alloc::alloc::Global;
use arrayvec::ArrayVec;
use core::alloc::Allocator;
use ruint::aliases::B160;

use super::history_list::HistoryList;

///
/// Generic message content to be saved in the storage
///
#[derive(Clone, Debug)]
pub struct GenericMessageContent<
    const N: usize,
    IOTypes: SystemIOTypesConfig,
    A: Allocator = Global,
> {
    pub tx_number: u32,
    pub address: IOTypes::Address,
    pub topics: ArrayVec<IOTypes::SignalingKey, N>,
    pub data: UsizeAlignedByteBox<A>,
    pub data_hash: Bytes32,
}

///
/// Generic message content reference to be passed into the system during emit
///
#[derive(Clone, Debug)]
pub struct GenericMessageContentRef<'a, const N: usize, IOTypes: SystemIOTypesConfig> {
    // NOTE: sender doesn't know TX number
    pub address: &'a IOTypes::Address,
    pub topics: &'a ArrayVec<IOTypes::SignalingKey, N>,
    pub data: &'a [u8],
}

const L2_TO_L1_LOG_SERIALIZE_SIZE: usize = 88;

///
/// Generic event content reference to be returned from the storage
///
#[derive(Clone, Debug)]
pub struct GenericMessageContentWithTxRef<'a, const N: usize, IOTypes: SystemIOTypesConfig> {
    pub tx_number: u32,
    pub address: &'a IOTypes::Address,
    pub topics: &'a ArrayVec<IOTypes::SignalingKey, N>,
    pub data: &'a [u8],
    pub data_hash: &'a Bytes32,
}

#[allow(type_alias_bounds)]
pub type MessageContent<const N: usize, A: Allocator = Global> =
    GenericMessageContent<N, EthereumIOTypesConfig, A>;

pub type MessagesStorageStackCheck<SCC: const StackCtorConst, A: Allocator, const N: usize> =
    [(); SCC::extra_const_param::<(MessageContent<N, A>, u32), A>()];

pub struct MessagesStorage<
    const N: usize,
    SC: StackCtor<SCC>,
    SCC: const StackCtorConst,
    A: Allocator + Clone = Global,
> where
    MessagesStorageStackCheck<SCC, A, N>:,
{
    list: HistoryList<MessageContent<N, A>, u32, SC, SCC, A>,
    _marker: core::marker::PhantomData<A>,
}

impl<const N: usize, SC: StackCtor<SCC>, SCC: const StackCtorConst, A: Allocator + Clone>
    MessagesStorage<N, SC, SCC, A>
where
    MessagesStorageStackCheck<SCC, A, N>:,
{
    pub fn new_from_parts(allocator: A) -> Self {
        Self {
            list: HistoryList::new(allocator),
            _marker: core::marker::PhantomData,
        }
    }

    pub fn begin_new_tx(&mut self) {}

    #[track_caller]
    pub fn start_frame(&mut self) -> usize {
        self.list.snapshot()
    }

    pub fn push_message(
        &mut self,
        tx_number: u32,
        address: &B160,
        topics: &ArrayVec<Bytes32, N>,
        data: UsizeAlignedByteBox<A>,
        data_hash: Bytes32,
    ) -> Result<(), SystemError> {
        // assert!(self.rollback_depths_and_pubdata_stack.len() > 0, "we are pushing in no-frame");

        // We are publishing message data(4 bytes to encode length) and underlying log
        let total_pubdata = 4 + B160::BYTES + data.len() + L2_TO_L1_LOG_SERIALIZE_SIZE;
        let total_pubdata = total_pubdata as u32;

        let total_pubdata = self
            .list
            .peek()
            .map_or(total_pubdata, |(_, m)| *m + total_pubdata);

        self.list.push(
            MessageContent {
                tx_number,
                address: *address,
                topics: topics.clone(),
                data,
                data_hash,
            },
            total_pubdata,
        );

        Ok(())
    }

    #[track_caller]
    pub fn finish_frame(&mut self, rollback_handle: Option<usize>) {
        if let Some(x) = rollback_handle {
            self.list.rollback(x);
        }
    }

    pub fn iter_net_diff(&self) -> impl Iterator<Item = &MessageContent<N, A>> {
        self.list.iter()
    }

    pub fn messages_ref_iter(
        &self,
    ) -> impl Iterator<Item = GenericMessageContentWithTxRef<{ N }, EthereumIOTypesConfig>> {
        self.list
            .iter()
            .map(|message| GenericMessageContentWithTxRef {
                tx_number: message.tx_number,
                address: &message.address,
                topics: &message.topics,
                data: message.data.as_slice(),
                data_hash: &message.data_hash,
            })
    }
}
