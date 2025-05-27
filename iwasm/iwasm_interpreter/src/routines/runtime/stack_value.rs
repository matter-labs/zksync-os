#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StackValue(u64);

impl StackValue {
    pub const FUNCREF_MASK: u64 = 1u64 << 63;

    pub const fn empty() -> Self {
        Self(0u64)
    }

    pub const fn new_bool(value: bool) -> Self {
        Self(value as u64)
    }

    pub const fn new_i32(value: i32) -> Self {
        Self(value as u32 as u64)
    }

    pub const fn new_i64(value: i64) -> Self {
        Self(value as u64)
    }

    pub const fn new_funcref(func_idx: u16) -> Self {
        Self(func_idx as u64 | Self::FUNCREF_MASK)
    }

    pub const fn new_nullref() -> Self {
        Self(0u64)
    }

    #[allow(clippy::result_unit_err)]
    pub const fn get_func_from_ref(&self) -> Result<u16, ()> {
        if self.0 & Self::FUNCREF_MASK != 0 {
            Ok(self.0 as u16)
        } else {
            Err(())
        }
    }

    pub const fn as_i32(self) -> i32 {
        self.0 as u32 as i32
    }

    pub const fn as_i64(self) -> i64 {
        self.0 as i64
    }
}

// pub(crate) fn mem_read_into_buffer<const N: usize>(
//     memory: &Vec<Vec<u8>>,
//     dst: &mut [u8; N],
//     offset: u32,
//     num_bytes: u32,
// ) -> Result<(), ()> {
//     let mem_len = PAGE_SIZE * memory.len();
//     let end = offset.checked_add(num_bytes).ok_or(())?;
//     if end > mem_len as u32 {
//         return Err(());
//     }

//     if num_bytes == 0 {
//         return Ok(());
//     }

//     let mut src_offset = offset as usize;
//     let mut dst_offset = 0 as usize;
//     let mut len = num_bytes as usize;

//     // we will proceed by identifying per-page subranges that can be copied with just "copy_from_slice"
//     loop {
//         let src_in_page_len = PAGE_SIZE - src_offset % PAGE_SIZE;
//         let len_to_copy = core::cmp::min(src_in_page_len, len);

//         let src_page = src_offset / PAGE_SIZE;
//         let src_in_page_offset = src_offset % PAGE_SIZE;

//         // we have to do ptr::copy due to borrow checker
//         unsafe {
//             core::ptr::copy(
//                 memory[src_page][src_in_page_offset..].as_ptr(),
//                 dst[dst_offset..].as_mut_ptr(),
//                 len_to_copy,
//             );
//         }

//         src_offset += len_to_copy;
//         dst_offset += len_to_copy;
//         len -= len_to_copy;
//         if len == 0 {
//             break;
//         }
//     }

//     Ok(())
// }
