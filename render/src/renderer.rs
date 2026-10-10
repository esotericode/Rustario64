//! wgpu presentation of engine-owned `VisualModel`s. The renderer reads imported
//! content and a presentation camera; it holds no gameplay state and writes none.
use crate::camera::FlyCamera;
use rustario64::content::visual::{
    BlendMode, LAYER_COUNT, Material, TextureFilter, VisualModel, VisualVertex, WrapMode,
};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const VERTEX_STRIDE: u64 = 24;
const MATERIAL_BYTES: usize = 208;
/// Byte offset of the per-model transform inside the material uniform.
const TRANSFORM_OFFSET: u64 = 192;
const FRAME_BYTES: usize = 144;

/// Graphics-only options. None of these can alter authoritative simulation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderOptions {
    pub cull_faces: bool,
    /// Original gsSPFogPosition fog; disabling it is an inspection aid.
    pub fog: bool,
    /// Clear color used until the original skybox is imported.
    pub clear_color: [f64; 4],
    pub sample_count: u32,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            cull_faces: true,
            fog: true,
            clear_color: [0.40, 0.58, 0.85, 1.0],
            sample_count: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PipelineKey {
    blend: BlendMode,
    depth_test: bool,
    depth_write: bool,
    decal: bool,
    cull: Option<wgpu::Face>,
}

struct GpuBatch {
    /// The model batch this was uploaded from.
    source: usize,
    layer: u8,
    key: PipelineKey,
    vertices: wgpu::Buffer,
    count: u32,
    bind_group: wgpu::BindGroup,
    material: wgpu::Buffer,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    color_format: wgpu::TextureFormat,
    options: RenderOptions,
    shader: wgpu::ShaderModule,
    material_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    frame_buffer: wgpu::Buffer,
    frame_group: wgpu::BindGroup,
    /// Uploaded models with their visibility; drawn together by layer.
    models: Vec<(bool, Vec<GpuBatch>)>,
}

fn rgba(c: [u8; 4]) -> [f32; 4] {
    c.map(|v| f32::from(v) / 255.0)
}

fn push_f32s(out: &mut Vec<u8>, values: &[f32]) {
    for v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

/// Vertex buffer contents: position, uv, then the color bytes.
fn pack_vertices(vertices: &[VisualVertex]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vertices.len() * VERTEX_STRIDE as usize);
    for v in vertices {
        push_f32s(&mut bytes, &v.position);
        push_f32s(&mut bytes, &v.uv);
        bytes.extend_from_slice(&v.color);
    }
    bytes
}

