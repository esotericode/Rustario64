//! Presentation for the core's play session (`rustario64::play`): the view
//! the original camera computed, Mario's posed model in the renderer, and a
//! placeholder for when his model is unavailable. Nothing here reaches the
//! simulation; the camera itself runs in the session's frames.
use crate::{Renderer, camera::FlyCamera};
use rustario64::{
    content::visual::{
        BlendMode, CombinerCycle, DrawBatch, GeoCamera, LAYER_OPAQUE, Material, VisualModel,
        VisualVertex,
    },
    play::CameraView,
    presentation::mario::MarioFrame,
    presentation::objects::{BillboardBasis, ObjectFrame},
    presentation::shadow::ShadowFrame,
};
use std::{collections::HashMap, f32::consts::PI};

/// Original angle units to radians.
pub fn radians(angle: i16) -> f32 {
    f32::from(angle) * PI / 32768.0
}

/// The renderer camera for the original camera's view: Lakitu's position
/// looking at his focus, the perspective node's field of view, the area
/// frustum's near and far planes, and the screen roll. Before the first
/// frame's render pass sets it, the perspective node holds the geo layout's
/// field of view.
pub fn reference_view(view: CameraView, frustum: Option<GeoCamera>) -> FlyCamera {
    let mut camera = FlyCamera::looking_at(view.pos, view.focus);
    camera.fov_y_degrees = if view.fov > 0.0 {
        view.fov
    } else {
        frustum.map_or(45.0, |f| f32::from(f.fov_degrees))
    };
    camera.roll = radians(view.roll);
    if let Some(original) = frustum {
        camera.near = f32::from(original.near);
        camera.far = f32::from(original.far);
    }
    camera
}

/// geo_process_level_of_detail's distance: the depth of Mario's origin in
/// front of the camera, as the integer part of the view-space z it reads.
pub fn lod_distance(camera: &FlyCamera, position: [f32; 3]) -> i16 {
    let f = camera.forward();
    let depth: f32 = (0..3)
        .map(|i| (position[i] - camera.position[i]) * f[i])
        .sum();
    // -GET_HIGH_S16_OF_32(z) with z = -depth in 16.16 fixed point.
    depth.ceil().clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
}

/// Mario's model in a renderer: one uploaded model per build (draw list),
/// rewritten in place each frame, with only the drawn build visible.
#[derive(Debug, Default)]
pub struct MarioModelView {
    uploaded: HashMap<usize, usize>,
    shown: Option<usize>,
}

impl MarioModelView {
    /// Show `frame`, or hide Mario when it is None.
    pub fn show(&mut self, renderer: &mut Renderer, frame: Option<MarioFrame<'_>>) {
        let next = frame.map(|frame| {
            let index = *self
                .uploaded
                .entry(frame.build)
                .or_insert_with(|| renderer.add_model(frame.template));
            renderer.update_vertices(index, &frame.vertices);
            index
        });
        if let Some(old) = self.shown.filter(|&old| Some(old) != next) {
            renderer.set_visible(old, false);
        }
        if let Some(index) = next {
            renderer.set_visible(index, true);
        }
        self.shown = next;
    }

    /// The number of builds uploaded so far.
    pub fn uploaded(&self) -> usize {
        self.uploaded.len()
    }
}

/// Original shadow meshes share the same upload/update lifecycle as Mario,
/// with the original render layer serving as the mesh identity.
#[derive(Debug, Default)]
pub struct ShadowModelView(MarioModelView);

impl ShadowModelView {
    pub fn show(&mut self, renderer: &mut Renderer, frame: Option<ShadowFrame<'_>>) {
        self.0.show(
            renderer,
            frame.map(|f| MarioFrame {
                build: usize::from(f.layer),
                template: f.template,
                vertices: f.vertices,
            }),
        );
    }
}

/// Camera axes before projection/screen roll, for mtxf_billboard's roll 0.
pub fn billboard_basis(camera: &FlyCamera) -> BillboardBasis {
    let right = camera.right();
    let f = camera.forward();
    BillboardBasis {
        right,
        up: [
            right[1] * f[2] - right[2] * f[1],
            right[2] * f[0] - right[0] * f[2],
            right[0] * f[1] - right[1] * f[0],
        ],
        toward: f.map(|v| -v),
    }
}

