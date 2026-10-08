use super::{ImportError, Result, reader::Reader};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texture {
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
}

/// N64 image formats and texel sizes (pinned sm64 include/PR/gbi.h G_IM_FMT/G_IM_SIZ).
pub const FMT_RGBA: u8 = 0;
pub const FMT_YUV: u8 = 1;
pub const FMT_CI: u8 = 2;
pub const FMT_IA: u8 = 3;
pub const FMT_I: u8 = 4;
pub const SIZ_4B: u8 = 0;
pub const SIZ_8B: u8 = 1;
pub const SIZ_16B: u8 = 2;
pub const SIZ_32B: u8 = 3;

const MAX_PIXELS: usize = 1024 * 1024;

/// Bits per texel for an image size code.
pub fn bits_per_texel(size: u8) -> Option<usize> {
    match size {
        SIZ_4B => Some(4),
        SIZ_8B => Some(8),
        SIZ_16B => Some(16),
        SIZ_32B => Some(32),
        _ => None,
    }
}

/// Integer channel expansions from sm64tools n64graphics.c (MIT).
fn scale_5_8(v: u16) -> u8 {
    (v * 255 / 31) as u8
}
fn scale_4_8(v: u8) -> u8 {
    v * 0x11
}
fn scale_3_8(v: u8) -> u8 {
    v * 0x24
}

fn rgba5551(p: u16) -> [u8; 4] {
    [
        scale_5_8((p >> 11) & 31),
        scale_5_8((p >> 6) & 31),
        scale_5_8((p >> 1) & 31),
        if p & 1 != 0 { 255 } else { 0 },
    ]
}

/// RGBA5551 as documented by sm64tools n64graphics.c (MIT).
/// Integer channel expansion matches its SCALE_5_8 truncation.
pub fn rgba16(bytes: &[u8], width: u16, height: u16) -> Result<Texture> {
    decode(bytes, FMT_RGBA, SIZ_16B, width, height, None, None)
}

/// TLUT entries used by color-indexed textures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteFormat {
    Rgba16,
    Ia16,
}

/// Decode an N64 texture image into RGBA8.
///
/// `row_bytes` is the source stride; `None` means tightly packed rows. CI formats
/// need a big-endian palette of 16-bit entries. Layouts follow sm64tools
/// n64graphics.c (MIT), except that I4/I8 replicate intensity into alpha as the
/// RDP does for TEXEL alpha; sm64tools exports opaque images instead.
pub fn decode(
    bytes: &[u8],
    format: u8,
    size: u8,
    width: u16,
    height: u16,
    row_bytes: Option<usize>,
    palette: Option<(&[u8], PaletteFormat)>,
) -> Result<Texture> {
    let pixels = usize::from(width) * usize::from(height);
    if pixels == 0 || pixels > MAX_PIXELS {
        return Err(ImportError::new(
            "texture",
            0,
            "invalid dimensions or pixel limit exceeded",
        ));
    }
    let bits = bits_per_texel(size)
        .ok_or_else(|| ImportError::new("texture", 0, format!("invalid size code {size}")))?;
    let packed_row = (usize::from(width) * bits).div_ceil(8);
    let stride = row_bytes.unwrap_or(packed_row);
    if stride < packed_row {
        return Err(ImportError::new(
            "texture",
            0,
            format!("row stride {stride} is smaller than a {packed_row}-byte row"),
        ));
    }
    let r = Reader::new(bytes, "texture");
    r.slice(0, stride * (usize::from(height) - 1) + packed_row)?;
    let texel = |x: usize, y: usize| -> Result<u32> {
        let bit = x * bits;
        let at = y * stride + bit / 8;
        Ok(match bits {
            4 => u32::from(if bit.is_multiple_of(8) {
                r.u8(at)? >> 4
            } else {
                r.u8(at)? & 0xF
            }),
            8 => u32::from(r.u8(at)?),
            16 => u32::from(r.u16(at)?),
            _ => r.u32(at)?,
        })
    };
    let lookup = |index: u32| -> Result<[u8; 4]> {
        let (palette, kind) = palette.ok_or_else(|| {
            ImportError::new("texture", 0, "color-indexed texture without a palette")
        })?;
        let entry = Reader::new(palette, "texture palette").u16(index as usize * 2)?;
        Ok(match kind {
            PaletteFormat::Rgba16 => rgba5551(entry),
            PaletteFormat::Ia16 => {
                let i = (entry >> 8) as u8;
                [i, i, i, entry as u8]
            }
        })
    };
    let mut rgba = Vec::with_capacity(pixels * 4);
    for y in 0..usize::from(height) {
        for x in 0..usize::from(width) {
            let t = texel(x, y)?;
            let pixel = match (format, size) {
                (FMT_RGBA, SIZ_16B) => rgba5551(t as u16),
                (FMT_RGBA, SIZ_32B) => t.to_be_bytes(),
                (FMT_IA, SIZ_16B) => {
                    let i = (t >> 8) as u8;
                    [i, i, i, t as u8]
                }
                (FMT_IA, SIZ_8B) => {
                    let i = scale_4_8((t >> 4) as u8);
                    [i, i, i, scale_4_8((t & 0xF) as u8)]
                }
                (FMT_IA, SIZ_4B) => {
                    let i = scale_3_8((t >> 1) as u8 & 7);
                    [i, i, i, if t & 1 != 0 { 255 } else { 0 }]
                }
                (FMT_I, SIZ_8B) => [t as u8; 4],
                (FMT_I, SIZ_4B) => [scale_4_8(t as u8); 4],
                (FMT_CI, SIZ_4B | SIZ_8B) => lookup(t)?,
                _ => {
                    return Err(ImportError::new(
                        "texture",
                        0,
                        format!("unsupported image format {format}, size {size}"),
                    ));
                }
            };
            rgba.extend_from_slice(&pixel);
        }
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
        for pixel in self.rgba.as_chunks::<4>().0.iter() {
            out.extend_from_slice(&pixel[..3]);
        }
        out
    }
}
