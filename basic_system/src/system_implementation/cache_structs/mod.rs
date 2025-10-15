pub mod storage_values;

use ruint::aliases::B160;
use zk_ee::utils::BitsOrd;
pub type BitsOrd160 = BitsOrd<{ B160::BITS }, { B160::LIMBS }>;

#[derive(Default, Clone)]
pub struct AccountPropertiesMetadataNoPubdata {
    /// None if the account hasn't been deployed in the current block.
    pub deployed_in_tx: Option<u32>,
    /// Transaction where this account was last accessed.
    /// Considered warm if equal to Some(current_tx)
    pub last_touched_in_tx: Option<u32>,
    /// Marks if account is marked for deconstruction is transaction
    pub is_marked_for_deconstruction: bool,
}

impl AccountPropertiesMetadataNoPubdata {
    pub fn considered_warm(&self, current_tx_number: u32) -> bool {
        self.last_touched_in_tx == Some(current_tx_number)
    }
}
