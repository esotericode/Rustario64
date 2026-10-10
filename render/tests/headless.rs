//! Offscreen rendering of independently authored models. Without a GPU adapter
//! these tests skip, unless RUSTARIO64_REQUIRE_GPU=1 (CI installs Mesa lavapipe).
use rustario64::content::visual::*;
use rustario64_render::{RenderOptions, Renderer, camera::FlyCamera};

// Native drivers create worker pools per device. Keep adapter lifetimes
// sequential so the test runner does not exhaust driver resources.
static GPU_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
fn msaa_switches_rebuild_uploaded_pipelines_and_resize_targets() {
    let _gpu = GPU_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    let Some(mut renderer) = renderer(options()) else {
        return;
    };
    let model = VisualModel {
        textures: vec![],
        batches: vec![DrawBatch {
            material: material(SHADE),
            source: 0,
            vertices: triangle([255, 0, 0, 255], false),
        }],
    };
    let index = renderer.add_model(&model);
    let baseline = renderer.capture(64, 64, &camera()).unwrap();
    assert_eq!(centre(&baseline, 64), [255, 0, 0, 255]);
    for (samples, width, height) in [(4, 80, 64), (1, 96, 80), (4, 64, 64), (1, 64, 64)] {
        assert_eq!(renderer.set_sample_count(samples), samples);
        assert!(renderer.is_visible(index));
        let pixels = renderer.capture(width, height, &camera()).unwrap();
        let i = ((height / 2 * width + width / 2) * 4) as usize;
        assert_eq!(&pixels[i..i + 4], &[255, 0, 0, 255]);
        if samples == 4 {
            assert!(
                pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| { p[0] > 0 && p[0] < 255 && p[2] > 0 && p[2] < 255 }),
                "MSAA produced no partially covered edge pixels"
            );
        } else if width == 64 {
            assert_eq!(pixels, baseline);
        }
    }
}

