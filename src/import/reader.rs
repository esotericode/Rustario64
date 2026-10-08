use super::{ImportError, Result};

/// Checked big-endian reads. Errors carry an offset in this reader's address space.
#[derive(Clone, Copy)]
pub struct Reader<'a> {
    bytes: &'a [u8],
    context: &'static str,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8], context: &'static str) -> Self {
        Self { bytes, context }
    }

    pub fn len(self) -> usize {
        self.bytes.len()
    }
    pub fn is_empty(self) -> bool {
        self.bytes.is_empty()
    }

    pub fn slice(self, offset: usize, len: usize) -> Result<&'a [u8]> {
        let end = offset
            .checked_add(len)
            .ok_or_else(|| ImportError::new(self.context, offset, "range overflow"))?;
        self.bytes.get(offset..end).ok_or_else(|| {
            ImportError::new(
                self.context,
                offset,
                format!("need {len} bytes; input has {}", self.bytes.len()),
            )
        })
    }

    pub fn u8(self, offset: usize) -> Result<u8> {
        Ok(self.slice(offset, 1)?[0])
    }
    pub fn u16(self, offset: usize) -> Result<u16> {
        let b = self.slice(offset, 2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    pub fn i16(self, offset: usize) -> Result<i16> {
        Ok(self.u16(offset)? as i16)
    }
    pub fn u32(self, offset: usize) -> Result<u32> {
        let b = self.slice(offset, 4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
}
