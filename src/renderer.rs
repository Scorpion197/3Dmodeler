use std::sync::Arc;
use wgpu::Device;
use wgpu::Queue;
use wgpu::Surface;
use wgpu::SurfaceConfiguration;
use winit::window::Window;

pub struct Renderer {
    surface: Surface<'static>,
    device: Device,
    queue: Queue,
    config: SurfaceConfiguration,
}

impl Renderer {
    pub fn new(window: Arc<Window>) -> Self {
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();

        // request an adapter and device
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .unwrap();

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        let size = window.inner_size();
        let config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();

        if size.width > 0 && size.height > 0 {
            surface.configure(&device, &config);
        }

        return Self {
            surface,
            device,
            queue,
            config,
        };
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;

        if width == 0 || height == 0 {
            return;
        }

        // reconfigure the surface with the new size
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render(&mut self) {
        if self.config.width == 0 || self.config.height == 0 {
            return;
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            _ => {
                println!("Failed to acquire next swap chain texture");
                return;
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        let color_attachment = wgpu::RenderPassColorAttachment {
            view: &view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                store: wgpu::StoreOp::Store,
            },
        };

        let pass_descriptor = wgpu::RenderPassDescriptor {
            color_attachments: &[Some(color_attachment)],
            ..Default::default()
        };

        {
            let _render_pass = encoder.begin_render_pass(&pass_descriptor);
        }

        let command_buffer = encoder.finish();

        self.queue.submit([command_buffer]);
        self.queue.present(frame);
    }
}
