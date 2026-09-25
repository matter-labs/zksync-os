//! Values exchanged with the oracle by value, as a single word.

use crate::internal_error;
use crate::system::errors::internal::InternalError;

/// A value that is sent to the oracle by value: it is the input word of the query.
///
/// Only for types whose encoding fits into the machine word of the proving target (`u32`), so the value
/// takes one oracle word in both forward and proving mode. Query IDs are sent the same way.
pub trait ShortSerializable: Copy {
    fn to_short_word(self) -> u32;
}

/// A value that the oracle sends as a single response word.
pub trait ShortDeserializable: Sized {
    /// Decodes an untrusted word, and fails if the word does not encode a value of `Self`.
    fn from_short_word(word: u32) -> Result<Self, InternalError>;
}

impl ShortSerializable for bool {
    #[inline(always)]
    fn to_short_word(self) -> u32 {
        u32::from(self)
    }
}

impl ShortDeserializable for bool {
    /// Any non-zero word is `true`.
    #[inline(always)]
    fn from_short_word(word: u32) -> Result<Self, InternalError> {
        Ok(word != 0)
    }
}

macro_rules! impl_short_for_narrow_unsigned {
    ($($t:ty),+) => {$(
        impl ShortSerializable for $t {
            #[inline(always)]
            fn to_short_word(self) -> u32 {
                u32::from(self)
            }
        }

        impl ShortDeserializable for $t {
            #[inline(always)]
            fn from_short_word(word: u32) -> Result<Self, InternalError> {
                <$t>::try_from(word).map_err(|_| {
                    internal_error!(concat!(stringify!($t), " from oracle is out of range"))
                })
            }
        }
    )+};
}

impl_short_for_narrow_unsigned!(u8, u16);

impl ShortSerializable for u32 {
    #[inline(always)]
    fn to_short_word(self) -> u32 {
        self
    }
}

impl ShortDeserializable for u32 {
    #[inline(always)]
    fn from_short_word(word: u32) -> Result<Self, InternalError> {
        Ok(word)
    }
}
