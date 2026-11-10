use crate::{
    camera::Camera,
    mesh::{Geometry, Shape},
    renderer::Renderer,
    scene::{PALETTE, Scene},
};
use glam::Vec3;
use std::{
    collections::HashSet,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
    window::{Window, WindowId},
};

pub const HELP: &str = r#"
RUST MODELER — controls

Left click: select.
Shift+click: toggle/add selection, including whole groups.

Right drag: orbit camera.
Middle drag: pan camera.
Mouse wheel / trackpad scroll: zoom.

1 / 2 / 3: add cube / sphere / pyramid at camera target.

T / R / S: translate / rotate / scale mode.
X / Y / Z: choose axis.
Right / Up: positive step.
Left / Down: negative step.
Hold Shift: smaller step.

Scaling is uniform. The axis affects translation and rotation.

C: cycle color.
D: duplicate.
Delete / Backspace: delete selection.
G: group selection.
U: ungroup selection.
A: select all.
Escape: clear selection.

F: focus selected objects.
Home: reset camera.
H: print this help.

Ctrl/Cmd+S: save scene.json.
Ctrl/Cmd+O: load scene.json.
Ctrl/Cmd+Z: undo.
Ctrl/Cmd+Shift+Z or Ctrl/Cmd+Y: redo.

The title bar shows the current mode, axis, selection, and status.
Save/load uses scene.json in the directory where you run the app.
"#;

#[derive(Debug, Clone, Copy)]
enum Mode {
    Translate,
    Rotate,
    Scale,
}

#[derive(Clone)]
struct Snapshot {
    scene: Scene,
    selected: HashSet<usize>,
}

pub struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    scene: Scene,
    camera: Camera,
    geometries: Vec<Geometry>,
    selected: HashSet<usize>,
    cursor: Option<(f32, f32)>,
    orbit: bool,
    pan: bool,
    modifiers: ModifiersState,
    mode: Mode,
    axis: usize,
    status: String,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    retry: Option<Instant>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            window: None,
            renderer: None,
            scene: Scene::initial(),
            camera: Camera::default(),

            geometries: [Shape::Cube, Shape::Sphere, Shape::Pyramid]
                .into_iter()
                .map(Geometry::new)
                .collect(),

            selected: HashSet::new(),
            cursor: None,
            orbit: false,
            pan: false,
            modifiers: ModifiersState::empty(),
            mode: Mode::Translate,
            axis: 0,
            status: "H: help | 1/2/3: add | Click: select".into(),
            undo: vec![],
            redo: vec![],
            retry: None,
        }
    }
}

impl App {
    fn title(&self) {
        if let Some(window) = &self.window {
            window.set_title(&format!(
                "Modeler | {:?} {} | {} selected / {} objects | {}",
                self.mode,
                ["X", "Y", "Z"][self.axis],
                self.selected.len(),
                self.scene.objects.len(),
                self.status
            ));
        }
    }

