//! Development viewer for imported levels. `screenshot` renders offscreen to PNG
//! (no window needed); `view` opens a window with a free inspection camera or,
//! in Mario mode, Mario driven by the tick compared with the decomp. Both are
//! development entry points straight into Bob-omb Battlefield.
use rustario64::{
    content::{
        animation::MarioAnimations,
        visual::{AreaVisual, VisualModel},
    },
    import::{
        animation, bob, engine,
        mario::{self as mario_model, MarioModelSource},
        rom::Rom,
    },
    play::{self, BOB_SCRIPT_START, Pad, Session},
    presentation::{GraphicsOptions, mario::MarioDrawer},
    simulation::{
        FixedClock, collision::CollisionWorld, mario::tick::LevelEntry, math::TrigTables,
    },
};
use rustario64_render::{
    RenderOptions, Renderer, camera, camera::FlyCamera, overlay, play as present,
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
    window::{CursorGrabMode, Window, WindowId},
};

type AppResult<T> = Result<T, Box<dyn std::error::Error>>;

const HELP: &str = "Rustario64 development viewer (Bob-omb Battlefield, area 1)\n\n\
  rustario64-viewer screenshot /path/to/sm64.z64 --out private/bob.png [--view start|overview|summit|top] [--size 1280x960] [--msaa 4] [--no-cull] [--no-fog] [--collision] [--placements] [--mario-ticks N]\n\
  rustario64-viewer view /path/to/sm64.z64 [--view start] [--msaa 4] [--no-fog] [--frames N] [--mario] [--no-interpolation] [--record NEW_PRIVATE_DIR]\n\n\
  View controls: WASD move, Q/E down/up, Shift faster, hold right mouse or arrow keys to look,\n\
  1-4 select presets, C collision overlay, P placement markers, F fog, M Mario mode, Esc quits.\n\
  The camera is a presentation-only inspection camera, not the original camera.\n\n\
  Mario mode (--mario or M): WASD stick (hold Shift to walk), Space A, J B, K Z, Left/Right\n\
  arrows turn the follow camera, R re-enters the level, M returns to the free camera (Mario\n\
  pauses). Mario runs the tick compared with the decomp from the level script's start. The\n\
  follow camera is not the original camera; its yaw is the camera input Mario reads. Mario's\n\
  model and animations come from the ROM (a placeholder box if they cannot be imported). Play\n\
  stops at paths the port does not support and at warps (falling off the course); R re-enters.\n\
  --mario-ticks N renders Mario after N ticks of holding the stick up, from the follow camera.\n\
  --record writes each run's tick inputs to a new directory for exact replay against the decomp:\n\
  cargo run -p rustario64-oracle --example tick_trace -- ROM NEW_DIR --inputs RUN.inputs.json\n";

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
    entry: LevelEntry,
    rom_sha1: &'static str,
    /// Mario's model, or None when it could not be imported.
    mario_model: Option<&'static MarioModelSource>,
}

impl Level {
    fn session(&self) -> Session<'static> {
        Session::new(self.world, self.trig, self.anims, self.entry)
    }
}

/// Mario's imported model, posed from each completed tick.
struct MarioModel {
    drawer: MarioDrawer<'static>,
    view: present::MarioModelView,
}

