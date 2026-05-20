use super::WordLayout;

impl WordLayout for () {
    const WORD_COUNT: Option<usize> = Some(0);
    fn write_words(&self, _: &mut impl FnMut(u32)) {}
    fn read_words(_: &mut impl FnMut() -> u32) -> Self {}
}

impl WordLayout for bool {
    const WORD_COUNT: Option<usize> = Some(1);
    fn write_words(&self, w: &mut impl FnMut(u32)) {
        w(*self as u32);
    }
    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        r() != 0
    }
}

impl WordLayout for u8 {
    const WORD_COUNT: Option<usize> = Some(1);
    fn write_words(&self, w: &mut impl FnMut(u32)) {
        w(*self as u32);
    }
    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        r() as u8
    }
}

impl WordLayout for u16 {
    const WORD_COUNT: Option<usize> = Some(1);
    fn write_words(&self, w: &mut impl FnMut(u32)) {
        w(*self as u32);
    }
    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        r() as u16
    }
}

impl WordLayout for u32 {
    const WORD_COUNT: Option<usize> = Some(1);
    fn write_words(&self, w: &mut impl FnMut(u32)) {
        w(*self);
    }
    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        r()
    }
}

impl WordLayout for u64 {
    const WORD_COUNT: Option<usize> = Some(2);
    fn write_words(&self, w: &mut impl FnMut(u32)) {
        w(*self as u32);
        w((*self >> 32) as u32);
    }
    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        let lo = r() as u64;
        let hi = r() as u64;
        lo | (hi << 32)
    }
}

impl<A: WordLayout, B: WordLayout> WordLayout for (A, B) {
    const WORD_COUNT: Option<usize> = match (A::WORD_COUNT, B::WORD_COUNT) {
        (Some(a), Some(b)) => Some(a + b),
        _ => None,
    };

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        self.0.write_words(w);
        self.1.write_words(w);
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        (A::read_words(r), B::read_words(r))
    }
}
