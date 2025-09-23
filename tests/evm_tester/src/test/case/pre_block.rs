//! Data used to construct a block.
//! Contains the env in which the block is to be ran and
//! the list of transactions.
use crate::test::case::transaction::Transaction;
use crate::test::test_structure::env_section::EnvSection;

#[derive(Debug, Clone)]
pub struct PreBlock {
    pub env: EnvSection,
    pub transactions: Vec<Transaction>,
    pub expect_exception: bool,
}
