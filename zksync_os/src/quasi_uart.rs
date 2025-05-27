use super::csr_io::*;

#[derive(Default)]
pub struct QuasiUART {
    buffer: [u8; 4],
    len: usize,
}

impl QuasiUART {
    const HELLO_MARKER: u32 = u32::MAX;

    #[inline(never)]
    pub const fn new() -> Self {
        Self {
            buffer: [0u8; 4],
            len: 0,
        }
    }

    #[inline(never)]
    pub fn write_entry_sequence(&mut self, message_len: usize) {
        csr_write_word(Self::HELLO_MARKER as usize);
        // now write length is words for query
        csr_write_word(message_len.next_multiple_of(4) / 4 + 1);
        csr_write_word(message_len);
    }

    #[inline(never)]
    pub fn write_word(&self, word: u32) {
        csr_write_word(word as usize);
    }

    #[inline(never)]
    pub fn read_word(&self) -> usize {
        csr_read_word()
    }

    #[inline(never)]
    fn write_byte(&mut self, byte: u8) {
        self.buffer[self.len] = byte;
        self.len += 1;
        if self.len == 4 {
            self.len = 0;
            let word = u32::from_le_bytes(self.buffer);
            self.write_word(word);
        }
    }

    fn flush(&mut self) {
        if self.len == 0 {
            // cleanup and return
            for dst in self.buffer.iter_mut() {
                *dst = 0;
            }
            return;
        }
        for i in self.len..4 {
            self.buffer[i] = 0u8;
        }
        self.len = 0;
        csr_write_word(u32::from_le_bytes(self.buffer) as usize);
    }

    #[inline(never)]
    pub fn write_debug<T: core::fmt::Debug>(value: &T) {
        use core::fmt::Write;
        let mut writer = Self::new();
        let mut string = heapless::String::<64>::new(); // 64 byte string buffer
        let Ok(_) = write!(string, "{:?}", value) else {
            let _ = writer.write_str("too long debug");
            return;
        };
        let _ = writer.write_str(&string);
    }
}

impl core::fmt::Write for QuasiUART {
    fn write_str(&mut self, s: &str) -> Result<(), core::fmt::Error> {
        self.write_entry_sequence(s.len());
        for c in s.bytes() {
            self.write_byte(c);
        }
        self.flush();

        Ok(())
    }
}

impl proof_running_system::zk_ee::system::logger::Logger for QuasiUART {
    fn log_data(&mut self, src: impl ExactSizeIterator<Item = u8>) -> core::fmt::Result {
        let expected_len = src.len() * 2;
        self.write_entry_sequence(expected_len);
        let mut string = heapless::String::<4>::new();
        for byte in src {
            use core::fmt::Write;
            let _ = write!(&mut string, "{:02x}", byte);
            for c in string.bytes() {
                self.write_byte(c);
            }
            string.clear();
        }
        self.flush();

        Ok(())
    }
}
