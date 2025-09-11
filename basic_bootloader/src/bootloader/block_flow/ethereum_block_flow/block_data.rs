use crate::bootloader::block_flow::ethereum_block_flow::rlp_encodings::CellEnvelope;
use crate::bootloader::block_flow::ethereum_block_flow::rlp_encodings::ReceiptEncoder;
use crate::bootloader::block_flow::ethereum_block_flow::rlp_ordering_and_key_for_index;
use crate::bootloader::block_flow::ethereum_block_flow::EthereumBlockMetadata;
use crate::bootloader::block_flow::BlockTransactionsDataCollector;
use crate::bootloader::transaction::ethereum_tx_format::EthereumTransactionWithBuffer;
use crate::bootloader::transaction_flow::ethereum::EthereumTransactionFlow;
use crate::bootloader::transaction_flow::ethereum::LogsBloom;
use crate::bootloader::BasicTransactionFlow;
use crate::bootloader::ExecutionResult;
use alloc::collections::BTreeMap;
use basic_system::system_implementation::ethereum_storage_model::BoxInterner;
use basic_system::system_implementation::ethereum_storage_model::ByteBuffer;
use basic_system::system_implementation::ethereum_storage_model::EthereumMPT;
use basic_system::system_implementation::ethereum_storage_model::LazyEncodable;
use basic_system::system_implementation::ethereum_storage_model::LazyLeafValue;
use basic_system::system_implementation::ethereum_storage_model::LeafValue;
use basic_system::system_implementation::ethereum_storage_model::MPTInternalCapacities;
use basic_system::system_implementation::ethereum_storage_model::Path;
use core::alloc::Allocator;
use core::fmt::Write;
use crypto::MiniDigest;
use zk_ee::common_structs::GenericEventContentRef;
use zk_ee::memory::skip_list_quasi_vec::ListVec;
use zk_ee::memory::vec_trait::VecLikeCtor;
use zk_ee::system::logger::Logger;
use zk_ee::system::*;
use zk_ee::utils::Bytes32;

impl<A: Allocator> LazyEncodable for EthereumTransactionWithBuffer<A> {
    fn encoding_len_and_first_byte(&self) -> (usize, u8) {
        // transactions can not be of length 1, so we are fine
        (self.tx_encoding().len(), 0xff)
    }

    fn encode(&self, into: &mut dyn ByteBuffer) {
        into.write_slice(self.tx_encoding());
    }
}

// We just need a sequence of success/not, cumulative gas uses. We do not really benefit from in-process capturable logs,
// as formation of receipt leaf is painfully dependent on not just the transaction number, but the total number
// of transactions

pub struct EthereumBasicTransactionDataKeeper<A: Allocator + Clone, B: Allocator> {
    pub current_transaction_number: u32,
    pub block_gas_used: u64,
    pub per_tx_data: ListVec<(bool, u64, usize, LogsBloom), 32, A>, // status, cumulative gas, number of events, in-flight computed logs bloom
    pub executed_transactions: ListVec<EthereumTransactionWithBuffer<B>, 32, A>,
}

impl<A: Allocator + Clone, B: Allocator> core::fmt::Debug
    for EthereumBasicTransactionDataKeeper<A, B>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EthereumBasicTransactionDataKeeper")
            .field(
                "current_transaction_number",
                &self.current_transaction_number,
            )
            .field("block_gas_used", &self.block_gas_used)
            .field("per_tx_data", &self.per_tx_data)
            .field("executed_transactions", &self.executed_transactions)
            .finish()
    }
}

pub fn short_digits_from_key(key: &[u8; 4]) -> [u8; 8] {
    let mut result = [0u8; 8];
    for (src, dst) in key.iter().zip(result.as_chunks_mut::<2>().0.iter_mut()) {
        let low = *src & 0x0f;
        let high = *src >> 4;
        dst[0] = high;
        dst[1] = low;
    }

    result
}

pub struct EthereumBasicTransactionDataKeeperHeaderValues {
    pub block_gas_used: u64,
    pub transactions_root: Bytes32,
    pub receipts_root: Bytes32,
    pub block_bloom: LogsBloom,
}

impl<A: Allocator + Clone, B: Allocator> EthereumBasicTransactionDataKeeper<A, B> {
    pub fn new_in(allocator: A) -> Self {
        Self {
            current_transaction_number: 0,
            block_gas_used: 0,
            per_tx_data: ListVec::new_in(allocator.clone()),
            executed_transactions: ListVec::new_in(allocator),
        }
    }

