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
}
