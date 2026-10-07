//! Zero-copy view of a receipt as the receipts trie stores it: a typed
//! envelope `type || rlp([status, cumulative_gas_used, bloom, logs])`, or the
//! bare list for a legacy transaction.

use super::super::GlamsterdamError;
use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::Rlp;
use ruint::aliases::B160;

type Result<T> = core::result::Result<T, GlamsterdamError>;

#[derive(Clone, Copy)]
pub struct ReceiptReflection<'a> {
    /// EIP-2718 type, 0 for a legacy transaction
    pub tx_type: u8,
    pub status: bool,
    pub cumulative_gas_used: u64,
    pub bloom: &'a [u8; 256],
    logs: Rlp<'a>,
}

#[derive(Clone, Copy)]
pub struct LogReflection<'a> {
    pub address: B160,
    topics: Rlp<'a>,
    pub data: &'a [u8],
}

impl<'a> ReceiptReflection<'a> {
    pub fn parse(item: &'a [u8]) -> Result<Self> {
        let error = GlamsterdamError::ReceiptEncoding;
        let (tx_type, list) = match item.first() {
            Some(first) if *first < 0x80 => (*first, &item[1..]),
            Some(first) if *first >= 0xc0 => (0, item),
            _ => return Err(error),
        };
        let mut outer = Rlp::new(list);
        let mut r = outer.list().map_err(|_| error)?;
        if !outer.is_empty() {
            return Err(error);
        }
        let status = r.bool().map_err(|_| error)?;
        let cumulative_gas_used = r.u64().map_err(|_| error)?;
        let bloom = r.bytes_exact::<256>().map_err(|_| error)?;
        let logs = r.list().map_err(|_| error)?;
        if !r.is_empty() {
            return Err(error);
        }
        Ok(Self {
            tx_type,
            status,
            cumulative_gas_used,
            bloom,
            logs,
        })
    }

    /// The logs, in order
    pub fn logs(&self) -> LogsIter<'a> {
        LogsIter(self.logs)
    }
}

pub struct LogsIter<'a>(Rlp<'a>);

impl<'a> Iterator for LogsIter<'a> {
    type Item = Result<LogReflection<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            return None;
        }
        let error = GlamsterdamError::ReceiptEncoding;
        Some((|| {
            let mut r = self.0.list().map_err(|_| error)?;
            let address = r.b160().map_err(|_| error)?;
            let topics = r.list().map_err(|_| error)?;
            let data = r.bytes().map_err(|_| error)?;
            if !r.is_empty() {
                return Err(error);
            }
            Ok(LogReflection {
                address,
                topics,
                data,
            })
        })())
    }
}

impl<'a> LogReflection<'a> {
    /// The topics, in order
    pub fn topics(&self) -> TopicsIter<'a> {
        TopicsIter(self.topics)
    }
}

pub struct TopicsIter<'a>(Rlp<'a>);

impl<'a> Iterator for TopicsIter<'a> {
    type Item = Result<&'a [u8; 32]>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            return None;
        }
        Some(
            self.0
                .bytes_exact::<32>()
                .map_err(|_| GlamsterdamError::ReceiptEncoding),
        )
    }
}
