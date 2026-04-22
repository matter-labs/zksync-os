//! Thin wrapper around airbender-rt's `QuasiUart` that adds the zk_ee `Logger`
//! trait (and its `log_data` hex-encoding method).

use airbender::rt::uart::QuasiUart;

#[derive(Default)]
pub struct QuasiUartLogger {
    inner: QuasiUart,
}

impl QuasiUartLogger {
    pub const fn new() -> Self {
        Self {
            inner: QuasiUart::new(),
        }
    }
}

impl core::fmt::Write for QuasiUartLogger {
    #[inline]
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.inner.write_str(s)
    }
}

impl proof_running_system::zk_ee::system::logger::Logger for QuasiUartLogger {
    fn log_data(&mut self, src: impl ExactSizeIterator<Item = u8>) -> core::fmt::Result {
        self.inner.write_entry_sequence(src.len() * 2);
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for byte in src {
            self.inner.write_byte(HEX[(byte >> 4) as usize]);
            self.inner.write_byte(HEX[(byte & 0x0f) as usize]);
        }
        self.inner.flush();
        Ok(())
    }
}
