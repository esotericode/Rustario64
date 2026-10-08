//! Offscreen rendering of independently authored models. Without a GPU adapter
//! these tests skip, unless RUSTARIO64_REQUIRE_GPU=1 (CI installs Mesa lavapipe).
use rustario64::content::visual::*;
use rustario64_render::{RenderOptions, Renderer, camera::FlyCamera};

const SHADE: CombinerCycle = CombinerCycle {
    rgb: [15, 15, 31, 4],
    alpha: [7, 7, 7, 4],
};
const MODULATE: CombinerCycle = CombinerCycle {
    rgb: [1, 15, 4, 7],
    alpha: [1, 7, 4, 7],
};

fn renderer(options: RenderOptions) -> Option<Renderer> {
    match rustario64_render::headless(options) {
        Ok((_, renderer)) => Some(renderer),
        Err(e) if std::env::var_os("RUSTARIO64_REQUIRE_GPU").is_none() => {
            eprintln!("skipping GPU test: {e}");
            None
        }
        Err(e) => panic!("GPU required: {e}"),
    }
}

fn material(combiner: CombinerCycle) -> Material {
    Material {
        layer: LAYER_OPAQUE,
        combiner: [combiner, combiner],
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
        cull_back: true,
        cull_front: false,
    }
}

/// A counter-clockwise (front-facing) triangle covering the view centre at z = 0.
fn triangle(color: [u8; 4], flip: bool) -> Vec<VisualVertex> {
    let mut v = vec![
        VisualVertex {
            position: [-500.0, -500.0, 0.0],
            uv: [0.0, 0.0],
            color,
        },
        VisualVertex {
            position: [500.0, -500.0, 0.0],
            uv: [1.0, 0.0],
            color,
        },
        VisualVertex {
            position: [0.0, 500.0, 0.0],
            uv: [0.5, 1.0],
            color,
        },
    ];
    if flip {
        v.swap(1, 2);
    }
    v
}

fn camera() -> FlyCamera {
    FlyCamera::looking_at([0.0, 0.0, 1000.0], [0.0, 0.0, 0.0])
}

fn centre(pixels: &[u8], size: u32) -> [u8; 4] {
    let i = ((size / 2) * size + size / 2) as usize * 4;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}

fn options() -> RenderOptions {
    RenderOptions {
        clear_color: [0.0, 0.0, 1.0, 1.0],
        ..RenderOptions::default()
    }
}

#[test]
fn shade_combiner_draws_vertex_colors_and_culls_back_faces() {
    let Some(mut renderer) = renderer(options()) else {
        return;
    };
    let model = VisualModel {
        textures: vec![],
        batches: vec![DrawBatch {
            material: material(SHADE),
            vertices: triangle([255, 0, 0, 255], false),
            source: 0,
        }],
    };
    renderer.load_model(&model);
    let pixels = renderer.capture(64, 64, &camera()).unwrap();
    assert_eq!(centre(&pixels, 64), [255, 0, 0, 255]);
    // Same triangle wound clockwise is culled by G_CULL_BACK.
    let culled = VisualModel {
        textures: vec![],
        batches: vec![DrawBatch {
            material: material(SHADE),
            vertices: triangle([255, 0, 0, 255], true),
            source: 0,
        }],
    };
    renderer.load_model(&culled);
    let pixels = renderer.capture(64, 64, &camera()).unwrap();
    assert_eq!(centre(&pixels, 64), [0, 0, 255, 255]);
}

#[test]
fn modulate_combiner_multiplies_texture_by_shade_and_cutout_discards() {
    let Some(mut renderer) = renderer(options()) else {
        return;
    };
    let mut textured = material(MODULATE);
    textured.texture = Some(TextureBinding {
        texture: 0,
        wrap: [WrapMode::Clamp, WrapMode::Clamp],
        filter: TextureFilter::Point,
    });
    let texture = |rgba: [u8; 4]| TextureImage {
        width: 1,
        height: 1,
        rgba: rgba.to_vec(),
        source: 0,
        format: 0,
        size: 2,
    };
    let model = VisualModel {
        textures: vec![texture([255, 255, 0, 255])],
        batches: vec![DrawBatch {
            material: textured,
            vertices: triangle([255, 0, 255, 255], false),
            source: 0,
        }],
    };
    renderer.load_model(&model);
    let pixels = renderer.capture(64, 64, &camera()).unwrap();
    // (255, 255, 0) * (255, 0, 255) = (255, 0, 0).
    assert_eq!(centre(&pixels, 64), [255, 0, 0, 255]);
    let mut cutout = textured;
    cutout.blend = BlendMode::Cutout;
    let model = VisualModel {
        textures: vec![texture([255, 255, 255, 0])],
        batches: vec![DrawBatch {
            material: cutout,
            vertices: triangle([255, 255, 255, 255], false),
            source: 0,
        }],
    };
    renderer.load_model(&model);
    let pixels = renderer.capture(64, 64, &camera()).unwrap();
    assert_eq!(centre(&pixels, 64), [0, 0, 255, 255]);
}
