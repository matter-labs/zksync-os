use crypto::MiniDigest;

///
/// A minimal `no_std`-friendly trait for writing contiguous byte slices to a destination.
///
/// This is useful when a function needs to write its output into *different kinds of destinations*.
/// For example `pubdata` needs to be written to hasher or another accumulator depending on the commitment.
///
pub trait WriteBytes {
    fn write(&mut self, buf: &[u8]);

    /// Writes a single byte. Defaults to a one-byte [`WriteBytes::write`], but
    /// implementors can override it if they have a cheaper path.
    fn write_byte(&mut self, byte: u8) {
        self.write(&[byte]);
    }
}

// implement it for hashers by default
impl<T: MiniDigest> WriteBytes for T {
    fn write(&mut self, buf: &[u8]) {
        self.update(buf);
    }
}
