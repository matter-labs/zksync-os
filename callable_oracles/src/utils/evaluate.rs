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
