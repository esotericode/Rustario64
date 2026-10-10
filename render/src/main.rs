//! Development viewer for imported levels. `screenshot` renders offscreen to PNG
//! (no window needed); `view` opens a window with a free inspection camera or,
//! in Mario mode, Mario and the original camera driven by the frame compared
//! with the decomp. Both are development entry points straight into
//! Bob-omb Battlefield.
use rustario64::{
    content::{
        animation::MarioAnimations,
        visual::{AreaVisual, VisualModel},
    },
    import::{
        animation, bob, engine,
        mario::{self as mario_model, MarioModelSource},
        objects,
        rom::Rom,
        shadow::{self, ShadowSource},
    },
    play::{self, BOB_SCRIPT_START, Pad, Session},
    presentation::{
        GraphicsOptions,
        mario::{MarioDrawer, shadow_origin},
        objects::{LevelModels, ObjectDrawer},
        shadow::{ShadowDrawer, player_shadow},
    },
    simulation::{
        collision::CollisionWorld, game::GameEntry, math::TrigTables,
        object::render::visible_objects,
    },
};
use rustario64_render::{
    RenderOptions, Renderer, camera,
    camera::FlyCamera,
    controller::Controllers,
    desktop::{FrameClock, Settings, Ui},
    overlay, play as present,
};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, DeviceId, ElementState, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Fullscreen, Window, WindowId},
};

type AppResult<T> = Result<T, Box<dyn std::error::Error>>;

const HELP: &str = "Rustario64 development viewer (Bob-omb Battlefield, area 1)\n\n\
  rustario64-viewer [launch]   Open the local ROM launcher and presentation settings.\n\
  rustario64-viewer screenshot /path/to/sm64.z64 --out private/bob.png [--view start|overview|summit|top] [--size 1280x960] [--msaa 4] [--no-cull] [--no-fog] [--collision] [--placements] [--mario-ticks N] [--start X,Y,Z[,YAW]]\n\
  rustario64-viewer view /path/to/sm64.z64 [--view start] [--msaa 4] [--no-fog] [--frames N] [--mario] [--no-interpolation] [--record NEW_PRIVATE_DIR] [--start X,Y,Z[,YAW]]\n\n\
  View controls: WASD move, Q/E down/up, Shift faster, hold right mouse or arrow keys to look,\n\
  1-4 select presets, C collision overlay, P placement markers, F fog, M Mario mode, Esc pause/settings.\n\
  The camera is a presentation-only inspection camera, not the original camera.\n\n\
  Mario mode (--mario or M): WASD stick (hold Shift to walk), Space A, J B, K Z, arrow keys\n\
  the C buttons (Up/Down zoom and first person, Left/Right rotate), E the R button (Lakitu or\n\
  Mario camera), R re-enters the level, M returns to the free camera (Mario pauses). Mario and\n\
  the original camera run the frame compared with the decomp from the level script's start;\n\
  the window draws from the camera's view. Mario's model and animations come from the ROM (a\n\
  placeholder box if they cannot be imported). Play stops at paths the port does not support\n\
  and at warps (falling off the course); R re-enters.\n\
  --mario-ticks N renders Mario after N frames of holding the stick up, from the original camera.\n\
  --start X,Y,Z[,YAW] enters the level with Mario at another point (development entry; yaw in degrees).\n\
  --record writes each run's tick inputs to a new directory for exact replay against the decomp:\n\
  cargo run -p rustario64-oracle --example tick_trace -- ROM NEW_DIR --inputs RUN.inputs.json\n\
  Presentation: --size WIDTHxHEIGHT, --fullscreen, --no-vsync; Esc pauses, resumes or opens Quit.\n";

struct Options {
    view: Option<String>,
    size: (u32, u32),
    msaa: u32,
    cull: bool,
    fog: bool,
    collision: bool,
    placements: bool,
    out: Option<String>,
    frames: Option<u64>,
    mario: bool,
    mario_ticks: Option<u64>,
    interpolation: bool,
    record: Option<PathBuf>,
    vsync: bool,
    fullscreen: bool,
    /// A development start point for Mario: position and yaw in degrees.
    start: Option<([i16; 3], i16)>,
}

fn parse(args: &[String]) -> AppResult<Options> {
    let mut options = Options {
        view: None,
        size: (1280, 960),
        msaa: 1,
        cull: true,
        fog: true,
        collision: false,
        placements: false,
        out: None,
        frames: None,
        mario: false,
        mario_ticks: None,
        interpolation: true,
        record: None,
        vsync: true,
        fullscreen: false,
        start: None,
    };
    let mut i = 0;
    while i < args.len() {
        let value = |i: usize| -> AppResult<&String> {
            args.get(i + 1)
                .ok_or_else(|| format!("{} needs a value", args[i]).into())
        };
        let flag = match args[i].as_str() {
            "--no-cull" => &mut options.cull,
            "--no-fog" => &mut options.fog,
            "--no-interpolation" => &mut options.interpolation,
            "--collision" => &mut options.collision,
            "--placements" => &mut options.placements,
            "--mario" => &mut options.mario,
            "--no-vsync" => &mut options.vsync,
            "--fullscreen" => &mut options.fullscreen,
            _ => {
                match args[i].as_str() {
                    "--view" => options.view = Some(value(i)?.clone()),
                    "--out" => options.out = Some(value(i)?.clone()),
                    "--size" => {
                        let (w, h) = value(i)?
                            .split_once('x')
                            .ok_or("size must look like 1280x960")?;
                        options.size = (w.parse()?, h.parse()?);
                    }
                    "--msaa" => options.msaa = value(i)?.parse()?,
                    "--frames" => options.frames = Some(value(i)?.parse()?),
                    "--mario-ticks" => options.mario_ticks = Some(value(i)?.parse()?),
                    "--record" => options.record = Some(value(i)?.into()),
                    "--start" => {
                        let parts: Vec<i16> = value(i)?
                            .split(',')
                            .map(str::parse)
                            .collect::<Result<_, _>>()?;
                        options.start = match parts[..] {
                            [x, y, z] => Some(([x, y, z], 0)),
                            [x, y, z, yaw] => Some(([x, y, z], yaw)),
                            _ => return Err("--start takes X,Y,Z or X,Y,Z,YAW".into()),
                        };
                    }
                    other => return Err(format!("unknown option {other}\n\n{HELP}").into()),
                }
                i += 2;
                continue;
            }
        };
        // Flags turn on a feature or turn off a default.
        *flag = !args[i].starts_with("--no-");
        i += 1;
    }
    if !matches!(options.msaa, 1 | 4) {
        return Err("--msaa must be 1 or 4".into());
    }
    if options.size.0 == 0 || options.size.1 == 0 || options.size.0 > 8192 || options.size.1 > 8192
    {
        return Err("size must be within 1..=8192 per side".into());
    }
    Ok(options)
}

