use airbender::rt::sys::{read_word, write_word};

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
        write_word(Self::HELLO_MARKER);
        write_word(message_len.div_ceil(4) as u32 + 1);
        write_word(message_len as u32);
    }

    #[inline(never)]
    fn write_byte(&mut self, byte: u8) {
        self.buffer[self.len] = byte;
        self.len += 1;
        if self.len == 4 {
            self.len = 0;
            let word = u32::from_le_bytes(self.buffer);
            write_word(word);
        }
    }

    #[inline(never)]
    pub fn write_debug<T: core::fmt::Debug>(value: &T) {
        use core::fmt::Write;
        let mut writer = Self::new();
        let _ = write!(writer, "{:?}", value);
    }

    fn flush(&mut self) {
        if self.len == 0 {
            self.buffer.fill(0);
            return;
        }
        for i in self.len..4 {
            self.buffer[i] = 0u8;
        }
        self.len = 0;
        write_word(u32::from_le_bytes(self.buffer));
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
        for byte in src {
            // Write two hex digits per byte without heapless
            let hi = byte >> 4;
            let lo = byte & 0x0f;
            let hi_char = if hi < 10 { b'0' + hi } else { b'a' + hi - 10 };
            let lo_char = if lo < 10 { b'0' + lo } else { b'a' + lo - 10 };
            self.write_byte(hi_char);
            self.write_byte(lo_char);
        }
        self.flush();

        Ok(())
    }
}