#[test]
fn original_shadow_blends_on_coplanar_floor_and_hides_when_absent() {
    let _gpu = GPU_TESTS.lock().unwrap_or_else(|e| e.into_inner());
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
    let _gpu = GPU_TESTS.lock().unwrap_or_else(|e| e.into_inner());
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
    let _gpu = GPU_TESTS.lock().unwrap_or_else(|e| e.into_inner());
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
    let _gpu = GPU_TESTS.lock().unwrap_or_else(|e| e.into_inner());
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
                    opacity: 255,
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
    let _gpu = GPU_TESTS.lock().unwrap_or_else(|e| e.into_inner());
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
    let _gpu = GPU_TESTS.lock().unwrap_or_else(|e| e.into_inner());
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

/// The window's dialog box painted over an offscreen capture: opaque on the
/// first capture (no egui fade-in), darkening the scene behind it with the
/// original's alpha, with white text on the open page. Authored text.
#[test]
fn dialog_overlay_draws_on_the_first_offscreen_capture() {
    let _gpu = GPU_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    use rustario64::{
        content::dialog::{DialogEntry, DialogTable},
        simulation::dialog::{DialogBox, MENU_STATE_DIALOG_OPEN},
    };
    let Some(mut r) = renderer(RenderOptions::default()) else {
        return;
    };
    let (w, h) = (640, 480);
    let table = DialogTable {
        entries: vec![DialogEntry {
            unused: 1,
            lines_per_box: 4,
            left_offset: 95,
            width: 200,
            // "HELLO HELLO\nHELLO" in the US font codes.
            text: vec![
                0x11, 0x0E, 0x15, 0x15, 0x18, 0x9E, 0x11, 0x0E, 0x15, 0x15, 0x18, 0xFE, 0x11, 0x0E,
                0x15, 0x15, 0x18, 0xFF,
            ],
        }],
    };
    let mut dialog = DialogBox::default();
    dialog.create(0);
    dialog.angle = 0.0;
    dialog.menu_state = MENU_STATE_DIALOG_OPEN;
    dialog.next_page_start = -1;
    let plain = r.capture(w, h, &camera()).unwrap();
    let boxed = r
        .capture_with_overlay(w, h, &camera(), |device, queue, view, format| {
            rustario64_render::desktop::paint_offscreen(device, queue, view, format, 1.0, |ctx| {
                rustario64_render::play::show_dialog_box(ctx, &dialog, &table);
            });
        })
        .unwrap();
    let pixel = |p: &[u8], x: u32, y: u32| {
        let i = ((y * w + x) * 4) as usize;
        [p[i], p[i + 1], p[i + 2]]
    };
    // A corner of the box, away from the text: the scene times 105/255.
    let (x, y) = (w / 2 + 150, (h as f32 * 0.08) as u32 + 4);
    let (before, after) = (pixel(&plain, x, y), pixel(&boxed, x, y));
    for c in 0..3 {
        let expected = f32::from(before[c]) * (105.0 / 255.0);
        assert!(
            (f32::from(after[c]) - expected).abs() <= 3.0,
            "box corner {after:?} is not the scene {before:?} at alpha 150"
        );
    }
    // The page's text: some bright pixels inside the box's text area.
    let top = (h as f32 * 0.08) as u32;
    let bright = (top..top + 80)
        .flat_map(|y| (w / 2 - 180..w / 2 + 60).map(move |x| (x, y)))
        .filter(|&(x, y)| pixel(&boxed, x, y).iter().all(|&v| v > 200))
        .count();
    assert!(bright > 40, "no text drawn ({bright} bright pixels)");
    // Nothing outside the box changes.
    assert_eq!(pixel(&plain, 5, h - 5), pixel(&boxed, 5, h - 5));
}

#[test]
#[ignore = "requires RUSTARIO64_ROM and a GPU adapter; mist's completed opacity"]
fn rom_mist_fades_with_the_original_environment_alpha() {
    let _gpu = GPU_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    use rustario64::{
        import::{engine, objects, rom::Rom},
        presentation::objects::ObjectDrawer,
        simulation::{
            mario::constants as c,
            object::{ObjectId, render::VisibleObject},
        },
    };
    use rustario64_render::play::{ObjectModelView, billboard_basis};
    let Some(mut renderer) = renderer(options()) else {
        return;
    };
    let path = std::env::var_os("RUSTARIO64_ROM").expect("set RUSTARIO64_ROM");
    let rom = Rom::open(std::path::Path::new(&path)).unwrap();
    let content = objects::import(&rom).unwrap();
    let trig = engine::trig_tables(&rom).unwrap();
    let mut drawer = ObjectDrawer::new(&content, &trig);
    let mut view = ObjectModelView::default();
    let camera = FlyCamera::looking_at([0.0, 0.0, 1000.0], [0.0; 3]);
    let mut images = vec![];
    for opacity in [255, 128, 0] {
        drawer
            .update(vec![VisibleObject {
                id: ObjectId(1),
                generation: 1,
                behavior: 0,
                model: c::MODEL_MIST as u16,
                pos: [0.0; 3],
                angle: [0; 3],
                scale: [1.0; 3],
                opacity,
                billboard: true,
                throw_matrix: None,
                animation: None,
                cases: vec![],
            }])
            .unwrap();
        let frames = drawer.frame(1.0, false, billboard_basis(&camera));
        assert!(
            frames
                .iter()
                .flat_map(|f| &f.template.batches)
                .any(|b| b.material.env_color[3] == opacity)
        );
        view.show(&mut renderer, frames);
        images.push(renderer.capture(128, 128, &camera).unwrap());
    }
    let intensity = |pixels: &Vec<u8>| {
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| u64::from(p[0]) + u64::from(p[1]))
            .sum::<u64>()
    };
    assert!(intensity(&images[0]) > intensity(&images[1]));
    assert!(intensity(&images[1]) > 0);
    assert_eq!(intensity(&images[2]), 0);
    assert!(
        images[2]
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [0, 0, 255, 255])
    );
}