struct Level {
    visual: AreaVisual,
    collision: VisualModel,
    placements: VisualModel,
    mario_start: Option<(i16, [i16; 3])>,
    /// Mario's simulation data. The viewer keeps one level for the life of
    /// the process, so sessions borrow it for 'static.
    world: &'static CollisionWorld,
    trig: &'static TrigTables,
    anims: &'static MarioAnimations,
    /// The level's objects: the ROM's behavior scripts, models and BOB's
    /// placements (act 1).
    objects: &'static objects::LevelObjectContent,
    entry: GameEntry,
    rom_sha1: &'static str,
    /// Mario's model, or None when it could not be imported.
    mario_model: Option<&'static MarioModelSource>,
    shadow: Option<&'static ShadowSource>,
}

impl Level {
    fn session(&self) -> Session<'static> {
        Session::new(
            self.world,
            self.trig,
            self.anims,
            self.objects.level_objects(),
            self.entry,
        )
    }
}

/// Mario's imported model, posed from each completed tick.
struct MarioModel {
    drawer: MarioDrawer<'static>,
    view: present::MarioModelView,
    shadow: Option<(ShadowDrawer, &'static ShadowSource)>,
    shadow_view: present::ShadowModelView,
    trig: &'static TrigTables,
    anims: &'static MarioAnimations,
}

impl MarioModel {
    fn new(level: &Level, session: &Session<'_>, camera: &FlyCamera) -> Option<Self> {
        let mut model = Self {
            drawer: MarioDrawer::new(level.mario_model?, level.trig, level.anims),
            view: present::MarioModelView::default(),
            shadow: level.shadow.map(|s| (ShadowDrawer::new(s), s)),
            shadow_view: present::ShadowModelView::default(),
            trig: level.trig,
            anims: level.anims,
        };
        model.tick(session, camera);
        Some(model)
    }

    /// Pose the latest tick; the level of detail uses the current camera.
    fn tick(&mut self, session: &Session<'_>, camera: &FlyCamera) {
        let pose = session.mario_pose();
        let lod = present::lod_distance(camera, pose.position);
        if let Err(error) = self.drawer.update(&pose, Some(lod)) {
            eprintln!("warning: Mario's model could not be built: {error}");
        }
        if let Some((drawer, source)) = &mut self.shadow {
            let origin = shadow_origin(&pose, self.anims, self.trig, source.child_scale);
            let anim = &session.mario().obj.gfx.anim;
            drawer.update(
                pose.visible
                    .then(|| {
                        player_shadow(
                            session.world().collision,
                            self.trig,
                            origin,
                            (f32::from(source.scale) * pose.scale[0]) as i32 as i16,
                            source.solidity,
                            anim.anim_id,
                            anim.anim_frame,
                        )
                    })
                    .flatten(),
            );
        }
    }

    /// A new level entry: snap, then pose the entry state.
    fn reset(&mut self, session: &Session<'_>, camera: &FlyCamera) {
        self.drawer.reset();
        if let Some((drawer, _)) = &mut self.shadow {
            drawer.reset();
        }
        self.tick(session, camera);
    }

    fn snap(&mut self) {
        self.drawer.snap();
        if let Some((drawer, _)) = &mut self.shadow {
            drawer.snap();
        }
    }

    fn show(&mut self, renderer: &mut Renderer, alpha: f32, graphics: GraphicsOptions) {
        self.view
            .show(renderer, self.drawer.frame(alpha, graphics.interpolation));
        self.shadow_view.show(
            renderer,
            self.shadow
                .as_ref()
                .and_then(|(d, _)| d.frame(alpha, graphics.interpolation)),
        );
    }
}

/// Object models share cached templates across all instances (coins,
/// sparkles, explosions and smoke baked per switch case; Bob-ombs skinned).
struct ObjectDrawing {
    drawer: ObjectDrawer<'static>,
    view: present::ObjectModelView,
}

impl ObjectDrawing {
    fn new(level: &Level, session: &Session<'_>) -> Self {
        let mut drawing = Self {
            drawer: ObjectDrawer::for_level(
                &level.objects.content,
                LevelModels {
                    segments: &level.objects.level_segments,
                    registrations: &level.objects.level_models,
                    animations: &level.objects.animations,
                },
                level.trig,
            ),
            view: present::ObjectModelView::default(),
        };
        drawing.tick(session);
        drawing
    }

    fn tick(&mut self, session: &Session<'_>) {
        let objects = visible_objects(
            session.world(),
            &session.game().camera.graph,
            &session.game().rendered_matrices,
        );
        if let Err(error) = self.drawer.update(objects) {
            eprintln!("warning: object models could not be built: {error}");
            // Avoid retaining stale coins after a failed presentation import.
            self.drawer.reset();
        }
    }

    fn reset(&mut self, session: &Session<'_>) {
        self.drawer.reset();
        self.tick(session);
    }

