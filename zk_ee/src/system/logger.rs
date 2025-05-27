pub trait Logger: 'static + core::fmt::Write {
    fn log_data(&mut self, src: impl ExactSizeIterator<Item = u8>) -> core::fmt::Result;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NullLogger;

impl core::fmt::Write for NullLogger {
    #[inline(always)]
    fn write_str(&mut self, _s: &str) -> core::fmt::Result {
        Ok(())
    }

    #[inline(always)]
    fn write_char(&mut self, _c: char) -> core::fmt::Result {
        Ok(())
    }

    #[inline(always)]
    fn write_fmt(&mut self, _args: core::fmt::Arguments<'_>) -> core::fmt::Result {
        Ok(())
    }
}

impl Logger for NullLogger {
    #[inline(always)]
    fn log_data(&mut self, _src: impl ExactSizeIterator<Item = u8>) -> core::fmt::Result {
        Ok(())
    }
}