fn push_u32s(out: &mut Vec<u8>, values: &[u32]) {
    for v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

fn material_bytes(m: &Material) -> Vec<u8> {
    let mut out = Vec::with_capacity(MATERIAL_BYTES);
    for cycle in m.combiner {
        push_u32s(&mut out, &cycle.rgb.map(u32::from));
        push_u32s(&mut out, &cycle.alpha.map(u32::from));
    }
    push_f32s(&mut out, &rgba(m.prim_color));
    push_f32s(&mut out, &rgba(m.env_color));
    let fog_color = m.fog.map_or([0; 4], |f| f.color);
    push_f32s(&mut out, &rgba(fog_color));
    let (ambient, diffuse, direction) = match m.lights {
        Some(l) => (
            [l.ambient[0], l.ambient[1], l.ambient[2], 255],
            [l.diffuse[0], l.diffuse[1], l.diffuse[2], 255],
            l.direction.map(f32::from),
        ),
        None => ([0; 4], [0; 4], [0.0, 0.0, 1.0]),
    };
    push_f32s(&mut out, &rgba(ambient));
    push_f32s(&mut out, &rgba(diffuse));
    push_f32s(&mut out, &[direction[0], direction[1], direction[2], 0.0]);
    let mut flags = 0u32;
    if m.lights.is_some() {
        flags |= 1;
    }
    if m.texture.is_some() {
        flags |= 2;
    }
    if m.two_cycle {
        flags |= 4;
    }
    if m.fog.is_some() {
        flags |= 8;
    }
    if m.blend == BlendMode::Cutout {
        flags |= 16;
    }
    if m.texture_gen {
        flags |= 32;
    }
    push_u32s(&mut out, &[flags, 0, 0, 0]);
    let (multiplier, offset) = m.fog.map_or((0.0, 0.0), |f| {
        (f32::from(f.multiplier), f32::from(f.offset))
    });
    push_f32s(&mut out, &[multiplier, offset, 0.0, 0.0]);
    // Model transform: translation and yaw, identity until set_transform.
    push_f32s(&mut out, &[0.0, 0.0, 0.0, 0.0]);
    debug_assert_eq!(out.len(), MATERIAL_BYTES);
    out
}

fn address_mode(mode: WrapMode) -> wgpu::AddressMode {
    match mode {
        WrapMode::Repeat => wgpu::AddressMode::Repeat,
        WrapMode::MirrorRepeat => wgpu::AddressMode::MirrorRepeat,
        WrapMode::Clamp => wgpu::AddressMode::ClampToEdge,
    }
}

impl Renderer {
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        color_format: wgpu::TextureFormat,
        options: RenderOptions,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fast3d batches"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let uniform = |binding, visibility| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame"),
            entries: &[uniform(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
        });
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material"),
            entries: &[
                uniform(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fast3d"),
            bind_group_layouts: &[Some(&frame_layout), Some(&material_layout)],
            immediate_size: 0,
        });
        let frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame"),
            size: FRAME_BYTES as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buffer.as_entire_binding(),
            }],
        });
        Self {
            device,
            queue,
            color_format,
            options,
            shader,
            material_layout,
            pipeline_layout,
            pipelines: HashMap::new(),
            frame_buffer,
            frame_group,
            models: vec![],
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn options(&self) -> RenderOptions {
        self.options
    }

    /// Per-frame options (fog, clear color) apply on the next frame. Culling and
    /// MSAA are baked into pipelines and targets, so they apply on the next upload.
    pub fn options_mut(&mut self) -> &mut RenderOptions {
        &mut self.options
    }

    /// Replace every uploaded model with this one.
    pub fn load_model(&mut self, model: &VisualModel) {
        self.models.clear();
        self.add_model(model);
    }

    pub fn set_visible(&mut self, model: usize, visible: bool) {
        if let Some(entry) = self.models.get_mut(model) {
            entry.0 = visible;
        }
    }

    pub fn is_visible(&self, model: usize) -> bool {
        self.models.get(model).is_some_and(|m| m.0)
    }

    /// Place a model: rotate its vertices about +Y by `yaw` (radians, with the
    /// original's convention that yaw 0 faces +Z and positive yaw turns toward
    /// +X), then translate. Presentation only, e.g. for an interpolated pose.
    pub fn set_transform(&mut self, model: usize, translation: [f32; 3], yaw: f32) {
        let mut bytes = Vec::with_capacity(16);
        push_f32s(
            &mut bytes,
            &[translation[0], translation[1], translation[2], yaw],
        );
        if let Some((_, batches)) = self.models.get(model) {
            for batch in batches {
                self.queue
                    .write_buffer(&batch.material, TRANSFORM_OFFSET, &bytes);
            }
        }
    }

    /// Rewrite an uploaded model's vertices in place, batch by batch, for a
    /// model posed anew each frame. Each list must keep its batch's vertex
    /// count from upload; other lists are ignored.
    pub fn update_vertices(&mut self, model: usize, vertices: &[Vec<VisualVertex>]) {
        let Some((_, batches)) = self.models.get(model) else {
            return;
        };
        for batch in batches {
            if let Some(list) = vertices.get(batch.source)
                && list.len() == batch.count as usize
            {
                self.queue
                    .write_buffer(&batch.vertices, 0, &pack_vertices(list));
            }
        }
    }

    /// Repeat a cached object template for a variable number of instances.
    /// Buffers grow when needed and retain capacity when objects disappear.
    pub fn update_dynamic_vertices(&mut self, model: usize, vertices: &[Vec<VisualVertex>]) {
        let Some((_, batches)) = self.models.get_mut(model) else {
            return;
        };
        for batch in batches {
            let list = vertices.get(batch.source).map_or(&[][..], Vec::as_slice);
            let bytes = pack_vertices(list);
            if bytes.len() as u64 > batch.vertices.size() {
                batch.vertices = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("object instances"),
                    size: (bytes.len() as u64).next_power_of_two(),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            if !bytes.is_empty() {
                self.queue.write_buffer(&batch.vertices, 0, &bytes);
            }
            batch.count = list.len() as u32;
        }
    }

    /// Upload a model's textures and batches; returns its index (initially visible).
    pub fn add_model(&mut self, model: &VisualModel) -> usize {
        let views: Vec<_> = model
            .textures
            .iter()
            .map(|t| {
                let texture = self.device.create_texture_with_data(
                    &self.queue,
                    &wgpu::TextureDescriptor {
                        label: Some("imported texture"),
                        size: wgpu::Extent3d {
                            width: u32::from(t.width),
                            height: u32::from(t.height),
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        // Raw values: original colors are display-referred.
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    },
                    wgpu::util::TextureDataOrder::LayerMajor,
                    &t.rgba,
                );
                texture.create_view(&wgpu::TextureViewDescriptor::default())
            })
            .collect();
        let white = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: Some("untextured"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &[255; 4],
        );
        let white = white.create_view(&wgpu::TextureViewDescriptor::default());
        let mut samplers: HashMap<(WrapMode, WrapMode, TextureFilter), wgpu::Sampler> =
            HashMap::new();
        let mut batches = vec![];
        for (index, batch) in model.batches.iter().enumerate() {
            let m = &batch.material;
            let (wrap, filter) = m.texture.map_or(
                (
                    (WrapMode::Repeat, WrapMode::Repeat),
                    TextureFilter::Bilinear,
                ),
                |t| ((t.wrap[0], t.wrap[1]), t.filter),
            );
            let sampler = samplers.entry((wrap.0, wrap.1, filter)).or_insert_with(|| {
                let f = match filter {
                    TextureFilter::Point => wgpu::FilterMode::Nearest,
                    TextureFilter::Bilinear => wgpu::FilterMode::Linear,
                };
                self.device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("imported sampler"),
                    address_mode_u: address_mode(wrap.0),
                    address_mode_v: address_mode(wrap.1),
                    mag_filter: f,
                    min_filter: f,
                    ..Default::default()
                })
            });
            let uniform = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("material"),
                    contents: &material_bytes(m),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });
            let view = m.texture.map_or(&white, |t| &views[t.texture]);
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("material"),
                layout: &self.material_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            });
            let vertices = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("batch vertices"),
                    contents: &pack_vertices(&batch.vertices),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });
            let cull = if !self.options.cull_faces {
                None
            } else if m.cull_back && m.cull_front {
                // G_CULL_BOTH draws nothing; skip the batch entirely.
                continue;
            } else if m.cull_back {
                Some(wgpu::Face::Back)
            } else if m.cull_front {
                Some(wgpu::Face::Front)
            } else {
                None
            };
            batches.push(GpuBatch {
                source: index,
                layer: m.layer,
                key: PipelineKey {
                    blend: m.blend,
                    depth_test: m.depth_test,
                    depth_write: m.depth_write,
                    decal: m.decal,
                    cull,
                },
                vertices,
                count: batch.vertices.len() as u32,
                bind_group,
                material: uniform,
            });
        }
        for key in batches.iter().map(|b| b.key).collect::<Vec<_>>() {
            self.pipeline(key);
        }
        self.models.push((true, batches));
        self.models.len() - 1
    }

    fn pipeline(&mut self, key: PipelineKey) -> &wgpu::RenderPipeline {
        let device = &self.device;
        let layout = &self.pipeline_layout;
        let shader = &self.shader;
        let format = self.color_format;
        let samples = self.options.sample_count;
        self.pipelines.entry(key).or_insert_with(|| {
            let blend = match key.blend {
                BlendMode::Translucent => Some(wgpu::BlendState::ALPHA_BLENDING),
                _ => None,
            };
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("fast3d batch"),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: VERTEX_STRIDE,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x3, 1 => Float32x2, 2 => Uint8x4
                        ],
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: key.cull,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(key.depth_write),
                    depth_compare: Some(if !key.depth_test {
                        wgpu::CompareFunction::Always
                    } else if key.decal {
                        wgpu::CompareFunction::LessEqual
                    } else {
                        wgpu::CompareFunction::Less
                    }),
                    stencil: wgpu::StencilState::default(),
                    bias: if key.decal {
                        wgpu::DepthBiasState {
                            constant: -4,
                            slope_scale: -1.0,
                            clamp: 0.0,
                        }
                    } else {
                        wgpu::DepthBiasState::default()
                    },
                }),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        })
    }

    pub fn create_depth(&self, width: u32, height: u32) -> wgpu::TextureView {
        self.device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("depth"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: self.options.sample_count,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// Draw the loaded scene: layers 0..7 in turn, batches in original order.
    pub fn render(
        &mut self,
        target: &wgpu::TextureView,
        resolve: Option<&wgpu::TextureView>,
        depth: &wgpu::TextureView,
        size: (u32, u32),
        camera: &FlyCamera,
    ) {
        let aspect = size.0 as f32 / size.1.max(1) as f32;
        let mut frame = Vec::with_capacity(FRAME_BYTES);
        for column in camera.view().iter().chain(camera.projection(aspect).iter()) {
            push_f32s(&mut frame, column);
        }
        let srgb = if self.color_format.is_srgb() {
            1.0
        } else {
            0.0
        };
        let fog = if self.options.fog { 1.0 } else { 0.0 };
        push_f32s(&mut frame, &[camera.near, camera.far, srgb, fog]);
        self.queue.write_buffer(&self.frame_buffer, 0, &frame);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        {
            let [r, g, b, a] = self.options.clear_color;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: resolve,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.frame_group, &[]);
            let visible: Vec<_> = self.models.iter().filter(|m| m.0).collect();
            for layer in 0..LAYER_COUNT as u8 {
                let batches = visible.iter().flat_map(|m| m.1.iter());
                for batch in batches.filter(|b| b.layer == layer) {
                    pass.set_pipeline(&self.pipelines[&batch.key]);
                    pass.set_bind_group(1, &batch.bind_group, &[]);
                    pass.set_vertex_buffer(0, batch.vertices.slice(..));
                    pass.draw(0..batch.count, 0..1);
                }
            }
        }
        self.queue.submit([encoder.finish()]);
    }

    /// Render offscreen and read back tightly packed RGBA8 rows.
    pub fn capture(
        &mut self,
        width: u32,
        height: u32,
        camera: &FlyCamera,
    ) -> Result<Vec<u8>, String> {
        let make_target = |samples: u32, usage| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("capture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: self.color_format,
                usage,
                view_formats: &[],
            })
        };
        let resolved = make_target(
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let multisampled = (self.options.sample_count > 1).then(|| {
            make_target(
                self.options.sample_count,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            )
        });
        let resolved_view = resolved.create_view(&wgpu::TextureViewDescriptor::default());
        let depth = self.create_depth(width, height);
        match &multisampled {
            Some(ms) => {
                let view = ms.create_view(&wgpu::TextureViewDescriptor::default());
                self.render(&view, Some(&resolved_view), &depth, (width, height), camera);
            }
            None => self.render(&resolved_view, None, &depth, (width, height), camera),
        }
        let padded = (width * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture readback"),
            size: u64::from(padded * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("readback"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &resolved,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        receiver
            .recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        let data = slice.get_mapped_range().map_err(|e| e.to_string())?;
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for row in data.chunks_exact(padded as usize) {
            pixels.extend_from_slice(&row[..(width * 4) as usize]);
        }
        drop(data);
        buffer.unmap();
        if matches!(
            self.color_format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            for p in pixels.as_chunks_mut::<4>().0.iter_mut() {
                p.swap(0, 2);
            }
        }
        Ok(pixels)
    }
}
