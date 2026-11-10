use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Shape {
    Cube,
    Sphere,
    Pyramid,
}

impl Shape {
    pub fn index(self) -> usize {
        match self {
            Self::Cube => 0,
            Self::Sphere => 1,
            Self::Pyramid => 2,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

pub struct Geometry {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Geometry {
    fn triangle(&mut self, a: Vec3, b: Vec3, c: Vec3) {
        let normal = (b - a).cross(c - a).normalize().to_array();
        let start = self.vertices.len() as u32;

        self.vertices.extend([a, b, c].map(|p| Vertex {
            position: p.to_array(),
            normal,
        }));

        self.indices.extend([start, start + 1, start + 2]);
    }

    pub fn new(shape: Shape) -> Self {
        let mut mesh = Self {
            vertices: vec![],
            indices: vec![],
        };

        match shape {
            Shape::Cube => {
                // Each face has separate vertices because its normal differs.
                for (normal, u, v) in [
                    (Vec3::X, -Vec3::Z, Vec3::Y),
                    (-Vec3::X, Vec3::Z, Vec3::Y),
                    (Vec3::Y, Vec3::X, -Vec3::Z),
                    (-Vec3::Y, Vec3::X, Vec3::Z),
                    (Vec3::Z, Vec3::X, Vec3::Y),
                    (-Vec3::Z, -Vec3::X, Vec3::Y),
                ] {
                    let center = normal * 0.5;

                    let a = center - u * 0.5 - v * 0.5;
                    let b = center + u * 0.5 - v * 0.5;
                    let c = center + u * 0.5 + v * 0.5;
                    let d = center - u * 0.5 + v * 0.5;

                    mesh.triangle(a, b, c);
                    mesh.triangle(a, c, d);
                }
            }

            Shape::Pyramid => {
                let a = Vec3::new(-0.5, -0.5, 0.5);
                let b = Vec3::new(0.5, -0.5, 0.5);
                let c = Vec3::new(0.5, -0.5, -0.5);
                let d = Vec3::new(-0.5, -0.5, -0.5);
                let top = Vec3::new(0.0, 0.5, 0.0);

                for (x, y) in [(a, b), (b, c), (c, d), (d, a)] {
                    mesh.triangle(x, y, top);
                }

                mesh.triangle(a, d, c);
                mesh.triangle(a, c, b);
            }

            Shape::Sphere => {
                let rings = 16_u32;
                let sectors = 24_u32;

                for r in 0..=rings {
                    let theta = std::f32::consts::PI * r as f32 / rings as f32;

                    for s in 0..=sectors {
                        let phi = std::f32::consts::TAU * s as f32 / sectors as f32;

                        let normal = Vec3::new(
                            theta.sin() * phi.cos(),
                            theta.cos(),
                            theta.sin() * phi.sin(),
                        );

                        mesh.vertices.push(Vertex {
                            position: (normal * 0.5).to_array(),
                            normal: normal.to_array(),
                        });
                    }
                }

                for r in 0..rings {
                    for s in 0..sectors {
                        let a = r * (sectors + 1) + s;
                        let b = a + sectors + 1;

                        if r != 0 {
                            mesh.indices.extend([a, a + 1, b]);
                        }

                        if r != rings - 1 {
                            mesh.indices.extend([a + 1, b + 1, b]);
                        }
                    }
                }
            }
        }

        mesh
    }
}

// Return the distance along a ray where it intersects a triangle.
pub fn ray_triangle(origin: Vec3, direction: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let e1 = b - a;
    let e2 = c - a;
    let p = direction.cross(e2);
    let det = e1.dot(p);

    if det.abs() < 1e-7 {
        return None;
    }

    let inverse = 1.0 / det;
    let t = origin - a;
    let u = t.dot(p) * inverse;

    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let q = t.cross(e1);
    let v = direction.dot(q) * inverse;

    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let distance = e2.dot(q) * inverse;

    (distance > 0.0).then_some(distance)
}
