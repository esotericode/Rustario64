use super::{ImportError, Result, reader::Reader};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texture {
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
}

/// RGBA5551 as documented by sm64tools n64graphics.c (MIT).
/// Integer channel expansion matches its SCALE_5_8 truncation.
pub fn rgba16(bytes: &[u8], width: u16, height: u16) -> Result<Texture> {
    let pixels = usize::from(width) * usize::from(height);
    if pixels == 0 || pixels > 1024 * 1024 {
        return Err(ImportError::new(
            "RGBA16",
            0,
            "invalid dimensions or pixel limit exceeded",
        ));
    }
    let r = Reader::new(bytes, "RGBA16");
    r.slice(0, pixels * 2)?;
    let mut rgba = Vec::with_capacity(pixels * 4);
    for i in 0..pixels {
        let p = r.u16(i * 2)?;
        for shift in [11, 6, 1] {
            rgba.push((((p >> shift) & 31) * 255 / 31) as u8);
        }
        rgba.push(if p & 1 != 0 { 255 } else { 0 });
    }
    Ok(Texture {
        width,
        height,
        rgba,
    })
}

impl Texture {
    /// PPM preview discards alpha; the RGBA export retains it.
    pub fn to_ppm(&self) -> Vec<u8> {
        let mut out = format!("P6\n{} {}\n255\n", self.width, self.height).into_bytes();
        for pixel in self.rgba.chunks_exact(4) {
            out.extend_from_slice(&pixel[..3]);
        }
        out
    }
}