    pub fn compute_header_values<S: EthereumLikeTypes, VC: VecLikeCtor>(
        self,
        system: &System<S>,
    ) -> EthereumBasicTransactionDataKeeperHeaderValues
    where
        S::IO: IOSubsystemExt + IOTeardown<S::IOTypes>,
    {
        use zk_ee::memory::stack_trait::Stack;

        let Self {
            current_transaction_number,
            block_gas_used,
            per_tx_data,
            executed_transactions,
        } = self;

        let allocator = system.get_allocator();
        let mut hasher = crypto::sha3::Keccak256::new();
        let mut block_bloom = LogsBloom::default();

        let mut tmp_map = BTreeMap::new_in(allocator.clone());

        // NOTE: we do not expect insanely large integers here, so any integer would fit into 4 bytes buffer;

        // Ugly thing of this loop is that transaction number 0 corresponds to key RLP([]) = 0x80,
        // but transaction number 1 has key RLP(0x01) = 0x01, so we should also reorder

        let mut all_events_it = system.io.events_iterator();
        for (tx_number, ((tx_status, cumulative_gas, num_events, bloom), tx)) in per_tx_data
            .iter()
            .zip(executed_transactions.iter())
            .enumerate()
        {
            let events_it = all_events_it.clone().take(*num_events).map(move |el| {
                debug_assert_eq!(tx_number, el.tx_number as usize);

                GenericEventContentRef {
                    address: el.address,
                    topics: el.topics,
                    data: el.data,
                }
            });
            for _ in 0..*num_events {
                let _ = all_events_it.next().unwrap();
            }

            block_bloom.merge(bloom);

            let tx_type = tx.tx_type();
            let receipt_encoder = ReceiptEncoder::new_from_fields(
                tx_type,
                tx_status,
                cumulative_gas,
                bloom,
                events_it,
            );

            let (ordering_key, tx_number_rlp) = rlp_ordering_and_key_for_index(tx_number as u32);
            // we will also remake index, so we can sequentially insert below
            tmp_map.insert(
                ordering_key,
                (tx_number_rlp, CellEnvelope::new(receipt_encoder), tx),
            );
        }
        assert_eq!(all_events_it.len(), 0);

        // we need to get data from system to compute receipt root. We also have enough to compute transaction root
        let mut interner = BoxInterner::with_capacity_in(1 << 20, allocator.clone());
        let receipts_mpt_capacity = MPTInternalCapacities::<S::Allocator, VC>::with_capacity_in(
            current_transaction_number as usize,
            allocator.clone(),
        );
        let mut receipts_mpt = EthereumMPT::empty_with_preallocated_capacities(
            receipts_mpt_capacity,
            allocator.clone(),
        );
        let transactions_mpt_capacity = MPTInternalCapacities::<S::Allocator, VC>::with_capacity_in(
            current_transaction_number as usize,
            allocator.clone(),
        );
        let mut transactions_mpt = EthereumMPT::empty_with_preallocated_capacities(
            transactions_mpt_capacity,
            allocator.clone(),
        );

        for (_i, ((key, len), receipt, tx)) in tmp_map.iter() {
            let digits = short_digits_from_key(key);
            let path = Path::new(&digits[..(*len * 2)]);
            let value = LeafValue::LazyEncodable {
                value: LazyLeafValue::from_value(receipt),
                cached_encoding_len_with_metadata: 0,
            };
            // {
            //     use basic_system::system_implementation::ethereum_storage_model::Interner;
            //     use basic_system::system_implementation::ethereum_storage_model::InterningBuffer;
            //     let mut interner = BoxInterner::with_capacity_in(1 << 25, system.get_allocator());
            //     let mut buffer = interner.get_buffer(receipt.required_buffer_len()).unwrap();
            //     receipt.encode_into(&mut buffer);
            //     let encoding = buffer.flush();
            //     let _ = system
            //         .get_logger()
            //         .write_fmt(format_args!("Receipt encoding =\n",));

            //     let _ = system.get_logger().log_data(encoding.iter().copied());

            //     let _ = system.get_logger().write_fmt(format_args!("\n",));
            // }
            receipts_mpt
                .insert_lazy_value(path, value, &mut (), &mut interner, &mut hasher)
                .expect("must insert receipts encoder");
            let value = LeafValue::LazyEncodable {
                value: LazyLeafValue::from_value(*tx),
                cached_encoding_len_with_metadata: 0,
            };
            transactions_mpt
                .insert_lazy_value(path, value, &mut (), &mut interner, &mut hasher)
                .expect("must insert receipts encoder");
        }
        receipts_mpt
            .recompute(&mut (), &mut interner, &mut hasher)
            .expect("must compute receipts root");
        let receipts_root = Bytes32::from_array(receipts_mpt.root(&mut hasher));

        transactions_mpt
            .recompute(&mut (), &mut interner, &mut hasher)
            .expect("must compute transactions root");
        let transactions_root = Bytes32::from_array(transactions_mpt.root(&mut hasher));

        let _ = system
            .get_logger()
            .write_fmt(format_args!("Receipts root = {:?}\n", &receipts_root,));

        let _ = system.get_logger().write_fmt(format_args!(
            "Transactions root = {:?}\n",
            &transactions_root,
        ));

        let _ = system
            .get_logger()
            .write_fmt(format_args!("Block bloom =\n",));

        let _ = system
            .get_logger()
            .log_data(block_bloom.as_bytes().iter().copied());

        let _ = system.get_logger().write_fmt(format_args!("\n",));

        EthereumBasicTransactionDataKeeperHeaderValues {
            block_gas_used,
            transactions_root,
            receipts_root,
            block_bloom,
        }
    }
}

