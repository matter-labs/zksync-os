//! Basic account properties, can be extended by specific
//! storage models to add extra information (e.g. pubdata related
//! considerations).

#[derive(Default, Clone)]
pub struct BasicAccountPropertiesMetadata {
    /// None if the account hasn't been deployed in the current block.
    pub deployed_in_tx: Option<u32>,
    /// Transaction where this account was last accessed.
    /// Considered warm if equal to Some(current_tx)
    pub last_touched_in_tx: Option<u32>,
    /// Marks if account is marked for deconstruction is transaction
    pub is_marked_for_deconstruction: bool,
    /// Transaction where the persist cost (0x8003 write + preimage hash) was
    /// proactively charged. None means not yet charged in the current block.
    pub persist_charged_in_tx: Option<u32>,
    /// Whether the cold NEW-account read extra was already charged for this
    /// account. Kept in rollback-aware metadata rather than derived from cache
    /// presence: entries materialized by a transaction that is later dropped
    /// from the block stay in the cache, but their metadata updates are rolled
    /// back together with the charge, so charging never depends on dropped
    /// transactions (which the proving run doesn't re-execute).
    pub new_read_extra_charged: bool,
}

impl BasicAccountPropertiesMetadata {
    pub fn considered_warm(&self, current_tx_number: u32) -> bool {
        self.last_touched_in_tx == Some(current_tx_number)
    }
}
