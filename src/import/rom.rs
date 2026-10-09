use super::{ImportError, Result, reader::Reader, version};
use serde::Serialize;
use std::{fs::File, io::Read, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ByteOrder {
    Z64,
    V64,
    N64,
}

/// Normalize byte order without assigning a revision. Parser fixtures use this too.
pub fn normalize(mut bytes: Vec<u8>) -> Result<(Vec<u8>, ByteOrder)> {
    let order = match Reader::new(&bytes, "ROM header").u32(0)? {
        0x80371240 => ByteOrder::Z64,
        0x37804012 => ByteOrder::V64,
        0x40123780 => ByteOrder::N64,
        magic => {
            return Err(ImportError::new(
                "ROM header",
                0,
                format!("unknown N64 byte-order magic 0x{magic:08X}"),
            ));
        }
    };
    let width = match order {
        ByteOrder::Z64 => 1,
        ByteOrder::V64 => 2,
        ByteOrder::N64 => 4,
    };
    if !bytes.len().is_multiple_of(width) {
        return Err(ImportError::new(
            "ROM",
            bytes.len(),
            "truncated byte-order group",
        ));
    }
    if width > 1 {
        for chunk in bytes.chunks_exact_mut(width) {
            chunk.reverse();
        }
    }
    Ok((bytes, order))
}

/// Construction succeeds only for the complete, fingerprinted US reference ROM.
pub struct Rom {
    bytes: Vec<u8>,
    pub input_order: ByteOrder,
}

impl Rom {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).map_err(|e| ImportError::new("ROM file", 0, e.to_string()))?;
        let mut bytes = Vec::new();
        file.take((version::ROM_LEN + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| ImportError::new("ROM file", bytes.len(), e.to_string()))?;
        Self::from_bytes(bytes)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() != version::ROM_LEN {
            return Err(ImportError::new(
                "ROM",
                bytes.len(),
                format!("expected exactly {} bytes for US v1.0", version::ROM_LEN),
            ));
        }
        let (bytes, input_order) = normalize(bytes)?;
        let digest = super::sha1_hex(&bytes);
        if digest != version::US_SHA1 {
            return Err(ImportError::new(
                "ROM identity",
                0,
                format!(
                    "unsupported normalized SHA-1 {digest}; expected {}",
                    version::US_SHA1
                ),
            ));
        }
        Ok(Self { bytes, input_order })
    }

    pub fn reader(&self) -> Reader<'_> {
        Reader::new(&self.bytes, "ROM")
    }
    pub fn fingerprint(&self) -> &'static str {
        version::US_SHA1
    }
}
