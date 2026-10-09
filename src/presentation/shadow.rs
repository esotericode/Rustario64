//! Mario's original nine-vertex blob shadow, translated from pinned CC0
//! src/game/shadow.c. BOB's floor, water and ice paths are supported; special
//! lava levels and flying carpets belong with those levels' implementations.
//! Queries are read-only and no render frame advances gameplay or animation.
use crate::{
    content::visual::*,
    import::shadow::ShadowSource,
    simulation::{
        collision::{CollisionFlags, CollisionWorld, f32_to_s16},
        mario::constants::*,
        math::TrigTables,
    },
};

/// Original signed object-relative Vtx coordinates, 10.5 texture coordinates
/// and opacity. Kept before presentation interpolation for native comparison.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShadowVertex {
    pub position: [i16; 3],
    pub uv: [i16; 2],
    pub alpha: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Shadow {
    pub origin: [f32; 3],
    pub layer: u8,
    pub vertices: [ShadowVertex; 9],
}

fn floor(
    world: &CollisionWorld,
    p: [f32; 3],
) -> (f32, Option<crate::simulation::collision::SurfaceIndex>) {
    world.find_floor(p[0], p[1], p[2], &mut CollisionFlags::default())
}

fn scale_with_distance(initial: f32, distance: f32) -> f32 {
    if distance <= 0.0 {
        initial
    } else if distance >= 600.0 {
        (f64::from(initial) * 0.5) as f32
    } else {
        (f64::from(initial) * (1.0 - 0.5 * f64::from(distance) / 600.0)) as f32
    }
}

fn dim_with_distance(solidity: u8, distance: f32) -> u8 {
    if solidity < 121 || distance <= 0.0 {
        solidity
    } else if distance >= 600.0 {
        120
    } else {
        let product = (120 - i32::from(solidity)) as f32 * distance;
        let ret = (f64::from(product) / 600.0 + f64::from(solidity)) as f32;
        ret as u8
    }
}

fn ledge_alpha(solidity: u8, frame: i16, start: i16, end: i16) -> u8 {
    if frame >= 0 && frame < start {
        0
    } else if frame > end {
        solidity
    } else {
        (f32::from(solidity) * f32::from(frame - start) / f32::from(end - start)) as u8
    }
}

fn rounded(value: f32) -> i16 {
    // round_float uses double literals, then narrows to a wrapping s16.
    (if value >= 0.0 {
        f64::from(value) + 0.5
    } else {
        f64::from(value) - 0.5
    }) as i32 as i16
}

