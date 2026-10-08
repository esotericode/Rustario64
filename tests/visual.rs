//! Independently authored geo-layout, Fast3D, and texture fixtures.
//! Command words were produced by compiling the pinned decompilation's gbi.h
//! macros with F3D_OLD (see docs/ROM_VALIDATION.md), not by this decoder.
use rustario64::content::visual::*;
use rustario64::import::{
    geo::{self, GeoNodeKind},
    gfx::{self, Builder, IDENTITY},
    model,
    segments::Segments,
    texture::{self, PaletteFormat},
};

fn words(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}

fn shorts(values: &[i16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}

const CC_SHADE: [u32; 2] = [0xFCFF_FFFF, 0xFFFE_793C];
const CC_MODULATERGB_PASS2: [u32; 2] = [0xFC12_7FFF, 0xFFFF_F838];
const VTX3_SLOT0: u32 = 0x0420_0030; // gsSPVertex(v, 3, 0)
const TRI_0_1_2: [u32; 2] = [0xBF00_0000, 0x0000_0A14];
const ENDDL: [u32; 2] = [0xB800_0000, 0];

fn vertex(x: i16, y: i16, z: i16, s: i16, t: i16, color: [u8; 4]) -> Vec<u8> {
    let mut v = shorts(&[x, y, z, 0, s, t]);
    v.extend_from_slice(&color);
    v
}

/// Segment 7: three vertices at 0x000, a 4x4 RGBA16 texture at 0x100, lists at 0x200.
fn gfx_segments(list: &[u32]) -> Segments {
    let mut seg = vec![0u8; 0x200];
    let mut verts = vertex(0, 0, 0, 0, 0, [255, 0, 0, 255]);
    verts.extend(vertex(100, 0, 0, 4 * 32, 0, [0, 255, 0, 255]));
    verts.extend(vertex(0, 0, 100, 0, 4 * 32, [0, 0, 255, 128]));
    seg[..verts.len()].copy_from_slice(&verts);
    for i in 0..16 {
        let p: u16 = if i % 2 == 0 { 0xF801 } else { 0x07C1 };
        seg[0x100 + i * 2..0x102 + i * 2].copy_from_slice(&p.to_be_bytes());
    }
    seg.extend(words(list));
    let mut segments = Segments::default();
    segments.insert(7, seg).unwrap();
    segments
}

fn run(list: &[u32], layer: u8) -> (VisualModel, Vec<rustario64::content::ImportIssue>) {
    let segments = gfx_segments(list);
    let mut builder = Builder::new(&segments);
    builder.run(0x0700_0200, layer, &IDENTITY, true).unwrap();
    builder.finish()
}

#[test]
fn combiner_selectors_decode_from_gbi_words() {
    let shade = gfx::decode_combiner(CC_SHADE[0] & 0xFF_FFFF, CC_SHADE[1]);
    // G_CC_SHADE: 0, 0, 0, SHADE in both cycles (0 encodes as the per-input ZERO).
    assert_eq!(shade[0].rgb, [15, 15, 31, 4]);
    assert_eq!(shade[0].alpha, [7, 7, 7, 4]);
    assert_eq!(shade[1], shade[0]);
    let modulate =
        gfx::decode_combiner(CC_MODULATERGB_PASS2[0] & 0xFF_FFFF, CC_MODULATERGB_PASS2[1]);
    // TEXEL0 * SHADE, alpha SHADE; then G_CC_PASS2 = COMBINED.
    assert_eq!(modulate[0].rgb, [1, 15, 4, 7]);
    assert_eq!(modulate[0].alpha, [7, 7, 7, 4]);
    assert_eq!(modulate[1].rgb, [15, 15, 31, 0]);
    assert_eq!(modulate[1].alpha, [7, 7, 7, 0]);
}

#[test]
fn untextured_triangle_keeps_order_positions_colors_and_layer_state() {
    let list = [
        CC_SHADE[0],
        CC_SHADE[1],
        0xB600_0000,
        gfx::G_LIGHTING,
        VTX3_SLOT0,
        0x0700_0000,
        TRI_0_1_2[0],
        TRI_0_1_2[1],
        ENDDL[0],
        ENDDL[1],
    ];
    let (model, issues) = run(&list, LAYER_OPAQUE);
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(model.batches.len(), 1);
    let batch = &model.batches[0];
    assert_eq!(batch.source, 0x0700_0218);
    assert_eq!(batch.material.layer, LAYER_OPAQUE);
    assert_eq!(batch.material.texture, None);
    assert_eq!(batch.material.lights, None);
    assert_eq!(batch.material.blend, BlendMode::Opaque);
    assert!(batch.material.depth_test && batch.material.depth_write && batch.material.cull_back);
    let positions: Vec<_> = batch.vertices.iter().map(|v| v.position).collect();
    assert_eq!(
        positions,
        [[0.0, 0.0, 0.0], [100.0, 0.0, 0.0], [0.0, 0.0, 100.0]]
    );
    assert_eq!(batch.vertices[2].color, [0, 0, 255, 128]);
}

#[test]
fn textured_triangle_loads_block_and_normalizes_tile_coordinates() {
    let list = [
        CC_MODULATERGB_PASS2[0],
        CC_MODULATERGB_PASS2[1],
        0xFD10_0000,
        0x0700_0100, // gsDPSetTextureImage(RGBA, 16b, 1, addr)
        0xF510_0000,
        0x0700_0000, // gsDPSetTile(RGBA, 16b, 0, 0, G_TX_LOADTILE, ...)
        0xF300_0000,
        0x0700_F800, // gsDPLoadBlock(7, 0, 0, 15, CALC_DXT(4, 2) = 0x800)
        0xF510_0200,
        0x0000_0000, // gsDPSetTile(RGBA, 16b, line 1, tmem 0, tile 0)
        0xF200_0000,
        0x0000_C00C, // gsDPSetTileSize(0, 0, 0, 3 << 2, 3 << 2)
        0xBB00_0001,
        0xFFFF_FFFF, // gsSPTexture(0xFFFF, 0xFFFF, 0, 0, G_ON)
        VTX3_SLOT0,
        0x0700_0000,
        TRI_0_1_2[0],
        TRI_0_1_2[1],
        ENDDL[0],
        ENDDL[1],
    ];
    let (model, issues) = run(&list, LAYER_OPAQUE);
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(model.textures.len(), 1);
    let texture = &model.textures[0];
    assert_eq!(
        (texture.width, texture.height, texture.source),
        (4, 4, 0x0700_0100)
    );
    assert_eq!(texture.rgba[..8], [255, 0, 0, 255, 0, 255, 0, 255]);
    let binding = model.batches[0].material.texture.unwrap();
    assert_eq!(binding.texture, 0);
    assert_eq!(binding.wrap, [WrapMode::Repeat, WrapMode::Repeat]);
    // s = 4 texels * 32 scaled by 0xFFFF/0x10000, divided by the 4-texel tile.
    let uv = model.batches[0].vertices[1].uv;
    assert!((uv[0] - 0.99998).abs() < 1e-4 && uv[1] == 0.0, "{uv:?}");
}

#[test]
fn display_list_calls_branches_and_reports_unsupported_commands() {
    // 0x200: call 0x240, then TEX RECT (unsupported), then end. 0x240: branch to 0x260.
    let mut list = vec![0x0600_0000, 0x0700_0240, 0xE400_0000, 0, ENDDL[0], ENDDL[1]];
    list.resize(16, 0);
    list.extend([0x0601_0000, 0x0700_0260, ENDDL[0], ENDDL[1]]);
    list.resize(24, 0);
    list.extend([
        VTX3_SLOT0,
        0x0700_0000,
        TRI_0_1_2[0],
        TRI_0_1_2[1],
        ENDDL[0],
        ENDDL[1],
    ]);
    let (model, issues) = run(&list, LAYER_ALPHA);
    assert_eq!(model.triangle_count(), 1);
    assert_eq!(model.batches[0].material.blend, BlendMode::Cutout);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].address, 0x0700_0208);
    assert!(issues[0].feature.contains("0xE4"));
}

