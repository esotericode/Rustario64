//! Static visual-model builder: walks a decoded geo layout in original order
//! (pinned CC0 sm64 src/game/rendering_graph_node.c) and runs each display list
//! through the Fast3D importer with the accumulated node transform.
//!
//! Runtime-only nodes are reported rather than guessed: native callbacks are not
//! executed, switch cases use their initial case, level-of-detail nodes use the
//! nearest range, and animated parts use their rest translation.
use super::{
    Result,
    geo::{GRAPH_RENDER_Z_BUFFER, GeoLayout, GeoNodeKind},
    gfx::{self, Builder, IDENTITY, Mat4},
    segments::Segments,
};
use crate::content::{
    ImportIssue,
    visual::{AreaVisual, Background, GeoCamera, VisualModel},
};

/// mtxf_rotate_zxy_and_translate with float trigonometry. Presentation only;
/// gameplay must use the original sine table when it is ported.
pub fn rotate_zxy_and_translate(translation: [i16; 3], rotation: [i16; 3]) -> Mat4 {
    let angle = |a: i16| f32::from(a) * (std::f32::consts::TAU / 65536.0);
    let (sx, cx) = angle(rotation[0]).sin_cos();
    let (sy, cy) = angle(rotation[1]).sin_cos();
    let (sz, cz) = angle(rotation[2]).sin_cos();
    let [tx, ty, tz] = translation.map(f32::from);
    [
        [
            cy * cz + sx * sy * sz,
            cx * sz,
            -sy * cz + sx * cy * sz,
            0.0,
        ],
        [
            -cy * sz + sx * sy * cz,
            cx * cz,
            sy * sz + sx * cy * cz,
            0.0,
        ],
        [cx * sy, -sx, cx * cy, 0.0],
        [tx, ty, tz, 1.0],
    ]
}

fn translate(translation: [i16; 3]) -> Mat4 {
    rotate_zxy_and_translate(translation, [0; 3])
}

pub struct BuiltModel {
    pub model: VisualModel,
    pub background: Option<Background>,
    pub camera: Option<GeoCamera>,
    pub issues: Vec<ImportIssue>,
}

struct Walker<'a, 'b> {
    layout: &'a GeoLayout,
    builder: Builder<'b>,
    issues: Vec<ImportIssue>,
    background: Option<Background>,
    camera: Option<GeoCamera>,
    perspective: Option<(i16, i16, i16, Option<u32>)>,
}

impl Walker<'_, '_> {
    fn issue(&mut self, address: u32, feature: String) {
        self.issues.push(ImportIssue { address, feature });
    }

