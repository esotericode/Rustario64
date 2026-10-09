//! Original blob-shadow texture and Mario's GEO_SHADOW parameters. Pixels
//! come only from the identified owner ROM (pinned assets.json/segment2.c).
use super::{
    ImportError, Result, geo::GeoNodeKind, mario::MarioModelSource, mio0, rom::Rom, texture,
    version,
};
use crate::content::visual::TextureImage;

#[derive(Debug, Clone)]
pub struct ShadowSource {
    pub texture: TextureImage,
    pub scale: i16,
    pub solidity: u8,
    /// The first child scale used by geo_process_shadow's root translation.
    pub child_scale: f32,
}

pub fn import(rom: &Rom, mario: &MarioModelSource) -> Result<ShadowSource> {
    let nodes: Vec<_> = mario
        .geo
        .nodes
        .iter()
        .filter(|n| matches!(n.kind, GeoNodeKind::Shadow { .. }))
        .collect();
    let [node] = nodes.as_slice() else {
        return Err(ImportError::new(
            "Mario shadow",
            0,
            "expected one GEO_SHADOW",
        ));
    };
    let GeoNodeKind::Shadow {
        shadow_type,
        solidity,
        scale,
    } = node.kind
    else {
        unreachable!()
    };
    if shadow_type != 99 {
        return Err(ImportError::new(
            "Mario shadow",
            0,
            "expected player shadow type 99",
        ));
    }
    let child_scale = node
        .children
        .first()
        .map_or(1.0, |&i| match mario.geo.nodes[i].kind {
            GeoNodeKind::Scale { scale, .. } => scale as f32 / 65536.0,
            _ => 1.0,
        });
    let range = version::SEGMENT2_MIO0;
    let segment = mio0::decode(
        rom.reader().slice(range.start, range.len())?,
        version::MAX_SEGMENT_BYTES,
    )?;
    let offset = version::SHADOW_CIRCLE_TEXTURE;
    let bytes = segment
        .get(offset..offset + 256)
        .ok_or_else(|| ImportError::new("shadow texture", offset, "truncated IA8 image"))?;
    let decoded = texture::decode(bytes, texture::FMT_IA, texture::SIZ_8B, 16, 16, None, None)?;
    Ok(ShadowSource {
        texture: TextureImage {
            width: 16,
            height: 16,
            rgba: decoded.rgba,
            source: 0x02000000 | offset as u32,
            format: texture::FMT_IA,
            size: texture::SIZ_8B,
        },
        scale,
        solidity,
        child_scale,
    })
}