#[test]
fn display_list_errors_are_located() {
    let segments = gfx_segments(&[TRI_0_1_2[0], 0x0000_0A15]);
    let mut builder = Builder::new(&segments);
    let error = builder.run(0x0700_0200, 1, &IDENTITY, true).unwrap_err();
    assert_eq!(error.offset, 0x0700_0200);
    let segments = gfx_segments(&[TRI_0_1_2[0], TRI_0_1_2[1]]);
    let error = Builder::new(&segments)
        .run(0x0700_0200, 1, &IDENTITY, true)
        .unwrap_err();
    assert!(error.detail.contains("unloaded vertex slot"));
    // A self-call recurses until the bounded display-list stack is exhausted.
    let segments = gfx_segments(&[0x0600_0000, 0x0700_0200]);
    let error = Builder::new(&segments)
        .run(0x0700_0200, 1, &IDENTITY, true)
        .unwrap_err();
    assert!(error.detail.contains("depth"));
    // A self-branch never ends: the command budget stops it.
    let segments = gfx_segments(&[0x0601_0000, 0x0700_0200]);
    let error = Builder::new(&segments)
        .run(0x0700_0200, 1, &IDENTITY, true)
        .unwrap_err();
    assert!(error.detail.contains("budget"));
    let segments = gfx_segments(&[0x0420_0030, 0x0700_0000, ENDDL[0], ENDDL[1]]);
    assert!(
        Builder::new(&segments)
            .run(0x0700_0200, 8, &IDENTITY, true)
            .is_err()
    );
    let segments = gfx_segments(&[0x04F0_0010, 0x0700_0000]);
    assert!(
        Builder::new(&segments)
            .run(0x0700_0200, 1, &IDENTITY, true)
            .is_err()
    );
}

