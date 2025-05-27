impl<'a> AugmentedSlice<'a> {
    pub fn parse_element_section_prevalidated(&mut self) -> ElementSection {
        let segment_flags = self.read_u32().unwrap();
        match segment_flags {
            0 => {
                let start_idx = self.absolute_offset() as u32;
                // we expect constant expression and then list of function indexes
                let _table_idx = 0usize;
                let _offset = self.parse_i32_constant_expression().unwrap();
                let num_indexes = self.read_u32().unwrap();
                for _ in 0..num_indexes {
                    let _func_idx = self.read_u32().unwrap();
                }
                let end_idx = self.absolute_offset() as u32;

                ElementSection::ActiveFuncRefExternval { start_idx, end_idx }
            }
            _ => unsafe { core::hint::unreachable_unchecked() },
        }
    }
}