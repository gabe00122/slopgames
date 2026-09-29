//! wgpu renderer.
//!
//! Frame layout:
//! 1. compute: raycast one screen column per invocation into `hits`
//! 2. render (low-res target): walls/floors/sky fullscreen pass writing depth,
//!    then world sprites depth-tested against it, then the 2D overlay
//! 3. render (window): sharp-bilinear upscale + tint

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::map::Map;
use crate::math::V2;
use crate::sprites::{Sprites, SPR};
use crate::textures::{self, Image, NUM_SURFACES, SKY_H, SKY_W, TEX};

pub const FAR: f32 = 64.0;
pub const MAX_LIGHTS: usize = 16;
const MAX_COLUMNS: u32 = 4096;
const MAX_CELLS: usize = 128 * 128;
const MAX_QUADS: usize = 16384;
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Vertical half-FOV tangent for a full-height view.
const TAN_V: f32 = 0.58;
/// Selectable internal render heights (F2 cycles).
pub const RESOLUTIONS: [u32; 4] = [300, 400, 600, 800];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default, Debug)]
pub struct Quad {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    pub color: [f32; 4],
    pub params: [f32; 4],
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Light {
    pub pos: [f32; 3],
    pub radius: f32,
    pub color: [f32; 3],
}

/// Sizes the game needs to lay out sprites and the HUD.
#[derive(Clone, Copy, Debug)]
pub struct View {
    /// Internal render target size.
    pub w: f32,
    pub h: f32,
    /// Height of the 3D view (above the status bar).
    pub view_h: f32,
    pub tan_h: f32,
    pub tan_v: f32,
    /// HUD unit in pixels.
    pub u: f32,
}

pub struct Frame {
    pub cam_pos: V2,
    pub cam_angle: f32,
    pub eye_z: f32,
    pub time: f32,
    pub lights: Vec<Light>,
    pub sprites: Vec<Quad>,
    pub ui: Vec<Quad>,
    pub tint: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuLight {
    pos_r: [f32; 4],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    cam: [f32; 4],
    right_tan: [f32; 4],
    view: [f32; 4],
    misc: [f32; 4],
    lights: [GpuLight; MAX_LIGHTS],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Post {
    tint: [f32; 4],
    size: [f32; 4],
    flags: [f32; 4],
}

/// GPU mirror of the WGSL `Hit` struct (32 bytes per column).
const HIT_SIZE: u64 = 32;

fn shader_source(body: &str) -> String {
    use textures::*;
    let header = format!(
        "const FAR: f32 = {FAR:.1};\nconst MAX_LIGHTS: u32 = {MAX_LIGHTS}u;\nconst TEX_DOORTRAK: u32 = {T_DOORTRAK}u;\n\
         const F_NUKAGE: u32 = {F_NUKAGE}u;\nconst F_LAVA: u32 = {F_LAVA}u;\nconst SKY: u32 = {SKY}u;\n"
    );
    format!("{header}{}\n{body}", include_str!("shaders/common.wgsl"))
}

enum Target {
    Window { surface: wgpu::Surface<'static>, config: wgpu::SurfaceConfiguration },
    Offscreen { texture: wgpu::Texture, width: u32, height: u32 },
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    target: Target,
    out_format: wgpu::TextureFormat,
    pub res_index: usize,
    scene_w: u32,
    scene_h: u32,
    scene_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    globals: wgpu::Buffer,
    post: wgpu::Buffer,
    cells: wgpu::Buffer,
    doors: wgpu::Buffer,
    quads: wgpu::Buffer,
    map_size: (u32, u32),
    raycast_pipeline: wgpu::ComputePipeline,
    world_pipeline: wgpu::RenderPipeline,
    sprite_pipeline: wgpu::RenderPipeline,
    ui_pipeline: wgpu::RenderPipeline,
    blit_pipeline: wgpu::RenderPipeline,
    raycast_bg: wgpu::BindGroup,
    world_bg: wgpu::BindGroup,
    sprite_bg: wgpu::BindGroup,
    blit_bgl: wgpu::BindGroupLayout,
    blit_bg: wgpu::BindGroup,
    blit_sampler: wgpu::Sampler,
}

fn storage_entry(binding: u32, stage: wgpu::ShaderStages, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: stage,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn uniform_entry(binding: u32, stage: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: stage,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    }
}

fn texture_entry(binding: u32, dim: wgpu::TextureViewDimension) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: dim,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

/// Upload a stack of equally sized images with their full mip chains.
fn upload_array(device: &wgpu::Device, queue: &wgpu::Queue, label: &str, layers: &[Image], cutout: bool) -> wgpu::TextureView {
    let size = layers[0].w as u32;
    let mips = size.ilog2() + 1;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: layers.len() as u32 },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (layer, img) in layers.iter().enumerate() {
        for (level, mip) in img.mip_chain(cutout).iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: layer as u32 },
                    aspect: wgpu::TextureAspect::All,
                },
                &mip.to_rgba8(),
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * mip.w as u32), rows_per_image: Some(mip.h as u32) },
                wgpu::Extent3d { width: mip.w as u32, height: mip.h as u32, depth_or_array_layers: 1 },
            );
        }
    }
    texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    })
}