#[test]
fn lights_fog_and_translucent_render_modes_are_captured() {
    let list = [
        0x0386_0010,
        0x0700_0188, // gsSPLight(&l.l, 1)
        0x0388_0010,
        0x0700_0180, // gsSPLight(&l.a, 2)
        0xBA00_1402,
        0x0010_0000, // gsDPSetCycleType(G_CYC_2CYCLE)
        0xB900_031D,
        0xC811_2078, // gsDPSetRenderMode(G_RM_FOG_SHADE_A, G_RM_AA_ZB_OPA_SURF2)
        0xF800_0000,
        0xA0A0_A0FF, // gsDPSetFogColor(160, 160, 160, 255)
        0xBC00_0008,
        0x1900_E800, // gsSPFogPosition(980, 1000)
        0xB700_0000,
        0x0001_0000, // gsSPSetGeometryMode(G_FOG)
        VTX3_SLOT0,
        0x0700_0000,
        TRI_0_1_2[0],
        TRI_0_1_2[1],
        0xB900_031D,
        0x0050_49D8, // G_RM_AA_ZB_XLU_SURF | G_RM_AA_ZB_XLU_SURF2
        0xBA00_1402,
        0x0000_0000, // 1-cycle
        VTX3_SLOT0,
        0x0700_0000,
        TRI_0_1_2[0],
        TRI_0_1_2[1],
        ENDDL[0],
        ENDDL[1],
    ];
    // gdSPDefLights1(0x66, 0x66, 0x66, 0xFF, 0xFF, 0xFF, 0x28, 0x28, 0x28) at 0x180.
    let lights = [
        0x66, 0x66, 0x66, 0, 0x66, 0x66, 0x66, 0, 0xFF, 0xFF, 0xFF, 0, 0xFF, 0xFF, 0xFF, 0, 0x28,
        0x28, 0x28, 0, 0, 0, 0, 0,
    ];
    let mut bytes = gfx_segments(&list)
        .read(0x0700_0000, 0x200 + list.len() * 4)
        .unwrap()
        .to_vec();
    bytes[0x180..0x198].copy_from_slice(&lights);
    let mut segments_bytes = Segments::default();
    segments_bytes.insert(7, bytes).unwrap();
    let mut builder = Builder::new(&segments_bytes);
    builder
        .run(0x0700_0200, LAYER_OPAQUE, &IDENTITY, true)
        .unwrap();
    let (model, issues) = builder.finish();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(model.batches.len(), 2);
    let fogged = model.batches[0].material;
    assert!(fogged.two_cycle);
    assert_eq!(
        fogged.lights,
        Some(Lights {
            ambient: [0x66; 3],
            diffuse: [0xFF; 3],
            direction: [0x28; 3]
        })
    );
    assert_eq!(
        fogged.fog,
        Some(Fog {
            multiplier: 6400,
            offset: -6144,
            color: [160, 160, 160, 255]
        })
    );
    assert_eq!(fogged.blend, BlendMode::Opaque);
    let translucent = model.batches[1].material;
    assert_eq!(translucent.blend, BlendMode::Translucent);
    assert!(translucent.depth_test && !translucent.depth_write);
    assert_eq!(translucent.fog, None);
}

