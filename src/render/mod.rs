//! wgpu renderer: voxel chunks, dynamic entity geometry, weapon viewmodel and egui.

pub mod atlas;
pub mod mesher;
pub mod models;
pub mod vertex;

use std::collections::HashMap;
use std::sync::Arc;

use glam::{IVec3, Mat4, Vec3, Vec4};
use wgpu::util::DeviceExt;
use winit::window::Window;

pub use vertex::{MeshBuilder, Vertex};

use crate::world::{World, CHUNK_SIZE};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Right-handed perspective projection with WebGPU clip conventions (depth 0..1).
pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    glam::camera::rh::proj::directx::perspective(fov_y, aspect, near, far)
}

/// Right-handed view matrix looking along `dir`.
pub fn look_to(eye: Vec3, dir: Vec3, up: Vec3) -> Mat4 {
    glam::camera::rh::view::look_to_mat4(eye, dir, up)
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    cam_pos: [f32; 4],
    sun_dir: [f32; 4],
    fog_color: [f32; 4],
    fog: [f32; 4],
}

struct ChunkGpu {
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    count: u32,
    min: Vec3,
    max: Vec3,
}

/// Growable GPU buffers for per-frame geometry.
struct DynamicMesh {
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    vcap: u64,
    icap: u64,
    count: u32,
}

impl DynamicMesh {
    fn new(device: &wgpu::Device, label: &str) -> Self {
        let vcap = 64 * 1024;
        let icap = 64 * 1024;
        Self {
            vbuf: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: vcap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            ibuf: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: icap,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            vcap,
            icap,
            count: 0,
        }
    }

    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, mb: &MeshBuilder) {
        self.count = mb.indices.len() as u32;
        if mb.indices.is_empty() {
            return;
        }
        let vbytes: &[u8] = bytemuck::cast_slice(&mb.vertices);
        let ibytes: &[u8] = bytemuck::cast_slice(&mb.indices);
        if vbytes.len() as u64 > self.vcap {
            self.vcap = (vbytes.len() as u64).next_power_of_two();
            self.vbuf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("dynamic vertices"),
                size: self.vcap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if ibytes.len() as u64 > self.icap {
            self.icap = (ibytes.len() as u64).next_power_of_two();
            self.ibuf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("dynamic indices"),
                size: self.icap,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        queue.write_buffer(&self.vbuf, 0, vbytes);
        queue.write_buffer(&self.ibuf, 0, ibytes);
    }

    fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if self.count == 0 {
            return;
        }
        pass.set_vertex_buffer(0, self.vbuf.slice(..));
        pass.set_index_buffer(self.ibuf.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.count, 0, 0..1);
    }
}

/// Everything the renderer needs to draw a 3D frame.
pub struct FrameScene<'a> {
    pub draw_world: bool,
    pub view_proj: Mat4,
    pub cam_pos: Vec3,
    pub sun_dir: Vec3,
    /// sRGB sky colour (also used as fog colour).
    pub sky_color: [f32; 3],
    pub fog_start: f32,
    pub fog_end: f32,
    pub ambient_boost: f32,
    pub time: f32,
    /// World-space dynamic geometry (entities, particles, tracers).
    pub dynamic: &'a MeshBuilder,
    /// View-space weapon geometry and its projection matrix.
    pub viewmodel: Option<(&'a MeshBuilder, Mat4)>,
}

pub struct EguiFrame {
    pub primitives: Vec<egui::ClippedPrimitive>,
    pub textures_delta: egui::TexturesDelta,
    pub pixels_per_point: f32,
}

