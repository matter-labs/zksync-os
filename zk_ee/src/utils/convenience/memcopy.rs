pub fn copy_and_zeropad_nonoverlapping(src: &[u8], dst: &mut [u8]) {
    let to_copy = core::cmp::min(src.len(), dst.len());
    dst[..to_copy].copy_from_slice(&src[..to_copy]);
    dst[to_copy..].fill(0);
}