    fn show(
        &mut self,
        renderer: &mut Renderer,
        camera: &FlyCamera,
        alpha: f32,
        graphics: GraphicsOptions,
    ) {
        self.view.show(
            renderer,
            self.drawer.frame(
                alpha,
                graphics.interpolation,
                present::billboard_basis(camera),
            ),
        );
    }
}

/// Model indices in the renderer: terrain, collision overlay, placement
/// markers, Mario.
const TERRAIN: usize = 0;
const COLLISION: usize = 1;
const PLACEMENTS: usize = 2;
const MARIO: usize = 3;

fn upload(renderer: &mut Renderer, level: &Level, collision: bool, placements: bool, mario: bool) {
    renderer.load_model(&level.visual.model);
    renderer.add_model(&level.collision);
    renderer.add_model(&level.placements);
    renderer.add_model(&present::marker());
    renderer.set_visible(COLLISION, collision);
    renderer.set_visible(PLACEMENTS, placements);
    renderer.set_visible(MARIO, mario);
    debug_assert!(renderer.is_visible(TERRAIN));
}

fn load(rom_path: &Path) -> AppResult<Level> {
    let rom = Rom::open(rom_path)?;
    let imported = bob::import(&rom)?;
    let visual = imported.visual.ok_or("no visible geometry imported")?;
    // The area's GEO_CAMERA node creates the camera (create_camera).
    let camera = visual.camera.ok_or("the area has no camera node")?;
    let entry = GameEntry::script_start(&imported.level, &camera)?;
    let world = CollisionWorld::load_area_terrain(&imported.collision)?;
    let act = rustario64::content::Act::new(1).expect("act 1");
    let level_objects = objects::bob(&rom, &imported.level, act)?;
    let mario_model = match mario_model::import(&rom) {
        Ok(source) => Some(&*Box::leak(Box::new(source))),
        Err(error) => {
            eprintln!("warning: Mario's model was not imported ({error}); drawing a placeholder");
            None
        }
    };
    let shadow = mario_model.and_then(|mario| match shadow::import(&rom, mario) {
        Ok(source) => Some(&*Box::leak(Box::new(source))),
        Err(error) => {
            eprintln!("warning: Mario's shadow was not imported: {error}");
            None
        }
    });
    Ok(Level {
        collision: overlay::collision(&imported.collision),
        placements: overlay::placements(&imported.level, &imported.collision),
        mario_start: imported.level.mario_start.map(|(_, yaw, pos)| (yaw, pos)),
        visual,
        world: Box::leak(Box::new(world)),
        trig: Box::leak(Box::new(engine::trig_tables(&rom)?)),
        anims: Box::leak(Box::new(animation::mario_animations(&rom)?)),
        objects: Box::leak(Box::new(level_objects)),
        entry,
        rom_sha1: rom.fingerprint(),
        mario_model,
        shadow,
    })
}

fn preset(name: &str, level: &Level) -> AppResult<FlyCamera> {
    let mut camera = camera::preset(name, level.mario_start).ok_or_else(|| {
        format!(
            "unknown view {name}; choose one of {}",
            camera::PRESETS.join(", ")
        )
    })?;
    if let Some(original) = level.visual.camera {
        camera.fov_y_degrees = f32::from(original.fov_degrees);
        camera.near = f32::from(original.near);
        camera.far = f32::from(original.far);
    }
    Ok(camera)
}

fn render_options(options: &Options) -> RenderOptions {
    RenderOptions {
        cull_faces: options.cull,
        fog: options.fog,
        sample_count: options.msaa,
        ..RenderOptions::default()
    }
}

fn graphics_options(options: &Options) -> GraphicsOptions {
    GraphicsOptions {
        interpolation: options.interpolation,
        ..GraphicsOptions::default()
    }
}

fn describe(session: &Session<'_>) -> String {
    let m = session.mario();
    format!(
        "{} at ({:.0}, {:.0}, {:.0}) facing {:#06x}, {}, tick {}",
        play::action_name(m.action),
        m.pos[0],
        m.pos[1],
        m.pos[2],
        m.face_angle[1] as u16,
        play::camera_mode_name(session.game().camera.rig.camera.mode),
        session.inputs().len()
    )
}

fn screenshot(rom_path: &Path, options: &Options) -> AppResult<()> {
    let out = options.out.as_deref().ok_or("screenshot needs --out")?;
    if options.record.is_some() || options.mario {
        return Err("--mario and --record are window options; use --mario-ticks N".into());
    }
    let mut level = load(rom_path)?;
    start_at(&mut level, options);
    let (info, mut renderer) = rustario64_render::headless(render_options(options))?;
    let mario = options.mario_ticks.is_some();
    upload(
        &mut renderer,
        &level,
        options.collision,
        options.placements,
        mario && level.mario_model.is_none(),
    );
    let view = options.view.as_deref().unwrap_or("start");
    let graphics = graphics_options(options);
    let mut model_builds = None;
    let camera = match options.mario_ticks {
        Some(ticks) => {
            let mut session = level.session();
            let follow = |session: &Session<'_>| {
                let camera = present::reference_view(
                    session.camera_view(1.0, graphics),
                    level.visual.camera,
                );
                match &options.view {
                    Some(name) => preset(name, &level),
                    None => Ok(camera),
                }
            };
            let mut model = MarioModel::new(&level, &session, &follow(&session)?);
            let mut objects = ObjectDrawing::new(&level, &session);
            let hold_up = Pad {
                up: true,
                ..Pad::default()
            };
            for _ in 0..ticks {
                if !session.step(&hold_up) {
                    break;
                }
                objects.tick(&session);
                if let Some(model) = model.as_mut() {
                    model.tick(&session, &follow(&session)?);
                }
            }
            let pose = session.pose(1.0, graphics);
            renderer.set_transform(MARIO, pose.position, present::radians(pose.yaw));
            if let Some(model) = model.as_mut() {
                model.show(&mut renderer, 1.0, graphics);
                model_builds = Some(
                    model
                        .drawer
                        .frame(1.0, false)
                        .map(|_| model.drawer.builds()),
                );
            }
            println!("Mario: {}", describe(&session));
            objects.show(&mut renderer, &follow(&session)?, 1.0, graphics);
            println!(
                "Objects: {} cached builds; HUD coins: {} (window overlay)",
                objects.drawer.builds(),
                session.world().hud.coins
            );
            if let Some(stop) = session.stopped() {
                println!("Play stopped: {stop}");
            }
            follow(&session)?
        }
        None => preset(view, &level)?,
    };
    let (w, h) = options.size;
    let pixels = renderer.capture(w, h, &camera)?;
    rustario64_render::write_png(Path::new(out), w, h, &pixels)?;
    let from = match (&options.view, mario) {
        (None, true) => "the original camera".to_owned(),
        _ => format!("view '{view}'"),
    };
    println!(
        "Rendered {} triangles ({} batches, {} textures) from {from} at {w}x{h} on {} ({:?}) to {out}",
        level.visual.model.triangle_count(),
        level.visual.model.batches.len(),
        level.visual.model.textures.len(),
        info.name,
        info.backend
    );
    if options.collision {
        println!(
            "Collision overlay: floors blue, walls red, ceilings yellow; non-default surface types brighter."
        );
    }
    if options.placements {
        println!(
            "Placement markers: Mario start red, script objects orange, macro objects yellow, specials cyan."
        );
    }
    match (mario, model_builds) {
        (true, Some(Some(builds))) => println!(
            "Mario's model from the ROM, posed by the tick's animation ({builds} draw lists built); shadow {}.",
            if level.shadow.is_some() {
                "imported"
            } else {
                "unavailable"
            }
        ),
        (true, Some(None)) => println!(
            "Mario is drawn from his first tick on, as the original renders after his first update."
        ),
        (true, None) => println!("Mario is a placeholder box (red, blue front)."),
        _ => {}
    }
    println!(
        "Mario mode draws the simulated coins and sparkles. Other objects and the skybox remain missing; the sky is a placeholder clear color."
    );
    Ok(())
}

struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    depth: wgpu::TextureView,
    multisampled: Option<wgpu::TextureView>,
    ui: Ui,
}

#[derive(Default)]
struct Keys {
    forward: f32,
    right: f32,
    up: f32,
    yaw: f32,
    pitch: f32,
    fast: bool,
}

/// Writes each run's inputs (one level entry to the next) to a new directory.
struct Recorder {
    dir: PathBuf,
    runs: u32,
}

impl Recorder {
    fn create(dir: &Path) -> AppResult<Self> {
        if let Some(parent) = dir.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        fs::create_dir(dir)
            .map_err(|e| format!("{}: {e} (choose a new directory)", dir.display()))?;
        Ok(Self {
            dir: dir.to_owned(),
            runs: 0,
        })
    }

