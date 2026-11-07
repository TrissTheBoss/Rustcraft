#![cfg(windows)]

use std::num::NonZeroIsize;

use bytemuck::{Pod, Zeroable};
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
};
use wgpu::util::DeviceExt;

use crate::render::{Camera, Mesh};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuVertex {
    position: [f32; 3],
    uv: [f32; 2],
    texture_layer: u32,
    color: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    view_projection: [[f32; 4]; 4],
}

pub struct GpuRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    depth: wgpu::TextureView,
    msaa: wgpu::TextureView,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    texture_bind_group: wgpu::BindGroup,
    texture_array: wgpu::Texture,
    materials_uploaded: bool,
    meshes: Vec<(wgpu::Buffer, wgpu::Buffer, u32)>,
}

impl GpuRenderer {
    pub fn new(hwnd: isize, width: u32, height: u32) -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let window = Win32WindowHandle::new(NonZeroIsize::new(hwnd).ok_or("invalid HWND")?);
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
                    raw_window_handle: RawWindowHandle::Win32(window),
                })
                .map_err(|error| error.to_string())?
        };
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or("no compatible GPU adapter")?;
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("voxel-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        ))
        .map_err(|error| error.to_string())?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats[0];
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        let depth = depth_view(&device, width, height);
        let msaa = multisample_view(&device, width, height, format);
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera-uniform"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
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
        let texture_array = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("block-texture-array"),
            size: wgpu::Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 64,
            },
            mip_level_count: 5,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = texture_array.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            array_layer_count: Some(64),
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture-bind-group"),
            layout: &texture_layout,
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
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera-bind-group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("voxel-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("voxel-pipeline-layout"),
            bind_group_layouts: &[&layout, &texture_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("voxel-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GpuVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Uint32, 3 => Float32x3],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState { count: 4, mask: !0, alpha_to_coverage_enabled: false },
            multiview: None,
        });
        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            depth,
            msaa,
            uniform,
            bind_group,
            texture_bind_group,
            texture_array,
            materials_uploaded: false,
            meshes: Vec::new(),
        })
    }

    pub fn upload_meshes(&mut self, meshes: &[&Mesh], materials: &crate::render::MaterialRegistry) {
        if !self.materials_uploaded {
            for id in 0..materials.texture_count().min(64) {
                if let Some(image) = materials.texture(id as u16) {
                    let mut rgba = Vec::with_capacity(16 * 16 * 4);
                    for y in 0..16 {
                        for x in 0..16 {
                            let source_x = x * image.width as usize / 16;
                            let source_y = y * image.height as usize / 16;
                            let pixel = image.pixels[source_y * image.width as usize + source_x];
                            rgba.extend([pixel[0], pixel[1], pixel[2], 255]);
                        }
                    }
                    let mut mip = rgba;
                    let mut mip_size = 16u32;
                    for level in 0..5 {
                        self.queue.write_texture(
                            wgpu::ImageCopyTexture {
                                texture: &self.texture_array,
                                mip_level: level,
                                origin: wgpu::Origin3d {
                                    x: 0,
                                    y: 0,
                                    z: id as u32,
                                },
                                aspect: wgpu::TextureAspect::All,
                            },
                            &mip,
                            wgpu::ImageDataLayout {
                                offset: 0,
                                bytes_per_row: Some(mip_size * 4),
                                rows_per_image: Some(mip_size),
                            },
                            wgpu::Extent3d {
                                width: mip_size,
                                height: mip_size,
                                depth_or_array_layers: 1,
                            },
                        );
                        if level < 4 {
                            let next_size = (mip_size / 2).max(1);
                            let mut next = vec![0u8; (next_size * next_size * 4) as usize];
                            for y in 0..next_size {
                                for x in 0..next_size {
                                    for c in 0..4 {
                                        let mut sum = 0u16;
                                        for dy in 0..2 {
                                            for dx in 0..2 {
                                                let i = (((y * 2 + dy) * mip_size + x * 2 + dx) * 4
                                                    + c)
                                                    as usize;
                                                sum += mip[i] as u16;
                                            }
                                        }
                                        next[((y * next_size + x) * 4 + c) as usize] =
                                            (sum / 4) as u8;
                                    }
                                }
                            }
                            mip = next;
                            mip_size = next_size;
                        }
                    }
                }
            }
            self.materials_uploaded = true;
        }
        self.meshes = meshes
            .iter()
            .map(|mesh| {
                let vertices: Vec<_> = mesh
                    .vertices
                    .iter()
                    .map(|v| {
                        let material = materials.get(v.block);
                        let texture_layer = v
                            .texture
                            .filter(|layer| *layer < 64)
                            .map_or(u32::MAX, u32::from);
                        GpuVertex {
                            position: [v.position.x, v.position.y, v.position.z],
                            uv: v.uv,
                            texture_layer,
                            color: material.color.map(|component| component as f32 / 255.0),
                        }
                    })
                    .collect();
                let vertex = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("persistent-mesh-vertices"),
                        contents: bytemuck::cast_slice(&vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                let index = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("persistent-mesh-indices"),
                        contents: bytemuck::cast_slice(&mesh.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                (vertex, index, mesh.indices.len() as u32)
            })
            .collect();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.depth = depth_view(&self.device, self.config.width, self.config.height);
        self.msaa = multisample_view(
            &self.device,
            self.config.width,
            self.config.height,
            self.config.format,
        );
    }

    pub fn render(&mut self, camera: &Camera) -> Result<(), String> {
        let matrix = camera
            .view_projection(self.config.width as f32 / self.config.height as f32)
            .map_err(str::to_string)?;
        self.queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&Uniforms {
                // Mat4 stores rows for the CPU renderer; WGSL matrices are
                // column-major, so transpose at the API boundary.
                view_projection: transpose(matrix.0),
            }),
        );
        let frame = self
            .surface
            .get_current_texture()
            .map_err(|error| error.to_string())?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("voxel-command"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("voxel-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.msaa,
                    resolve_target: Some(&view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.36,
                            g: 0.56,
                            b: 0.75,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_bind_group(1, &self.texture_bind_group, &[]);
            for (vertex, index, index_count) in &self.meshes {
                pass.set_vertex_buffer(0, vertex.slice(..));
                pass.set_index_buffer(index.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..*index_count, 0, 0..1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}

fn depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

fn multisample_view(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("4x-msaa-color"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

fn transpose(matrix: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    std::array::from_fn(|row| std::array::from_fn(|column| matrix[column][row]))
}
