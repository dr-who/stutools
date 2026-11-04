//! Memory for direct I/O: zeroed memory whose start is aligned to the device block size.
use std::ops::{Deref, DerefMut};

/// Direct I/O needs buffers, offsets and lengths aligned to the device sector size.
pub const DIRECT_ALIGN: usize = 4096;

/// Zeroed buffer whose data starts on an `DIRECT_ALIGN` boundary.
pub struct AlignedBuf {
    raw: Vec<u8>,
    start: usize,
    len: usize,
}
impl AlignedBuf {
    pub fn new(len: usize) -> Self {
        let raw = vec![0; len + DIRECT_ALIGN];
        let start = raw.as_ptr().align_offset(DIRECT_ALIGN);
        Self { raw, start, len }
    }
}
impl Deref for AlignedBuf {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.raw[self.start..self.start + self.len]
    }
}
impl DerefMut for AlignedBuf {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.raw[self.start..self.start + self.len]
    }
}