#[test]
fn texture_formats_decode_with_reference_channel_expansion() {
    let ia8 = texture::decode(
        &[0xF0, 0x3C],
        texture::FMT_IA,
        texture::SIZ_8B,
        2,
        1,
        None,
        None,
    )
    .unwrap();
    assert_eq!(ia8.rgba, [255, 255, 255, 0, 0x33, 0x33, 0x33, 0xCC]);
    let ia4 = texture::decode(&[0xF3], texture::FMT_IA, texture::SIZ_4B, 2, 1, None, None).unwrap();
    assert_eq!(ia4.rgba, [252, 252, 252, 255, 0x24, 0x24, 0x24, 255]);
    let ia16 = texture::decode(
        &[0x80, 0x40],
        texture::FMT_IA,
        texture::SIZ_16B,
        1,
        1,
        None,
        None,
    )
    .unwrap();
    assert_eq!(ia16.rgba, [0x80, 0x80, 0x80, 0x40]);
    let i4 = texture::decode(&[0x1F], texture::FMT_I, texture::SIZ_4B, 2, 1, None, None).unwrap();
    assert_eq!(i4.rgba, [0x11, 0x11, 0x11, 0x11, 255, 255, 255, 255]);
    let i8 = texture::decode(&[0x7A], texture::FMT_I, texture::SIZ_8B, 1, 1, None, None).unwrap();
    assert_eq!(i8.rgba, [0x7A; 4]);
    let rgba32 = texture::decode(
        &[1, 2, 3, 4],
        texture::FMT_RGBA,
        texture::SIZ_32B,
        1,
        1,
        None,
        None,
    )
    .unwrap();
    assert_eq!(rgba32.rgba, [1, 2, 3, 4]);
    let palette = [0xF8, 0x01, 0x00, 0x3F];
    let ci4 = texture::decode(
        &[0x01],
        texture::FMT_CI,
        texture::SIZ_4B,
        2,
        1,
        None,
        Some((&palette, PaletteFormat::Rgba16)),
    )
    .unwrap();
    assert_eq!(ci4.rgba, [255, 0, 0, 255, 0, 0, 255, 255]);
    let ci8 = texture::decode(
        &[0x01],
        texture::FMT_CI,
        texture::SIZ_8B,
        1,
        1,
        None,
        Some((&palette, PaletteFormat::Ia16)),
    )
    .unwrap();
    assert_eq!(ci8.rgba, [0x00, 0x00, 0x00, 0x3F]);
    // Rows may be padded to a TMEM line; padding is skipped.
    let strided = texture::decode(
        &[0x10, 0xEE, 0x20],
        texture::FMT_I,
        texture::SIZ_8B,
        1,
        2,
        Some(2),
        None,
    )
    .unwrap();
    assert_eq!(
        strided.rgba,
        [0x10, 0x10, 0x10, 0x10, 0x20, 0x20, 0x20, 0x20]
    );
    assert!(texture::decode(&[0], texture::FMT_CI, texture::SIZ_8B, 1, 1, None, None).is_err());
    assert!(
        texture::decode(
            &[0; 2],
            texture::FMT_YUV,
            texture::SIZ_16B,
            1,
            1,
            None,
            None
        )
        .is_err()
    );
    assert!(
        texture::decode(
            &[0; 3],
            texture::FMT_RGBA,
            texture::SIZ_16B,
            2,
            1,
            None,
            None
        )
        .is_err()
    );
    assert!(
        texture::decode(
            &[0; 4],
            texture::FMT_I,
            texture::SIZ_8B,
            2,
            2,
            Some(1),
            None
        )
        .is_err()
    );
}

/// Segment 0x0E geo layout calling a sub-layout and a branch with return.
fn geo_segments() -> Segments {
    let mut g = words(&[
        0x0800_000A,
        0x00A0_0078,
        0x00A0_0078, // 0x00 GEO_NODE_SCREEN_AREA(10, 160, 120, 160, 120)
        0x0400_0000, // 0x0C GEO_OPEN_NODE
        0x0C01_0000, // 0x10 GEO_ZBUFFER(1)
        0x0400_0000, // 0x14 GEO_OPEN_NODE
        0x1501_0000,
        0x0700_0200, // 0x18 GEO_DISPLAY_LIST(1, ...)
        0x0000_0000,
        0x0E00_0080, // 0x20 GEO_BRANCH_AND_LINK(0x80)
        0x0201_0000,
        0x0E00_00A0, // 0x28 GEO_BRANCH(1, 0xA0)
        0x1084_0000,
        0x0064_00C8,
        0x012C_002D,
        0x00B4_FF4C,
        0x0700_0200, // 0x30 GEO_TRANSLATE_ROTATE_WITH_DL(4, ...)
        0x0500_0000,
        0x0500_0000,
        0x0100_0000, // 0x44 close, close, end
    ]);
    g.resize(0x80, 0);
    // 0x80: sub-layout: translate node (layer 5 DL) then end.
    g.extend(words(&[0x1185_000A, 0x0014_001E, 0x0700_0200, 0x0100_0000]));
    g.resize(0xA0, 0);
    // 0xA0: culling radius then return.
    g.extend(words(&[0x2000_0100, 0x0300_0000]));
    let mut segments = gfx_segments(&[
        VTX3_SLOT0,
        0x0700_0000,
        TRI_0_1_2[0],
        TRI_0_1_2[1],
        ENDDL[0],
        ENDDL[1],
    ]);
    segments.insert(0x0E, g).unwrap();
    segments
}

