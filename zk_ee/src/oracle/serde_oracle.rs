use crate::system::errors::internal::InternalError;
use core::num::NonZeroU32;
use serde::{de::DeserializeOwned, Serialize};

use super::query_ids::NEXT_TX_SIZE_QUERY_ID;

pub trait SerdeIOOracle: 'static + Sized {
    fn query<I: Serialize, O: DeserializeOwned + Serialize>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<O, InternalError>;

    fn query_with_empty_input<O: DeserializeOwned + Serialize>(
        &mut self,
        query_type: u32,
    ) -> Result<O, InternalError> {
        self.query::<(), O>(query_type, &())
    }

    fn try_begin_next_tx(&mut self) -> Result<Option<NonZeroU32>, InternalError> {
        let size: u32 = self.query_with_empty_input(NEXT_TX_SIZE_QUERY_ID)?;
        Ok(NonZeroU32::new(size))
    }

    fn query_bytes<I: Serialize>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<alloc::vec::Vec<u8>, InternalError> {
        self.query::<I, alloc::vec::Vec<u8>>(query_type, input)
    }
}