    fn save(&mut self, session: &Session<'_>, rom_sha1: &str) -> AppResult<()> {
        if session.inputs().is_empty() {
            return Ok(());
        }
        self.runs += 1;
        let path = self.dir.join(format!("run-{:03}.inputs.json", self.runs));
        let log = session.input_log("rustario64-viewer", rom_sha1, BOB_SCRIPT_START);
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?
            .write_all(&log.to_json_pretty())?;
        println!("Recorded {} ticks to {}", log.inputs.len(), path.display());
        Ok(())
    }
}

/// Mario mode: held keys drive the session, which ticks with the fixed clock.
struct Play {
    session: Session<'static>,
    pad: Pad,
    /// Buttons pressed since the last tick. A tap released before the next
    /// tick still reaches that tick, as a press longer than a frame would.
    taps: Pad,
    /// Keys drive Mario and the original camera; otherwise the free camera
    /// moves and Mario pauses.
    active: bool,
    graphics: GraphicsOptions,
    recorder: Option<Recorder>,
    reported_stop: bool,
}

impl Play {
    /// The controls for the next tick: held keys plus unconsumed taps.
    fn next_pad(&mut self) -> Pad {
        let pad = self.pad.with_taps(&self.taps);
        self.taps = Pad::default();
        pad
    }

    fn release_all(&mut self) {
        self.pad = Pad::default();
        self.taps = Pad::default();
    }

    fn end_run(&mut self, rom_sha1: &str) -> AppResult<()> {
        match self.recorder.as_mut() {
            Some(recorder) => recorder.save(&self.session, rom_sha1),
            None => Ok(()),
        }
    }
}

struct App {
    level: Level,
    options: RenderOptions,
    camera: FlyCamera,
    gpu: Option<Gpu>,
    keys: Keys,
    looking: bool,
    clock: FrameClock,
    last: Option<Instant>,
    ticks: u64,
    frames: u64,
    max_frames: Option<u64>,
    play: Play,
    /// Mario's imported model; None draws the placeholder box instead.
    mario: Option<MarioModel>,
    objects: ObjectDrawing,
    error: Option<String>,
    paused: bool,
    focused: bool,
    settings: Settings,
    settings_error: Option<String>,
    initial_overlays: (bool, bool),
    controllers: Controllers,
    choose_rom: bool,
}

fn create_gpu(
    event_loop: &ActiveEventLoop,
    options: RenderOptions,
    size: [u32; 2],
) -> AppResult<Gpu> {
    let window = Arc::new(
        event_loop.create_window(
            Window::default_attributes()
                .with_title("Rustario64 — Bob-omb Battlefield (development viewer)")
                .with_inner_size(PhysicalSize::new(size[0], size[1])),
        )?,
    );
    let instance = rustario64_render::instance(Some(Box::new(event_loop.owned_display_handle())));
    let surface = instance.create_surface(window.clone())?;
    let (adapter, device, queue) =
        pollster::block_on(rustario64_render::device(&instance, Some(&surface)))?;
    let size = window.inner_size();
    let mut config = surface
        .get_default_config(&adapter, size.width.max(1), size.height.max(1))
        .ok_or("surface is not supported by the adapter")?;
    let capabilities = surface.get_capabilities(&adapter);
    // Prefer a non-sRGB target: original colors are display-referred.
    if let Some(format) = capabilities.formats.iter().find(|f| !f.is_srgb()) {
        config.format = *format;
    }
    config.present_mode = wgpu::PresentMode::AutoVsync;
    surface.configure(&device, &config);
    let ui = Ui::new(&window, &device, config.format);
    let renderer = Renderer::new(device, queue, config.format, options);
    let mut gpu = Gpu {
        window,
        surface,
        depth: renderer.create_depth(config.width, config.height),
        multisampled: None,
        config,
        renderer,
        ui,
    };
    resize_targets(&mut gpu);
    println!(
        "Viewer on {} ({:?}), surface {:?}",
        adapter.get_info().name,
        adapter.get_info().backend,
        gpu.config.format
    );
    Ok(gpu)
}

impl App {
    fn prepare_gpu(&mut self, gpu: &mut Gpu) {
        upload(
            &mut gpu.renderer,
            &self.level,
            self.initial_overlays.0,
            self.initial_overlays.1,
            self.mario.is_none(),
        );
        apply_window_settings(gpu, &self.settings);
    }

    fn pause(&mut self, paused: bool) {
        self.paused = paused;
        self.clock.reset();
        self.last = None;
        self.keys = Keys::default();
        self.play.release_all();
        self.controllers.clear();
        self.play.session.snap_presentation();
        self.objects.drawer.snap();
        if let Some(mario) = self.mario.as_mut() {
            mario.snap();
        }
        self.looking = false;
        if let Some(gpu) = &self.gpu {
            let _ = gpu.window.set_cursor_grab(CursorGrabMode::None);
            gpu.window.set_cursor_visible(true);
        }
    }

    fn restart(&mut self) {
        if let Err(e) = self.play.end_run(self.level.rom_sha1) {
            eprintln!("error: {e}");
        }
        self.play.session.reset();
        self.objects.reset(&self.play.session);
        self.play.reported_stop = false;
        self.play.release_all();
        self.controllers.clear();
        self.clock.reset();
        if let Some(mario) = self.mario.as_mut() {
            mario.reset(&self.play.session, &self.camera);
        }
    }