fn upload_2d(device: &wgpu::Device, queue: &wgpu::Queue, img: &Image) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sky"),
        size: wgpu::Extent3d { width: img.w as u32, height: img.h as u32, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        &img.to_rgba8(),
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * img.w as u32), rows_per_image: Some(img.h as u32) },
        wgpu::Extent3d { width: img.w as u32, height: img.h as u32, depth_or_array_layers: 1 },
    );
    texture.create_view(&Default::default())
}

fn fullscreen_pipeline(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    depth: Option<wgpu::DepthStencilState>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState { module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: depth,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

impl Renderer {
    /// Renderer presenting to a window.
    pub async fn new_windowed(window: Arc<winit::window::Window>, sprites: &Sprites, vsync: bool) -> Renderer {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(window.clone())));
        let surface = instance.create_surface(window).expect("create surface");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("no suitable GPU adapter");
        let (device, queue) = Self::device(&adapter).await;
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("surface unsupported by adapter");
        config.format = format;
        config.present_mode = if vsync { wgpu::PresentMode::AutoVsync } else { wgpu::PresentMode::AutoNoVsync };
        config.desired_maximum_frame_latency = 2;
        surface.configure(&device, &config);
        eprintln!("hellcast: {} ({:?}), surface {:?} {:?}", adapter.get_info().name, adapter.get_info().backend, format, config.present_mode);
        Self::build(device, queue, Target::Window { surface, config }, format, sprites)
    }

    /// Renderer drawing into an offscreen texture (screenshots, tests).
    pub async fn new_headless(width: u32, height: u32, sprites: &Sprites) -> Renderer {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .expect("no suitable GPU adapter");
        let (device, queue) = Self::device(&adapter).await;
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        Self::build(device, queue, Target::Offscreen { texture, width, height }, format, sprites)
    }

    async fn device(adapter: &wgpu::Adapter) -> (wgpu::Device, wgpu::Queue) {
        adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("hellcast"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .expect("request device")
    }

    fn build(device: wgpu::Device, queue: wgpu::Queue, target: Target, out_format: wgpu::TextureFormat, sprites: &Sprites) -> Renderer {
        let t0 = std::time::Instant::now();
        let surfaces = textures::build_surfaces();
        assert_eq!(surfaces.len(), NUM_SURFACES);
        assert!(surfaces.iter().all(|s| s.w == TEX));
        let surf_view = upload_array(&device, &queue, "surfaces", &surfaces, false);
        let sky = textures::build_sky();
        assert_eq!((sky.w, sky.h), (SKY_W, SKY_H));
        let sky_view = upload_2d(&device, &queue, &sky);
        assert!(sprites.layers.iter().all(|s| s.w == SPR));
        let sprite_view = upload_array(&device, &queue, "sprites", &sprites.layers, true);
        eprintln!("hellcast: textures ready in {:?}", t0.elapsed());

        let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("nearest repeat"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let sprite_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sprites"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blit"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let post = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("post"),
            size: std::mem::size_of::<Post>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let cells = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cells"),
            contents: bytemuck::cast_slice(&vec![0u32; MAX_CELLS]),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let doors = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("doors"),
            contents: bytemuck::cast_slice(&vec![-1.0f32; MAX_CELLS]),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let hits = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("column hits"),
            size: MAX_COLUMNS as u64 * HIT_SIZE,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let quads = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("quads"),
            size: (MAX_QUADS * std::mem::size_of::<Quad>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // --- raycast compute
        let cs = wgpu::ShaderStages::COMPUTE;
        let raycast_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("raycast"),
            entries: &[uniform_entry(0, cs), storage_entry(1, cs, true), storage_entry(2, cs, true), storage_entry(3, cs, false)],
        });
        let raycast_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("raycast"),
            layout: &raycast_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: cells.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: doors.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: hits.as_entire_binding() },
            ],
        });
        let raycast_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("raycast.wgsl"),
            source: wgpu::ShaderSource::Wgsl(shader_source(include_str!("shaders/raycast.wgsl")).into()),
        });
        let raycast_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("raycast"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("raycast"),
                bind_group_layouts: &[Some(&raycast_bgl)],
                immediate_size: 0,
            })),
            module: &raycast_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // --- walls / floors / sky
        let fs = wgpu::ShaderStages::FRAGMENT;
        let world_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("world"),
            entries: &[
                uniform_entry(0, fs),
                storage_entry(1, fs, true),
                storage_entry(2, fs, true),
                texture_entry(3, wgpu::TextureViewDimension::D2Array),
                texture_entry(4, wgpu::TextureViewDimension::D2),
                sampler_entry(5),
            ],
        });
        let world_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("world"),
            layout: &world_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: cells.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: hits.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&surf_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&sky_view) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&nearest) },
            ],
        });
        let world_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("world.wgsl"),
            source: wgpu::ShaderSource::Wgsl(shader_source(include_str!("shaders/world.wgsl")).into()),
        });
        let world_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("world"),
            bind_group_layouts: &[Some(&world_bgl)],
            immediate_size: 0,
        });
        let world_pipeline = fullscreen_pipeline(
            &device,
            "world",
            &world_layout,
            &world_module,
            COLOR_FORMAT,
            Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
        );

        // --- sprites and UI share one shader
        let sprite_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprites"),
            entries: &[texture_entry(0, wgpu::TextureViewDimension::D2Array), sampler_entry(1)],
        });
        let sprite_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprites"),
            layout: &sprite_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&sprite_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sprite_sampler) },
            ],
        });
        let sprite_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite.wgsl"),
            source: wgpu::ShaderSource::Wgsl(shader_source(include_str!("shaders/sprite.wgsl")).into()),
        });
        let sprite_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprites"),
            bind_group_layouts: &[Some(&sprite_bgl)],
            immediate_size: 0,
        });
        let quad_attrs = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4];
        let quad_pipeline = |label: &str, entry: &str, blend: Option<wgpu::BlendState>, depth_test: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&sprite_layout),
                vertex: wgpu::VertexState {
                    module: &sprite_module,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Quad>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &quad_attrs,
                    })],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(depth_test),
                    depth_compare: Some(if depth_test { wgpu::CompareFunction::LessEqual } else { wgpu::CompareFunction::Always }),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &sprite_module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: COLOR_FORMAT, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let sprite_pipeline = quad_pipeline("sprites", "fs_sprite", None, true);
        let ui_pipeline = quad_pipeline("ui", "fs_ui", Some(wgpu::BlendState::ALPHA_BLENDING), false);

        // --- upscale to the output
        let blit_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit"),
            entries: &[texture_entry(0, wgpu::TextureViewDimension::D2), sampler_entry(1), uniform_entry(2, fs)],
        });
        let blit_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/blit.wgsl").into()),
        });
        let blit_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit"),
            bind_group_layouts: &[Some(&blit_bgl)],
            immediate_size: 0,
        });
        let blit_pipeline = fullscreen_pipeline(&device, "blit", &blit_layout, &blit_module, out_format, None);

        let placeholder = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let placeholder_view = placeholder.create_view(&Default::default());
        let blit_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &blit_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&placeholder_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&blit_sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: post.as_entire_binding() },
            ],
        });

        let mut r = Renderer {
            device,
            queue,
            target,
            out_format,
            res_index: 1,
            scene_w: 0,
            scene_h: 0,
            scene_view: placeholder_view.clone(),
            depth_view: placeholder_view,
            globals,
            post,
            cells,
            doors,
            quads,
            map_size: (1, 1),
            raycast_pipeline,
            world_pipeline,
            sprite_pipeline,
            ui_pipeline,
            blit_pipeline,
            raycast_bg,
            world_bg,
            sprite_bg,
            blit_bgl,
            blit_bg,
            blit_sampler,
        };
        r.recreate_scene();
        r
    }

    pub fn output_size(&self) -> (u32, u32) {
        match &self.target {
            Target::Window { config, .. } => (config.width, config.height),
            Target::Offscreen { width, height, .. } => (*width, *height),
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if let Target::Window { surface, config } = &mut self.target {
            if width == 0 || height == 0 {
                return;
            }
            config.width = width;
            config.height = height;
            surface.configure(&self.device, config);
        }
        self.recreate_scene();
    }

    pub fn set_resolution(&mut self, index: usize) {
        self.res_index = index.min(RESOLUTIONS.len() - 1);
        self.recreate_scene();
    }

    pub fn cycle_resolution(&mut self) -> u32 {
        self.res_index = (self.res_index + 1) % RESOLUTIONS.len();
        self.recreate_scene();
        self.scene_h
    }

    /// (Re)create the low-resolution scene target to match the output aspect.
    fn recreate_scene(&mut self) {
        let (ow, oh) = self.output_size();
        let h = RESOLUTIONS[self.res_index].min(oh.max(1));
        let w = ((h as f32 * ow as f32 / oh.max(1) as f32).round() as u32).clamp(1, MAX_COLUMNS);
        if (w, h) == (self.scene_w, self.scene_h) {
            return;
        }
        self.scene_w = w;
        self.scene_h = h;
        let make = |label, format, usage| {
            self.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        self.scene_view = make("scene", COLOR_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING);
        self.depth_view = make("depth", DEPTH_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT);
        self.blit_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit"),
            layout: &self.blit_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.scene_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.blit_sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: self.post.as_entire_binding() },
            ],
        });
    }

    /// Layout information for this frame.
    pub fn view(&self, statusbar: bool) -> View {
        let (w, h) = (self.scene_w as f32, self.scene_h as f32);
        let mut u = (h / 400.0).round().max(1.0);
        while u > 1.0 && crate::hud::BAR_W * u > w {
            u -= 1.0;
        }
        let view_h = if statusbar { h - crate::hud::BAR_H * u } else { h };
        let tan_h = TAN_V * w / h;
        View { w, h, view_h, tan_h, tan_v: tan_h * view_h / w, u }
    }

    pub fn upload_map(&mut self, map: &Map) {
        assert!(map.cells.len() <= MAX_CELLS, "map too large");
        self.map_size = (map.w as u32, map.h as u32);
        self.queue.write_buffer(&self.cells, 0, bytemuck::cast_slice(&map.gpu_cells()));
    }

    pub fn render(&mut self, frame: &Frame, map: &Map, view: &View) {
        self.queue.write_buffer(&self.doors, 0, bytemuck::cast_slice(&map.gpu_doors()));

        let dir = V2::from_angle(frame.cam_angle);
        let right = dir.right();
        let mut lights = [GpuLight { pos_r: [0.0; 4], color: [0.0; 4] }; MAX_LIGHTS];
        for (dst, l) in lights.iter_mut().zip(&frame.lights) {
            *dst = GpuLight { pos_r: [l.pos[0], l.pos[1], l.pos[2], l.radius], color: [l.color[0], l.color[1], l.color[2], 0.0] };
        }
        let globals = Globals {
            cam: [frame.cam_pos.x, frame.cam_pos.y, dir.x, dir.y],
            right_tan: [right.x, right.y, view.tan_h, view.tan_v],
            view: [view.w, view.view_h, frame.eye_z, frame.time],
            misc: [self.map_size.0 as f32, self.map_size.1 as f32, frame.lights.len().min(MAX_LIGHTS) as f32, 0.0],
            lights,
        };
        self.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));

        let n_sprites = frame.sprites.len().min(MAX_QUADS);
        let n_ui = frame.ui.len().min(MAX_QUADS - n_sprites);
        let mut quads = Vec::with_capacity(n_sprites + n_ui);
        quads.extend_from_slice(&frame.sprites[..n_sprites]);
        quads.extend_from_slice(&frame.ui[..n_ui]);
        if !quads.is_empty() {
            self.queue.write_buffer(&self.quads, 0, bytemuck::cast_slice(&quads));
        }

        let (ow, oh) = self.output_size();
        let post = Post {
            tint: frame.tint,
            size: [self.scene_w as f32, self.scene_h as f32, ow as f32, oh as f32],
            flags: [if self.out_format.is_srgb() { 1.0 } else { 0.0 }, 0.35, 0.0, 0.0],
        };
        self.queue.write_buffer(&self.post, 0, bytemuck::bytes_of(&post));

        let surface_tex = match &self.target {
            Target::Window { surface, config } => match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => Some(t),
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    surface.configure(&self.device, config);
                    return;
                }
                _ => return,
            },
            Target::Offscreen { .. } => None,
        };
        let out_view = match (&surface_tex, &self.target) {
            (Some(t), _) => t.texture.create_view(&Default::default()),
            (None, Target::Offscreen { texture, .. }) => texture.create_view(&Default::default()),
            _ => unreachable!(),
        };

        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("raycast"), timestamp_writes: None });
            pass.set_pipeline(&self.raycast_pipeline);
            pass.set_bind_group(0, &self.raycast_bg, &[]);
            pass.dispatch_workgroups((view.w as u32).div_ceil(64), 1, 1);
        }
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.scene_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_viewport(0.0, 0.0, view.w, view.view_h, 0.0, 1.0);
            pass.set_pipeline(&self.world_pipeline);
            pass.set_bind_group(0, &self.world_bg, &[]);
            pass.draw(0..3, 0..1);
            pass.set_vertex_buffer(0, self.quads.slice(..));
            pass.set_bind_group(0, &self.sprite_bg, &[]);
            if n_sprites > 0 {
                pass.set_pipeline(&self.sprite_pipeline);
                pass.draw(0..6, 0..n_sprites as u32);
            }
            if n_ui > 0 {
                pass.set_viewport(0.0, 0.0, view.w, view.h, 0.0, 1.0);
                pass.set_pipeline(&self.ui_pipeline);
                pass.draw(0..6, n_sprites as u32..(n_sprites + n_ui) as u32);
            }
        }
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &out_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.blit_pipeline);
            pass.set_bind_group(0, &self.blit_bg, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([enc.finish()]);
        if let Some(t) = surface_tex {
            self.queue.present(t);
        }
    }

    /// Read back the offscreen target as tightly packed RGBA8.
    pub fn read_pixels(&self) -> Option<(u32, u32, Vec<u8>)> {
        let Target::Offscreen { texture, width, height } = &self.target else {
            return None;
        };
        let (w, h) = (*width, *height);
        let stride = (4 * w).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (stride * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(stride), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);
        buf.map_async(wgpu::MapMode::Read, .., |r| r.expect("map readback"));
        self.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        let data = buf.get_mapped_range(..).expect("mapped range");
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for row in data.chunks_exact(stride as usize) {
            out.extend_from_slice(&row[..(w * 4) as usize]);
        }
        Some((w, h, out))
    }
}