/// Original player-shadow branch, given geo_process_shadow's root position
/// and scaled diameter. anim_id is AnimInfo.animID, not the DMA clip entry.
pub fn player_shadow(
    world: &CollisionWorld,
    trig: &TrigTables,
    origin: [f32; 3],
    scale: i16,
    solidity: u8,
    anim_id: i16,
    frame: i16,
) -> Option<Shadow> {
    let (mut height, surface) = floor(world, origin);
    let water = world.find_water_level(origin[0], origin[2]);
    let above_water =
        world.environment.is_some() && water >= -10000.0 && origin[1] >= water && height <= water;
    let (normal, offset) = if above_water {
        height = water;
        ([0.0, 1.0, 0.0], -water)
    } else {
        if height < -10000.0 {
            return None;
        }
        // The original can dereference NULL after an intangible-floor retry.
        // Suppress that undefined case rather than invent floor geometry.
        let surface = world.surface(surface?);
        if surface.normal[1] <= 0.0 {
            return None;
        }
        (surface.normal, surface.origin_offset)
    };
    let distance = origin[1] - height;
    let alpha = match i32::from(anim_id) {
        MARIO_ANIM_IDLE_ON_LEDGE => return None,
        MARIO_ANIM_FAST_LEDGE_GRAB => ledge_alpha(solidity, frame, 5, 14),
        MARIO_ANIM_SLOW_LEDGE_GRAB => ledge_alpha(solidity, frame, 21, 33),
        MARIO_ANIM_CLIMB_DOWN_LEDGE => {
            if (0..=5).contains(&frame) {
                (f64::from(solidity) * (1.0 - f64::from(f32::from(frame) / 5.0))) as u8
            } else {
                0
            }
        }
        _ => dim_with_distance(solidity, distance),
    };
    let size = scale_with_distance(f32::from(scale), distance);
    let atan_deg = |a, b| (f64::from(trig.atan2s(a, b)) / 65535.0 * 360.0) as f32;
    let angle = atan_deg(normal[2], normal[0]);
    let steepness = (normal[0] * normal[0] + normal[2] * normal[2]).sqrt();
    let tilt = if steepness == 0.0 {
        0.0
    } else {
        (90.0 - f64::from(atan_deg(steepness, normal[1]))) as f32
    };
    let radians = |degrees: f32| (f64::from(degrees) * std::f64::consts::PI / 180.0) as f32;
    let tilted_size = radians(tilt).cos() * size;
    let (sin, cos) = radians(angle).sin_cos();
    let vertices = std::array::from_fn(|index| {
        let x_unit = index as i32 % 3 - 1;
        let z_unit = index as i32 / 3 - 1;
        let half = (f64::from(x_unit as f32 * size) / 2.0) as f32;
        let tilted_half = (f64::from(z_unit as f32 * tilted_size) / 2.0) as f32;
        let x = tilted_half * sin + half * cos + origin[0];
        let z = tilted_half * cos - half * sin + origin[2];
        let mut y = if above_water {
            height
        } else {
            floor(world, [x, origin[1], z]).0
        };
        let mut vertex_alpha = if above_water { 200 } else { alpha };
        let dot =
            (x - origin[0]) * normal[0] + (y - height) * normal[1] + (z - origin[2]) * normal[2];
        if !above_water && f32_to_s16(dot) != 0 {
            y = -(normal[0] * x + normal[2] * z + offset) / normal[1];
            vertex_alpha = 0;
        }
        ShadowVertex {
            position: [
                rounded(x - origin[0]),
                rounded(y - origin[1]),
                rounded(z - origin[2]),
            ],
            uv: [(x_unit * 15 * 32) as i16, (z_unit * 15 * 32) as i16],
            alpha: vertex_alpha,
        }
    });
    let ice = surface.is_some_and(|i| world.surface(i).surface_type == SURFACE_ICE);
    Some(Shadow {
        origin,
        layer: if above_water {
            LAYER_ALPHA
        } else if ice {
            LAYER_TRANSPARENT
        } else {
            LAYER_TRANSPARENT_DECAL
        },
        vertices,
    })
}

/// dl_shadow_9_verts, in original triangle order (bin/segment2.c).
const TRIANGLES: [usize; 24] = [
    0, 3, 4, 0, 4, 1, 1, 4, 2, 2, 4, 5, 3, 6, 4, 4, 6, 7, 4, 7, 8, 4, 8, 5,
];

fn vertices(shadow: &Shadow) -> Vec<VisualVertex> {
    TRIANGLES
        .iter()
        .map(|&i| {
            let vertex = shadow.vertices[i];
            VisualVertex {
                position: std::array::from_fn(|axis| {
                    shadow.origin[axis] + f32::from(vertex.position[axis])
                }),
                uv: vertex.uv.map(|v| f32::from(v) / (32.0 * 16.0)),
                color: [255, 255, 255, vertex.alpha],
            }
        })
        .collect()
}

pub struct ShadowFrame<'a> {
    pub template: &'a VisualModel,
    pub layer: u8,
    pub vertices: Vec<Vec<VisualVertex>>,
}

/// One mesh per original layer, uploaded once. Completed-tick geometry and
/// alpha are interpolated independently of the simulation at display time.
pub struct ShadowDrawer {
    models: [VisualModel; 3],
    previous: Option<Shadow>,
    current: Option<Shadow>,
}