    fn redraw(&mut self) {
        let actions = self.controllers.poll(
            self.play.active
                && self.focused
                && !self.paused
                && self.play.session.stopped().is_none(),
            self.focused,
        );
        if actions.pause {
            self.pause(!self.paused);
        }
        let now = Instant::now();
        let elapsed = self.last.map_or(Duration::ZERO, |last| now - last);
        self.last = Some(now);
        // Fixed 30 Hz gameplay cadence: Mario ticks only on drained clock ticks.
        let running = self.play.active
            && self.focused
            && !self.paused
            && self.play.session.stopped().is_none();
        let drained = self.clock.advance(now, running);
        self.ticks += u64::from(drained);
        let play = &mut self.play;
        if running {
            for _ in 0..drained {
                let pad = play.next_pad().combined(&self.controllers.next_pad());
                if play.session.step(&pad) {
                    self.objects.tick(&play.session);
                    // The render pass picks the level of detail from the
                    // camera of the frame it draws.
                    let camera = present::reference_view(
                        play.session.camera_view(1.0, play.graphics),
                        self.level.visual.camera,
                    );
                    if let Some(mario) = self.mario.as_mut() {
                        mario.tick(&play.session, &camera);
                    }
                }
            }
        }
        if let Some(stop) = play.session.stopped()
            && !play.reported_stop
        {
            play.reported_stop = true;
            println!(
                "Mario stopped ({}): {stop}. Press R to re-enter the level.",
                describe(&play.session)
            );
        }
        // A paused or stopped session holds its latest pose.
        let alpha = if running { self.clock.alpha() } else { 1.0 };
        let pose = play.session.pose(alpha, play.graphics);
        if play.active {
            self.camera = present::reference_view(
                play.session.camera_view(alpha, play.graphics),
                self.level.visual.camera,
            );
        } else if self.focused && !self.paused {
            // The inspection camera is presentation-only, so wall-clock motion is fine.
            let seconds = elapsed.as_secs_f32().min(0.1);
            let speed = if self.keys.fast { 6000.0 } else { 1500.0 } * seconds;
            self.camera.translate([
                self.keys.right * speed,
                self.keys.up * speed,
                self.keys.forward * speed,
            ]);
            self.camera.rotate(
                self.keys.yaw * 1.6 * seconds,
                self.keys.pitch * 1.6 * seconds,
            );
        }
        let Some(gpu) = self.gpu.as_mut() else {
            return;
        };
        match self.mario.as_mut() {
            Some(mario) => mario.show(&mut gpu.renderer, alpha, play.graphics),
            None => gpu
                .renderer
                .set_transform(MARIO, pose.position, present::radians(pose.yaw)),
        }
        self.objects
            .show(&mut gpu.renderer, &self.camera, alpha, play.graphics);
        let frame = match gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                gpu.surface.configure(gpu.renderer.device(), &gpu.config);
                return;
            }
            _ => return,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let size = (gpu.config.width, gpu.config.height);
        match &gpu.multisampled {
            Some(ms) => gpu
                .renderer
                .render(ms, Some(&view), &gpu.depth, size, &self.camera),
            None => gpu
                .renderer
                .render(&view, None, &gpu.depth, size, &self.camera),
        }
        let before = self.settings.clone();
        let mut resume = false;
        let mut restart = false;
        let mut quit = false;
        let input = gpu.ui.input.take_egui_input(&gpu.window);
        let output = gpu.ui.context.run_ui(input, |root| {
            let ctx = root.ctx();
            if play.active {
                present::show_coin_counter(ctx, &play.session.world().hud);
            }
            if self.paused {
                egui::Window::new("Paused")
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .resizable(false)
                    .collapsible(false)
                    .default_width(380.0)
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height((ctx.content_rect().height() - 60.0).max(100.0))
                            .show(ui, |ui| {
                                ui.heading("Bob-omb Battlefield");
                                if let Some(stop) = play.session.stopped() {
                                    ui.label(format!("Play stopped: {stop}"));
                                }
                                self.settings.controls(ui);
                                ui.separator();
                                ui.label("WASD move · Space jump · J attack · K crouch");
                                ui.label("Arrows: C buttons · E: R camera · R: restart");
                                self.controllers.controls(ui);
                                if let Some(error) = &self.settings_error {
                                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                                }
                                resume = ui.button("Resume · Esc / Start").clicked();
                                restart = ui.button("Restart course").clicked();
                                self.choose_rom = ui.button("Choose another ROM").clicked();
                                quit = ui.button("Quit").clicked();
                            });
                    });
            }
        });
        gpu.ui.paint(
            &gpu.window,
            gpu.renderer.device(),
            gpu.renderer.queue(),
            &view,
            output,
        );
        if self.settings != before {
            play.graphics.interpolation = self.settings.interpolation;
            gpu.renderer.options_mut().fog = self.settings.fog;
            self.settings_error = self.settings.save().err();
        }
        gpu.window.pre_present_notify();
        gpu.renderer.queue().present(frame);
        // Reconfigure only after presenting the acquired surface texture.
        if self.settings != before {
            apply_window_settings(gpu, &self.settings);
        }
        self.frames += 1;
        if self.frames.is_multiple_of(10) {
            let title = if play.active {
                let stop = play.session.stopped().map_or(String::new(), |stop| {
                    format!(" — stopped: {stop}; R re-enters")
                });
                format!("Rustario64 — BOB — Mario {}{stop}", describe(&play.session))
            } else {
                let p = self.camera.position;
                format!(
                    "Rustario64 — BOB viewer — {} ticks @30 Hz — camera ({:.0}, {:.0}, {:.0}) — M plays Mario",
                    self.ticks, p[0], p[1], p[2]
                )
            };
            gpu.window.set_title(&title);
        }
        if restart {
            self.restart();
        }
        if resume || restart {
            self.pause(false);
        }
        if quit {
            self.max_frames = Some(self.frames);
        }
    }

    /// Mario mode's keys; returns whether the key was one of them.
    fn play_key(&mut self, code: KeyCode, down: bool, repeat: bool) -> bool {
        let (pad, taps) = (&mut self.play.pad, &mut self.play.taps);
        match code {
            KeyCode::KeyW => pad.up = down,
            KeyCode::KeyS => pad.down = down,
            KeyCode::KeyA => pad.left = down,
            KeyCode::KeyD => pad.right = down,
            KeyCode::ShiftLeft | KeyCode::ShiftRight => pad.walk = down,
            KeyCode::Space => (pad.a, taps.a) = (down, taps.a || down),
            KeyCode::KeyJ => (pad.b, taps.b) = (down, taps.b || down),
            KeyCode::KeyK => (pad.z, taps.z) = (down, taps.z || down),
            KeyCode::KeyE => (pad.r, taps.r) = (down, taps.r || down),
            KeyCode::ArrowUp => (pad.c_up, taps.c_up) = (down, taps.c_up || down),
            KeyCode::ArrowDown => (pad.c_down, taps.c_down) = (down, taps.c_down || down),
            KeyCode::ArrowLeft => (pad.c_left, taps.c_left) = (down, taps.c_left || down),
            KeyCode::ArrowRight => (pad.c_right, taps.c_right) = (down, taps.c_right || down),
            KeyCode::KeyR => {
                if down && !repeat {
                    self.restart();
                }
            }
            _ => return false,
        }
        true
    }

    fn key(&mut self, _: &ActiveEventLoop, event: KeyEvent) {
        let PhysicalKey::Code(code) = event.physical_key else {
            return;
        };
        // A key held while opening a menu must not re-latch gameplay from
        // OS repeat events after resume. egui has already received the event.
        if event.repeat {
            return;
        }
        let down = event.state == ElementState::Pressed;
        if code == KeyCode::Escape && down && !event.repeat {
            self.pause(!self.paused);
            return;
        }
        if self.paused || !self.focused {
            return;
        }
        if self.play.active && self.play_key(code, down, event.repeat) {
            return;
        }
        let v = if down { 1.0 } else { 0.0 };
        match code {
            KeyCode::KeyW => self.keys.forward = v,
            KeyCode::KeyS => self.keys.forward = -v,
            KeyCode::KeyD => self.keys.right = v,
            KeyCode::KeyA => self.keys.right = -v,
            KeyCode::KeyE => self.keys.up = v,
            KeyCode::KeyQ => self.keys.up = -v,
            KeyCode::ArrowLeft => self.keys.yaw = v,
            KeyCode::ArrowRight => self.keys.yaw = -v,
            KeyCode::ArrowUp => self.keys.pitch = v,
            KeyCode::ArrowDown => self.keys.pitch = -v,
            KeyCode::ShiftLeft | KeyCode::ShiftRight => self.keys.fast = down,
            KeyCode::KeyM if down && !event.repeat => {
                // Held keys belong to the mode they were pressed in.
                self.keys = Keys::default();
                self.play.release_all();
                self.controllers.clear();
                self.play.active ^= true;
                self.clock.reset();
                self.play.session.snap_presentation();
                self.objects.drawer.snap();
                if let Some(mario) = self.mario.as_mut() {
                    mario.snap();
                }
            }
            KeyCode::KeyC | KeyCode::KeyP if down => {
                if let Some(gpu) = self.gpu.as_mut() {
                    let model = if code == KeyCode::KeyC {
                        COLLISION
                    } else {
                        PLACEMENTS
                    };
                    let visible = gpu.renderer.is_visible(model);
                    gpu.renderer.set_visible(model, !visible);
                }
            }
            KeyCode::KeyF if down => {
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.renderer.options_mut().fog ^= true;
                    self.settings.fog = gpu.renderer.options().fog;
                }
            }
            KeyCode::Digit1 | KeyCode::Digit2 | KeyCode::Digit3 | KeyCode::Digit4 if down => {
                let index = match code {
                    KeyCode::Digit1 => 0,
                    KeyCode::Digit2 => 1,
                    KeyCode::Digit3 => 2,
                    _ => 3,
                };
                if let Ok(camera) = preset(camera::PRESETS[index], &self.level) {
                    self.camera = camera;
                }
            }
            _ => {}
        }
    }
}

