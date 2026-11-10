use crate::{
    camera::Camera,
    mesh::{Geometry, Shape, Vertex},
    scene::{PALETTE, Scene},
};
use glam::{Mat4, Vec3};
use std::{collections::HashSet, sync::Arc};
use wgpu::util::DeviceExt;
use winit::window::Window;

const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    mvp: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    normal: [[f32; 4]; 4],
    color: [f32; 4],
    options: [f32; 4],
}

struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
}

struct GpuObject {
    buffer: wgpu::Buffer,
    group: wgpu::BindGroup,
}

pub struct Renderer {
    instance: wgpu::Instance,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: (u32, u32),
    depth: wgpu::TextureView,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    meshes: Vec<GpuMesh>,
    objects: Vec<GpuObject>,
}

impl Renderer {
    pub fn new(window: Arc<Window>, geometries: &[Geometry]) -> Result<Self, String> {
        let instance = wgpu::Instance::default();

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| e.to_string())?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .map_err(|e| e.to_string())?;

        let size = window.inner_size();

        let config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("No compatible surface format")?;

        if size.width > 0 && size.height > 0 {
            surface.configure(&device, &config);
        }

        let depth = Self::depth(&device, config.width, config.height);

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Object uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Uniform>() as u64),
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Modeler layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Modeler shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("triangle.wgsl").into()),
        });

        let attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Lit objects"),
            layout: Some(&pipeline_layout),

            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },

            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),

            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },

            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),

            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        let meshes = geometries
            .iter()
            .map(|geometry| GpuMesh {
                vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Mesh vertices"),
                    contents: bytemuck::cast_slice(&geometry.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                }),

                indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Mesh indices"),
                    contents: bytemuck::cast_slice(&geometry.indices),
                    usage: wgpu::BufferUsages::INDEX,
                }),

                count: geometry.indices.len() as u32,
            })
            .collect();

        Ok(Self {
            instance,
            window,
            surface,
            device,
            queue,
            config,
            size: (size.width, size.height),
            depth,
            pipeline,
            layout,
            meshes,
            objects: vec![],
        })
    }

    fn depth(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("Depth"),

                size: wgpu::Extent3d {
                    width: width.max(1),
                    height: height.max(1),
                    depth_or_array_layers: 1,
                },

                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.size = (width, height);

        if width == 0 || height == 0 {
            return;
        }

        self.config.width = width;
        self.config.height = height;

        self.surface.configure(&self.device, &self.config);
        self.depth = Self::depth(&self.device, width, height);
    }

    fn ensure_objects(&mut self, count: usize) {
        self.objects.truncate(count);

        while self.objects.len() < count {
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Object transform"),
                size: std::mem::size_of::<Uniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Object"),
                layout: &self.layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });

            self.objects.push(GpuObject { buffer, group });
        }
    }

    // Returns true when the app should retry drawing shortly.
    pub fn render(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        selected: &HashSet<usize>,
    ) -> Result<bool, String> {
        if self.size.0 == 0 || self.size.1 == 0 {
            return Ok(false);
        }

        let (frame, suboptimal) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),

            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),

            wgpu::CurrentSurfaceTexture::Timeout => return Ok(true),

            wgpu::CurrentSurfaceTexture::Occluded => return Ok(false),

            wgpu::CurrentSurfaceTexture::Outdated => {
                self.resize(self.size.0, self.size.1);
                return Ok(true);
            }

            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self
                    .instance
                    .create_surface(self.window.clone())
                    .map_err(|e| e.to_string())?;

                self.resize(self.size.0, self.size.1);
                return Ok(true);
            }

            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("Surface validation error; see GPU diagnostics".into());
            }
        };

        // One extra object is used for the floor.
        self.ensure_objects(scene.objects.len() + 1);

        let view_projection = camera.matrix(self.size.0 as f32 / self.size.1 as f32);

        for i in 0..self.objects.len() {
            let (model, color, options) = if let Some(object) = scene.objects.get(i) {
                let color = PALETTE[object.color];

                (
                    object.model(),
                    [color[0], color[1], color[2], 1.0],
                    [if selected.contains(&i) { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
                )
            } else {
                // The floor is a flattened cube.
                (
                    Mat4::from_scale_rotation_translation(
                        Vec3::new(30.0, 0.05, 30.0),
                        glam::Quat::IDENTITY,
                        Vec3::new(0.0, -0.8, 0.0),
                    ),
                    [0.16, 0.19, 0.23, 1.0],
                    [0.0, 1.0, 0.0, 0.0],
                )
            };

            let uniform = Uniform {
                mvp: (view_projection * model).to_cols_array_2d(),
                model: model.to_cols_array_2d(),
                normal: model.inverse().transpose().to_cols_array_2d(),
                color,
                options,
            };

            self.queue
                .write_buffer(&self.objects[i].buffer, 0, bytemuck::bytes_of(&uniform));
        }

        let view = frame.texture.create_view(&Default::default());

        let mut encoder = self.device.create_command_encoder(&Default::default());

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Scene"),

                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,

                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.025,
                            g: 0.035,
                            b: 0.055,
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

                ..Default::default()
            });

            pass.set_pipeline(&self.pipeline);

            for (i, gpu_object) in self.objects.iter().enumerate() {
                let shape = scene
                    .objects
                    .get(i)
                    .map(|object| object.shape)
                    .unwrap_or(Shape::Cube);

                let mesh = &self.meshes[shape.index()];

                pass.set_bind_group(0, &gpu_object.group, &[]);
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);

                pass.draw_indexed(0..mesh.count, 0, 0..1);
            }
        }

        self.queue.submit([encoder.finish()]);

        self.window.pre_present_notify();
        self.queue.present(frame);

        if suboptimal {
            self.resize(self.size.0, self.size.1);
        }

        Ok(false)
    }
}
