//! Transaction-range validation of a Glamsterdam block: a contiguous range of
//! block access indices is executed and checked against the block access list,
//! the transactions trie and the receipts trie. See `witness` for what the
//! range witnesses, `index_trie` for the trie lookups, `receipt` for the
//! receipt decoding and `aggregation` for the checks over the outputs of the
//! ranges of a block.

pub mod aggregation;
pub mod block_flow;
pub mod index_trie;
pub mod io;
pub mod metadata_op;
pub mod oracle_queries;
pub mod receipt;
pub mod storage_model;
pub mod storage_policy;
pub mod witness;