impl ShadowDrawer {
    pub fn new(source: &ShadowSource) -> Self {
        const MODULATE_IA: CombinerCycle = CombinerCycle {
            rgb: [1, 15, 4, 7],
            alpha: [1, 7, 4, 7],
        };
        let models = std::array::from_fn(|i| {
            let layer = i as u8 + LAYER_ALPHA;
            VisualModel {
                textures: vec![source.texture.clone()],
                batches: vec![DrawBatch {
                    source: 0x020145D8,
                    vertices: vec![
                        VisualVertex {
                            position: [0.0; 3],
                            uv: [0.0; 2],
                            color: [255; 4]
                        };
                        24
                    ],
                    material: Material {
                        layer,
                        combiner: [MODULATE_IA; 2],
                        two_cycle: false,
                        texture: Some(TextureBinding {
                            texture: 0,
                            wrap: [WrapMode::MirrorRepeat; 2],
                            filter: TextureFilter::Bilinear,
                        }),
                        lights: None,
                        texture_gen: false,
                        prim_color: [0; 4],
                        env_color: [0; 4],
                        fog: None,
                        blend: if layer == LAYER_ALPHA {
                            BlendMode::Cutout
                        } else {
                            BlendMode::Translucent
                        },
                        depth_test: true,
                        depth_write: layer == LAYER_ALPHA,
                        decal: layer == LAYER_TRANSPARENT_DECAL,
                        cull_back: false,
                        cull_front: false,
                    },
                }],
            }
        });
        Self {
            models,
            previous: None,
            current: None,
        }
    }
    pub fn update(&mut self, shadow: Option<Shadow>) {
        self.previous = self.current.take();
        self.current = shadow;
    }
    pub fn snap(&mut self) {
        self.previous = None;
    }
    pub fn reset(&mut self) {
        self.previous = None;
        self.current = None;
    }
    pub fn frame(&self, alpha: f32, interpolation: bool) -> Option<ShadowFrame<'_>> {
        let current = self.current.as_ref()?;
        let mut result = vertices(current);
        if let Some(previous) = self
            .previous
            .as_ref()
            .filter(|p| interpolation && alpha.is_finite() && p.layer == current.layer)
        {
            let alpha = alpha.clamp(0.0, 1.0);
            for (v, p) in result.iter_mut().zip(vertices(previous)) {
                for axis in 0..3 {
                    v.position[axis] =
                        p.position[axis] + (v.position[axis] - p.position[axis]) * alpha;
                }
                v.color[3] = (f32::from(p.color[3])
                    + (f32::from(v.color[3]) - f32::from(p.color[3])) * alpha)
                    as u8;
            }
        }
        Some(ShadowFrame {
            template: &self.models[usize::from(current.layer - LAYER_ALPHA)],
            layer: current.layer,
            vertices: vec![result],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drawer() -> ShadowDrawer {
        ShadowDrawer::new(&ShadowSource {
            texture: TextureImage {
                width: 1,
                height: 1,
                rgba: vec![0, 0, 0, 255],
                source: 0,
                format: 3,
                size: 1,
            },
            scale: 100,
            solidity: 180,
            child_scale: 0.25,
        })
    }

    fn shadow(x: f32, alpha: u8, layer: u8) -> Shadow {
        Shadow {
            origin: [x, 0.0, 0.0],
            layer,
            vertices: [ShadowVertex {
                position: [0; 3],
                uv: [0; 2],
                alpha,
            }; 9],
        }
    }

    #[test]
    fn display_frames_interpolate_position_and_opacity_without_advancing_ticks() {
        let mut d = drawer();
        d.update(Some(shadow(0.0, 180, LAYER_TRANSPARENT_DECAL)));
        d.update(Some(shadow(100.0, 120, LAYER_TRANSPARENT_DECAL)));
        for (alpha, x, opacity) in [
            (0.0, 0.0, 180),
            (0.25, 25.0, 165),
            (0.5, 50.0, 150),
            (1.0, 100.0, 120),
        ] {
            let frame = d.frame(alpha, true).unwrap();
            let v = frame.vertices[0][0];
            assert_eq!((v.position[0], v.color[3]), (x, opacity));
        }
        assert_eq!(d.frame(0.5, true).unwrap().vertices[0][0].position[0], 50.0);
        assert_eq!(
            d.frame(0.0, false).unwrap().vertices[0][0].position[0],
            100.0
        );
        assert_eq!(
            d.frame(f32::NAN, true).unwrap().vertices[0][0].position[0],
            100.0
        );
        d.snap();
        assert_eq!(
            d.frame(0.0, true).unwrap().vertices[0][0].position[0],
            100.0
        );
        d.reset();
        assert!(d.frame(1.0, true).is_none());
    }

    #[test]
    fn missing_shadow_and_render_layer_changes_clear_interpolation() {
        let mut d = drawer();
        d.update(Some(shadow(0.0, 180, LAYER_TRANSPARENT_DECAL)));
        d.update(None);
        assert!(d.frame(0.5, true).is_none());
        d.update(Some(shadow(50.0, 200, LAYER_ALPHA)));
        assert_eq!(d.frame(0.0, true).unwrap().vertices[0][0].position[0], 50.0);
        d.update(Some(shadow(100.0, 180, LAYER_TRANSPARENT)));
        let frame = d.frame(0.0, true).unwrap();
        assert_eq!(frame.layer, LAYER_TRANSPARENT);
        assert_eq!(frame.vertices[0][0].position[0], 100.0);
    }
}
