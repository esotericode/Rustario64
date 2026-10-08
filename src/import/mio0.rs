//! Rust adaptation of queueRAM/sm64tools libmio0.c:mio0_decode.
//! MIT, Copyright (c) 2015 Q. Full notice: LICENSES/sm64tools-MIT.txt.
//! Adds bounded streams, allocation limits, and strict malformed-token rejection.
use super::{ImportError, Result, reader::Reader};

pub fn decode(input: &[u8], max_output: usize) -> Result<Vec<u8>> {
    let r = Reader::new(input, "MIO0");
    if r.slice(0, 4)? != b"MIO0" {
        return Err(ImportError::new("MIO0", 0, "bad magic"));
    }
    let size = r.u32(4)? as usize;
    let mut compressed = r.u32(8)? as usize;
    let mut raw = r.u32(12)? as usize;
    if size > max_output {
        return Err(ImportError::new("MIO0", 4, "output exceeds limit"));
    }
    if !(16 <= compressed && compressed <= raw && raw <= input.len()) {
        return Err(ImportError::new("MIO0", 8, "invalid stream offsets"));
    }
    let compressed_end = raw;
    let mask_end = compressed;
    let mut bit = 0usize;
    let mut output = Vec::with_capacity(size);
    while output.len() < size {
        let mask_offset = 16 + bit / 8;
        if mask_offset >= mask_end {
            return Err(ImportError::new(
                "MIO0 mask",
                mask_offset,
                "mask overlaps token stream",
            ));
        }
        let literal = r.u8(mask_offset)? & (0x80 >> (bit % 8)) != 0;
        if literal {
            output.push(r.u8(raw)?);
            raw += 1;
        } else {
            if compressed + 2 > compressed_end {
                return Err(ImportError::new(
                    "MIO0 token",
                    compressed,
                    "truncated back-reference",
                ));
            }
            let token = r.u16(compressed)?;
            let length = usize::from(token >> 12) + 3;
            let distance = usize::from(token & 0x0FFF) + 1;
            if distance > output.len() {
                return Err(ImportError::new(
                    "MIO0 token",
                    compressed,
                    "back-reference before output",
                ));
            }
            if length > size - output.len() {
                return Err(ImportError::new(
                    "MIO0 token",
                    compressed,
                    "back-reference exceeds declared output",
                ));
            }
            // Copy one byte at a time: overlapping references are part of the format.
            for _ in 0..length {
                output.push(output[output.len() - distance]);
            }
            compressed += 2;
        }
        bit += 1;
    }
    Ok(output)
}