impl MarioModel {
    fn new(level: &Level, session: &Session<'_>, camera: &FlyCamera) -> Option<Self> {
        let mut model = Self {
            drawer: MarioDrawer::new(level.mario_model?, level.trig, level.anims),
            view: present::MarioModelView::default(),
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
    }

    /// A new level entry: snap, then pose the entry state.
    fn reset(&mut self, session: &Session<'_>, camera: &FlyCamera) {
        self.drawer.reset();
        self.tick(session, camera);
    }

    fn show(&mut self, renderer: &mut Renderer, alpha: f32, graphics: GraphicsOptions) {
        self.view
            .show(renderer, self.drawer.frame(alpha, graphics.interpolation));
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
    // create_camera takes Mario's camera mode from the area's GEO_CAMERA node.
    let camera_mode = visual.camera.ok_or("the area has no camera node")?.mode;
    let entry = LevelEntry::script_start(&imported.level, camera_mode)?;
    let world = CollisionWorld::load_area_terrain(&imported.collision)?;
    let mario_model = match mario_model::import(&rom) {
        Ok(source) => Some(&*Box::leak(Box::new(source))),
        Err(error) => {
            eprintln!("warning: Mario's model was not imported ({error}); drawing a placeholder");
            None
        }
    };
    Ok(Level {
        collision: overlay::collision(&imported.collision),
        placements: overlay::placements(&imported.level, &imported.collision),
        mario_start: imported.level.mario_start.map(|(_, yaw, pos)| (yaw, pos)),
        visual,
        world: Box::leak(Box::new(world)),
        trig: Box::leak(Box::new(engine::trig_tables(&rom)?)),
        anims: Box::leak(Box::new(animation::mario_animations(&rom)?)),
        entry,
        rom_sha1: rom.fingerprint(),
        mario_model,
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
        "{} at ({:.0}, {:.0}, {:.0}) facing {:#06x}, tick {}",
        play::action_name(m.action),
        m.pos[0],
        m.pos[1],
        m.pos[2],
        m.face_angle[1] as u16,
        session.inputs().len()
    )
}

fn screenshot(rom_path: &Path, options: &Options) -> AppResult<()> {
    let out = options.out.as_deref().ok_or("screenshot needs --out")?;
    if options.record.is_some() || options.mario {
        return Err("--mario and --record are window options; use --mario-ticks N".into());
    }
    let level = load(rom_path)?;
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
                let position = session.pose(1.0, graphics).position;
                let camera =
                    present::follow_view(session.camera(), position, 1.0, level.visual.camera);
                match &options.view {
                    Some(name) => preset(name, &level),
                    None => Ok(camera),
                }
            };
            let mut model = MarioModel::new(&level, &session, &follow(&session)?);
            let hold_up = Pad {
                up: true,
                ..Pad::default()
            };
            for _ in 0..ticks {
                if !session.step(&hold_up) {
                    break;
                }
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
        (None, true) => "the follow camera".to_owned(),
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
            "Mario's model from the ROM, posed by the tick's animation ({builds} draw lists built); no shadow yet."
        ),
        (true, Some(None)) => println!(
            "Mario is drawn from his first tick on, as the original renders after his first update."
        ),
        (true, None) => println!("Mario is a placeholder box (red, blue front)."),
        _ => {}
    }
    println!("Skybox and objects are not drawn yet; the sky is a placeholder clear color.");
    Ok(())
}

struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    depth: wgpu::TextureView,
    multisampled: Option<wgpu::TextureView>,
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
    /// Keys drive Mario and the follow camera; otherwise the free camera
    /// moves and Mario pauses.
    active: bool,
    graphics: GraphicsOptions,
    recorder: Option<Recorder>,
    reported_stop: bool,
}

