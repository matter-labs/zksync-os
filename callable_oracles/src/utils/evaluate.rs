use core::mem::MaybeUninit;

use risc_v_simulator::abstractions::memory::{AccessType, MemorySource};
use risc_v_simulator::cycle::status_registers::TrapReason;

pub fn read_memory_as_u8<M: MemorySource>(
    memory: &M,
    offset: u32,
    len: u32,
) -> Result<Vec<u8>, ()> {
    let (_, of) = offset.overflowing_add(len);
    if of == true {
        return Err(());
    }

    let mut offset = offset;
    let mut len = len;

    let mut result = Vec::with_capacity(len as usize);

    let mut trap = TrapReason::NoTrap;

    if offset % 4 != 0 {
        let max_take_bytes = 4 - offset;
        let take_bytes = std::cmp::min(max_take_bytes, len);
        let aligned = (offset >> 2) << 2;
        let value = memory.get(aligned as u64, AccessType::MemLoad, &mut trap);
        if trap != TrapReason::NoTrap {
            return Err(());
        }
        let value = value.to_le_bytes();
        result.extend_from_slice(&value[offset as usize % 4..][..take_bytes as usize]);
        offset += max_take_bytes;
        len -= take_bytes;
    }
    // then aligned w
    while len >= 4 {
        let value = memory.get(offset as u64, AccessType::MemLoad, &mut trap);
        if trap != TrapReason::NoTrap {
            return Err(());
        }
        let value = value.to_le_bytes();
        result.extend_from_slice(&value[..]);
        offset += 4;
        len -= 4;
    }
    // then tail
    if len != 0 {
        let value = memory.get(offset as u64, AccessType::MemLoad, &mut trap);
        if trap != TrapReason::NoTrap {
            return Err(());
        }
        let value = value.to_le_bytes();
        result.extend_from_slice(&value[..len as usize]);
        len = 0;
    }

    assert_eq!(len, 0);

    Ok(result)
}


pub fn read_memory_as_u64<M: MemorySource>(
    memory: &M,
    mut offset: u32,
    len_u64_words: u32,
) -> Result<Vec<u64>, ()> {
    let mut len_u32_words = len_u64_words * 2;

    let (_, of) = offset.overflowing_add(len_u32_words);
    if of == true {
        return Err(());
    }

    let mut result = Vec::with_capacity(len_u32_words as usize * 2);

    let mut trap = TrapReason::NoTrap;

    if offset % 4 != 0 {
        return Err(());
    }

    while len_u32_words >= 2 {
        let value1 = memory.get(offset as u64, AccessType::MemLoad, &mut trap);
        if trap != TrapReason::NoTrap { return Err(()); }

        let value2 = memory.get(offset as u64 + 4, AccessType::MemLoad, &mut trap);
        if trap != TrapReason::NoTrap { return Err(()); }

        let value = (value2 as u64) << 32 | value1 as u64;

        result.push(value);
        offset += 8;
        len_u32_words -= 2;
    }

    assert_eq!(len_u32_words, 0);

    Ok(result)
}

// Safety: the data in the memory should actually be T.
pub unsafe fn read_struct<T, M: MemorySource>(
    memory: &M,
    offset: u32,
) -> Result<T, ()>
{
    if core::mem::size_of::<T>() % 4 != 0 {
        todo!()
    }

    if offset as usize % core::mem::align_of::<T>() != 0 {
        return Err(());
    }

    let mut r = MaybeUninit::<T>::uninit();
    let mut trap = TrapReason::NoTrap;

    let ptr = r.as_mut_ptr();

    for i in (0 .. core::mem::size_of::<T>()).step_by(4) {
        let v = memory.get(offset as u64 + i as u64, AccessType::MemLoad, &mut trap);
        if trap != TrapReason::NoTrap { return Err(()) }

        // Safety: iterating over size of T, add will not overflow.
        unsafe { ptr.cast::<u32>().add(i / 4).write(v) };
    }

    // Safety: written all bytes.
    unsafe { Ok(r.assume_init()) }
}
