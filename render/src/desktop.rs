//! Desktop presentation state. Pauses gate the scheduler; settings never enter
//! the authoritative game frame or replay metadata.
use rustario64::simulation::FixedClock;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, time::Instant};
use winit::window::Window;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub interpolation: bool,
    pub fog: bool,
    pub vsync: bool,
    pub fullscreen: bool,
    pub size: [u32; 2],
    /// Opt-in path only; never the ROM or extracted content.
    pub rom_path: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            interpolation: true,
            fog: true,
            vsync: true,
            fullscreen: false,
            size: [1280, 960],
            rom_path: None,
        }
    }
}

impl Settings {
    pub fn path() -> Option<PathBuf> {
        let base = if cfg!(target_os = "windows") {
            std::env::var_os("APPDATA").map(PathBuf::from)
        } else if cfg!(target_os = "macos") {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        }?;
        Some(base.join("rustario64/settings.json"))
    }

    pub fn load() -> Result<Self, String> {
        let Some(path) = Self::path() else {
            return Ok(Self::default());
        };
        match fs::read(&path) {
            Ok(bytes) => Self::decode(&bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("Could not read settings: {e}")),
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let settings: Self =
            serde_json::from_slice(bytes).map_err(|e| format!("Invalid settings: {e}"))?;
        if settings.size.iter().any(|&s| !(320..=8192).contains(&s)) {
            return Err("Window dimensions must be between 320 and 8192".into());
        }
        Ok(settings)
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path().ok_or("No user settings directory is available")?;
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::write(
            path,
            serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("Could not save settings: {e}"))
    }

    pub fn controls(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.interpolation, "Smooth Mario and camera motion");
        ui.checkbox(&mut self.fog, "Original distance fog");
        ui.checkbox(&mut self.vsync, "VSync");
        ui.checkbox(&mut self.fullscreen, "Borderless fullscreen");
    }
}

#[derive(Default)]
pub struct FrameClock {
    clock: FixedClock,
    last: Option<Instant>,
}

impl FrameClock {
    pub fn reset(&mut self) {
        self.clock = FixedClock::default();
        self.last = None;
    }

    /// Time in menus, inspection mode and unfocused windows is discarded.
    /// While running, stalls keep their backlog and drain at most eight ticks
    /// per frame, exactly as the original viewer's documented policy.
    pub fn advance(&mut self, now: Instant, running: bool) -> u32 {
        if !running {
            self.reset();
            return 0;
        }
        let elapsed = self
            .last
            .replace(now)
            .map_or(std::time::Duration::ZERO, |last| now - last);
        if self.clock.add_elapsed(elapsed).is_err() {
            return 0;
        }
        self.clock.drain(8)
    }

    pub fn alpha(&self) -> f32 {
        self.clock.alpha()
    }
}

/// egui paints into the resolved surface after the scene, at one sample.
pub struct Ui {
    pub context: egui::Context,
    pub input: egui_winit::State,
    renderer: egui_wgpu::Renderer,
}

impl Ui {
    pub fn new(window: &Window, device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let context = egui::Context::default();
        context.set_visuals(egui::Visuals::dark());
        context.style_mut_of(egui::Theme::Dark, |style| {
            style.spacing.item_spacing = egui::vec2(12.0, 12.0);
            style.spacing.button_padding = egui::vec2(16.0, 10.0);
        });
        let input = egui_winit::State::new(
            context.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(device.limits().max_texture_dimension_2d as usize),
        );
        let renderer =
            egui_wgpu::Renderer::new(device, format, egui_wgpu::RendererOptions::default());
        Self {
            context,
            input,
            renderer,
        }
    }

    pub fn paint(
        &mut self,
        window: &Window,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        mut output: egui::FullOutput,
    ) {
        self.input
            .handle_platform_output(window, std::mem::take(&mut output.platform_output));
        paint_ui(
            &mut self.renderer,
            &self.context,
            device,
            queue,
            target,
            output,
        );
    }
}

