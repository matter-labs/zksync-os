use zk_ee::system::logger::Logger;

#[derive(Clone, Copy, Debug, Default)]
pub struct StdIOLogger;

impl core::fmt::Write for StdIOLogger {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        use std::io::Write;
        handle.write_all(s.as_bytes()).unwrap();

        Ok(())
    }
}

impl Logger for StdIOLogger {
    fn log_data(&mut self, src: impl ExactSizeIterator<Item = u8>) -> core::fmt::Result {
        let stdout = std::io::stdout();
        let handle = stdout.lock();
        let mut handle = std::io::BufWriter::new(handle);
        use std::io::Write;
        for el in src {
            write!(&mut handle, "{el:02x}").unwrap();
        }
        writeln!(&mut handle).unwrap();
        Ok(())
    }
}