    fn node(&mut self, index: usize, matrix: &Mat4, z_buffer: bool) -> Result<()> {
        let node = &self.layout.nodes[index];
        let address = node.source_address;
        let layer = (node.flags >> 8) as u8;
        let mut local = None;
        let mut display_list = None;
        let mut children: Vec<usize> = node.children.clone();
        let mut z_buffer = z_buffer;
        match node.kind {
            GeoNodeKind::Root { .. } | GeoNodeKind::Start | GeoNodeKind::Ortho { .. } => {}
            GeoNodeKind::CullingRadius { .. } => {}
            GeoNodeKind::MasterList { .. } => z_buffer = node.flags & GRAPH_RENDER_Z_BUFFER != 0,
            GeoNodeKind::Perspective {
                fov,
                near,
                far,
                callback,
            } => {
                self.perspective = Some((fov, near, far, callback));
                if let Some(callback) = callback {
                    self.issue(
                        address,
                        format!("perspective callback 0x{callback:08X} not executed"),
                    );
                }
            }
            GeoNodeKind::Camera {
                mode,
                position,
                focus,
                callback,
            } => {
                let (fov_degrees, near, far, perspective_callback) =
                    self.perspective.unwrap_or((45, 100, 30000, None));
                self.camera = Some(GeoCamera {
                    mode,
                    position,
                    focus,
                    fov_degrees,
                    near,
                    far,
                    callback,
                    perspective_callback,
                });
                self.issue(
                    address,
                    format!("camera callback 0x{callback:08X} not executed; viewer camera used"),
                );
            }
            GeoNodeKind::TranslationRotation {
                translation,
                rotation,
                display_list: dl,
                ..
            } => {
                local = Some(rotate_zxy_and_translate(translation, rotation));
                display_list = dl;
            }
            GeoNodeKind::Translation {
                translation,
                display_list: dl,
                ..
            } => {
                local = Some(translate(translation));
                display_list = dl;
            }
            GeoNodeKind::Rotation {
                rotation,
                display_list: dl,
                ..
            } => {
                local = Some(rotate_zxy_and_translate([0; 3], rotation));
                display_list = dl;
            }
            GeoNodeKind::Scale {
                scale,
                display_list: dl,
                ..
            } => {
                let s = scale as f32 / 65536.0;
                let mut m = IDENTITY;
                for (i, row) in m.iter_mut().take(3).enumerate() {
                    row[i] = s;
                }
                local = Some(m);
                display_list = dl;
            }
            GeoNodeKind::AnimatedPart {
                translation,
                display_list: dl,
                ..
            } => {
                local = Some(translate(translation));
                display_list = dl;
                self.issue(
                    address,
                    "animated part drawn at its rest translation".into(),
                );
            }
            GeoNodeKind::Billboard {
                translation,
                display_list: dl,
                ..
            } => {
                local = Some(translate(translation));
                display_list = dl;
                self.issue(address, "billboard drawn without camera facing".into());
            }
            GeoNodeKind::DisplayList {
                display_list: dl, ..
            } => display_list = Some(dl),
            GeoNodeKind::LevelOfDetail {
                min_distance,
                max_distance,
            } => {
                if !(min_distance <= 0 && 0 < max_distance) {
                    children.clear();
                }
                self.issue(
                    address,
                    format!(
                        "level of detail {min_distance}..{max_distance} resolved at distance 0"
                    ),
                );
            }
            GeoNodeKind::SwitchCase { callback, .. } => {
                // init_graph_node_switch_case starts every switch at case 0.
                children = children.first().copied().into_iter().collect();
                self.issue(
                    address,
                    format!("switch callback 0x{callback:08X} not executed; case 0 drawn"),
                );
            }
            GeoNodeKind::Background {
                background,
                callback,
            } => {
                self.background = Some(match callback {
                    Some(callback) => {
                        self.issue(
                            address,
                            format!(
                                "skybox {background} callback 0x{callback:08X} not executed; skybox not imported"
                            ),
                        );
                        Background::Skybox(background)
                    }
                    None => Background::Color(background as u16),
                });
            }
            GeoNodeKind::Generated { param, callback } => self.issue(
                address,
                format!(
                    "generated-geometry callback 0x{callback:08X} (param {param}) not executed"
                ),
            ),
            GeoNodeKind::ObjectParent => {}
            GeoNodeKind::Shadow { .. } => {
                self.issue(address, "shadow node not drawn".into());
            }
            GeoNodeKind::HeldObject { callback, .. } => self.issue(
                address,
                format!("held-object callback 0x{callback:08X} not executed"),
            ),
        }
        let matrix = match local {
            Some(local) => gfx::mat_mul(&local, matrix),
            None => *matrix,
        };
        if node.flags & super::geo::GRAPH_RENDER_ACTIVE == 0 {
            return Ok(());
        }
        if let Some(dl) = display_list {
            self.builder.run(dl, layer, &matrix, z_buffer)?;
        }
        for child in children {
            self.node(child, &matrix, z_buffer)?;
        }
        Ok(())
    }
}

/// Build a static model from a geo layout. Display lists must be in mapped segments.
pub fn build(segments: &Segments, layout: &GeoLayout) -> Result<BuiltModel> {
    let mut walker = Walker {
        layout,
        builder: Builder::new(segments),
        issues: vec![],
        background: None,
        camera: None,
        perspective: None,
    };
    for &detached in &layout.detached {
        walker.issue(
            layout.nodes[detached].source_address,
            "node registered after the root is discarded, as in the original".into(),
        );
    }
    if let Some(root) = layout.root {
        walker.node(root, &IDENTITY, false)?;
    }
    let (model, mut dl_issues) = walker.builder.finish();
    walker.issues.append(&mut dl_issues);
    Ok(BuiltModel {
        model,
        background: walker.background,
        camera: walker.camera,
        issues: walker.issues,
    })
}

/// Convenience for an area: decode the layout, then build its visual model.
pub fn area(
    segments: &Segments,
    area: u8,
    layout_address: u32,
) -> Result<(AreaVisual, Vec<ImportIssue>)> {
    let layout = super::geo::decode(segments, layout_address)?;
    let built = build(segments, &layout)?;
    Ok((
        AreaVisual {
            area,
            model: built.model,
            background: built.background,
            camera: built.camera,
        },
        built.issues,
    ))
}
