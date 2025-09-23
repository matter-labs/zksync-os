use alloy::primitives::*;

///
/// The compiler test outcome event.
///
#[derive(Debug, Clone, serde::Serialize)]
pub struct Event {
    /// The event address.
    address: Option<Address>,
    /// The event topics.
    topics: Vec<U256>,
    /// The event values.
    values: Vec<U256>,
}

impl Event {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(address: Option<Address>, topics: Vec<U256>, values: Vec<U256>) -> Self {
        Self {
            address,
            topics,
            values,
        }
    }
}

impl PartialEq<Self> for Event {
    fn eq(&self, other: &Self) -> bool {
        if let (Some(address1), Some(address2)) = (self.address, other.address) {
            if address1 != address2 {
                return false;
            }
        };

        if self.topics.len() != other.topics.len() {
            return false;
        }
        if self.values.len() != other.values.len() {
            return false;
        }

        for index in 0..self.topics.len() {
            let (value1, value2) = (&self.topics[index], &other.topics[index]);

            if value1 != value2 {
                return false;
            }
        }

        for index in 0..self.values.len() {
            let (value1, value2) = (&self.values[index], &other.values[index]);

            if value1 != value2 {
                return false;
            }
        }

        true
    }
}