#[derive(Default)]
pub struct ObjectModelView {
    uploaded: HashMap<usize, usize>,
}

impl ObjectModelView {
    pub fn show(&mut self, renderer: &mut Renderer, frames: Vec<ObjectFrame<'_>>) {
        for &index in self.uploaded.values() {
            renderer.set_visible(index, false);
        }
        for frame in frames {
            let index = *self
                .uploaded
                .entry(frame.build)
                .or_insert_with(|| renderer.add_model(frame.template));
            renderer.update_dynamic_vertices(index, &frame.vertices);
            renderer.set_visible(index, true);
        }
    }
}

/// The simulated HUD value, including its original every-other-frame count-up.
pub fn coin_count(hud: &rustario64::simulation::hud::HudDisplay) -> Option<i16> {
    use rustario64::simulation::mario::constants::HUD_DISPLAY_FLAG_COIN_COUNT;
    (hud.flags & HUD_DISPLAY_FLAG_COIN_COUNT as i16 != 0).then_some(hud.coins)
}

/// Development HUD typography; original glyphs and the rest of hud.c are pending.
pub fn show_coin_counter(ctx: &egui::Context, hud: &rustario64::simulation::hud::HudDisplay) {
    if let Some(coins) = coin_count(hud) {
        egui::Area::new(egui::Id::new("coin counter"))
            .anchor(egui::Align2::RIGHT_TOP, [-24.0, 20.0])
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_black_alpha(150))
                    .corner_radius(8.0)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(format!("Coins × {coins}"))
                                .size(26.0)
                                .color(egui::Color32::from_rgb(255, 222, 64)),
                        );
                    });
            });
    }
}

