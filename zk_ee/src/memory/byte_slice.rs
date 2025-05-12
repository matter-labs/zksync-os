use u256::U256;

pub trait MinimalByteAddressableSlice {
    fn len(&self) -> usize;
    fn iter<'a>(&'a self) -> impl Iterator<Item = &'a u8> + 'a
    where
        Self: 'a;
}

impl MinimalByteAddressableSlice for [u8] {
    fn len(&self) -> usize {
        Self::len(self)
    }

    fn iter<'a>(&'a self) -> impl Iterator<Item = &'a u8> + 'a
    where
        Self: 'a,
    {
        Self::iter(self)
    }
}

#[derive(Default)]
pub struct ArrayBuilder {
    bytes: [u8; 32],
    offset: usize,
}

impl ArrayBuilder {
    pub fn build(self) -> [u8; 32] {
        assert!(self.offset == 32);
        self.bytes
    }

    pub fn is_empty(&self) -> bool {
        self.offset == 0
    }
}

impl Extend<u8> for ArrayBuilder {
    fn extend<T: IntoIterator<Item = u8>>(&mut self, iter: T) {
        for byte in iter {
            self.bytes[self.offset] = byte;
            self.offset += 1;
        }
    }
}

pub struct U256Builder {
    bytes: [u8; 32],
    previously_written: usize,
}

impl Default for U256Builder {
    fn default() -> Self {
        Self {
            bytes: [0; 32],
            previously_written: 32,
        }
    }
}

impl U256Builder {
    pub fn build(self) -> U256 {
        assert!(self.previously_written == 0);
        U256::from_le_bytes(&self.bytes)
    }
}

impl Extend<u8> for U256Builder {
    fn extend<T: IntoIterator<Item = u8>>(&mut self, iter: T) {
        for byte in iter {
            assert!(self.previously_written > 0, "receiving more than 32 bytes");
            self.previously_written -= 1;
            self.bytes[self.previously_written] = byte;
        }
    }
}
