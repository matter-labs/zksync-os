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
}

impl BasicAccountPropertiesMetadata {
    pub fn considered_warm(&self, current_tx_number: u32) -> bool {
        self.last_touched_in_tx == Some(current_tx_number)
    }
}