pub struct Renderer {
    pub window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth_view: wgpu::TextureView,
    pipeline: wgpu::RenderPipeline,
    globals_world: wgpu::Buffer,
    globals_world_bg: wgpu::BindGroup,
    globals_vm: wgpu::Buffer,
    globals_vm_bg: wgpu::BindGroup,
    atlas_bg: wgpu::BindGroup,
    chunks: HashMap<IVec3, ChunkGpu>,
    dyn_world: DynamicMesh,
    dyn_vm: DynamicMesh,
    pub egui_renderer: egui_wgpu::Renderer,
    srgb_format: wgpu::TextureFormat,
    can_screenshot: bool,
    /// Save the next presented frame to this PNG path.
    pub screenshot_request: Option<std::path::PathBuf>,
    pub adapter_info: String,
    pub stats_chunks_drawn: usize,
    pub stats_triangles: usize,
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn create_depth(device: &wgpu::Device, w: u32, h: u32) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size: wgpu::Extent3d {
            width: w.max(1),
            height: h.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}

impl Renderer {
    pub async fn new(
        window: Arc<Window>,
        display: winit::event_loop::OwnedDisplayHandle,
    ) -> Result<Self, String> {
        let size = window.inner_size();
        // Prefer the native APIs (Vulkan/Metal/DX12); fall back to GL if unavailable.
        let env_backends = std::env::var("WGPU_BACKEND").is_ok();
        let mut attempt = 0;
        let (instance, surface, adapter) = loop {
            let mut desc = wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(display.clone()));
            if attempt == 0 && !env_backends {
                desc.backends = wgpu::Backends::PRIMARY;
            }
            let instance = wgpu::Instance::new(desc);
            let surface = instance
                .create_surface(window.clone())
                .map_err(|e| format!("create_surface failed: {e}"))?;
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                    ..Default::default()
                })
                .await;
            match adapter {
                Ok(a) => break (instance, surface, a),
                Err(e) if attempt == 0 && !env_backends => {
                    log::warn!("no primary-backend adapter ({e}); retrying with all backends");
                    attempt += 1;
                }
                Err(e) => return Err(format!("no suitable GPU adapter: {e}")),
            }
        };
        let _ = &instance;
        let info = adapter.get_info();
        let adapter_info = format!("{} ({:?})", info.name, info.backend);
        log::info!("Using adapter: {adapter_info}");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("voxel-raid device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|e| format!("request_device failed: {e}"))?;

        // The swapchain uses a non-sRGB format (egui expects that) and the 3D
        // passes render through an sRGB view of it for correct gamma.
        let caps = surface.get_capabilities(&adapter);
        // Prefer plain 8-bit formats that have an sRGB twin (10-bit / float
        // swapchains have no sRGB view and would break gamma and screenshots).
        let surface_format = caps
            .formats
            .iter()
            .copied()
            .find(|f| matches!(f, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm))
            .or_else(|| {
                caps.formats
                    .iter()
                    .copied()
                    .find(|f| matches!(f, wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Rgba8UnormSrgb))
            })
            .unwrap_or(caps.formats[0]);
        log::debug!("Surface formats: {:?}, using {:?}", caps.formats, surface_format);
        let srgb_format = surface_format.add_srgb_suffix();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("surface not supported by adapter")?;
        config.format = surface_format;
        config.present_mode = if std::env::var("VOXEL_RAID_NO_VSYNC").is_ok() {
            wgpu::PresentMode::AutoNoVsync
        } else {
            wgpu::PresentMode::AutoVsync
        };
        if srgb_format != surface_format {
            config.view_formats = vec![srgb_format];
        }
        let can_screenshot = caps.usages.contains(wgpu::TextureUsages::COPY_SRC)
            && matches!(
                surface_format.remove_srgb_suffix(),
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
            );
        if can_screenshot {
            config.usage |= wgpu::TextureUsages::COPY_SRC;
        }
        surface.configure(&device, &config);

        let depth_view = create_depth(&device, config.width, config.height);

        // --- Atlas texture ---
        let atlas_data = atlas::generate();
        let atlas_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("atlas"),
            size: wgpu::Extent3d {
                width: atlas::ATLAS_PX,
                height: atlas::ATLAS_PX,
                depth_or_array_layers: 1,
            },
            mip_level_count: atlas::MIP_LEVELS,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (level, data) in atlas_data.levels.iter().enumerate() {
            let dim = atlas::ATLAS_PX >> level;
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &atlas_tex,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(dim * 4),
                    rows_per_image: Some(dim),
                },
                wgpu::Extent3d {
                    width: dim,
                    height: dim,
                    depth_or_array_layers: 1,
                },
            );
        }
        let atlas_view = atlas_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("atlas sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: (atlas::MIP_LEVELS - 1) as f32,
            ..Default::default()
        });

        // --- Bind group layouts ---
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("atlas layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let make_globals = |label: &str| {
            let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::bytes_of(&Globals::zeroed_default()),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &globals_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                }],
            });
            (buf, bg)
        };
        let (globals_world, globals_world_bg) = make_globals("globals world");
        let (globals_vm, globals_vm_bg) = make_globals("globals viewmodel");

        let atlas_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atlas bg"),
            layout: &atlas_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("voxel shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("voxel pipeline layout"),
            bind_group_layouts: &[Some(&globals_layout), Some(&atlas_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("voxel pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(Vertex::layout())],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: srgb_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let egui_renderer = egui_wgpu::Renderer::new(
            &device,
            config.format,
            egui_wgpu::RendererOptions::default(),
        );

        let dyn_world = DynamicMesh::new(&device, "dynamic world");
        let dyn_vm = DynamicMesh::new(&device, "dynamic viewmodel");

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            depth_view,
            pipeline,
            globals_world,
            globals_world_bg,
            globals_vm,
            globals_vm_bg,
            atlas_bg,
            chunks: HashMap::new(),
            dyn_world,
            dyn_vm,
            egui_renderer,
            srgb_format,
            can_screenshot,
            screenshot_request: None,
            adapter_info,
            stats_chunks_drawn: 0,
            stats_triangles: 0,
        })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn aspect(&self) -> f32 {
        self.config.width as f32 / self.config.height.max(1) as f32
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
        self.depth_view = create_depth(&self.device, w, h);
    }

    fn upload_chunk(&mut self, c: IVec3, mesh: mesher::ChunkMesh) {
        if mesh.indices.is_empty() {
            self.chunks.remove(&c);
            return;
        }
        let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("chunk vb"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ibuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("chunk ib"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let min = (c * CHUNK_SIZE).as_vec3();
        self.chunks.insert(
            c,
            ChunkGpu {
                vbuf,
                ibuf,
                count: mesh.indices.len() as u32,
                min,
                max: min + Vec3::splat(CHUNK_SIZE as f32),
            },
        );
    }

    /// Drop all chunk meshes and mesh the whole world (used when switching maps).
    pub fn load_world(&mut self, world: &mut World) {
        self.chunks.clear();
        let _ = world.take_dirty();
        let coords: Vec<IVec3> = world.chunk_coords().collect();
        let t0 = std::time::Instant::now();
        let meshes = mesher::mesh_chunks_parallel(world, &coords);
        for (c, m) in meshes {
            self.upload_chunk(c, m);
        }
        log::info!(
            "Meshed {} chunks in {:.1} ms",
            coords.len(),
            t0.elapsed().as_secs_f32() * 1000.0
        );
    }

    pub fn clear_world(&mut self) {
        self.chunks.clear();
    }

    /// Remesh chunks that changed since last frame.
    pub fn sync_world(&mut self, world: &mut World) {
        let dirty = world.take_dirty();
        if dirty.is_empty() {
            return;
        }
        let meshes = if dirty.len() > 4 {
            mesher::mesh_chunks_parallel(world, &dirty)
        } else {
            dirty.iter().map(|c| (*c, mesher::mesh_chunk(world, *c))).collect()
        };
        for (c, m) in meshes {
            self.upload_chunk(c, m);
        }
    }

    pub fn render(&mut self, scene: &FrameScene, egui_frame: EguiFrame) {
        let EguiFrame {
            primitives,
            mut textures_delta,
            pixels_per_point,
        } = egui_frame;

        // egui textures must always be applied, even if we skip this frame.
        for (id, deltas) in textures_delta.set.drain() {
            for delta in deltas {
                self.egui_renderer
                    .update_texture(&self.device, &self.queue, id, &delta);
            }
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                for id in textures_delta.free.drain() {
                    self.egui_renderer.free_texture(&id);
                }
                return;
            }
            _ => {
                for id in textures_delta.free.drain() {
                    self.egui_renderer.free_texture(&id);
                }
                return;
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let srgb_view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.srgb_format),
            ..Default::default()
        });

        let sky_lin = [
            srgb_to_linear(scene.sky_color[0]),
            srgb_to_linear(scene.sky_color[1]),
            srgb_to_linear(scene.sky_color[2]),
        ];
        let globals = Globals {
            view_proj: scene.view_proj.to_cols_array_2d(),
            cam_pos: scene.cam_pos.extend(1.0).to_array(),
            sun_dir: scene.sun_dir.normalize().extend(0.0).to_array(),
            fog_color: [sky_lin[0], sky_lin[1], sky_lin[2], 1.0],
            fog: [scene.fog_start, scene.fog_end, scene.time, scene.ambient_boost],
        };
        self.queue
            .write_buffer(&self.globals_world, 0, bytemuck::bytes_of(&globals));
        self.dyn_world.upload(&self.device, &self.queue, scene.dynamic);
        if let Some((vm, proj)) = &scene.viewmodel {
            let vm_globals = Globals {
                view_proj: proj.to_cols_array_2d(),
                cam_pos: [0.0, 0.0, 0.0, 1.0],
                sun_dir: globals.sun_dir,
                fog_color: globals.fog_color,
                fog: [1000.0, 2000.0, scene.time, scene.ambient_boost + 0.1],
            };
            self.queue
                .write_buffer(&self.globals_vm, 0, bytemuck::bytes_of(&vm_globals));
            self.dyn_vm.upload(&self.device, &self.queue, vm);
        } else {
            self.dyn_vm.count = 0;
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });

        // --- World pass ---
        {
            let clear = if scene.draw_world {
                wgpu::Color {
                    r: sky_lin[0] as f64,
                    g: sky_lin[1] as f64,
                    b: sky_lin[2] as f64,
                    a: 1.0,
                }
            } else {
                wgpu::Color {
                    r: 0.008,
                    g: 0.01,
                    b: 0.012,
                    a: 1.0,
                }
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("world pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &srgb_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
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
            self.stats_chunks_drawn = 0;
            self.stats_triangles = 0;
            if scene.draw_world {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.globals_world_bg, &[]);
                pass.set_bind_group(1, &self.atlas_bg, &[]);
                let planes = frustum_planes(scene.view_proj);
                for chunk in self.chunks.values() {
                    if !aabb_in_frustum(&planes, chunk.min, chunk.max) {
                        continue;
                    }
                    pass.set_vertex_buffer(0, chunk.vbuf.slice(..));
                    pass.set_index_buffer(chunk.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..chunk.count, 0, 0..1);
                    self.stats_chunks_drawn += 1;
                    self.stats_triangles += chunk.count as usize / 3;
                }
                self.dyn_world.draw(&mut pass);
            }
        }

        // --- Viewmodel pass (own depth so the gun never clips into walls) ---
        if scene.draw_world && self.dyn_vm.count > 0 {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viewmodel pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &srgb_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
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
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.globals_vm_bg, &[]);
            pass.set_bind_group(1, &self.atlas_bg, &[]);
            self.dyn_vm.draw(&mut pass);
        }

        // --- egui pass ---
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point,
        };
        let extra_cmds = self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &primitives,
            &screen,
        );
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let mut pass = pass.forget_lifetime();
            self.egui_renderer.render(&mut pass, &primitives, &screen);
        }

        let capture = match self.screenshot_request.take() {
            Some(path) if self.can_screenshot => Some((path, self.encode_capture(&mut encoder, &frame.texture))),
            Some(_) => {
                log::warn!("screenshots are not supported on this surface");
                None
            }
            None => None,
        };

        self.queue
            .submit(extra_cmds.into_iter().chain(std::iter::once(encoder.finish())));
        if let Some((path, (buffer, padded_row))) = capture {
            self.save_capture(&path, &buffer, padded_row);
        }
        self.queue.present(frame);

        for id in textures_delta.free.drain() {
            self.egui_renderer.free_texture(&id);
        }
    }
}

