//! Presentation for the core's play session (`rustario64::play`): where the
//! follow camera's eye goes, Mario's posed model in the renderer, and a
//! placeholder for when his model is unavailable. Nothing here reaches the
//! simulation; the camera yaw the tick reads comes from the session's
//! `FollowCamera`, which turns only once per tick.
use crate::{Renderer, camera::FlyCamera};
use rustario64::{
    content::visual::{
        BlendMode, CombinerCycle, DrawBatch, GeoCamera, LAYER_OPAQUE, Material, VisualModel,
        VisualVertex,
    },
    play::FollowCamera,
    presentation::mario::MarioFrame,
};
use std::{collections::HashMap, f32::consts::PI};

/// How far behind and above Mario the follow camera's eye sits.
pub const FOLLOW_DISTANCE: f32 = 1000.0;
pub const FOLLOW_HEIGHT: f32 = 400.0;
/// The camera looks at this height above Mario's feet.
pub const FOLLOW_FOCUS_HEIGHT: f32 = 120.0;

/// Original angle units to radians.
pub fn radians(angle: i16) -> f32 {
    f32::from(angle) * PI / 32768.0
}

/// A presentation camera behind `focus` (Mario's interpolated position) at
/// the follow camera's interpolated yaw, with the area camera's frustum.
pub fn follow_view(
    camera: &FollowCamera,
    focus: [f32; 3],
    alpha: f32,
    frustum: Option<GeoCamera>,
) -> FlyCamera {
    let yaw = radians(camera.presentation_yaw(alpha));
    let eye = [
        focus[0] + yaw.sin() * FOLLOW_DISTANCE,
        focus[1] + FOLLOW_HEIGHT,
        focus[2] + yaw.cos() * FOLLOW_DISTANCE,
    ];
    let target = [focus[0], focus[1] + FOLLOW_FOCUS_HEIGHT, focus[2]];
    let mut view = FlyCamera::looking_at(eye, target);
    if let Some(original) = frustum {
        view.fov_y_degrees = f32::from(original.fov_degrees);
        view.near = f32::from(original.near);
        view.far = f32::from(original.far);
    }
    view
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
    use rustario64::play::Pad;

    #[test]
    fn follow_view_sits_behind_mario_and_interpolates_its_yaw() {
        // Behind a Mario facing +X is toward -X.
        let mut camera = FollowCamera::behind(0x4000);
        let view = follow_view(&camera, [0.0; 3], 1.0, None);
        assert!(view.position[0] < -900.0 && view.position[2].abs() < 1.0);
        assert!(view.position[1] > 0.0 && view.forward()[0] > 0.9);
        camera.advance(&Pad {
            camera_left: true,
            ..Pad::default()
        });
        // Halfway between ticks the presentation yaw is halfway.
        let half = follow_view(&camera, [0.0; 3], 0.5, None).position[2];
        let full = follow_view(&camera, [0.0; 3], 1.0, None).position[2];
        assert!(half > 0.0 && half < full);
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
        assert_eq!(
            follow_view(&camera, [0.0; 3], 1.0, Some(frustum)).far,
            12800.0
        );
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
