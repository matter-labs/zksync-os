//! Memory-based oracle protocol: query inputs are passed to the oracle by address, and responses are
//! written by the oracle straight into their destination, memcpy-like.
//!
//! The iterator-based [`IOOracle`](super::IOOracle) feeds every input word into the oracle and pulls every
//! response word through an iterator, behind a length prefix and with per-word checks. Here:
//!
//! - A query is exactly two words: the query ID and one *input word*, which is
//!   - `0` for a query without input,
//!   - the value itself for a [`ShortSerializable`] input (`bool`, `u8`, `u16`, `u32`),
//!   - the address of the input for a [`ContinuousSerializable`] one (`U256`, `Bytes32`, ...),
//!   - for a composite input, a tuple of two to four references to [`ContinuousSerializable`] values
//!     ([`CompositeSerializable`]), the address of an array holding the addresses of the elements.
//!
//!   The oracle reads what it needs from the memory of the querier while it receives the input word.
//! - The response is exactly what the querier reads, without a length prefix:
//!   - a [`ShortDeserializable`] value is one word, range-checked on receipt;
//!   - a [`ContinuousDeserializable`] value is written by the oracle into its destination as whole aligned
//!     `u32` words ([`MemoryOracle::write`]), then validated in place
//!     ([`ContinuousDeserializable::validate`]);
//!   - a composite output, a tuple of two to four destinations ([`CompositeDeserializable`]), is the
//!     concatenation of its elements, each written and validated in turn.
//! - A dynamically sized response keeps the existing behavior: the oracle first claims its length, which
//!   must fit into the destination ([`DynamicDestination`]), then writes the words. The claim counts `u32`
//!   words, so that a response is the same sequence of words on every target.
//!
//! [`OracleQuery`] and [`DynamicOracleQuery`] bind the input and output types of a query to its ID, as
//! [`SimpleOracleQuery`](super::simple_oracle_query::SimpleOracleQuery) does.
//!
//! The transport, [`MemoryOracle`], is `airbender_oracle`'s, shared with the hints of `airbender-crypto`:
//! its errors are [`InternalError`](crate::system::errors::internal::InternalError)s here, and
//! [`MemoryOracleExt`] adds the typed encodings above on top of it. On the proving target the oracle is a
//! word channel (the non-determinism CSR), and receiving a response comes down to "write the next oracle
//! word to the next aligned address" ([`WordChannelOracle`]). The oracle side of the protocol, reading the
//! inputs from the memory of the querier and encoding the responses, is in [`host`].
//!
//! # Security
//!
//! Everything the oracle writes is untrusted. Types opt into being written by the oracle through `unsafe`
//! traits, whose contract is that validation rejects any bytes that do not form a valid value. Validation
//! only makes a value well-formed: whether it is *correct* is still for the querier to check, as with the
//! iterator-based protocol.

mod composite;
mod continuous;
mod dynamic;
pub mod host;
mod impls;
mod oracle;
mod query;
mod short;

#[cfg(test)]
mod tests;

pub use self::composite::{CompositeDeserializable, CompositeSerializable};
pub use self::continuous::{normalize_bool, ContinuousDeserializable, ContinuousSerializable};
pub use self::dynamic::DynamicDestination;
pub(crate) use self::dynamic::{write_usize_words, U32_WORDS_PER_USIZE};
pub use self::oracle::MemoryOracleExt;
pub use self::query::{
    get_bytes_with_length_query, DynamicOracleQuery, OracleQuery, QueryInput, QueryOutput,
};
pub use self::short::{ShortDeserializable, ShortSerializable};
pub use airbender_oracle::{MemoryOracle, OracleError, WordChannel, WordChannelOracle};

/// The errors of the transport, located where it makes them.
impl OracleError for crate::system::errors::internal::InternalError {
    #[track_caller]
    #[inline(always)]
    fn new(message: &'static str) -> Self {
        let location = core::panic::Location::caller();
        Self::new(
            message,
            crate::system::errors::location::ErrorLocation::new(location.file(), location.line()),
        )
    }
}