#[test]
fn geo_layout_builds_original_parenting_and_control_flow() {
    let segments = geo_segments();
    let layout = geo::decode(&segments, 0x0E00_0000).unwrap();
    let mut order = vec![];
    layout.walk(|_, node, depth| order.push((node.source_address, depth)));
    assert_eq!(
        order,
        [
            (0x0E00_0000, 0),
            (0x0E00_0010, 1),
            (0x0E00_0018, 2),
            (0x0E00_0080, 2),
            (0x0E00_00A0, 2),
            (0x0E00_0030, 2),
        ]
    );
    let kinds: Vec<_> = layout.nodes.iter().map(|n| n.kind.clone()).collect();
    assert_eq!(
        kinds[0],
        GeoNodeKind::Root {
            views: 10,
            x: 160,
            y: 120,
            width: 160,
            height: 120
        }
    );
    assert_eq!(kinds[1], GeoNodeKind::MasterList { z_buffer: true });
    assert_eq!(
        layout.nodes[1].flags,
        geo::GRAPH_RENDER_ACTIVE | geo::GRAPH_RENDER_Z_BUFFER
    );
    assert_eq!(
        kinds[5],
        GeoNodeKind::TranslationRotation {
            layer: 4,
            translation: [100, 200, 300],
            // 45, 180, -180 degrees through (deg << 15) / 180.
            rotation: [8192, -32768, -32768],
            display_list: Some(0x0700_0200),
        }
    );
    assert_eq!(layout.nodes[5].flags >> 8, 4);
    assert_eq!(layout.nodes[3].flags >> 8, 5);
    let built = model::build(&segments, &layout).unwrap();
    assert_eq!(built.model.triangle_count(), 3);
    let layers: Vec<_> = built
        .model
        .batches
        .iter()
        .map(|b| b.material.layer)
        .collect();
    assert_eq!(layers, [1, 5, 4]);
    // The translate node moves its display list by (10, 20, 30).
    assert_eq!(
        built.model.batches[1].vertices[1].position,
        [110.0, 20.0, 30.0]
    );
}

#[test]
fn geo_layout_rejects_unknown_commands_imbalance_and_cycles() {
    let mut segments = Segments::default();
    segments
        .insert(0x0E, words(&[0x2100_0000, 0x0100_0000]))
        .unwrap();
    let error = geo::decode(&segments, 0x0E00_0000).unwrap_err();
    assert!(error.detail.contains("0x21"));
    let mut segments = Segments::default();
    segments
        .insert(0x0E, words(&[0x0500_0000, 0x0100_0000]))
        .unwrap();
    assert!(geo::decode(&segments, 0x0E00_0000).is_err());
    let mut segments = Segments::default();
    segments
        .insert(0x0E, words(&[0x0200_0000, 0x0E00_0000]))
        .unwrap();
    assert!(
        geo::decode(&segments, 0x0E00_0000)
            .unwrap_err()
            .detail
            .contains("budget")
    );
    let mut segments = Segments::default();
    segments
        .insert(0x0E, words(&[0x0201_0000, 0x0E00_0000]))
        .unwrap();
    assert!(
        geo::decode(&segments, 0x0E00_0000)
            .unwrap_err()
            .detail
            .contains("stack")
    );
    let mut segments = Segments::default();
    segments.insert(0x0E, words(&[0x1500_0000])).unwrap();
    assert!(geo::decode(&segments, 0x0E00_0000).is_err());
    let mut segments = Segments::default();
    segments.insert(0x0E, words(&[0x0300_0000])).unwrap();
    assert!(geo::decode(&segments, 0x0E00_0000).is_err());
}
