use core::mem::MaybeUninit;

use oracle_provider::RamPeek;

pub fn read_memory_as_u8(memory: &dyn RamPeek, offset: u32, len: u32) -> Result<Vec<u8>, ()> {
    let (_, of) = offset.overflowing_add(len);
    if of == true {
        return Err(());
    }

    let mut offset = offset;
    let mut len = len;

    let mut result = Vec::with_capacity(len as usize);

    if !offset.is_multiple_of(4) {
        let max_take_bytes = 4 - offset;
        let take_bytes = std::cmp::min(max_take_bytes, len);
        let aligned = (offset >> 2) << 2;
        let value = memory.peek_word(aligned);
        let value = value.to_le_bytes();
        result.extend_from_slice(&value[offset as usize % 4..][..take_bytes as usize]);
        offset += max_take_bytes;
        len -= take_bytes;
    }
    // then aligned w
    while len >= 4 {
        let value = memory.peek_word(offset);
        let value = value.to_le_bytes();
        result.extend_from_slice(&value[..]);
        offset += 4;
        len -= 4;
    }
    // then tail
    if len != 0 {
        let value = memory.peek_word(offset);
        let value = value.to_le_bytes();
        result.extend_from_slice(&value[..len as usize]);
        len = 0;
    }

    assert_eq!(len, 0);

    Ok(result)
}

pub fn read_memory_as_u64(
    memory: &dyn RamPeek,
    mut offset: u32,
    len_u64_words: u32,
) -> Result<Vec<u64>, ()> {
    let mut len_u32_words = len_u64_words.checked_mul(2).ok_or(())?;

    let byte_len = len_u32_words.checked_mul(4).ok_or(())?;
    let (_, of) = offset.overflowing_add(byte_len);
    if of == true {
        return Err(());
    }

    let mut result = Vec::with_capacity(len_u64_words as usize);

    if !offset.is_multiple_of(4) {
        return Err(());
    }

    while len_u32_words >= 2 {
        let value1 = memory.peek_word(offset);
        let value2 = memory.peek_word(offset + 4);

        let value = (value2 as u64) << 32 | value1 as u64;

        result.push(value);
        offset += 8;
        len_u32_words -= 2;
    }

    assert_eq!(len_u32_words, 0);

    Ok(result)
}

/// # Safety
/// The data in the memory at offset should actually be T.
pub unsafe fn read_struct<T>(memory: &dyn RamPeek, offset: u32) -> Result<T, ()> {
    if !core::mem::size_of::<T>().is_multiple_of(4) {
        todo!()
    }

    if !(offset as usize).is_multiple_of(core::mem::align_of::<T>()) {
        return Err(());
    }

    let mut r = MaybeUninit::<T>::uninit();

    let ptr = r.as_mut_ptr();

    for i in (0..core::mem::size_of::<T>()).step_by(4) {
        let v = memory.peek_word(offset + i as u32);

        // Safety: iterating over size of T, add will not overflow.
        unsafe { ptr.cast::<u32>().add(i / 4).write(v) };
    }

    // Safety: have written all bytes.
    unsafe { Ok(r.assume_init()) }
}