impl<A: Allocator + Clone, S: EthereumLikeTypes<Metadata = EthereumBlockMetadata>>
    BlockTransactionsDataCollector<S, EthereumTransactionFlow<S>>
    for EthereumBasicTransactionDataKeeper<A, S::Allocator>
where
    S::IO: IOSubsystemExt + IOTeardown<S::IOTypes>,
{
    fn record_transaction_results(
        &mut self,
        system: &System<S>,
        transaction: <EthereumTransactionFlow<S> as BasicTransactionFlow<S>>::Transaction<'_>,
        context: &<EthereumTransactionFlow<S> as BasicTransactionFlow<S>>::TransactionContext,
        result: &ExecutionResult<'_, <S as SystemTypes>::IOTypes>,
    ) {
        use zk_ee::memory::stack_trait::Stack;
        self.block_gas_used += context.gas_used;

        let tx_status = match result {
            ExecutionResult::Success { .. } => true,
            ExecutionResult::Revert { .. } => false,
        };

        // let _ = system.get_logger().write_fmt(format_args!(
        //     "Cumulative gas used for TX {} = {}\n",
        //     self.current_transaction_number, self.block_gas_used,
        // ));

        // {
        //     let _ = system.get_logger().write_fmt(format_args!(
        //         "Events for TX {}:\n",
        //         self.current_transaction_number
        //     ));

        //     for (event_idx, event) in system.io.events_in_this_tx_iterator().enumerate() {
        //         let _ = system.get_logger().write_fmt(format_args!(
        //             "Event {}: address: 0x{:040x}, topics [",
        //             event_idx,
        //             event.address.as_uint()
        //         ));
        //         for topic in event.topics.iter() {
        //             let _ = system.get_logger().write_fmt(format_args!("{:?},", topic,));
        //         }
        //         let _ = system.get_logger().write_fmt(format_args!("], data =\n",));
        //         let _ = system.get_logger().log_data(event.data.iter().copied());
        //         let _ = system.get_logger().write_fmt(format_args!("\n",));
        //     }

        //     let _ = system.get_logger().write_fmt(format_args!(
        //         "End of events for TX {}\n",
        //         self.current_transaction_number
        //     ));
        // }

        // compute bloom and log number of events
        let mut bloom = LogsBloom::default();
        let events_it = system.io.events_in_this_tx_iterator();
        let num_events = events_it.len();
        let mut hasher = crypto::sha3::Keccak256::new();
        bloom.mark_events(&mut hasher, events_it);

        // {
        //     let _ = system.get_logger().write_fmt(format_args!(
        //         "TX {} bloom =\n",
        //         self.current_transaction_number
        //     ));

        //     let _ = system
        //         .get_logger()
        //         .log_data(bloom.as_bytes().iter().copied());

        //     let _ = system.get_logger().write_fmt(format_args!("\n",));
        // }

        self.per_tx_data
            .push((tx_status, self.block_gas_used, num_events, bloom));
        self.executed_transactions.push(transaction);

        self.current_transaction_number += 1;
    }
}
