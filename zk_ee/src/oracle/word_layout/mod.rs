extern crate alloc;

mod arrays;
mod foreign_types;
mod primitives;
mod vec;

#[cfg(test)]
mod tests;

pub use word_layout_derive::WordLayout;

/// Word-aligned serialization for oracle IO. Every field is padded to u32
/// word boundaries. The format is architecture-independent (always u32).
///
/// Types implement this trait to be usable as oracle query inputs/outputs.
/// A `#[derive(WordLayout)]` macro generates implementations for structs.
pub trait WordLayout: Sized {
    /// Fixed word count, or `None` for variable-size types (e.g. `Vec<T>`).
    const WORD_COUNT: Option<usize>;

    /// Serialize to a sequence of u32 LE words.
    fn write_words(&self, write: &mut impl FnMut(u32));

    /// Deserialize from a sequence of u32 LE words.
    fn read_words(read: &mut impl FnMut() -> u32) -> Self;
}

/// Word transport for the proving oracle. The guest binary implements
/// this for its CSR-based transport.
pub trait WordSource: 'static {
    fn read_word(&mut self) -> u32;
}