fn paint_ui(
    renderer: &mut egui_wgpu::Renderer,
    context: &egui::Context,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target: &wgpu::TextureView,
    mut output: egui::FullOutput,
) {
    let jobs = context.tessellate(output.shapes, output.pixels_per_point);
    // Window size changes can arrive before the surface's resize event.
    // Clip and project against the acquired target for this frame, including
    // egui's final full-target scissor reset.
    let screen = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [target.texture().width(), target.texture().height()],
        pixels_per_point: output.pixels_per_point,
    };
    // Consume each applied update: egui checks for pending deltas on drop in
    // debug builds, even when the renderer has already uploaded their data.
    for (id, deltas) in output.textures_delta.set.drain() {
        for delta in deltas {
            renderer.update_texture(device, queue, id, &delta);
        }
    }
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("desktop UI"),
    });
    let commands = renderer.update_buffers(device, queue, &mut encoder, &jobs, &screen);
    {
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("desktop UI"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        renderer.render(&mut pass.forget_lifetime(), &jobs, &screen);
    }
    queue.submit(commands.into_iter().chain([encoder.finish()]));
    for id in output.textures_delta.free.drain() {
        renderer.free_texture(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn ui_renders_while_window_and_target_sizes_disagree() {
        let instance = crate::instance(None);
        let (adapter, device, queue) = match pollster::block_on(crate::device(&instance, None)) {
            Ok(gpu) => gpu,
            Err(error) if std::env::var_os("RUSTARIO64_REQUIRE_GPU").is_none() => {
                eprintln!("skipping GPU test: {error}");
                return;
            }
            Err(error) => panic!("GPU required: {error}"),
        };
        eprintln!("UI resize regression on {}", adapter.get_info().name);
        let context = egui::Context::default();
        let mut temporary_texture = Some(context.load_texture(
            "UI texture lifecycle regression",
            egui::ColorImage::new([1, 1], vec![egui::Color32::WHITE]),
            egui::TextureOptions::default(),
        ));
        let temporary_id = temporary_texture.as_ref().unwrap().id();
        let mut renderer = egui_wgpu::Renderer::new(
            &device,
            crate::CAPTURE_FORMAT,
            egui_wgpu::RendererOptions::default(),
        );
        // Launcher -> game, game -> launcher, fullscreen-style growth, and
        // fractional/integer display scaling. Render real egui draw commands.
        for (window, target, scale) in [
            ([1280, 960], [800, 720], 1.0),
            ([800, 720], [1280, 960], 1.0),
            ([1920, 1080], [800, 720], 1.25),
            ([2560, 1440], [800, 720], 2.0),
        ] {
            context.set_pixels_per_point(scale);
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("UI resize regression target"),
                size: wgpu::Extent3d {
                    width: target[0],
                    height: target[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: crate::CAPTURE_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(window[0] as f32 / scale, window[1] as f32 / scale),
                    )),
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default().show(root, |ui| {
                        ui.heading("Rustario64 launcher / pause");
                        ui.label("A resize is pending while this frame is drawn.");
                    });
                },
            );
            assert!(!output.shapes.is_empty());
            if temporary_texture.is_some() {
                assert!(output.textures_delta.set.contains_key(&temporary_id));
            } else if window == [800, 720] {
                assert!(output.textures_delta.free.contains(&temporary_id));
            }
            paint_ui(
                &mut renderer,
                &context,
                &device,
                &queue,
                &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                output,
            );
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            // The following frame must consume the corresponding free command.
            drop(temporary_texture.take());
        }
    }

    #[test]
    fn pause_focus_and_inspection_discard_time_and_backlog() {
        let start = Instant::now();
        let mut clock = FrameClock::default();
        assert_eq!(clock.advance(start, true), 0);
        assert_eq!(clock.advance(start + Duration::from_secs(1), true), 8);
        // A pending backlog must disappear as well as subsequent menu time.
        assert_eq!(clock.advance(start + Duration::from_secs(2), false), 0);
        assert_eq!(clock.advance(start + Duration::from_secs(90), false), 0);
        assert_eq!(clock.advance(start + Duration::from_secs(91), true), 0);
        assert_eq!(clock.advance(start + Duration::from_secs(92), true), 8);
        clock.reset();
        assert_eq!(clock.advance(start + Duration::from_secs(100), true), 0);
        assert_eq!(
            clock.advance(start + Duration::from_millis(100034), true),
            1
        );
    }

    #[test]
    fn settings_recover_defaults_and_reject_invalid_files() {
        assert_eq!(Settings::decode(b"{}").unwrap(), Settings::default());
        assert!(Settings::decode(br#"{"size":[0,960]}"#).is_err());
        assert!(Settings::decode(br#"{"physics_rate":60}"#).is_err());
        assert!(Settings::decode(b"not json").is_err());
        let settings = Settings {
            interpolation: false,
            rom_path: Some("local.z64".into()),
            ..Settings::default()
        };
        assert_eq!(
            Settings::decode(&serde_json::to_vec(&settings).unwrap()).unwrap(),
            settings
        );
    }
}
