#[derive(Debug)]
pub struct UsizeSliceIteratorOwned {
    slice: Box<[usize]>,
    cursor: u32,
}

impl UsizeSliceIteratorOwned {
    pub fn new(slice: Box<[usize]>) -> Self {
        Self { slice, cursor: 0 }
    }
}

impl ExactSizeIterator for UsizeSliceIteratorOwned {
    fn len(&self) -> usize {
        let (lower, upper) = self.size_hint();
        // Note: This assertion is overly defensive, but it checks the invariant
        // guaranteed by the trait. If this trait were rust-internal,
        // we could use debug_assert!; assert_eq! will check all Rust user
        // implementations too.
        core::assert_eq!(upper, Some(lower));
        lower
    }

    // fn is_empty(&self) -> bool {
    //     self.len() == 0
    // }
}

impl Iterator for UsizeSliceIteratorOwned {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        // if self.cursor == 0 {
        //     self.cursor += 1;
        //
        //     return Some(self.slice.len()); 
        // }
        //
        // if self.cursor -1 == self.slice.len() as u32 { return None; }
        //
        // let r = self.slice[self.cursor as usize - 1 ];
        //
        // self.cursor += 1;
        //
        // Some(r)
        //
        
        if self.cursor == self.slice.len() as u32 { return None; }

        let r = self.slice[self.cursor as usize ];

        self.cursor += 1;

        Some(r)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // let consumed = if self.cursor == 0 { 0 } else { self.cursor };
        // let r = self.slice.len() + 1 - consumed as usize;
        // (r, Some(r))
        let r = self.slice.len() - self.cursor as usize;

        (r, Some(r))
    }
}
