mod app;
mod camera;
mod mesh;
mod renderer;
mod scene;

use app::App;
use winit::event_loop::{ControlFlow, EventLoop};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", app::HELP);

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut App::default())?;

    Ok(())
}
