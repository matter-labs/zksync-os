use crate::utils::USIZE_SIZE;

pub const fn num_usize_words_for_u8_capacity(u8_capacity: usize) -> usize {
    let num_words = u8_capacity.div_ceil(USIZE_SIZE);
    num_words.next_multiple_of(2)
}
