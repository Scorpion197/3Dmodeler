use crate::mesh::{Geometry, Shape, ray_triangle};
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

pub const PALETTE: [[f32; 3]; 6] = [
    [0.15, 0.55, 0.85],
    [0.95, 0.35, 0.15],
    [0.2, 0.75, 0.4],
    [0.65, 0.3, 0.85],
    [0.95, 0.7, 0.15],
    [0.75, 0.78, 0.82],
];

#[derive(Clone, Serialize, Deserialize)]
pub struct Object {
    pub shape: Shape,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: f32,
    pub color: usize,
    pub group: Option<u64>,
}

impl Object {
    pub fn model(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            Vec3::splat(self.scale),
            Quat::from_array(self.rotation),
            Vec3::from_array(self.position),
        )
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Scene {
    pub objects: Vec<Object>,
}

impl Scene {
    pub fn initial() -> Self {
        let mut scene = Self::default();

        scene.add(Shape::Cube, Vec3::new(-1.5, 0.0, 0.0));
        scene.add(Shape::Sphere, Vec3::ZERO);
        scene.add(Shape::Pyramid, Vec3::new(1.5, 0.0, 0.0));

        scene
    }

    pub fn add(&mut self, shape: Shape, position: Vec3) -> usize {
        let index = self.objects.len();

        self.objects.push(Object {
            shape,
            position: position.to_array(),
            rotation: Quat::IDENTITY.to_array(),
            scale: 1.0,
            color: index % PALETTE.len(),
            group: None,
        });

        index
    }

    pub fn center(&self, selected: &HashSet<usize>) -> Option<Vec3> {
        let points: Vec<_> = selected
            .iter()
            .filter_map(|i| self.objects.get(*i))
            .map(|o| Vec3::from_array(o.position))
            .collect();

        (!points.is_empty()).then(|| points.iter().copied().sum::<Vec3>() / points.len() as f32)
    }

    pub fn translate(&mut self, selected: &HashSet<usize>, delta: Vec3) {
        for &i in selected {
            if let Some(object) = self.objects.get_mut(i) {
                object.position = (Vec3::from_array(object.position) + delta).to_array();
            }
        }
    }

    pub fn rotate(&mut self, selected: &HashSet<usize>, axis: Vec3, angle: f32) {
        if let Some(center) = self.center(selected) {
            let rotation = Quat::from_axis_angle(axis, angle);

            for &i in selected {
                if let Some(object) = self.objects.get_mut(i) {
                    object.position = (center
                        + rotation * (Vec3::from_array(object.position) - center))
                        .to_array();

                    object.rotation = (rotation * Quat::from_array(object.rotation))
                        .normalize()
                        .to_array();
                }
            }
        }
    }

    pub fn scale(&mut self, selected: &HashSet<usize>, factor: f32) {
        if let Some(center) = self.center(selected) {
            // Reject the whole operation if any object would exceed a limit.
            if selected.iter().any(|i| {
                self.objects
                    .get(*i)
                    .is_some_and(|o| !(0.05..=20.0).contains(&(o.scale * factor)))
            }) {
                return;
            }

            for &i in selected {
                if let Some(object) = self.objects.get_mut(i) {
                    object.position =
                        (center + (Vec3::from_array(object.position) - center) * factor).to_array();

                    object.scale *= factor;
                }
            }
        }
    }

    pub fn group(&mut self, selected: &HashSet<usize>) {
        if selected.len() < 2 {
            return;
        }

        let id = self
            .objects
            .iter()
            .filter_map(|o| o.group)
            .max()
            .unwrap_or(0)
            .saturating_add(1);

        for &i in selected {
            if let Some(object) = self.objects.get_mut(i) {
                object.group = Some(id);
            }
        }
    }

    pub fn ungroup(&mut self, selected: &HashSet<usize>) {
        for &i in selected {
            if let Some(object) = self.objects.get_mut(i) {
                object.group = None;
            }
        }
    }

    pub fn delete(&mut self, selected: &mut HashSet<usize>) {
        let mut i = 0;

        self.objects.retain(|_| {
            let keep = !selected.contains(&i);
            i += 1;
            keep
        });

        selected.clear();
    }

    pub fn duplicate(&mut self, selected: &mut HashSet<usize>) {
        if self.objects.len() + selected.len() > 1000 {
            return;
        }

        let mut source: Vec<_> = selected.iter().copied().collect();
        source.sort_unstable();

        let new_group = (source.len() > 1).then(|| {
            self.objects
                .iter()
                .filter_map(|o| o.group)
                .max()
                .unwrap_or(0)
                .saturating_add(1)
        });

        let copies: Vec<_> = source
            .iter()
            .filter_map(|i| self.objects.get(*i))
            .cloned()
            .collect();

        selected.clear();

        for mut object in copies {
            object.position =
                (Vec3::from_array(object.position) + Vec3::new(0.5, 0.0, 0.5)).to_array();

            object.group = new_group;

            selected.insert(self.objects.len());
            self.objects.push(object);
        }
    }

    pub fn pick_distance(
        &self,
        meshes: &[Geometry],
        origin: Vec3,
        direction: Vec3,
    ) -> Option<(usize, f32)> {
        let mut closest = f32::INFINITY;
        let mut result = None;

        for (i, object) in self.objects.iter().enumerate() {
            let inverse = object.model().inverse();

            let local_origin = inverse.transform_point3(origin);

            // Keep this unnormalized so distances remain comparable.
            let local_direction = inverse.transform_vector3(direction);

            let mesh = &meshes[object.shape.index()];

            for triangle in mesh.indices.chunks_exact(3) {
                let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                    .map(|index| Vec3::from_array(mesh.vertices[index as usize].position));

                if let Some(distance) = ray_triangle(local_origin, local_direction, a, b, c) {
                    if distance < closest {
                        closest = distance;
                        result = Some(i);
                    }
                }
            }
        }

        result.map(|i| (i, closest))
    }

    pub fn select(&self, selected: &mut HashSet<usize>, hit: Option<usize>, additive: bool) {
        if !additive {
            selected.clear();
        }

        if let Some(i) = hit {
            let group = self.objects[i].group;

            let members: Vec<_> = self
                .objects
                .iter()
                .enumerate()
                .filter(|(j, o)| *j == i || (group.is_some() && o.group == group))
                .map(|(j, _)| j)
                .collect();

            let remove = additive && selected.contains(&i);

            for j in members {
                if remove {
                    selected.remove(&j);
                } else {
                    selected.insert(j);
                }
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;

        let temporary = path.with_extension("json.tmp");

        std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&temporary, path).map_err(|e| e.to_string())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;

        if metadata.len() > 2_000_000 {
            return Err("Scene exceeds 2 MB".into());
        }

        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;

        let mut scene: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;

        if scene.objects.len() > 1000 {
            return Err("Maximum 1000 objects".into());
        }

        for object in &mut scene.objects {
            if !object
                .position
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 1000.0)
                || !object.rotation.iter().all(|v| v.is_finite())
                || !object.scale.is_finite()
                || !(0.05..=20.0).contains(&object.scale)
                || object.color >= PALETTE.len()
            {
                return Err("Invalid object data".into());
            }

            let rotation = Quat::from_array(object.rotation);

            if rotation.length_squared() < 1e-10 || !rotation.length_squared().is_finite() {
                return Err("Invalid rotation".into());
            }

            object.rotation = rotation.normalize().to_array();
        }

        Ok(scene)
    }
}
