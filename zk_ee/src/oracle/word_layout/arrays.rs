use super::WordLayout;

impl<const N: usize> WordLayout for [u8; N] {
    const WORD_COUNT: Option<usize> = Some(N.div_ceil(4));

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        let mut i = 0;
        while i < N {
            let mut buf = [0u8; 4];
            let take = if N - i < 4 { N - i } else { 4 };
            buf[..take].copy_from_slice(&self[i..i + take]);
            w(u32::from_le_bytes(buf));
            i += 4;
        }
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        let mut result = [0u8; N];
        let mut i = 0;
        while i < N {
            let word = r().to_le_bytes();
            let take = if N - i < 4 { N - i } else { 4 };
            result[i..i + take].copy_from_slice(&word[..take]);
            i += 4;
        }
        result
    }
}

impl<const N: usize> WordLayout for [u64; N] {
    const WORD_COUNT: Option<usize> = Some(N * 2);

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        for val in self {
            val.write_words(w);
        }
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        let mut result = [0u64; N];
        for val in &mut result {
            *val = u64::read_words(r);
        }
        result
    }
}

impl<const N: usize> WordLayout for [u32; N] {
    const WORD_COUNT: Option<usize> = Some(N);

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        for val in self {
            w(*val);
        }
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        let mut result = [0u32; N];
        for val in &mut result {
            *val = r();
        }
        result
    }
}
