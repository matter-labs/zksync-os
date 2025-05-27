pub struct FtsMapping<'a> {
    compact_size_cutoff: usize,
    cutoff_offset: usize,
    data: &'a [u8],
}

impl<'a> FtsMapping<'a> {
    pub fn new(data: &'a [u8], compact_size_cutoff: u16) -> Self {
        let compact_size_cutoff = compact_size_cutoff as usize;

        let cutoff_offset = match compact_size_cutoff % 2 {
            0 => compact_size_cutoff,
            1 => compact_size_cutoff + 1,
            _ => unreachable!(),
        };

        Self {
            data,
            cutoff_offset,
            compact_size_cutoff,
        }
    }

    ///
    /// TODO: document safety
    ///
    /// # Safety
    ///
    pub unsafe fn get_unchecked(&self, ix: usize) -> u16 {
        match ix < self.compact_size_cutoff {
            true => self.data[ix] as u16,
            false => {
                let wide_ix = ix - self.compact_size_cutoff;
                let ix = wide_ix * 2 + self.cutoff_offset;

                u16::from_le_bytes(self.data[ix..ix + 2].try_into().unwrap())
            }
        }
    }
}