/// A placeholder for Mario until his model is imported: a box the size of his
/// hitbox (radius 37, height 160), red with a blue front and nose so his
/// facing shows. Model space: feet at the origin, facing +Z.
pub fn marker() -> VisualModel {
    const SHADE: CombinerCycle = CombinerCycle {
        rgb: [15, 15, 31, 4],
        alpha: [7, 7, 7, 4],
    };
    let material = Material {
        layer: LAYER_OPAQUE,
        combiner: [SHADE, SHADE],
        two_cycle: false,
        texture: None,
        lights: None,
        texture_gen: false,
        prim_color: [0; 4],
        env_color: [0; 4],
        fog: None,
        blend: BlendMode::Opaque,
        depth_test: true,
        depth_write: true,
        decal: false,
        cull_back: false,
        cull_front: false,
    };
    let (r, h) = (37.0, 160.0);
    let mut vertices = vec![];
    let mut quad = |p: [[f32; 3]; 4], color: [u8; 4]| {
        for i in [0, 1, 2, 2, 1, 3] {
            vertices.push(VisualVertex {
                position: p[i],
                uv: [0.0, 0.0],
                color,
            });
        }
    };
    let red = [220, 30, 30, 255];
    let dark = [120, 15, 15, 255];
    let blue = [40, 70, 230, 255];
    // Front (+Z), back, left, right, top.
    quad([[-r, 0.0, r], [r, 0.0, r], [-r, h, r], [r, h, r]], blue);
    quad([[r, 0.0, -r], [-r, 0.0, -r], [r, h, -r], [-r, h, -r]], red);
    quad([[-r, 0.0, -r], [-r, 0.0, r], [-r, h, -r], [-r, h, r]], red);
    quad([[r, 0.0, r], [r, 0.0, -r], [r, h, r], [r, h, -r]], red);
    quad([[-r, h, r], [r, h, r], [-r, h, -r], [r, h, -r]], dark);
    // A nose in front at head height.
    quad(
        [
            [-12.0, 110.0, r + 30.0],
            [12.0, 110.0, r + 30.0],
            [-12.0, 134.0, r],
            [12.0, 134.0, r],
        ],
        blue,
    );
    VisualModel {
        textures: vec![],
        batches: vec![DrawBatch {
            material,
            vertices,
            source: 0,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coin_overlay_reads_the_hud_flags_and_count_and_billboards_ignore_screen_roll() {
        use rustario64::simulation::{
            hud::HudDisplay, mario::constants::HUD_DISPLAY_FLAG_COIN_COUNT,
        };
        let mut hud = HudDisplay {
            coins: 7,
            ..Default::default()
        };
        assert_eq!(coin_count(&hud), None);
        hud.flags = HUD_DISPLAY_FLAG_COIN_COUNT as i16;
        assert_eq!(coin_count(&hud), Some(7));
        let ctx = egui::Context::default();
        let mut text = String::new();
        // An Area establishes its anchored position on the first UI frame.
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |root| {
                show_coin_counter(root.ctx(), &hud);
            });
            output.textures_delta.clear();
            for shape in output.shapes {
                if let egui::epaint::Shape::Text(t) = shape.shape {
                    text.push_str(t.galley.text());
                }
            }
        }
        assert!(text.contains("Coins × 7"), "HUD text: {text}");

        let camera = FlyCamera::looking_at([1000.0, 500.0, 200.0], [0.0; 3]);
        let base = billboard_basis(&camera);
        let rolled = billboard_basis(&FlyCamera {
            roll: 1.3,
            ..camera
        });
        assert_eq!(base.right, rolled.right);
        assert_eq!(base.up, rolled.up);
        let dot = |a: [f32; 3], b: [f32; 3]| (0..3).map(|i| a[i] * b[i]).sum::<f32>();
        assert!(dot(base.up, base.right).abs() < 1e-5);
        assert!(dot(base.up, camera.forward()).abs() < 1e-5);
        assert!((dot(base.up, base.up) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn reference_view_looks_from_lakitu_at_the_focus_with_the_frustum() {
        let view = CameraView {
            pos: [0.0, 500.0, 1000.0],
            focus: [0.0, 500.0, 0.0],
            roll: 0x4000,
            fov: 30.0,
        };
        let frustum = GeoCamera {
            mode: 1,
            position: [0; 3],
            focus: [0; 3],
            fov_degrees: 45,
            near: 100,
            far: 12800,
            callback: 0,
            perspective_callback: None,
        };
        let camera = reference_view(view, Some(frustum));
        assert_eq!(camera.position, view.pos);
        assert!((camera.forward()[2] + 1.0).abs() < 1e-6);
        assert_eq!(
            (camera.fov_y_degrees, camera.near, camera.far),
            (30.0, 100.0, 12800.0)
        );
        assert!((camera.roll - PI / 2.0).abs() < 1e-6);
        // A quarter-turn roll maps view-space right onto up.
        let m = camera.view();
        let right = camera.right();
        let y: f32 = (0..3).map(|k| m[k][1] * right[k]).sum();
        assert!((y - 1.0).abs() < 1e-5, "{y}");
        // Before the first render pass, the frustum's field of view.
        let entry = CameraView { fov: 0.0, ..view };
        assert_eq!(reference_view(entry, Some(frustum)).fov_y_degrees, 45.0);
        assert_eq!(reference_view(entry, None).fov_y_degrees, 45.0);
    }

    #[test]
    fn lod_distance_is_the_depth_in_front_of_the_camera() {
        let camera = FlyCamera::looking_at([0.0, 0.0, 1000.0], [0.0, 0.0, 0.0]);
        assert_eq!(lod_distance(&camera, [0.0, 0.0, 0.0]), 1000);
        assert_eq!(lod_distance(&camera, [300.0, 0.0, 0.5]), 1000);
        assert_eq!(lod_distance(&camera, [0.0, 0.0, 1100.0]), -100);
    }

    #[test]
    fn marker_faces_plus_z_with_its_feet_at_the_origin() {
        let model = marker();
        let vertices = &model.batches[0].vertices;
        assert_eq!(vertices.len() % 3, 0);
        let lowest = vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::MAX, f32::min);
        let front = vertices
            .iter()
            .map(|v| v.position[2])
            .fold(f32::MIN, f32::max);
        assert_eq!(lowest, 0.0);
        assert!(front > 37.0);
    }
}
