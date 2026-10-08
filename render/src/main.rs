//! Development viewer for imported levels. `screenshot` renders offscreen to PNG
//! (no window needed); `view` opens a window with a free inspection camera.
//! Both are development entry points straight into Bob-omb Battlefield.
use rustario64::{
    content::visual::{AreaVisual, VisualModel},
    import::{bob, rom::Rom},
    simulation::FixedClock,
};
use rustario64_render::{RenderOptions, Renderer, camera, camera::FlyCamera, overlay};
use std::{
    path::Path,
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
  rustario64-viewer screenshot /path/to/sm64.z64 --out private/bob.png [--view start|overview|summit|top] [--size 1280x960] [--msaa 4] [--no-cull] [--no-fog] [--collision] [--placements]\n\
  rustario64-viewer view /path/to/sm64.z64 [--view start] [--msaa 4] [--no-fog] [--frames N]\n\n\
  View controls: WASD move, Q/E down/up, Shift faster, hold right mouse or arrow keys to look,\n\
  1-4 select presets, C collision overlay, P placement markers, F fog, Esc quits.\n\
  The camera is a presentation-only inspection camera, not the original camera.\n";

struct Options {
    view: String,
    size: (u32, u32),
    msaa: u32,
    cull: bool,
    fog: bool,
    collision: bool,
    placements: bool,
    out: Option<String>,
    frames: Option<u64>,
}

fn parse(args: &[String]) -> AppResult<Options> {
    let mut options = Options {
        view: "start".into(),
        size: (1280, 960),
        msaa: 1,
        cull: true,
        fog: true,
        collision: false,
        placements: false,
        out: None,
        frames: None,
    };
    let mut i = 0;
    while i < args.len() {
        let value = |i: usize| -> AppResult<&String> {
            args.get(i + 1)
                .ok_or_else(|| format!("{} needs a value", args[i]).into())
        };
        match args[i].as_str() {
            "--view" => options.view = value(i)?.clone(),
            "--out" => options.out = Some(value(i)?.clone()),
            "--size" => {
                let (w, h) = value(i)?
                    .split_once('x')
                    .ok_or("size must look like 1280x960")?;
                options.size = (w.parse()?, h.parse()?);
            }
            "--msaa" => options.msaa = value(i)?.parse()?,
            "--frames" => options.frames = Some(value(i)?.parse()?),
            "--no-cull" => {
                options.cull = false;
                i += 1;
                continue;
            }
            "--no-fog" => {
                options.fog = false;
                i += 1;
                continue;
            }
            "--collision" => {
                options.collision = true;
                i += 1;
                continue;
            }
            "--placements" => {
                options.placements = true;
                i += 1;
                continue;
            }
            other => return Err(format!("unknown option {other}\n\n{HELP}").into()),
        }
        i += 2;
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
}

/// Model indices in the renderer: terrain, collision overlay, placement markers.
const TERRAIN: usize = 0;
const COLLISION: usize = 1;
const PLACEMENTS: usize = 2;

fn upload(renderer: &mut Renderer, level: &Level, collision: bool, placements: bool) {
    renderer.load_model(&level.visual.model);
    renderer.add_model(&level.collision);
    renderer.add_model(&level.placements);
    renderer.set_visible(COLLISION, collision);
    renderer.set_visible(PLACEMENTS, placements);
    debug_assert!(renderer.is_visible(TERRAIN));
}

fn load(rom_path: &Path) -> AppResult<Level> {
    let rom = Rom::open(rom_path)?;
    let imported = bob::import(&rom)?;
    let visual = imported.visual.ok_or("no visible geometry imported")?;
    Ok(Level {
        visual,
        collision: overlay::collision(&imported.collision),
        placements: overlay::placements(&imported.level, &imported.collision),
        mario_start: imported.level.mario_start.map(|(_, yaw, pos)| (yaw, pos)),
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

fn screenshot(rom_path: &Path, options: &Options) -> AppResult<()> {
    let out = options.out.as_deref().ok_or("screenshot needs --out")?;
    let level = load(rom_path)?;
    let (info, mut renderer) = rustario64_render::headless(render_options(options))?;
    upload(&mut renderer, &level, options.collision, options.placements);
    let camera = preset(&options.view, &level)?;
    let (w, h) = options.size;
    let pixels = renderer.capture(w, h, &camera)?;
    rustario64_render::write_png(Path::new(out), w, h, &pixels)?;
    println!(
        "Rendered {} triangles ({} batches, {} textures) from view '{}' at {w}x{h} on {} ({:?}) to {out}",
        level.visual.model.triangle_count(),
        level.visual.model.batches.len(),
        level.visual.model.textures.len(),
        options.view,
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
    println!("Skybox, objects, and Mario are not drawn yet; the sky is a placeholder clear color.");
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
        // Prefer a non-sRGB target: original colors are display-referred values.
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
        upload(&mut gpu.renderer, &self.level, false, false);
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
        // Fixed 30 Hz gameplay cadence. No gameplay systems run yet; ticks are
        // counted so the loop structure is the one gameplay will use.
        if self.clock.add_elapsed(elapsed).is_ok() {
            self.ticks += u64::from(self.clock.drain(8));
        }
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
        let Some(gpu) = self.gpu.as_mut() else {
            return;
        };
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
        if self.frames.is_multiple_of(120) {
            let p = self.camera.position;
            gpu.window.set_title(&format!(
                "Rustario64 — BOB viewer — {} ticks @30 Hz — camera ({:.0}, {:.0}, {:.0})",
                self.ticks, p[0], p[1], p[2]
            ));
        }
    }

    fn key(&mut self, event_loop: &ActiveEventLoop, event: KeyEvent) {
        let PhysicalKey::Code(code) = event.physical_key else {
            return;
        };
        let down = event.state == ElementState::Pressed;
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
            WindowEvent::Focused(focused) => {
                if !focused {
                    self.last = None;
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
        {
            self.camera
                .rotate(-delta.0 as f32 * 0.003, -delta.1 as f32 * 0.003);
        }
    }
}

fn view(rom_path: &Path, options: &Options) -> AppResult<()> {
    let level = load(rom_path)?;
    let camera = preset(&options.view, &level)?;
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
        error: None,
    };
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.error {
        return Err(error.into());
    }
    println!(
        "Viewer closed after {} frames and {} fixed 30 Hz ticks",
        app.frames, app.ticks
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
