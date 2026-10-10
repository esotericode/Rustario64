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
fn original_shadow_blends_on_coplanar_floor_and_hides_when_absent() {
    use rustario64::{
        import::shadow::ShadowSource,
        presentation::shadow::{Shadow, ShadowDrawer, ShadowVertex},
    };
    use rustario64_render::play::ShadowModelView;
    let Some(mut renderer) = renderer(options()) else {
        return;
    };
    let mut floor_material = material(SHADE);
    floor_material.cull_back = false;
    let floor = VisualModel {
        textures: vec![],
        batches: vec![DrawBatch {
            material: floor_material,
            source: 0,
            vertices: [
                [-500.0, 0.0, -500.0],
                [-500.0, 0.0, 500.0],
                [500.0, 0.0, -500.0],
                [500.0, 0.0, -500.0],
                [-500.0, 0.0, 500.0],
                [500.0, 0.0, 500.0],
            ]
            .map(|position| VisualVertex {
                position,
                uv: [0.0; 2],
                color: [255; 4],
            })
            .to_vec(),
        }],
    };
    let camera = FlyCamera::looking_at([0.0, 600.0, 200.0], [0.0, 0.0, 0.0]);
    renderer.load_model(&floor);
    assert_eq!(
        centre(&renderer.capture(64, 64, &camera).unwrap(), 64),
        [255; 4]
    );
    let mut drawer = ShadowDrawer::new(&ShadowSource {
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
    });
    drawer.update(Some(Shadow {
        origin: [0.0; 3],
        layer: LAYER_TRANSPARENT_DECAL,
        vertices: std::array::from_fn(|i| ShadowVertex {
            position: [(i as i16 % 3 - 1) * 100, 0, (i as i16 / 3 - 1) * 100],
            uv: [0; 2],
            alpha: 180,
        }),
    }));
    let mut view = ShadowModelView::default();
    view.show(&mut renderer, drawer.frame(1.0, true));
    let pixel = centre(&renderer.capture(64, 64, &camera).unwrap(), 64);
    assert!(
        (70..=80).contains(&pixel[0]),
        "coplanar shadow pixel {pixel:?}"
    );
    assert_eq!(pixel[0], pixel[1]);
    assert_eq!(pixel[1], pixel[2]);
    // A foreground floor must occlude the shadow; the decal does not float
    // through geometry in front of its receiving floor.
    let mut foreground = floor.clone();
    for v in &mut foreground.batches[0].vertices {
        v.position[1] = 100.0;
    }
    let index = renderer.add_model(&foreground);
    assert_eq!(
        centre(&renderer.capture(64, 64, &camera).unwrap(), 64),
        [255; 4]
    );
    renderer.set_visible(index, false);
    view.show(&mut renderer, None);
    assert_eq!(
        centre(&renderer.capture(64, 64, &camera).unwrap(), 64),
        [255; 4]
    );
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
fn dynamic_object_batches_grow_shrink_empty_and_reappear_without_stale_geometry() {
    let Some(mut renderer) = renderer(options()) else {
        return;
    };
    let original = triangle([255, 0, 0, 255], false);
    let model = VisualModel {
        textures: vec![],
        batches: vec![DrawBatch {
            material: material(SHADE),
            vertices: original.clone(),
            source: 0,
        }],
    };
    renderer.load_model(&model);
    let aside: Vec<_> = original
        .iter()
        .map(|v| VisualVertex {
            position: [v.position[0] + 3000.0, v.position[1], v.position[2]],
            ..*v
        })
        .collect();
    // Growing to two instances must allocate a larger buffer.
    let mut both = aside.clone();
    both.extend(&original);
    renderer.update_dynamic_vertices(0, &[both]);
    assert_eq!(
        centre(&renderer.capture(64, 64, &camera()).unwrap(), 64),
        [255, 0, 0, 255]
    );
    // The old second triangle must stop drawing when the batch shrinks.
    renderer.update_dynamic_vertices(0, &[aside]);
    assert_eq!(
        centre(&renderer.capture(64, 64, &camera()).unwrap(), 64),
        [0, 0, 255, 255]
    );
    renderer.update_dynamic_vertices(0, &[vec![]]);
    assert_eq!(
        centre(&renderer.capture(64, 64, &camera()).unwrap(), 64),
        [0, 0, 255, 255]
    );
    renderer.update_dynamic_vertices(0, &[original]);
    assert_eq!(
        centre(&renderer.capture(64, 64, &camera()).unwrap(), 64),
        [255, 0, 0, 255]
    );
}

#[test]
#[ignore = "requires RUSTARIO64_ROM and a GPU adapter; draws every ROM coin/sparkle case"]
fn rom_coins_and_sparkles_draw_as_camera_facing_textured_cutouts() {
    use rustario64::{
        import::{engine, objects, rom::Rom},
        presentation::objects::ObjectDrawer,
        simulation::{
            mario::constants as c,
            object::{
                ObjectId,
                render::{RenderNodeKind, VisibleObject},
            },
        },
    };
    use rustario64_render::play::{ObjectModelView, billboard_basis};
    let Some(mut renderer) = renderer(options()) else {
        return;
    };
    let path = std::env::var_os("RUSTARIO64_ROM").expect("set RUSTARIO64_ROM");
    let rom = Rom::open(std::path::Path::new(&path)).unwrap();
    let content = objects::import(&rom).unwrap();
    let models = content.models(&[], None);
    let trig = engine::trig_tables(&rom).unwrap();
    let mut drawer = ObjectDrawer::new(&content, &trig);
    let mut view = ObjectModelView::default();
    for model in [
        c::MODEL_YELLOW_COIN,
        c::MODEL_YELLOW_COIN_NO_SHADOW,
        c::MODEL_SPARKLES,
    ] {
        let traversal = models.traversal(model as u16).unwrap();
        let (node, switch) = traversal
            .nodes
            .iter()
            .enumerate()
            .find(|(_, n)| matches!(n.kind, RenderNodeKind::AnimStateSwitch { .. }))
            .unwrap();
        for &child in &switch.children {
            drawer
                .update(vec![VisibleObject {
                    id: ObjectId(1),
                    generation: 1,
                    behavior: 0,
                    model: model as u16,
                    pos: [0.0, -32.0, 0.0],
                    angle: [0; 3],
                    scale: [1.0; 3],
                    billboard: true,
                    throw_matrix: None,
                    animation: None,
                    cases: vec![(node, child)],
                }])
                .unwrap();
            for camera in [
                FlyCamera::looking_at([0.0, 0.0, 400.0], [0.0; 3]),
                FlyCamera::looking_at([400.0, 0.0, 0.0], [0.0; 3]),
            ] {
                view.show(
                    &mut renderer,
                    drawer.frame(1.0, false, billboard_basis(&camera)),
                );
                let pixels = renderer.capture(128, 128, &camera).unwrap();
                let changed = pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|p| **p != [0, 0, 255, 255])
                    .count();
                assert!(
                    changed > 0 && changed < 1600,
                    "model {model}, child {child}: {changed} pixels"
                );
                // Transparent corners must leave the background visible.
                assert_eq!(&pixels[..4], &[0, 0, 255, 255]);
            }
        }
    }
    view.show(&mut renderer, vec![]);
    assert_eq!(
        centre(&renderer.capture(128, 128, &camera()).unwrap(), 128),
        [0, 0, 255, 255]
    );
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

#[test]
fn model_transform_translates_and_turns_a_model() {
    let Some(mut renderer) = renderer(options()) else {
        return;
    };
    let mut two_sided = material(SHADE);
    two_sided.cull_back = false;
    let model = VisualModel {
        textures: vec![],
        batches: vec![DrawBatch {
            material: two_sided,
            vertices: triangle([0, 255, 0, 255], false),
            source: 0,
        }],
    };
    renderer.load_model(&model);
    let pixels = renderer.capture(64, 64, &camera()).unwrap();
    assert_eq!(centre(&pixels, 64), [0, 255, 0, 255]);
    // Moved aside, the centre shows the clear color.
    renderer.set_transform(0, [2000.0, 0.0, 0.0], 0.0);
    let pixels = renderer.capture(64, 64, &camera()).unwrap();
    assert_eq!(centre(&pixels, 64), [0, 0, 255, 255]);
    // Turned a quarter about +Y, the triangle is edge-on to the camera.
    renderer.set_transform(0, [0.0, 0.0, 0.0], std::f32::consts::FRAC_PI_2);
    let pixels = renderer.capture(64, 64, &camera()).unwrap();
    assert_eq!(centre(&pixels, 64), [0, 0, 255, 255]);
    // A half turn faces it away; without culling it is drawn again.
    renderer.set_transform(0, [0.0, 0.0, 0.0], std::f32::consts::PI);
    let pixels = renderer.capture(64, 64, &camera()).unwrap();
    assert_eq!(centre(&pixels, 64), [0, 255, 0, 255]);
}