impl Renderer {
    fn encode_capture(&self, encoder: &mut wgpu::CommandEncoder, texture: &wgpu::Texture) -> (wgpu::Buffer, u32) {
        let (w, h) = (self.config.width, self.config.height);
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_row = (w * 4).div_ceil(align) * align;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screenshot"),
            size: (padded_row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        (buffer, padded_row)
    }

    fn save_capture(&self, path: &std::path::Path, buffer: &wgpu::Buffer, padded_row: u32) {
        let (w, h) = (self.config.width, self.config.height);
        buffer.map_async(wgpu::MapMode::Read, .., |_| {});
        if let Err(e) = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(5)),
        }) {
            log::warn!("screenshot poll failed: {e:?}");
            return;
        }
        let Ok(view) = buffer.get_mapped_range(..) else {
            log::warn!("screenshot buffer could not be mapped");
            return;
        };
        let bgra = matches!(
            self.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h {
            let start = (row * padded_row) as usize;
            let line = &view[start..start + (w * 4) as usize];
            for px in line.chunks_exact(4) {
                if bgra {
                    rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
                } else {
                    rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
                }
            }
        }
        drop(view);
        buffer.unmap();
        match std::fs::File::create(path) {
            Ok(file) => {
                let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
                enc.set_color(png::ColorType::Rgba);
                enc.set_depth(png::BitDepth::Eight);
                let result = enc.write_header().and_then(|mut wr| wr.write_image_data(&rgba));
                match result {
                    Ok(()) => log::info!("Saved screenshot to {}", path.display()),
                    Err(e) => log::warn!("failed to write screenshot: {e}"),
                }
            }
            Err(e) => log::warn!("failed to create {}: {e}", path.display()),
        }
    }
}

impl Globals {
    fn zeroed_default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            cam_pos: [0.0; 4],
            sun_dir: [0.0, 1.0, 0.0, 0.0],
            fog_color: [0.5, 0.6, 0.7, 1.0],
            fog: [100.0, 200.0, 0.0, 0.0],
        }
    }
}

/// Frustum planes (a, b, c, d) from a view-projection matrix (wgpu depth 0..1).
fn frustum_planes(m: Mat4) -> [Vec4; 6] {
    let r0 = m.row(0);
    let r1 = m.row(1);
    let r2 = m.row(2);
    let r3 = m.row(3);
    [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2]
}

fn aabb_in_frustum(planes: &[Vec4; 6], min: Vec3, max: Vec3) -> bool {
    for p in planes {
        let v = Vec3::new(
            if p.x >= 0.0 { max.x } else { min.x },
            if p.y >= 0.0 { max.y } else { min.y },
            if p.z >= 0.0 { max.z } else { min.z },
        );
        if p.x * v.x + p.y * v.y + p.z * v.z + p.w < 0.0 {
            return false;
        }
    }
    true
}