fn apply_window_settings(gpu: &mut Gpu, settings: &Settings) {
    let mode = if settings.vsync {
        wgpu::PresentMode::AutoVsync
    } else {
        wgpu::PresentMode::AutoNoVsync
    };
    if gpu.config.present_mode != mode {
        gpu.config.present_mode = mode;
        gpu.surface.configure(gpu.renderer.device(), &gpu.config);
    }
    if gpu.window.fullscreen().is_some() != settings.fullscreen {
        gpu.window
            .set_fullscreen(settings.fullscreen.then_some(Fullscreen::Borderless(None)));
    }
}

fn resize_targets(gpu: &mut Gpu) {
    let (w, h) = (gpu.config.width, gpu.config.height);
    gpu.depth = gpu.renderer.create_depth(w, h);
    let samples = gpu.renderer.options().sample_count;
    gpu.multisampled = (samples > 1).then(|| {
        gpu.renderer
            .device()
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("multisampled color"),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: gpu.config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    });
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_none() {
            match create_gpu(event_loop, self.options, self.settings.size) {
                Ok(mut gpu) => {
                    self.prepare_gpu(&mut gpu);
                    gpu.window.request_redraw();
                    self.gpu = Some(gpu);
                }
                Err(e) => {
                    self.error = Some(e.to_string());
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if let Some(gpu) = self.gpu.as_mut() {
            let _ = gpu.ui.input.on_window_event(&gpu.window, &event);
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = self.gpu.as_mut()
                    && size.width > 0
                    && size.height > 0
                {
                    gpu.config.width = size.width;
                    gpu.config.height = size.height;
                    gpu.surface.configure(gpu.renderer.device(), &gpu.config);
                    resize_targets(gpu);
                }
            }
            // Pause tick accumulation while unfocused; resume from a fresh anchor.
            // Held keys are released, since their release may go elsewhere.
            WindowEvent::Focused(focused) => {
                self.focused = focused;
                if !focused {
                    self.pause(true);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => self.key(event_loop, event),
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Right,
                ..
            } if !self.paused && self.focused => {
                self.looking = state == ElementState::Pressed;
                if let Some(gpu) = &self.gpu {
                    let mode = if self.looking {
                        CursorGrabMode::Locked
                    } else {
                        CursorGrabMode::None
                    };
                    let _ = gpu.window.set_cursor_grab(mode);
                    gpu.window.set_cursor_visible(!self.looking);
                }
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
                if self.max_frames.is_some_and(|max| self.frames >= max) {
                    event_loop.exit();
                } else if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event
            && self.looking
            && !self.play.active
            && !self.paused
            && self.focused
        {
            self.camera
                .rotate(-delta.0 as f32 * 0.003, -delta.1 as f32 * 0.003);
        }
    }
}

fn make_app(level: Level, options: &Options) -> AppResult<App> {
    if options.mario_ticks.is_some() {
        return Err("--mario-ticks is a screenshot option; use --mario in the window".into());
    }
    let recorder = options
        .record
        .as_deref()
        .map(Recorder::create)
        .transpose()?;
    let camera = preset(options.view.as_deref().unwrap_or("start"), &level)?;
    let play = Play {
        session: level.session(),
        pad: Pad::default(),
        taps: Pad::default(),
        active: options.mario,
        graphics: graphics_options(options),
        recorder,
        reported_stop: false,
    };
    let mario = MarioModel::new(&level, &play.session, &camera);
    let objects = ObjectDrawing::new(&level, &play.session);
    let remembered = Settings::load().ok().and_then(|s| s.rom_path);
    let app = App {
        level,
        options: render_options(options),
        camera,
        gpu: None,
        keys: Keys::default(),
        looking: false,
        clock: FrameClock::default(),
        last: None,
        ticks: 0,
        frames: 0,
        max_frames: options.frames,
        play,
        mario,
        objects,
        error: None,
        paused: false,
        focused: true,
        settings: Settings {
            interpolation: options.interpolation,
            fog: options.fog,
            vsync: options.vsync,
            fullscreen: options.fullscreen,
            size: [options.size.0, options.size.1],
            rom_path: remembered,
        },
        settings_error: None,
        initial_overlays: (options.collision, options.placements),
        controllers: Controllers::default(),
        choose_rom: false,
    };
    Ok(app)
}

fn finish_app(app: &mut App) -> AppResult<()> {
    app.play.end_run(app.level.rom_sha1)?;
    if let Some(error) = app.error.take() {
        return Err(error.into());
    }
    println!(
        "Viewer closed after {} frames and {} fixed 30 Hz ticks; Mario: {}",
        app.frames,
        app.ticks,
        describe(&app.play.session)
    );
    Ok(())
}

/// The development `--start` entry: the level script's entry with Mario
/// spawned elsewhere in the same area.
fn start_at(level: &mut Level, options: &Options) {
    if let Some((pos, yaw)) = options.start {
        let area = level.entry.mario.spawn.area_index as u8;
        level.entry.mario.spawn =
            rustario64::simulation::mario::core::SpawnPoint::from_level_script(1, area, yaw, pos);
    }
}

fn view(rom_path: &Path, options: &Options) -> AppResult<()> {
    let mut level = load(rom_path)?;
    start_at(&mut level, options);
    let mut app = make_app(level, options)?;
    app.controllers = Controllers::new();
    let mut desktop = Desktop {
        gpu: None,
        settings: app.settings.clone(),
        path: rom_path.to_string_lossy().into_owned(),
        remember: app.settings.rom_path.is_some(),
        game: Some(app),
        error: None,
        fatal: None,
        controllers: Controllers::default(),
        parked_level: None,
    };
    run_desktop(&mut desktop)
}

/// One event loop and one GPU/window across launcher and play.
#[derive(Default)]
struct Desktop {
    gpu: Option<Gpu>,
    game: Option<App>,
    settings: Settings,
    path: String,
    remember: bool,
    error: Option<String>,
    fatal: Option<String>,
    controllers: Controllers,
    /// The only supported ROM has one identity. Keep its imported data when
    /// returning to selection, and revalidate each selected file before reuse.
    parked_level: Option<Level>,
}

impl Desktop {
    fn return_to_launcher(&mut self) {
        let Some(mut game) = self.game.take() else {
            return;
        };
        self.error = finish_app(&mut game).err().map(|e| e.to_string());
        self.settings = game.settings;
        self.controllers = game.controllers;
        self.controllers.clear();
        self.parked_level = Some(game.level);
        if let Some(mut gpu) = game.gpu.take() {
            gpu.renderer.load_model(&VisualModel::default());
            gpu.window.set_fullscreen(None);
            gpu.window.set_title("Rustario64 — Select ROM");
            let _ = gpu.window.request_inner_size(PhysicalSize::new(800, 720));
            gpu.window.request_redraw();
            self.gpu = Some(gpu);
        }
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        self.controllers.poll(false, false);
        let Some(gpu) = self.gpu.as_mut() else { return };
        let frame = match gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                gpu.surface.configure(gpu.renderer.device(), &gpu.config);
                return;
            }
            _ => return,
        };
        let input = gpu.ui.input.take_egui_input(&gpu.window);
        let mut start = false;
        let output = gpu.ui.context.run_ui(input, |root| {
            start = launcher_form(
                root,
                &mut self.path,
                &mut self.remember,
                &mut self.settings,
                &mut self.controllers,
                &mut self.error,
            );
        });
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        gpu.renderer.render(
            &view,
            None,
            &gpu.depth,
            (gpu.config.width, gpu.config.height),
            &FlyCamera::looking_at([0.0, 0.0, 1.0], [0.0; 3]),
        );
        gpu.ui.paint(
            &gpu.window,
            gpu.renderer.device(),
            gpu.renderer.queue(),
            &view,
            output,
        );
        gpu.window.pre_present_notify();
        gpu.renderer.queue().present(frame);
        if start {
            // Validate identity and complete the import before replacing the
            // launcher; every error keeps a usable selection window.
            let path = PathBuf::from(self.path.trim());
            let result = load_selected(&path, &mut self.parked_level).and_then(|level| {
                let mut options = parse(&[])?;
                options.mario = true;
                options.interpolation = self.settings.interpolation;
                options.fog = self.settings.fog;
                options.vsync = self.settings.vsync;
                options.fullscreen = self.settings.fullscreen;
                options.size = (self.settings.size[0], self.settings.size[1]);
                make_app(level, &options)
            });
            match result {
                Ok(mut app) => {
                    self.settings.rom_path = self.remember.then_some(path);
                    app.settings = self.settings.clone();
                    app.settings_error = self.settings.save().err();
                    self.controllers.clear();
                    app.controllers = std::mem::take(&mut self.controllers);
                    let mut gpu = self.gpu.take().unwrap();
                    let _ = gpu.window.request_inner_size(PhysicalSize::new(
                        self.settings.size[0],
                        self.settings.size[1],
                    ));
                    app.prepare_gpu(&mut gpu);
                    app.gpu = Some(gpu);
                    self.game = Some(app);
                }
                Err(error) => {
                    self.error = Some(format!(
                        "Could not start: {error}. Choose a supported ROM and try again."
                    ))
                }
            }
        }
        let window = self
            .game
            .as_ref()
            .and_then(|a| a.gpu.as_ref())
            .or(self.gpu.as_ref());
        if let Some(gpu) = window {
            gpu.window.request_redraw();
        }
        let _ = event_loop;
    }
}

fn launcher_form(
    root: &mut egui::Ui,
    path: &mut String,
    remember: &mut bool,
    settings: &mut Settings,
    controllers: &mut Controllers,
    error: &mut Option<String>,
) -> bool {
    let width = (root.ctx().content_rect().width() - 48.0).clamp(260.0, 540.0);
    let height = (root.ctx().content_rect().height() - 60.0).max(100.0);
    let mut start = false;
    egui::CentralPanel::default().show(root, |_ui| {});
    egui::Window::new("Rustario64 launcher")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .default_width(width)
        .show(root.ctx(), |ui| {
            ui.set_width(width);
            egui::ScrollArea::vertical()
                .max_height(height)
                .show(ui, |ui| {
                    ui.heading(egui::RichText::new("Rustario64").size(42.0));
                    ui.label("Bob-omb Battlefield · development build");
                    ui.add_space(20.0);
                    ui.label("Choose your Super Mario 64 ROM");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(path)
                                .hint_text("Local .z64, .v64 or .n64 path")
                                .desired_width(width - 110.0),
                        );
                        if ui.button("Browse…").clicked()
                            && let Some(selected) = rfd::FileDialog::new()
                                .add_filter("Nintendo 64 ROM", &["z64", "v64", "n64"])
                                .pick_file()
                        {
                            *path = selected.to_string_lossy().into_owned();
                            *error = None;
                        }
                    });
                    ui.checkbox(remember, "Remember this ROM path on this computer");
                    ui.label("Original US v1.0 · ROM stays on your computer");
                    ui.add_space(10.0);
                    settings.controls(ui);
                    ui.separator();
                    controllers.controls(ui);
                    egui::ComboBox::from_label("Window size")
                        .selected_text(format!("{} × {}", settings.size[0], settings.size[1]))
                        .show_ui(ui, |ui| {
                            for size in [[960, 720], [1280, 960], [1920, 1080]] {
                                ui.selectable_value(
                                    &mut settings.size,
                                    size,
                                    format!("{} × {}", size[0], size[1]),
                                );
                            }
                        });
                    if let Some(error) = error.as_ref() {
                        ui.colored_label(egui::Color32::LIGHT_RED, error);
                    }
                    start = ui
                        .add_enabled(
                            !path.trim().is_empty(),
                            egui::Button::new("Play Bob-omb Battlefield"),
                        )
                        .clicked();
                    ui.small(
                        "Early exploration: objects, stars, audio and saves are still being built.",
                    );
                });
        });
    start
}

