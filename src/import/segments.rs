use super::{ImportError, Result};
use std::collections::BTreeMap;

/// Import-only address space. ROM pointers never enter the authoritative runtime.
#[derive(Default)]
pub struct Segments {
    data: BTreeMap<u8, Vec<u8>>,
}

impl Segments {
    pub fn is_mapped(&self, id: u8) -> bool {
        self.data.contains_key(&id)
    }

    pub fn insert(&mut self, id: u8, bytes: Vec<u8>) -> Result<()> {
        if id == 0 || id >= 32 || bytes.len() > 0x1000000 {
            return Err(ImportError::new(
                "segment",
                usize::from(id),
                "invalid ID or size",
            ));
        }
        if self.data.contains_key(&id) {
            return Err(ImportError::new(
                "segment",
                usize::from(id),
                "duplicate mapping",
            ));
        }
        self.data.insert(id, bytes);
        Ok(())
    }

    pub fn tail(&self, address: u32) -> Result<&[u8]> {
        let id = (address >> 24) as u8;
        let offset = (address & 0xFFFFFF) as usize;
        let bytes = self.data.get(&id).ok_or_else(|| {
            ImportError::new(
                "segmented address",
                address as usize,
                format!("unmapped segment 0x{id:02X}"),
            )
        })?;
        bytes.get(offset..).ok_or_else(|| {
            ImportError::new(
                "segmented address",
                address as usize,
                "offset outside segment",
            )
        })
    }

    pub fn read(&self, address: u32, len: usize) -> Result<&[u8]> {
        self.tail(address)?.get(..len).ok_or_else(|| {
            ImportError::new(
                "segmented address",
                address as usize,
                format!("need {len} bytes"),
            )
        })
    }
}
