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

    assert!(offset.is_multiple_of(4));
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
    let size = core::mem::size_of::<T>();
    if !size.is_multiple_of(4) {
        return Err(());
    }

    if !offset.is_multiple_of(4) || !(offset as usize).is_multiple_of(core::mem::align_of::<T>()) {
        return Err(());
    }

    // Words are read at `offset, offset + 4, ..., offset + size - 4`. Reject
    // pointers near the top of the address space so this arithmetic cannot
    // overflow `u32` — which would otherwise panic in debug builds and wrap
    // into low memory in release builds. Mirrors the up-front overflow checks
    // in `read_memory_as_u8`/`read_memory_as_u64`.
    let size_u32 = u32::try_from(size).map_err(|_| ())?;
    if offset.checked_add(size_u32).is_none() {
        return Err(());
    }

    let mut r = MaybeUninit::<T>::uninit();

    let ptr = r.as_mut_ptr();

    for i in (0..size).step_by(4) {
        let v = memory.peek_word(offset + i as u32);

        // Safety: `i < size` and `offset + size` fits in `u32` (checked above),
        // so `offset + i` cannot overflow. The destination write stays within
        // the `size / 4` words of the allocated `T`.
        unsafe { ptr.cast::<u32>().add(i / 4).write(v) };
    }

    // Safety: have written all bytes.
    unsafe { Ok(r.assume_init()) }
}

#[cfg(test)]
mod tests {
    use super::read_struct;
    use crate::test_utils::TestMemorySource;

    #[repr(C)]
    #[derive(Debug, PartialEq, Eq)]
    struct PackedWord(u32);

    #[repr(C)]
    #[derive(Debug, PartialEq, Eq)]
    struct BytePair(u8, u8);

    // Size is a multiple of a word (4) and alignment is 1, so neither the size
    // check nor the type-alignment check rejects it — only the word-offset check
    // can. This isolates the offset check that `BytePair` (size 2) would never
    // reach, since the size check rejects it first.
    #[repr(C)]
    #[derive(Debug, PartialEq, Eq)]
    struct FourBytes([u8; 4]);

    // A multi-word struct whose size and alignment checks both pass, so only
    // the address-space overflow guard can reject a near-top-of-memory pointer.
    #[repr(C)]
    #[derive(Debug, PartialEq, Eq)]
    struct TwoWords(u32, u32);

    #[test]
    fn read_struct_rejects_offset_overflowing_address_space() {
        let memory = TestMemorySource::default();

        // 0xffff_fffc is word-aligned and passes every other check, but reading
        // the second word would compute 0xffff_fffc + 4 and overflow `u32`.
        let result = unsafe { read_struct::<TwoWords>(&memory, 0xffff_fffc) };
        assert_eq!(result, Err(()));
    }

    #[test]
    fn read_struct_rejects_offsets_not_aligned_to_words() {
        let mut memory = TestMemorySource::default();
        memory.insert_u32(0, 0xdead_beef);

        let result = unsafe { read_struct::<FourBytes>(&memory, 2) };
        assert_eq!(result, Err(()));
    }

    #[test]
    fn read_struct_rejects_sizes_not_multiple_of_word() {
        let mut memory = TestMemorySource::default();
        memory.insert_u32(0, 0xdead_beef);

        let result = unsafe { read_struct::<BytePair>(&memory, 0) };
        assert_eq!(result, Err(()));
    }

    #[test]
    fn read_struct_reads_word_aligned_values() {
        let mut memory = TestMemorySource::default();
        memory.insert_u32(0, 0xdead_beef);

        let value = unsafe { read_struct::<PackedWord>(&memory, 0) }.unwrap();
        assert_eq!(value, PackedWord(0xdead_beef));
    }
}
