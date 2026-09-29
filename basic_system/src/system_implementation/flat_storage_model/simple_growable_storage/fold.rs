use super::*;

/// The path position of a fold node whose origin was not opened: its siblings are empty.
pub(super) const NO_PATH: usize = usize::MAX;

/// A node of the fold: its position at the current level, the leaf it was folded from and the
/// position of that leaf's path (or `NO_PATH`), and the read and write hashes, whose presence is in
/// bits.
pub(super) struct FoldNode {
    /// The position at the current level
    pub(super) index: u64,
    /// The index of the leaf the node was folded from
    pub(super) origin: u64,
    pub(super) path: usize,
    /// Bit 0: `read` is present; bit 1: `write` is present
    pub(super) present: u32,
    pub(super) read: Bytes32,
    pub(super) write: Bytes32,
}

impl FoldNode {
    #[inline(always)]
    pub(super) fn new(
        index: u64,
        origin: u64,
        path: usize,
        read: Option<Bytes32>,
        write: Option<Bytes32>,
    ) -> Self {
        Self {
            index,
            origin,
            path,
            present: u32::from(read.is_some()) | (u32::from(write.is_some()) << 1),
            read: read.unwrap_or(Bytes32::ZERO),
            write: write.unwrap_or(Bytes32::ZERO),
        }
    }

    #[inline(always)]
    pub(super) fn read(&self) -> Option<&Bytes32> {
        (self.present & 1 != 0).then_some(&self.read)
    }

    #[inline(always)]
    pub(super) fn write(&self) -> Option<&Bytes32> {
        (self.present & 2 != 0).then_some(&self.write)
    }
}
