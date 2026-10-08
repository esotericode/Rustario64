//! Inspection overlays built as ordinary visual models: original collision tinted
//! by surface class, and markers at placement positions. Presentation only; the
//! floor/wall/ceiling split here mirrors surface_load.c's normal-Y thresholds for
//! display and is not the gameplay collision implementation.
use rustario64::content::{
    CollisionMesh, ImportedLevel,
    visual::{
        BlendMode, CombinerCycle, DrawBatch, LAYER_TRANSPARENT, LAYER_TRANSPARENT_DECAL, Material,
        VisualModel, VisualVertex,
    },
};

/// G_CC_SHADE: output is the vertex color.
const SHADE: CombinerCycle = CombinerCycle {
    rgb: [15, 15, 31, 4],
    alpha: [7, 7, 7, 4],
};

fn material(layer: u8, decal: bool) -> Material {
    Material {
        layer,
        combiner: [SHADE, SHADE],
        two_cycle: false,
        texture: None,
        lights: None,
        texture_gen: false,
        prim_color: [0; 4],
        env_color: [0; 4],
        fog: None,
        blend: BlendMode::Translucent,
        depth_test: true,
        depth_write: false,
        decal,
        cull_back: false,
        cull_front: false,
    }
}

fn vertex(position: [f32; 3], color: [u8; 4]) -> VisualVertex {
    VisualVertex {
        position,
        uv: [0.0, 0.0],
        color,
    }
}

/// Floors blue, walls red, ceilings yellow; special surface types are brighter.
pub fn collision(mesh: &CollisionMesh) -> VisualModel {
    let mut vertices = Vec::with_capacity(mesh.triangles.len() * 3);
    for triangle in &mesh.triangles {
        let p = triangle
            .indices
            .map(|i| mesh.vertices[usize::from(i)].map(f32::from));
        let a = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
        let b = [p[2][0] - p[1][0], p[2][1] - p[1][1], p[2][2] - p[1][2]];
        let n = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let ny = if length > 0.0 { n[1] / length } else { 0.0 };
        let special = triangle.surface != 0;
        let base: [u8; 3] = if ny > 0.01 {
            [40, 90, 255]
        } else if ny < -0.01 {
            [255, 220, 40]
        } else {
            [255, 50, 40]
        };
        let lift: u8 = if special { 60 } else { 0 };
        let color = [
            base[0].saturating_add(lift),
            base[1].saturating_add(lift),
            base[2].saturating_add(lift),
            110,
        ];
        vertices.extend(p.map(|p| vertex(p, color)));
    }
    VisualModel {
        textures: vec![],
        batches: vec![DrawBatch {
            material: material(LAYER_TRANSPARENT_DECAL, true),
            vertices,
            source: 0,
        }],
    }
}

fn marker(out: &mut Vec<VisualVertex>, centre: [i16; 3], size: f32, color: [u8; 4]) {
    let [x, y, z] = centre.map(f32::from);
    let points = [
        [x + size, y, z],
        [x - size, y, z],
        [x, y + size, z],
        [x, y - size, z],
        [x, y, z + size],
        [x, y, z - size],
    ];
    for (a, b, c) in [
        (0, 2, 4),
        (4, 2, 1),
        (1, 2, 5),
        (5, 2, 0),
        (4, 3, 0),
        (1, 3, 4),
        (5, 3, 1),
        (0, 3, 5),
    ] {
        out.extend([points[a], points[b], points[c]].map(|p| vertex(p, color)));
    }
}

/// Markers: Mario start red, script objects orange, macro objects yellow,
/// collision special objects cyan. Positions are original integer units.
pub fn placements(level: &ImportedLevel, collision: &CollisionMesh) -> VisualModel {
    let mut vertices = vec![];
    if let Some((_, _, position)) = level.mario_start {
        marker(&mut vertices, position, 90.0, [255, 30, 30, 230]);
    }
    for area in &level.areas {
        for spawn in &area.spawns {
            marker(&mut vertices, spawn.position, 70.0, [255, 140, 0, 220]);
        }
        for spawn in &area.macro_spawns {
            marker(&mut vertices, spawn.position, 50.0, [255, 240, 40, 220]);
        }
    }
    for special in &collision.specials {
        marker(&mut vertices, special.position, 60.0, [40, 230, 255, 220]);
    }
    VisualModel {
        textures: vec![],
        batches: vec![DrawBatch {
            material: material(LAYER_TRANSPARENT, false),
            vertices,
            source: 0,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustario64::content::Triangle;

    #[test]
    fn collision_overlay_classifies_by_normal_y_and_highlights_special_types() {
        let mesh = CollisionMesh {
            vertices: vec![[0, 0, 0], [0, 0, 100], [100, 0, 0], [0, 100, 0]],
            triangles: vec![
                // Counter-clockwise from above: +Y normal, a floor.
                Triangle {
                    indices: [0, 1, 2],
                    surface: 0,
                    force: None,
                },
                // Reversed: -Y normal, a ceiling.
                Triangle {
                    indices: [0, 2, 1],
                    surface: 0,
                    force: None,
                },
                // Vertical: a wall of a non-default surface type.
                Triangle {
                    indices: [0, 3, 2],
                    surface: 0x0A,
                    force: None,
                },
            ],
            specials: vec![],
            environment: vec![],
        };
        let model = collision(&mesh);
        let colors: Vec<_> = model.batches[0]
            .vertices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| t[0].color)
            .collect();
        assert_eq!(
            colors,
            [
                [40, 90, 255, 110],
                [255, 220, 40, 110],
                [255, 110, 100, 110]
            ]
        );
        assert_eq!(model.batches[0].material.blend, BlendMode::Translucent);
        assert!(!model.batches[0].material.depth_write);
    }
}