impl Play {
    /// The controls for the next tick: held keys plus unconsumed taps.
    fn next_pad(&mut self) -> Pad {
        let pad = Pad {
            a: self.pad.a || self.taps.a,
            b: self.pad.b || self.taps.b,
            z: self.pad.z || self.taps.z,
            ..self.pad
        };
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
    clock: FixedClock,
    last: Option<Instant>,
    ticks: u64,
    frames: u64,
    max_frames: Option<u64>,
    play: Play,
    /// Mario's imported model; None draws the placeholder box instead.
    mario: Option<MarioModel>,
    error: Option<String>,
}

impl App {
    fn create_gpu(&mut self, event_loop: &ActiveEventLoop) -> AppResult<Gpu> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Rustario64 — Bob-omb Battlefield (development viewer)")
                    .with_inner_size(PhysicalSize::new(1280, 960)),
            )?,
        );
        let instance =
            rustario64_render::instance(Some(Box::new(event_loop.owned_display_handle())));
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
        let renderer = Renderer::new(device, queue, config.format, self.options);
        let mut gpu = Gpu {
            window,
            surface,
            depth: renderer.create_depth(config.width, config.height),
            multisampled: None,
            config,
            renderer,
        };
        upload(
            &mut gpu.renderer,
            &self.level,
            false,
            false,
            self.mario.is_none(),
        );
        resize_targets(&mut gpu);
        println!(
            "Viewer on {} ({:?}), surface {:?}",
            adapter.get_info().name,
            adapter.get_info().backend,
            gpu.config.format
        );
        Ok(gpu)
    }

    fn redraw(&mut self) {
        let now = Instant::now();
        let elapsed = self.last.map_or(Duration::ZERO, |last| now - last);
        self.last = Some(now);
        // Fixed 30 Hz gameplay cadence: Mario ticks only on drained clock ticks.
        let drained = if self.clock.add_elapsed(elapsed).is_ok() {
            self.clock.drain(8)
        } else {
            0
        };
        self.ticks += u64::from(drained);
        let play = &mut self.play;
        if play.active {
            for _ in 0..drained {
                let pad = play.next_pad();
                if play.session.step(&pad)
                    && let Some(mario) = self.mario.as_mut()
                {
                    mario.tick(&play.session, &self.camera);
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
        let alpha = if play.active && play.session.stopped().is_none() {
            self.clock.alpha()
        } else {
            1.0
        };
        let pose = play.session.pose(alpha, play.graphics);
        if play.active {
            self.camera = present::follow_view(
                play.session.camera(),
                pose.position,
                alpha,
                self.level.visual.camera,
            );
        } else {
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
        gpu.window.pre_present_notify();
        gpu.renderer.queue().present(frame);
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
            KeyCode::ArrowLeft => pad.camera_left = down,
            KeyCode::ArrowRight => pad.camera_right = down,
            KeyCode::KeyR => {
                if down && !repeat {
                    if let Err(e) = self.play.end_run(self.level.rom_sha1) {
                        eprintln!("error: {e}");
                    }
                    self.play.session.reset();
                    self.play.taps = Pad::default();
                    self.play.reported_stop = false;
                    if let Some(mario) = self.mario.as_mut() {
                        mario.reset(&self.play.session, &self.camera);
                    }
                }
            }
            _ => return false,
        }
        true
    }

    fn key(&mut self, event_loop: &ActiveEventLoop, event: KeyEvent) {
        let PhysicalKey::Code(code) = event.physical_key else {
            return;
        };
        let down = event.state == ElementState::Pressed;
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
            KeyCode::Escape if down => event_loop.exit(),
            KeyCode::KeyM if down && !event.repeat => {
                // Held keys belong to the mode they were pressed in.
                self.keys = Keys::default();
                self.play.release_all();
                self.play.active ^= true;
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
            match self.create_gpu(event_loop) {
                Ok(gpu) => {
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
                if !focused {
                    self.last = None;
                    self.keys = Keys::default();
                    self.play.release_all();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => self.key(event_loop, event),
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Right,
                ..
            } => {
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
        {
            self.camera
                .rotate(-delta.0 as f32 * 0.003, -delta.1 as f32 * 0.003);
        }
    }
}

fn view(rom_path: &Path, options: &Options) -> AppResult<()> {
    if options.mario_ticks.is_some() {
        return Err("--mario-ticks is a screenshot option; use --mario in the window".into());
    }
    let recorder = options
        .record
        .as_deref()
        .map(Recorder::create)
        .transpose()?;
    let level = load(rom_path)?;
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
    let mut app = App {
        level,
        options: render_options(options),
        camera,
        gpu: None,
        keys: Keys::default(),
        looking: false,
        clock: FixedClock::default(),
        last: None,
        ticks: 0,
        frames: 0,
        max_frames: options.frames,
        play,
        mario,
        error: None,
    };
    let event_loop = EventLoop::new()?;
    let result = event_loop.run_app(&mut app);
    let saved = app.play.end_run(app.level.rom_sha1);
    result?;
    saved?;
    if let Some(error) = app.error {
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

fn run(args: &[String]) -> AppResult<()> {
    match args {
        [cmd, rom, rest @ ..] if cmd == "screenshot" => screenshot(Path::new(rom), &parse(rest)?),
        [cmd, rom, rest @ ..] if cmd == "view" => view(Path::new(rom), &parse(rest)?),
        _ => {
            print!("{HELP}");
            Ok(())
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
