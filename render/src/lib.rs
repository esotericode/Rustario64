//! Optional wgpu presentation for Rustario64. This crate depends on the headless
//! core; the core never depends on it. Rendering reads imported content and a
//! presentation camera only, so graphics settings cannot alter gameplay state.
pub mod camera;
pub mod math;
pub mod overlay;
pub mod play;
pub mod renderer;

pub use renderer::{RenderOptions, Renderer};

/// Offscreen color format: raw display-referred values, no sRGB conversion.
pub const CAPTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Backends and flags may be overridden with wgpu's standard environment variables.
pub fn instance(display: Option<Box<dyn wgpu::wgt::WgpuHasDisplayHandle>>) -> wgpu::Instance {
    wgpu::Instance::new(match display {
        Some(display) => wgpu::InstanceDescriptor::new_with_display_handle_from_env(display),
        None => wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
    })
}

pub async fn device(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), String> {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: surface,
            apply_limit_buckets: false,
        })
        .await
        .map_err(|e| format!("no compatible GPU adapter: {e}"))?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("rustario64"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("device request failed: {e}"))?;
    Ok((adapter, device, queue))
}

/// A renderer that draws into offscreen textures (no window or surface).
pub fn headless(options: RenderOptions) -> Result<(wgpu::AdapterInfo, Renderer), String> {
    let instance = instance(None);
    let (adapter, device, queue) = pollster::block_on(self::device(&instance, None))?;
    Ok((
        adapter.get_info(),
        Renderer::new(device, queue, CAPTURE_FORMAT, options),
    ))
}

pub fn write_png(
    path: &std::path::Path,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(), String> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())
}