fn load_selected(path: &Path, cached: &mut Option<Level>) -> AppResult<Level> {
    if cached.is_some() {
        // Revalidate each file even when the only supported identity is cached.
        // Failures keep the imported level available for the next selection.
        Rom::open(path)?;
        Ok(cached.take().unwrap())
    } else {
        load(path)
    }
}

impl ApplicationHandler for Desktop {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(game) = self.game.as_mut() {
            game.resumed(event_loop);
            return;
        }
        if self.gpu.is_none() {
            match create_gpu(event_loop, RenderOptions::default(), [800, 720]) {
                Ok(gpu) => {
                    gpu.window.set_title("Rustario64 — Select ROM");
                    gpu.window.request_redraw();
                    self.gpu = Some(gpu);
                }
                Err(error) => {
                    self.fatal = Some(error.to_string());
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if let Some(game) = self.game.as_mut() {
            game.window_event(event_loop, id, event);
            if game.choose_rom {
                self.return_to_launcher();
            }
            return;
        }
        if let Some(gpu) = self.gpu.as_mut() {
            let _ = gpu.ui.input.on_window_event(&gpu.window, &event);
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::DroppedFile(path) => {
                self.path = path.to_string_lossy().into_owned();
                self.error = None;
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.config.width = size.width;
                    gpu.config.height = size.height;
                    gpu.surface.configure(gpu.renderer.device(), &gpu.config);
                    resize_targets(gpu);
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, event_loop: &ActiveEventLoop, id: DeviceId, event: DeviceEvent) {
        if let Some(game) = self.game.as_mut() {
            game.device_event(event_loop, id, event);
        }
    }
}

fn launch(rom_path: Option<&Path>) -> AppResult<()> {
    let (settings, error) = match Settings::load() {
        Ok(settings) => (settings, None),
        Err(error) => (
            Settings::default(),
            Some(format!("{error}. Using default settings.")),
        ),
    };
    let path = rom_path
        .or(settings.rom_path.as_deref())
        .map_or(String::new(), |p| p.to_string_lossy().into_owned());
    let mut desktop = Desktop {
        gpu: None,
        game: None,
        remember: settings.rom_path.is_some(),
        settings,
        path,
        error,
        fatal: None,
        controllers: Controllers::new(),
        parked_level: None,
    };
    run_desktop(&mut desktop)
}

fn run_desktop(desktop: &mut Desktop) -> AppResult<()> {
    EventLoop::new()?.run_app(desktop)?;
    if let Some(game) = desktop.game.as_mut() {
        finish_app(game)?;
    }
    if let Some(error) = desktop.fatal.take() {
        return Err(error.into());
    }
    Ok(())
}

fn run(args: &[String]) -> AppResult<()> {
    match args {
        [] => launch(None),
        [cmd] if cmd == "launch" => launch(None),
        [cmd, rom] if cmd == "launch" => launch(Some(Path::new(rom))),
        [cmd, rom, rest @ ..] if cmd == "screenshot" => screenshot(Path::new(rom), &parse(rest)?),
        [cmd, rom, rest @ ..] if cmd == "view" => view(Path::new(rom), &parse(rest)?),
        _ => {
            print!("{HELP}");
            if args == ["--help"] || args == ["-h"] {
                Ok(())
            } else {
                Err("unknown command; use --help".into())
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = run(&args) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_form_contains_rom_selection_and_controller_controls_on_small_windows() {
        let context = egui::Context::default();
        let mut desktop = Desktop::default();
        for size in [[800.0, 720.0], [640.0, 480.0]] {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(size[0], size[1]),
                )),
                ..egui::RawInput::default()
            };
            let mut text = String::new();
            // Anchored windows settle their position on the first UI frame.
            for _ in 0..2 {
                let mut start = false;
                let mut output = context.run_ui(input.clone(), |root| {
                    start = launcher_form(
                        root,
                        &mut desktop.path,
                        &mut desktop.remember,
                        &mut desktop.settings,
                        &mut desktop.controllers,
                        &mut desktop.error,
                    );
                });
                assert!(!start);
                output.textures_delta.clear();
                let frame_text: String = output
                    .shapes
                    .into_iter()
                    .filter_map(|shape| {
                        if let egui::epaint::Shape::Text(t) = shape.shape {
                            Some(t.galley.text().to_owned())
                        } else {
                            None
                        }
                    })
                    .collect();
                text.push_str(&frame_text);
            }
            assert!(text.contains("Rustario64"));
            assert!(text.contains("Choose your Super Mario 64 ROM"));
            assert!(text.contains("Browse"));
            assert!(text.contains("Keyboard only"));
        }
    }

    #[test]
    fn unreadable_rom_selection_is_a_recoverable_error() {
        let mut cached = None;
        assert!(load_selected(Path::new("missing-authored-fixture.z64"), &mut cached).is_err());
        assert!(cached.is_none());
    }

    #[test]
    #[ignore = "requires RUSTARIO64_ROM; local launcher import and return without a window"]
    fn local_launcher_import_return_and_reselection_validate_without_reimporting() {
        let path = PathBuf::from(std::env::var_os("RUSTARIO64_ROM").expect("set RUSTARIO64_ROM"));
        let level = load_selected(&path, &mut None).unwrap();
        let world = level.world as *const CollisionWorld;
        let anims = level.anims as *const MarioAnimations;
        let objects = level.objects as *const objects::LevelObjectContent;
        let mut options = parse(&[]).unwrap();
        options.mario = true;
        let mut app = make_app(level, &options).unwrap();
        assert!(app.play.active);
        assert!(app.play.session.step(&Pad::default()));
        let mut desktop = Desktop {
            game: Some(app),
            ..Desktop::default()
        };
        desktop.return_to_launcher();
        assert!(desktop.game.is_none());
        assert!(
            load_selected(
                Path::new("missing-authored-fixture.z64"),
                &mut desktop.parked_level
            )
            .is_err()
        );
        assert!(desktop.parked_level.is_some());
        let level = load_selected(&path, &mut desktop.parked_level).unwrap();
        assert_eq!(level.world as *const CollisionWorld, world);
        assert_eq!(level.anims as *const MarioAnimations, anims);
        assert_eq!(level.objects as *const objects::LevelObjectContent, objects);
        assert!(desktop.parked_level.is_none());
        let app = make_app(level, &options).unwrap();
        assert!(app.play.session.inputs().is_empty());
    }
}