    fn redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            scene: self.scene.clone(),
            selected: self.selected.clone(),
        }
    }

    fn checkpoint(&mut self) {
        self.undo.push(self.snapshot());

        if self.undo.len() > 50 {
            self.undo.remove(0);
        }

        self.redo.clear();
    }

    fn history(&mut self, redo: bool) {
        let target = if redo {
            self.redo.pop()
        } else {
            self.undo.pop()
        };

        if let Some(target) = target {
            let current = self.snapshot();

            if redo {
                self.undo.push(current);
            } else {
                self.redo.push(current);
            }

            self.scene = target.scene;
            self.selected = target.selected;
            self.status = if redo { "Redo" } else { "Undo" }.into();
        }
    }

    fn key(&mut self, key: KeyCode, repeat: bool) {
        let shortcut = self.modifiers.control_key() || self.modifiers.super_key();

        if shortcut {
            if repeat {
                return;
            }

            match key {
                KeyCode::KeyS => {
                    self.status = match self.scene.save(Path::new("scene.json")) {
                        Ok(()) => "Saved scene.json".into(),
                        Err(error) => format!("Save failed: {error}"),
                    };
                }

                KeyCode::KeyO => match Scene::load(Path::new("scene.json")) {
                    Ok(scene) => {
                        self.checkpoint();
                        self.scene = scene;
                        self.selected.clear();
                        self.status = "Loaded scene.json".into();
                    }

                    Err(error) => {
                        self.status = format!("Load failed: {error}");
                    }
                },

                KeyCode::KeyZ => {
                    self.history(self.modifiers.shift_key());
                }

                KeyCode::KeyY => {
                    self.history(true);
                }

                _ => {}
            }

            self.title();
            self.redraw();
            return;
        }

        let direction = match key {
            KeyCode::ArrowRight | KeyCode::ArrowUp => 1.0,
            KeyCode::ArrowLeft | KeyCode::ArrowDown => -1.0,
            _ => 0.0,
        };

        if direction != 0.0 {
            if !self.selected.is_empty() {
                self.checkpoint();

                let fine = if self.modifiers.shift_key() { 0.2 } else { 1.0 };

                let axis = [Vec3::X, Vec3::Y, Vec3::Z][self.axis];

                match self.mode {
                    Mode::Translate => {
                        self.scene
                            .translate(&self.selected, axis * direction * 0.1 * fine);
                    }

                    Mode::Rotate => {
                        self.scene.rotate(
                            &self.selected,
                            axis,
                            direction * 5_f32.to_radians() * fine,
                        );
                    }

                    Mode::Scale => {
                        self.scene
                            .scale(&self.selected, (direction * 0.1 * fine).exp());
                    }
                }

                self.status = "Edited selection".into();
            } else {
                self.status = "Click an object first".into();
            }
        } else if !repeat {
            match key {
                KeyCode::Digit1 | KeyCode::Digit2 | KeyCode::Digit3 => {
                    if self.scene.objects.len() < 1000 {
                        self.checkpoint();

                        let shape = match key {
                            KeyCode::Digit1 => Shape::Cube,
                            KeyCode::Digit2 => Shape::Sphere,
                            _ => Shape::Pyramid,
                        };

                        let index = self.scene.add(shape, self.camera.target);

                        self.selected = HashSet::from([index]);
                        self.status = format!("Added {shape:?}");
                    } else {
                        self.status = "Object limit: 1000".into();
                    }
                }

                KeyCode::KeyT => {
                    self.mode = Mode::Translate;
                    self.status = "Arrows: move along axis".into();
                }

                KeyCode::KeyR => {
                    self.mode = Mode::Rotate;
                    self.status = "Arrows: rotate around selection center".into();
                }

                KeyCode::KeyS => {
                    self.mode = Mode::Scale;
                    self.status = "Arrows: uniformly scale selection".into();
                }

                KeyCode::KeyX => self.axis = 0,
                KeyCode::KeyY => self.axis = 1,
                KeyCode::KeyZ => self.axis = 2,

                KeyCode::KeyC if !self.selected.is_empty() => {
                    self.checkpoint();

                    for &i in &self.selected {
                        self.scene.objects[i].color =
                            (self.scene.objects[i].color + 1) % PALETTE.len();
                    }
                }

                KeyCode::KeyD if !self.selected.is_empty() => {
                    self.checkpoint();
                    self.scene.duplicate(&mut self.selected);
                    self.status = "Duplicated selection".into();
                }

                KeyCode::Delete | KeyCode::Backspace if !self.selected.is_empty() => {
                    self.checkpoint();
                    self.scene.delete(&mut self.selected);
                    self.status = "Deleted selection".into();
                }

                KeyCode::KeyG if self.selected.len() > 1 => {
                    self.checkpoint();
                    self.scene.group(&self.selected);
                    self.status = "Grouped selection".into();
                }

                KeyCode::KeyU if !self.selected.is_empty() => {
                    self.checkpoint();
                    self.scene.ungroup(&self.selected);
                    self.status = "Ungrouped selection".into();
                }

                KeyCode::KeyA => {
                    self.selected = (0..self.scene.objects.len()).collect();
                }

                KeyCode::Escape => {
                    self.selected.clear();
                }

                KeyCode::Home => {
                    self.camera = Camera::default();
                }

                KeyCode::KeyF => {
                    if let Some(center) = self.scene.center(&self.selected) {
                        let extent = self
                            .selected
                            .iter()
                            .map(|i| {
                                let object = &self.scene.objects[*i];

                                Vec3::from_array(object.position).distance(center) + object.scale
                            })
                            .fold(0.0_f32, f32::max);

                        self.camera.target = center;
                        self.camera.distance = (extent * 3.0).clamp(0.3, 100.0);
                    }
                }

                KeyCode::KeyH => {
                    println!("{HELP}");
                    self.status = "Controls printed in terminal".into();
                }

                _ => {}
            }
        }

        self.title();
        self.redraw();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }

        if self.window.is_none() {
            let attributes = Window::default_attributes()
                .with_title("Modeler")
                .with_inner_size(winit::dpi::LogicalSize::new(1100.0, 760.0));

            match event_loop.create_window(attributes) {
                Ok(window) => {
                    self.window = Some(Arc::new(window));
                }

                Err(error) => {
                    eprintln!("Window creation failed: {error}");
                    event_loop.exit();
                    return;
                }
            }
        }

        match Renderer::new(self.window.as_ref().unwrap().clone(), &self.geometries) {
            Ok(renderer) => {
                self.renderer = Some(renderer);
            }

            Err(error) => {
                eprintln!("GPU initialization failed: {error}");
                event_loop.exit();
                return;
            }
        }

        self.title();
        self.redraw();
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.renderer = None;
        self.orbit = false;
        self.pan = false;
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(at) = self.retry {
            if Instant::now() >= at {
                self.retry = None;
                self.redraw();
                event_loop.set_control_flow(ControlFlow::Wait);
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(at));
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.window.as_ref().is_none_or(|window| window.id() != id) {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::RedrawRequested => {
                if let Some(renderer) = &mut self.renderer {
                    match renderer.render(&self.scene, &self.camera, &self.selected) {
                        Ok(true) => {
                            self.retry = Some(Instant::now() + Duration::from_millis(100));
                        }

                        Ok(false) => {}

                        Err(error) => {
                            eprintln!("Rendering failed: {error}");
                            event_loop.exit();
                        }
                    }
                }
            }

            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }

                self.redraw();
            }

            WindowEvent::Occluded(false) => {
                self.redraw();
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }

            WindowEvent::Focused(false) => {
                self.orbit = false;
                self.pan = false;
                self.modifiers = ModifiersState::empty();
            }

            WindowEvent::CursorLeft { .. } => {
                self.cursor = None;
            }

            WindowEvent::CursorMoved { position, .. } => {
                let next = (position.x as f32, position.y as f32);

                if let Some(old) = self.cursor {
                    if self.orbit {
                        self.camera.orbit(next.0 - old.0, next.1 - old.1);
                        self.redraw();
                    }

                    if self.pan {
                        let height = self.window.as_ref().unwrap().inner_size().height as f32;

                        self.camera.pan(next.0 - old.0, next.1 - old.1, height);

                        self.redraw();
                    }
                }

                self.cursor = Some(next);
            }

            WindowEvent::MouseInput { state, button, .. } => {
                let down = state == ElementState::Pressed;

                match button {
                    MouseButton::Right => self.orbit = down,
                    MouseButton::Middle => self.pan = down,

                    MouseButton::Left if down => {
                        if let Some((x, y)) = self.cursor {
                            let size = self.window.as_ref().unwrap().inner_size();

                            if size.width > 0 && size.height > 0 {
                                let (origin, direction) =
                                    self.camera.ray(x, y, size.width as f32, size.height as f32);

                                let picked =
                                    self.scene
                                        .pick_distance(&self.geometries, origin, direction);

                                let mut hit = picked.map(|(i, _)| i);

                                // Avoid selecting objects hidden by the floor.
                                if direction.y.abs() > 1e-6 {
                                    let distance = (-0.775 - origin.y) / direction.y;

                                    let point = origin + direction * distance;

                                    if distance > 0.0
                                        && point.x.abs() <= 15.0
                                        && point.z.abs() <= 15.0
                                    {
                                        if picked.is_some_and(|(_, object_distance)| {
                                            object_distance > distance
                                        }) {
                                            hit = None;
                                        }
                                    }
                                }

                                self.scene.select(
                                    &mut self.selected,
                                    hit,
                                    self.modifiers.shift_key(),
                                );

                                self.status = hit
                                    .map(|i| {
                                        format!(
                                            "Selected {:?} #{}",
                                            self.scene.objects[i].shape,
                                            i + 1
                                        )
                                    })
                                    .unwrap_or("Selection cleared".into());

                                self.title();
                                self.redraw();
                            }
                        }
                    }

                    _ => {}
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 / 60.0,
                };

                self.camera.zoom(amount);
                self.redraw();
            }

            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    self.key(key, event.repeat);
                }
            }

            _ => {}
        }
    }
}
