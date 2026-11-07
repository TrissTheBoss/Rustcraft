//! Small, dependency-free rendering pipeline for the clean-room engine.
//!
//! This module provides camera math, frame pacing, visible-face chunk meshing,
//! original PPM/PNG texture loading, directional lighting, CPU triangle rendering,
//! and frame profiling. A platform GPU/window adapter can consume the same mesh
//! and camera interfaces when the project can add its selected graphics crates.

use std::io::{self, Read};
use std::time::{Duration, Instant};

use image::ImageReader;

use crate::blocks::Aabb;
use crate::world::{BlockId, Chunk, CHUNK_SIZE};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    pub fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }
    pub fn cross(self, rhs: Self) -> Self {
        Self::new(
            self.y * rhs.z - self.z * rhs.y,
            self.z * rhs.x - self.x * rhs.z,
            self.x * rhs.y - self.y * rhs.x,
        )
    }
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }
    pub fn normalized(self) -> Self {
        let length = self.length();
        if length > f32::EPSILON {
            self / length
        } else {
            Self::ZERO
        }
    }
}

impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}
impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}
impl std::ops::Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}
impl std::ops::Div<f32> for Vec3 {
    type Output = Self;
    fn div(self, rhs: f32) -> Self {
        Self::new(self.x / rhs, self.y / rhs, self.z / rhs)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4(pub [[f32; 4]; 4]);

impl Mat4 {
    pub const IDENTITY: Self = Self([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);
    pub fn transform(self, p: [f32; 4]) -> [f32; 4] {
        let mut out = [0.0; 4];
        for (row, value) in out.iter_mut().enumerate() {
            *value = self.0[row][0] * p[0]
                + self.0[row][1] * p[1]
                + self.0[row][2] * p[2]
                + self.0[row][3] * p[3];
        }
        out
    }
    pub fn mul(self, rhs: Self) -> Self {
        let mut out = [[0.0; 4]; 4];
        for (r, row) in out.iter_mut().enumerate() {
            for (c, value) in row.iter_mut().enumerate() {
                *value = (0..4).map(|k| self.0[r][k] * rhs.0[k][c]).sum();
            }
        }
        Self(out)
    }
    pub fn perspective(
        fov_y_radians: f32,
        aspect: f32,
        near: f32,
        far: f32,
    ) -> Result<Self, &'static str> {
        if !aspect.is_finite()
            || aspect <= 0.0
            || !near.is_finite()
            || !far.is_finite()
            || near <= 0.0
            || far <= near
            || !(0.0..std::f32::consts::PI).contains(&fov_y_radians)
        {
            return Err("invalid perspective projection parameters");
        }
        let f = 1.0 / (fov_y_radians * 0.5).tan();
        Ok(Self([
            [f / aspect, 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [0.0, 0.0, far / (near - far), (far * near) / (near - far)],
            [0.0, 0.0, -1.0, 0.0],
        ]))
    }
    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Result<Self, &'static str> {
        let forward = (target - eye).normalized();
        let side = forward.cross(up).normalized();
        if forward == Vec3::ZERO || side == Vec3::ZERO {
            return Err("camera direction and up vector must be nonzero and nonparallel");
        }
        let up = side.cross(forward);
        Ok(Self([
            [side.x, side.y, side.z, -side.dot(eye)],
            [up.x, up.y, up.z, -up.dot(eye)],
            [-forward.x, -forward.y, -forward.z, forward.dot(eye)],
            [0.0, 0.0, 0.0, 1.0],
        ]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub position: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub vertical_fov_radians: f32,
    pub near: f32,
    pub far: f32,
    pub fov_degrees: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::new(24.0, 20.0, 32.0),
            target: Vec3::new(8.0, 8.0, 8.0),
            up: Vec3::new(0.0, 1.0, 0.0),
            vertical_fov_radians: 1.047_197_6,
            near: 0.1,
            far: 512.0,
            fov_degrees: 60.0,
        }
    }
}

impl Camera {
    pub fn view_projection(&self, aspect: f32) -> Result<Mat4, &'static str> {
        Ok(
            Mat4::perspective(self.fov_degrees.to_radians(), aspect, self.near, self.far)?
                .mul(Mat4::look_at(self.position, self.target, self.up)?),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputKey {
    Forward,
    Backward,
    Left,
    Right,
    Up,
    Down,
    Sprint,
}

#[derive(Clone, Debug, Default)]
pub struct InputState {
    pressed: Vec<InputKey>,
}

impl InputState {
    pub fn clear(&mut self) {
        self.pressed.clear();
    }

    pub fn set(&mut self, key: InputKey, pressed: bool) {
        if pressed && !self.pressed.contains(&key) {
            self.pressed.push(key);
        }
        if !pressed {
            self.pressed.retain(|value| *value != key);
        }
    }
    pub fn is_pressed(&self, key: InputKey) -> bool {
        self.pressed.contains(&key)
    }
    /// Move the camera in its horizontal view plane and world vertical axis.
    pub fn move_camera(&self, camera: &mut Camera, seconds: f32, speed: f32) {
        if !seconds.is_finite() || seconds <= 0.0 || !speed.is_finite() || speed < 0.0 {
            return;
        }
        let forward = Vec3::new(
            camera.target.x - camera.position.x,
            0.0,
            camera.target.z - camera.position.z,
        )
        .normalized();
        let right = forward.cross(Vec3::new(0.0, 1.0, 0.0)).normalized();
        let mut movement = Vec3::ZERO;
        if self.is_pressed(InputKey::Forward) {
            movement = movement + forward;
        }
        if self.is_pressed(InputKey::Backward) {
            movement = movement - forward;
        }
        if self.is_pressed(InputKey::Right) {
            movement = movement + right;
        }
        if self.is_pressed(InputKey::Left) {
            movement = movement - right;
        }
        if self.is_pressed(InputKey::Up) {
            movement.y += 1.0;
        }
        if self.is_pressed(InputKey::Down) {
            movement.y -= 1.0;
        }
        movement = movement.normalized() * (seconds * speed);
        camera.position = camera.position + movement;
        camera.target = camera.target + movement;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Face {
    NegativeX,
    PositiveX,
    NegativeY,
    PositiveY,
    NegativeZ,
    PositiveZ,
}

impl Face {
    pub const ALL: [Self; 6] = [
        Self::NegativeX,
        Self::PositiveX,
        Self::NegativeY,
        Self::PositiveY,
        Self::NegativeZ,
        Self::PositiveZ,
    ];
    fn offset(self) -> [i32; 3] {
        match self {
            Self::NegativeX => [-1, 0, 0],
            Self::PositiveX => [1, 0, 0],
            Self::NegativeY => [0, -1, 0],
            Self::PositiveY => [0, 1, 0],
            Self::NegativeZ => [0, 0, -1],
            Self::PositiveZ => [0, 0, 1],
        }
    }
    fn normal(self) -> Vec3 {
        let o = self.offset();
        Vec3::new(o[0] as f32, o[1] as f32, o[2] as f32)
    }
    fn corners(self, x: f32, y: f32, z: f32) -> [Vec3; 4] {
        let (x0, x1, y0, y1, z0, z1) = (x, x + 1.0, y, y + 1.0, z, z + 1.0);
        match self {
            Self::NegativeX => [
                Vec3::new(x0, y0, z0),
                Vec3::new(x0, y0, z1),
                Vec3::new(x0, y1, z1),
                Vec3::new(x0, y1, z0),
            ],
            Self::PositiveX => [
                Vec3::new(x1, y0, z1),
                Vec3::new(x1, y0, z0),
                Vec3::new(x1, y1, z0),
                Vec3::new(x1, y1, z1),
            ],
            Self::NegativeY => [
                Vec3::new(x0, y0, z1),
                Vec3::new(x0, y0, z0),
                Vec3::new(x1, y0, z0),
                Vec3::new(x1, y0, z1),
            ],
            Self::PositiveY => [
                Vec3::new(x0, y1, z0),
                Vec3::new(x0, y1, z1),
                Vec3::new(x1, y1, z1),
                Vec3::new(x1, y1, z0),
            ],
            Self::NegativeZ => [
                Vec3::new(x1, y0, z0),
                Vec3::new(x0, y0, z0),
                Vec3::new(x0, y1, z0),
                Vec3::new(x1, y1, z0),
            ],
            Self::PositiveZ => [
                Vec3::new(x0, y0, z1),
                Vec3::new(x1, y0, z1),
                Vec3::new(x1, y1, z1),
                Vec3::new(x0, y1, z1),
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub block: BlockId,
    /// Optional face-specific texture selected by the scene material setup.
    /// `None` keeps the block's default material texture.
    pub texture: Option<u16>,
    pub uv: [f32; 2],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
}

impl Mesh {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
    pub fn byte_len(&self) -> usize {
        self.vertices.len() * std::mem::size_of::<Vertex>()
            + self.indices.len() * std::mem::size_of::<u32>()
    }

    pub fn recalculate_bounds(&mut self) {
        if self.vertices.is_empty() {
            return;
        }
        self.bounds_min = self.vertices[0].position;
        self.bounds_max = self.vertices[0].position;
        for vertex in &self.vertices[1..] {
            self.bounds_min.x = self.bounds_min.x.min(vertex.position.x);
            self.bounds_min.y = self.bounds_min.y.min(vertex.position.y);
            self.bounds_min.z = self.bounds_min.z.min(vertex.position.z);
            self.bounds_max.x = self.bounds_max.x.max(vertex.position.x);
            self.bounds_max.y = self.bounds_max.y.max(vertex.position.y);
            self.bounds_max.z = self.bounds_max.z.max(vertex.position.z);
        }
    }
}

/// Build a small axis-aligned cube mesh for lightweight world entities such
/// as prototype dropped items. Positions are world-space block units.
pub fn cube_mesh(center: [f64; 3], half_extent: f64, block: BlockId) -> Mesh {
    if !center.iter().all(|value| value.is_finite())
        || !half_extent.is_finite()
        || half_extent <= 0.0
    {
        return Mesh::default();
    }
    let faces: [([f32; 3], [[f64; 3]; 4]); 6] = [
        (
            [1.0, 0.0, 0.0],
            [[1., 0., 0.], [1., 1., 0.], [1., 1., 1.], [1., 0., 1.]],
        ),
        (
            [-1.0, 0.0, 0.0],
            [[0., 0., 1.], [0., 1., 1.], [0., 1., 0.], [0., 0., 0.]],
        ),
        (
            [0.0, 1.0, 0.0],
            [[0., 1., 1.], [1., 1., 1.], [1., 1., 0.], [0., 1., 0.]],
        ),
        (
            [0.0, -1.0, 0.0],
            [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
        ),
        (
            [0.0, 0.0, 1.0],
            [[1., 0., 1.], [1., 1., 1.], [0., 1., 1.], [0., 0., 1.]],
        ),
        (
            [0.0, 0.0, -1.0],
            [[0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.]],
        ),
    ];
    let mut mesh = Mesh::default();
    for (normal, corners) in faces {
        let start = mesh.vertices.len() as u32;
        for (index, corner) in corners.into_iter().enumerate() {
            mesh.vertices.push(Vertex {
                position: Vec3::new(
                    (center[0] + (corner[0] - 0.5) * half_extent * 2.0) as f32,
                    (center[1] + (corner[1] - 0.5) * half_extent * 2.0) as f32,
                    (center[2] + (corner[2] - 0.5) * half_extent * 2.0) as f32,
                ),
                normal: Vec3::new(normal[0], normal[1], normal[2]),
                block,
                texture: None,
                uv: [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]][index],
            });
        }
        mesh.indices
            .extend([start, start + 1, start + 2, start, start + 2, start + 3]);
    }
    mesh.recalculate_bounds();
    mesh
}

/// Meshes exposed block faces. The callback receives signed chunk-local coordinates,
/// including coordinates just outside the chunk, so callers can hide shared borders.
pub fn mesh_chunk(chunk: &Chunk, mut is_solid: impl FnMut(i32, i32, i32) -> bool) -> Mesh {
    let mut mesh = Mesh::default();
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let block = chunk.get(x, y, z).expect("loop coordinates are in bounds");
                if block == BlockId(0) {
                    continue;
                }
                for face in Face::ALL {
                    let offset = face.offset();
                    if is_solid(
                        x as i32 + offset[0],
                        y as i32 + offset[1],
                        z as i32 + offset[2],
                    ) {
                        continue;
                    }
                    let first = mesh.vertices.len() as u32;
                    mesh.vertices.extend(
                        face.corners(x as f32, y as f32, z as f32)
                            .into_iter()
                            .zip([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]])
                            .map(|(position, uv)| Vertex {
                                position,
                                normal: face.normal(),
                                block,
                                texture: None,
                                uv,
                            }),
                    );
                    mesh.indices.extend_from_slice(&[
                        first,
                        first + 1,
                        first + 2,
                        first,
                        first + 2,
                        first + 3,
                    ]);
                }
            }
        }
    }
    mesh.recalculate_bounds();
    mesh
}

/// Greedy meshing merges adjacent exposed faces with the same block ID.
/// UVs scale with the merged rectangle, preserving repeatable texture sampling.
pub fn greedy_mesh_chunk(chunk: &Chunk, mut is_solid: impl FnMut(i32, i32, i32) -> bool) -> Mesh {
    let mut mesh = Mesh::default();
    for face in Face::ALL {
        for layer in 0..CHUNK_SIZE {
            let mut mask = [[None; CHUNK_SIZE]; CHUNK_SIZE];
            for v in 0..CHUNK_SIZE {
                for u in 0..CHUNK_SIZE {
                    let (x, y, z) = face_cell(face, layer, u, v);
                    let block = chunk.get(x, y, z).unwrap();
                    let o = face.offset();
                    if block != BlockId(0)
                        && !is_solid(x as i32 + o[0], y as i32 + o[1], z as i32 + o[2])
                    {
                        mask[v][u] = Some(block);
                    }
                }
            }
            let mut v = 0;
            while v < CHUNK_SIZE {
                let mut u = 0;
                while u < CHUNK_SIZE {
                    let Some(block) = mask[v][u] else {
                        u += 1;
                        continue;
                    };
                    let mut width = 1;
                    while u + width < CHUNK_SIZE && mask[v][u + width] == Some(block) {
                        width += 1;
                    }
                    let mut height = 1;
                    'height: while v + height < CHUNK_SIZE {
                        for x in u..u + width {
                            if mask[v + height][x] != Some(block) {
                                break 'height;
                            }
                        }
                        height += 1;
                    }
                    add_quad(&mut mesh, face, layer, u, v, width, height, block);
                    for row in v..v + height {
                        for x in u..u + width {
                            mask[row][x] = None;
                        }
                    }
                    u += width;
                }
                v += 1;
            }
        }
    }
    mesh.recalculate_bounds();
    mesh
}

/// Greedy-mesh full cubes and emit prototype model boxes for other blocks.
/// Geometry remains block-local so the caller can translate the resulting mesh.
pub fn mesh_chunk_with_shapes(
    chunk: &Chunk,
    mut is_full_cube: impl FnMut(i32, i32, i32) -> bool,
    mut visual_boxes_at: impl FnMut(i32, i32, i32) -> &'static [Aabb],
) -> Mesh {
    let mut full_cubes = Chunk::empty();
    let mut greedy_mask = vec![false; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE];
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let block = chunk.get(x, y, z).unwrap();
                if block == BlockId(0) || !is_full_cube(x as i32, y as i32, z as i32) {
                    continue;
                }
                let neighbor_offsets = [
                    [1, 0, 0],
                    [-1, 0, 0],
                    [0, 1, 0],
                    [0, -1, 0],
                    [0, 0, 1],
                    [0, 0, -1],
                ];
                let touches_partial_shape = neighbor_offsets.iter().any(|offset| {
                    let neighbor = [
                        x as i32 + offset[0],
                        y as i32 + offset[1],
                        z as i32 + offset[2],
                    ];
                    !is_full_cube(neighbor[0], neighbor[1], neighbor[2])
                        && !visual_boxes_at(neighbor[0], neighbor[1], neighbor[2]).is_empty()
                });
                if !touches_partial_shape {
                    full_cubes.set(x, y, z, block).unwrap();
                    let index = y * CHUNK_SIZE * CHUNK_SIZE + z * CHUNK_SIZE + x;
                    greedy_mask[index] = true;
                }
            }
        }
    }

    let mut mesh = greedy_mesh_chunk(&full_cubes, |x, y, z| is_full_cube(x, y, z));
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let block = chunk.get(x, y, z).unwrap();
                let index = y * CHUNK_SIZE * CHUNK_SIZE + z * CHUNK_SIZE + x;
                if block == BlockId(0) || greedy_mask[index] {
                    continue;
                }
                let own_boxes = visual_boxes_at(x as i32, y as i32, z as i32);
                for (shape_index, bounds) in own_boxes.iter().copied().enumerate() {
                    append_shape_box(
                        &mut mesh,
                        block,
                        [x, y, z],
                        own_boxes,
                        shape_index,
                        bounds,
                        &mut visual_boxes_at,
                    );
                }
            }
        }
    }
    mesh.recalculate_bounds();
    mesh
}

fn append_shape_box(
    mesh: &mut Mesh,
    block: BlockId,
    cell: [usize; 3],
    own_boxes: &'static [Aabb],
    own_shape_index: usize,
    bounds: Aabb,
    visual_boxes_at: &mut impl FnMut(i32, i32, i32) -> &'static [Aabb],
) {
    let min = [
        bounds.min[0] as f32,
        bounds.min[1] as f32,
        bounds.min[2] as f32,
    ];
    let max = [
        bounds.max[0] as f32,
        bounds.max[1] as f32,
        bounds.max[2] as f32,
    ];
    for axis in 0..3 {
        for sign in [-1, 1] {
            let plane = cell[axis] as f32 + if sign > 0 { max[axis] } else { min[axis] };
            let (u_axis, v_axis) = match axis {
                0 => (2, 1),
                1 => (0, 2),
                _ => (0, 1),
            };
            let face_rect = [
                cell[u_axis] as f32 + min[u_axis],
                cell[u_axis] as f32 + max[u_axis],
                cell[v_axis] as f32 + min[v_axis],
                cell[v_axis] as f32 + max[v_axis],
            ];
            let mut occluders = Vec::new();
            for owner_y in cell[1] as i32 - 1..=cell[1] as i32 + 1 {
                for owner_z in cell[2] as i32 - 1..=cell[2] as i32 + 1 {
                    for owner_x in cell[0] as i32 - 1..=cell[0] as i32 + 1 {
                        let owner = [owner_x, owner_y, owner_z];
                        let owner_boxes =
                            if owner == [cell[0] as i32, cell[1] as i32, cell[2] as i32] {
                                own_boxes
                            } else {
                                visual_boxes_at(owner_x, owner_y, owner_z)
                            };
                        for (other_index, other) in owner_boxes.iter().enumerate() {
                            if owner == [cell[0] as i32, cell[1] as i32, cell[2] as i32]
                                && other_index == own_shape_index
                            {
                                continue;
                            }
                            let other_min = [
                                owner_x as f32 + other.min[0] as f32,
                                owner_y as f32 + other.min[1] as f32,
                                owner_z as f32 + other.min[2] as f32,
                            ];
                            let other_max = [
                                owner_x as f32 + other.max[0] as f32,
                                owner_y as f32 + other.max[1] as f32,
                                owner_z as f32 + other.max[2] as f32,
                            ];
                            let crosses_outward = if sign > 0 {
                                other_min[axis] <= plane + 1.0e-6
                                    && other_max[axis] > plane + 1.0e-6
                            } else {
                                other_max[axis] >= plane - 1.0e-6
                                    && other_min[axis] < plane - 1.0e-6
                            };
                            if !crosses_outward {
                                continue;
                            }
                            let overlap = [
                                face_rect[0].max(other_min[u_axis]),
                                face_rect[1].min(other_max[u_axis]),
                                face_rect[2].max(other_min[v_axis]),
                                face_rect[3].min(other_max[v_axis]),
                            ];
                            if overlap[0] < overlap[1] && overlap[2] < overlap[3] {
                                occluders.push(overlap);
                            }
                        }
                    }
                }
            }
            for rect in exposed_face_rectangles(face_rect, &occluders) {
                append_face_rect(mesh, block, axis, sign, plane, rect);
            }
        }
    }
}

fn exposed_face_rectangles(rect: [f32; 4], occluders: &[[f32; 4]]) -> Vec<[f32; 4]> {
    let mut u_cuts = vec![rect[0], rect[1]];
    let mut v_cuts = vec![rect[2], rect[3]];
    for cover in occluders {
        u_cuts.extend([cover[0], cover[1]]);
        v_cuts.extend([cover[2], cover[3]]);
    }
    u_cuts.sort_by(f32::total_cmp);
    v_cuts.sort_by(f32::total_cmp);
    u_cuts.dedup_by(|first, second| (*first - *second).abs() <= 1.0e-6);
    v_cuts.dedup_by(|first, second| (*first - *second).abs() <= 1.0e-6);

    let mut exposed = Vec::new();
    for u_pair in u_cuts.windows(2) {
        let u0 = u_pair[0].max(rect[0]);
        let u1 = u_pair[1].min(rect[1]);
        if u0 >= u1 {
            continue;
        }
        for v_pair in v_cuts.windows(2) {
            let v0 = v_pair[0].max(rect[2]);
            let v1 = v_pair[1].min(rect[3]);
            if v0 >= v1 {
                continue;
            }
            let midpoint = [(u0 + u1) * 0.5, (v0 + v1) * 0.5];
            if !occluders.iter().any(|cover| {
                midpoint[0] > cover[0]
                    && midpoint[0] < cover[1]
                    && midpoint[1] > cover[2]
                    && midpoint[1] < cover[3]
            }) {
                exposed.push([u0, u1, v0, v1]);
            }
        }
    }
    exposed
}

fn append_face_rect(
    mesh: &mut Mesh,
    block: BlockId,
    axis: usize,
    sign: i32,
    plane: f32,
    rect: [f32; 4],
) {
    let (u_axis, v_axis) = match axis {
        0 => (2, 1),
        1 => (0, 2),
        _ => (0, 1),
    };
    let normal = match (axis, sign) {
        (0, 1) => [1.0, 0.0, 0.0],
        (0, -1) => [-1.0, 0.0, 0.0],
        (1, 1) => [0.0, 1.0, 0.0],
        (1, -1) => [0.0, -1.0, 0.0],
        (2, 1) => [0.0, 0.0, 1.0],
        _ => [0.0, 0.0, -1.0],
    };
    let point = |u: f32, v: f32| {
        let mut position = [0.0; 3];
        position[axis] = plane;
        position[u_axis] = u;
        position[v_axis] = v;
        Vec3::new(position[0], position[1], position[2])
    };
    let [u0, u1, v0, v1] = rect;
    let points = match (axis, sign) {
        (0, 1) => [point(u0, v0), point(u0, v1), point(u1, v1), point(u1, v0)],
        (0, -1) => [point(u1, v0), point(u1, v1), point(u0, v1), point(u0, v0)],
        (1, 1) => [point(u0, v1), point(u1, v1), point(u1, v0), point(u0, v0)],
        (1, -1) => [point(u0, v0), point(u1, v0), point(u1, v1), point(u0, v1)],
        (2, 1) => [point(u1, v0), point(u1, v1), point(u0, v1), point(u0, v0)],
        _ => [point(u0, v0), point(u0, v1), point(u1, v1), point(u1, v0)],
    };
    let uv = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];
    let first = mesh.vertices.len() as u32;
    for index in 0..4 {
        mesh.vertices.push(Vertex {
            position: points[index],
            normal: Vec3::new(normal[0], normal[1], normal[2]),
            block,
            texture: None,
            uv: uv[index],
        });
    }
    mesh.indices
        .extend_from_slice(&[first, first + 1, first + 2, first, first + 2, first + 3]);
}

fn face_cell(face: Face, layer: usize, u: usize, v: usize) -> (usize, usize, usize) {
    match face {
        Face::NegativeX | Face::PositiveX => (layer, v, u),
        Face::NegativeY | Face::PositiveY => (u, layer, v),
        Face::NegativeZ | Face::PositiveZ => (u, v, layer),
    }
}

fn add_quad(
    mesh: &mut Mesh,
    face: Face,
    layer: usize,
    u: usize,
    v: usize,
    width: usize,
    height: usize,
    block: BlockId,
) {
    let (x, y, z) = face_cell(face, layer, u, v);
    let (x, y, z) = (x as f32, y as f32, z as f32);
    let w = width as f32;
    let h = height as f32;
    let points = match face {
        Face::NegativeX => [
            Vec3::new(x, y, z),
            Vec3::new(x, y + h, z),
            Vec3::new(x, y + h, z + w),
            Vec3::new(x, y, z + w),
        ],
        Face::PositiveX => [
            Vec3::new(x + 1.0, y, z + w),
            Vec3::new(x + 1.0, y + h, z + w),
            Vec3::new(x + 1.0, y + h, z),
            Vec3::new(x + 1.0, y, z),
        ],
        Face::NegativeY => [
            Vec3::new(x, y, z + h),
            Vec3::new(x, y, z),
            Vec3::new(x + w, y, z),
            Vec3::new(x + w, y, z + h),
        ],
        Face::PositiveY => [
            Vec3::new(x, y + 1.0, z),
            Vec3::new(x, y + 1.0, z + h),
            Vec3::new(x + w, y + 1.0, z + h),
            Vec3::new(x + w, y + 1.0, z),
        ],
        Face::NegativeZ => [
            Vec3::new(x + w, y, z),
            Vec3::new(x, y, z),
            Vec3::new(x, y + h, z),
            Vec3::new(x + w, y + h, z),
        ],
        Face::PositiveZ => [
            Vec3::new(x, y, z + 1.0),
            Vec3::new(x + w, y, z + 1.0),
            Vec3::new(x + w, y + h, z + 1.0),
            Vec3::new(x, y + h, z + 1.0),
        ],
    };
    let first = mesh.vertices.len() as u32;
    for point in points {
        let uv = face_uv(face, point, x, y, z, w, h);
        mesh.vertices.push(Vertex {
            position: point,
            normal: face.normal(),
            block,
            texture: None,
            uv,
        });
    }
    mesh.indices
        .extend_from_slice(&[first, first + 1, first + 2, first, first + 2, first + 3]);
}

fn face_uv(face: Face, point: Vec3, x: f32, y: f32, z: f32, width: f32, height: f32) -> [f32; 2] {
    match face {
        Face::NegativeX | Face::PositiveX => {
            [(point.z - z).abs().min(width), height - (point.y - y)]
        }
        Face::NegativeY | Face::PositiveY => [
            (point.x - x).abs().min(width),
            (point.z - z).abs().min(height),
        ],
        Face::NegativeZ | Face::PositiveZ => {
            [(point.x - x).abs().min(width), height - (point.y - y)]
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirectionalLight {
    pub direction: Vec3,
    pub ambient: f32,
    pub intensity: f32,
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            direction: Vec3::new(-0.4, -1.0, -0.3).normalized(),
            ambient: 0.28,
            intensity: 0.72,
        }
    }
}

impl DirectionalLight {
    pub fn brightness(self, normal: Vec3) -> f32 {
        (self.ambient
            + self.intensity
                * normal
                    .normalized()
                    .dot(self.direction.normalized() * -1.0)
                    .max(0.0))
        .clamp(0.0, 1.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RgbImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[u8; 3]>,
}

impl RgbImage {
    pub fn new(width: u32, height: u32, color: [u8; 3]) -> Result<Self, &'static str> {
        let length = (width as usize)
            .checked_mul(height as usize)
            .ok_or("image dimensions overflow")?;
        if width == 0 || height == 0 {
            return Err("image dimensions must be nonzero");
        }
        Ok(Self {
            width,
            height,
            pixels: vec![color; length],
        })
    }
    pub fn write_ppm<W: io::Write>(&self, mut out: W) -> io::Result<()> {
        write!(out, "P6\n{} {}\n255\n", self.width, self.height)?;
        for pixel in &self.pixels {
            out.write_all(pixel)?;
        }
        Ok(())
    }
}

/// Reads the simple original P6 RGB resource format. Comments and arbitrary whitespace
/// are accepted in the header; image payloads are exactly width*height*3 bytes.
pub fn load_ppm<R: Read>(mut input: R) -> io::Result<RgbImage> {
    let mut bytes = Vec::new();
    input.read_to_end(&mut bytes)?;
    let mut cursor = 0;
    let magic = ppm_token(&bytes, &mut cursor)?;
    if magic != b"P6" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "expected binary P6 RGB image",
        ));
    }
    let width = parse_dimension(ppm_token(&bytes, &mut cursor)?)?;
    let height = parse_dimension(ppm_token(&bytes, &mut cursor)?)?;
    let max = ppm_token(&bytes, &mut cursor)?;
    if max != b"255" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PPM channel maximum must be 255",
        ));
    }
    if cursor >= bytes.len() || !bytes[cursor].is_ascii_whitespace() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing PPM payload separator",
        ));
    }
    if bytes[cursor] == b'\r' && bytes.get(cursor + 1) == Some(&b'\n') {
        cursor += 2;
    } else {
        cursor += 1;
    }
    let count = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(3))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "PPM dimensions overflow"))?;
    if bytes.len() - cursor != count {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PPM payload length does not match dimensions",
        ));
    }
    let pixels = bytes[cursor..]
        .chunks_exact(3)
        .map(|p| [p[0], p[1], p[2]])
        .collect();
    Ok(RgbImage {
        width,
        height,
        pixels,
    })
}

/// Loads a local PNG into the renderer's RGB texture format. Callers control
/// the path; the renderer performs no network access or asset discovery.
pub fn load_png(path: impl AsRef<std::path::Path>) -> io::Result<RgbImage> {
    let path = path.as_ref();
    let image = ImageReader::open(path)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
        .decode()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
        .to_rgba8();
    Ok(RgbImage {
        width: image.width(),
        height: image.height(),
        pixels: image
            .pixels()
            .map(|pixel| {
                let alpha = pixel[3] as u16;
                [
                    ((pixel[0] as u16 * alpha + 255 * (255 - alpha)) / 255) as u8,
                    ((pixel[1] as u16 * alpha + 255 * (255 - alpha)) / 255) as u8,
                    ((pixel[2] as u16 * alpha + 255 * (255 - alpha)) / 255) as u8,
                ]
            })
            .collect(),
    })
}

fn ppm_token<'a>(bytes: &'a [u8], cursor: &mut usize) -> io::Result<&'a [u8]> {
    loop {
        while *cursor < bytes.len() && bytes[*cursor].is_ascii_whitespace() {
            *cursor += 1;
        }
        if bytes.get(*cursor) != Some(&b'#') {
            break;
        }
        while *cursor < bytes.len() && bytes[*cursor] != b'\n' {
            *cursor += 1;
        }
    }
    let start = *cursor;
    while *cursor < bytes.len() && !bytes[*cursor].is_ascii_whitespace() && bytes[*cursor] != b'#' {
        *cursor += 1;
    }
    if start == *cursor {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "incomplete PPM header",
        ));
    }
    Ok(&bytes[start..*cursor])
}
fn parse_dimension(token: &[u8]) -> io::Result<u32> {
    let text = std::str::from_utf8(token)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid PPM dimension"))?;
    text.parse::<u32>()
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid PPM dimension"))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    pub color: [u8; 3],
    pub texture: Option<u16>,
}

#[derive(Clone, Debug)]
pub struct MaterialRegistry {
    entries: Vec<Material>,
    fallback: Material,
    textures: Vec<RgbImage>,
}

impl Default for MaterialRegistry {
    fn default() -> Self {
        Self {
            entries: vec![Material {
                color: [0, 0, 0],
                texture: None,
            }],
            fallback: Material {
                color: [196, 72, 48],
                texture: None,
            },
            textures: Vec::new(),
        }
    }
}

impl MaterialRegistry {
    pub fn register(&mut self, id: BlockId, material: Material) -> Result<(), &'static str> {
        let index = id.0 as usize;
        if index > 65_535 {
            return Err("material id is outside registry bounds");
        }
        self.entries.resize(index + 1, self.fallback);
        self.entries[index] = material;
        Ok(())
    }
    pub fn get(&self, id: BlockId) -> Material {
        self.entries
            .get(id.0 as usize)
            .copied()
            .unwrap_or(self.fallback)
    }
    pub fn register_texture(&mut self, image: RgbImage) -> Result<u16, &'static str> {
        let id = u16::try_from(self.textures.len()).map_err(|_| "texture registry is full")?;
        self.textures.push(image);
        Ok(id)
    }

    pub fn texture(&self, id: u16) -> Option<&RgbImage> {
        self.textures.get(id as usize)
    }

    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }
    fn sample(&self, material: Material, uv: [f32; 2]) -> [u8; 3] {
        let Some(texture_id) = material.texture else {
            return material.color;
        };
        let Some(texture) = self.textures.get(texture_id as usize) else {
            return material.color;
        };
        let u = uv[0].rem_euclid(1.0);
        let v = uv[1].rem_euclid(1.0);
        let x = ((u * texture.width as f32) as u32).min(texture.width - 1);
        let y = ((v * texture.height as f32) as u32).min(texture.height - 1);
        texture.pixels[(y * texture.width + x) as usize]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub color: Vec<[u8; 3]>,
    pub depth: Vec<f32>,
}

impl Frame {
    pub fn new(width: u32, height: u32, clear: [u8; 3]) -> Result<Self, &'static str> {
        let count = (width as usize)
            .checked_mul(height as usize)
            .ok_or("frame dimensions overflow")?;
        if width == 0 || height == 0 {
            return Err("frame dimensions must be nonzero");
        }
        Ok(Self {
            width,
            height,
            color: vec![clear; count],
            depth: vec![f32::INFINITY; count],
        })
    }
    pub fn image(&self) -> RgbImage {
        RgbImage {
            width: self.width,
            height: self.height,
            pixels: self.color.clone(),
        }
    }
    pub fn byte_len(&self) -> usize {
        self.color.len() * std::mem::size_of::<[u8; 3]>()
            + self.depth.len() * std::mem::size_of::<f32>()
    }
}

pub trait Renderer {
    fn render(
        &self,
        meshes: &[&Mesh],
        camera: &Camera,
        materials: &MaterialRegistry,
        light: DirectionalLight,
        width: u32,
        height: u32,
    ) -> Result<Frame, &'static str>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CpuRenderer {
    pub clear_color: [u8; 3],
}

impl Renderer for CpuRenderer {
    fn render(
        &self,
        meshes: &[&Mesh],
        camera: &Camera,
        materials: &MaterialRegistry,
        light: DirectionalLight,
        width: u32,
        height: u32,
    ) -> Result<Frame, &'static str> {
        let mut frame = Frame::new(width, height, self.clear_color)?;
        let matrix = camera.view_projection(width as f32 / height as f32)?;
        for mesh in meshes {
            if !mesh_visible(mesh, matrix) {
                continue;
            }
            for triangle in mesh.indices.chunks_exact(3) {
                let vertices = [
                    mesh.vertices[triangle[0] as usize],
                    mesh.vertices[triangle[1] as usize],
                    mesh.vertices[triangle[2] as usize],
                ];
                let clip = vertices.map(|vertex| ClipVertex {
                    vertex,
                    clip: matrix.transform([
                        vertex.position.x,
                        vertex.position.y,
                        vertex.position.z,
                        1.0,
                    ]),
                });
                let polygon = clip_near_plane(&clip);
                for triangle_index in 1..polygon.len().saturating_sub(1) {
                    let clipped = [
                        polygon[0],
                        polygon[triangle_index],
                        polygon[triangle_index + 1],
                    ];
                    let screen = clipped.map(|value| {
                        let inv_w = 1.0 / value.clip[3];
                        [
                            (value.clip[0] * inv_w * 0.5 + 0.5) * width as f32,
                            (0.5 - value.clip[1] * inv_w * 0.5) * height as f32,
                            value.clip[2] * inv_w,
                        ]
                    });
                    raster_triangle(
                        &mut frame,
                        screen,
                        clipped.map(|value| value.vertex),
                        clipped.map(|value| 1.0 / value.clip[3]),
                        materials,
                        light,
                    );
                }
            }
        }
        Ok(frame)
    }
}

#[derive(Clone, Copy)]
struct ClipVertex {
    vertex: Vertex,
    clip: [f32; 4],
}

fn clip_near_plane(input: &[ClipVertex; 3]) -> Vec<ClipVertex> {
    const EPSILON: f32 = 0.001;
    let mut output = Vec::with_capacity(4);
    for index in 0..input.len() {
        let current = input[index];
        let previous = input[(index + input.len() - 1) % input.len()];
        let current_inside = current.clip[3] >= EPSILON;
        let previous_inside = previous.clip[3] >= EPSILON;
        if current_inside != previous_inside {
            let amount = (EPSILON - previous.clip[3]) / (current.clip[3] - previous.clip[3]);
            output.push(interpolate_clip(previous, current, amount));
        }
        if current_inside {
            output.push(current);
        }
    }
    output
}

fn interpolate_clip(a: ClipVertex, b: ClipVertex, amount: f32) -> ClipVertex {
    let lerp = |x: f32, y: f32| x + (y - x) * amount;
    ClipVertex {
        vertex: Vertex {
            position: a.vertex.position + (b.vertex.position - a.vertex.position) * amount,
            normal: (a.vertex.normal + (b.vertex.normal - a.vertex.normal) * amount).normalized(),
            block: a.vertex.block,
            texture: a.vertex.texture,
            uv: [
                lerp(a.vertex.uv[0], b.vertex.uv[0]),
                lerp(a.vertex.uv[1], b.vertex.uv[1]),
            ],
        },
        clip: std::array::from_fn(|index| lerp(a.clip[index], b.clip[index])),
    }
}

fn mesh_visible(mesh: &Mesh, matrix: Mat4) -> bool {
    if mesh.vertices.is_empty() {
        return false;
    }
    let min = mesh.bounds_min;
    let max = mesh.bounds_max;
    let mut corners = Vec::with_capacity(8);
    for &x in &[min.x, max.x] {
        for &y in &[min.y, max.y] {
            for &z in &[min.z, max.z] {
                corners.push(matrix.transform([x, y, z, 1.0]));
            }
        }
    }
    if corners
        .iter()
        .any(|corner| !corner.iter().all(|value| value.is_finite()))
    {
        return false;
    }
    // A box intersecting the near plane is conservatively retained.
    if corners.iter().any(|corner| corner[3] <= 0.0) {
        return true;
    }
    for plane in 0..3 {
        let outside_negative = corners.iter().all(|corner| corner[plane] < -corner[3]);
        let outside_positive = corners.iter().all(|corner| corner[plane] > corner[3]);
        if outside_negative || outside_positive {
            return false;
        }
    }
    true
}

fn raster_triangle(
    frame: &mut Frame,
    p: [[f32; 3]; 3],
    v: [Vertex; 3],
    inv_w: [f32; 3],
    materials: &MaterialRegistry,
    light: DirectionalLight,
) {
    let min_x = p.iter().map(|v| v[0].floor() as i32).min().unwrap().max(0);
    let max_x = p
        .iter()
        .map(|v| v[0].ceil() as i32)
        .max()
        .unwrap()
        .min(frame.width as i32 - 1);
    let min_y = p.iter().map(|v| v[1].floor() as i32).min().unwrap().max(0);
    let max_y = p
        .iter()
        .map(|v| v[1].ceil() as i32)
        .max()
        .unwrap()
        .min(frame.height as i32 - 1);
    let edge = |a: [f32; 3], b: [f32; 3], x: f32, y: f32| {
        (x - a[0]) * (b[1] - a[1]) - (y - a[1]) * (b[0] - a[0])
    };
    let area = edge(p[0], p[1], p[2][0], p[2][1]);
    if area.abs() < 1e-6 {
        return;
    }
    let normal = (v[1].position - v[0].position)
        .cross(v[2].position - v[0].position)
        .normalized();
    let intensity = light.brightness(normal);
    let mut material = materials.get(v[0].block);
    if let Some(texture) = v[0].texture {
        material.texture = Some(texture);
    }
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = edge(p[1], p[2], px, py) / area;
            let w1 = edge(p[2], p[0], px, py) / area;
            let w2 = edge(p[0], p[1], px, py) / area;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let depth = w0 * p[0][2] + w1 * p[1][2] + w2 * p[2][2];
            let reciprocal_w = w0 * inv_w[0] + w1 * inv_w[1] + w2 * inv_w[2];
            let uv = [
                (w0 * v[0].uv[0] * inv_w[0]
                    + w1 * v[1].uv[0] * inv_w[1]
                    + w2 * v[2].uv[0] * inv_w[2])
                    / reciprocal_w,
                (w0 * v[0].uv[1] * inv_w[0]
                    + w1 * v[1].uv[1] * inv_w[1]
                    + w2 * v[2].uv[1] * inv_w[2])
                    / reciprocal_w,
            ];
            let index = y as usize * frame.width as usize + x as usize;
            if depth >= 0.0 && depth < frame.depth[index] {
                frame.depth[index] = depth;
                frame.color[index] = materials
                    .sample(material, uv)
                    .map(|c| (c as f32 * intensity).round().clamp(0.0, 255.0) as u8);
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FramePacer {
    target: Duration,
    previous: Instant,
}

impl FramePacer {
    pub fn new(target_hz: u32) -> Result<Self, &'static str> {
        if target_hz == 0 {
            return Err("target frame rate must be nonzero");
        }
        Ok(Self {
            target: Duration::from_secs_f64(1.0 / target_hz as f64),
            previous: Instant::now(),
        })
    }
    pub fn next_frame(&mut self) -> Duration {
        let elapsed = self.previous.elapsed();
        if elapsed < self.target {
            std::thread::sleep(self.target - elapsed);
        }
        let now = Instant::now();
        let delta = now.duration_since(self.previous);
        self.previous = now;
        delta
    }
}

#[derive(Clone, Debug, Default)]
pub struct FrameProfiler {
    samples: Vec<Duration>,
    capacity: usize,
}

impl FrameProfiler {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity),
            capacity,
        }
    }
    pub fn record(&mut self, duration: Duration) {
        if self.capacity == 0 {
            return;
        }
        if self.samples.len() == self.capacity {
            self.samples.remove(0);
        }
        self.samples.push(duration);
    }
    pub fn summary(&self) -> ProfileSummary {
        if self.samples.is_empty() {
            return ProfileSummary::default();
        }
        let mut sorted = self.samples.clone();
        sorted.sort_unstable();
        let total: Duration = sorted.iter().copied().sum();
        let percentile = |p: f32| sorted[((sorted.len() - 1) as f32 * p).ceil() as usize];
        ProfileSummary {
            samples: sorted.len(),
            mean: total / sorted.len() as u32,
            p50: percentile(0.50),
            p95: percentile(0.95),
            max: *sorted.last().unwrap(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ProfileSummary {
    pub samples: usize,
    pub mean: Duration,
    pub p50: Duration,
    pub p95: Duration,
    pub max: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_cube_mesh_has_six_faces_and_expected_bounds() {
        let mesh = cube_mesh([3.0, 4.0, 5.0], 0.125, BlockId(2));
        assert_eq!(mesh.vertices.len(), 24);
        assert_eq!(mesh.indices.len(), 36);
        assert_eq!(mesh.triangle_count(), 12);
        assert_eq!(mesh.bounds_min, Vec3::new(2.875, 3.875, 4.875));
        assert_eq!(mesh.bounds_max, Vec3::new(3.125, 4.125, 5.125));
        assert!(mesh
            .vertices
            .iter()
            .all(|vertex| vertex.block == BlockId(2)));
        assert!(cube_mesh([f64::NAN, 0.0, 0.0], 0.125, BlockId(2)).is_empty());
    }

    #[test]
    fn camera_rejects_invalid_projection_and_builds_view_projection() {
        let camera = Camera::default();
        assert!(camera.view_projection(0.0).is_err());
        assert!(camera.view_projection(16.0 / 9.0).is_ok());
        assert!(Mat4::look_at(Vec3::ZERO, Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0)).is_err());
    }

    #[test]
    fn input_moves_camera_without_changing_its_view_direction() {
        let mut camera = Camera::default();
        let direction = camera.target - camera.position;
        let mut input = InputState::default();
        input.set(InputKey::Forward, true);
        input.set(InputKey::Right, true);
        input.move_camera(&mut camera, 0.5, 4.0);
        assert!(((camera.target - camera.position).length() - direction.length()).abs() < 1e-5);
        assert!(camera.position != Camera::default().position);
        input.set(InputKey::Forward, false);
        assert!(!input.is_pressed(InputKey::Forward));
    }

    #[test]
    fn clearing_input_releases_all_held_actions() {
        let mut input = InputState::default();
        input.set(InputKey::Forward, true);
        input.set(InputKey::Sprint, true);
        input.clear();
        assert!(!input.is_pressed(InputKey::Forward));
        assert!(!input.is_pressed(InputKey::Sprint));
    }

    #[test]
    fn mesher_culls_internal_faces_and_keeps_chunk_boundary_faces() {
        let mut chunk = Chunk::empty();
        chunk.set(1, 1, 1, BlockId(1)).unwrap();
        chunk.set(2, 1, 1, BlockId(2)).unwrap();
        let mesh = mesh_chunk(&chunk, |x, y, z| {
            x >= 0
                && y >= 0
                && z >= 0
                && x < CHUNK_SIZE as i32
                && y < CHUNK_SIZE as i32
                && z < CHUNK_SIZE as i32
                && chunk.get(x as usize, y as usize, z as usize).unwrap() != BlockId(0)
        });
        assert_eq!(mesh.triangle_count(), 20);
        assert_eq!(mesh.vertices.len(), 40);
    }

    #[test]
    fn greedy_mesher_merges_a_flat_surface() {
        let mut chunk = Chunk::empty();
        for z in 0..4 {
            for x in 0..4 {
                chunk.set(x, 0, z, BlockId(1)).unwrap();
            }
        }
        let mesh = greedy_mesh_chunk(&chunk, |x, y, z| {
            x >= 0 && y >= 0 && z >= 0 && x < 4 && y < 1 && z < 4
        });
        assert!(mesh.triangle_count() < 4 * 4 * 6);
        assert_eq!(mesh.triangle_count(), 6 * 2);
        assert_eq!(mesh.vertices[0].block, BlockId(1));
    }

    #[test]
    fn shape_mesher_uses_state_geometry_instead_of_a_full_cube() {
        let mut chunk = Chunk::empty();
        chunk.set(1, 2, 3, BlockId(4)).unwrap();
        chunk.set(3, 2, 3, BlockId(6)).unwrap();
        let six_snow_layers = crate::blocks::BlockState {
            kind: crate::blocks::BlockKind::Snow,
            upper: false,
            layers: 6,
            connections: 0,
        };
        chunk.set_state(3, 2, 3, six_snow_layers).unwrap();
        let registry = crate::blocks::BlockRegistry::new();
        let mesh = mesh_chunk_with_shapes(
            &chunk,
            |_, _, _| false,
            |x, y, z| {
                if !(0..CHUNK_SIZE as i32).contains(&x)
                    || !(0..CHUNK_SIZE as i32).contains(&y)
                    || !(0..CHUNK_SIZE as i32).contains(&z)
                {
                    return crate::blocks::VoxelShape::EMPTY.boxes();
                }
                let block = chunk.get(x as usize, y as usize, z as usize).unwrap();
                let state = chunk.get_state(x as usize, y as usize, z as usize).unwrap();
                registry.visual_boxes(block, state)
            },
        );

        assert_eq!(mesh.triangle_count(), 24);
        assert_eq!(mesh.bounds_min, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(mesh.bounds_max, Vec3::new(4.0, 2.75, 4.0));
    }

    #[test]
    fn shape_mesher_removes_shared_faces_between_equal_partial_blocks() {
        let mut chunk = Chunk::empty();
        chunk.set(0, 0, 0, BlockId(4)).unwrap();
        chunk.set(1, 0, 0, BlockId(4)).unwrap();
        let registry = crate::blocks::BlockRegistry::new();
        let mesh = mesh_chunk_with_shapes(
            &chunk,
            |_, _, _| false,
            |x, y, z| {
                if !(0..CHUNK_SIZE as i32).contains(&x)
                    || !(0..CHUNK_SIZE as i32).contains(&y)
                    || !(0..CHUNK_SIZE as i32).contains(&z)
                {
                    return crate::blocks::VoxelShape::EMPTY.boxes();
                }
                let block = chunk.get(x as usize, y as usize, z as usize).unwrap();
                registry.visual_boxes(
                    block,
                    chunk.get_state(x as usize, y as usize, z as usize).unwrap(),
                )
            },
        );

        assert_eq!(mesh.triangle_count(), 20);
    }

    #[test]
    fn shape_mesher_splits_a_partial_face_at_the_occluder_edge() {
        let mut chunk = Chunk::empty();
        chunk.set(0, 0, 0, BlockId(6)).unwrap();
        chunk.set(1, 0, 0, BlockId(6)).unwrap();
        chunk
            .set_state(
                0,
                0,
                0,
                crate::blocks::BlockState {
                    kind: crate::blocks::BlockKind::Snow,
                    upper: false,
                    layers: 2,
                    connections: 0,
                },
            )
            .unwrap();
        chunk
            .set_state(
                1,
                0,
                0,
                crate::blocks::BlockState {
                    kind: crate::blocks::BlockKind::Snow,
                    upper: false,
                    layers: 6,
                    connections: 0,
                },
            )
            .unwrap();
        let registry = crate::blocks::BlockRegistry::new();
        let mesh = mesh_chunk_with_shapes(
            &chunk,
            |_, _, _| false,
            |x, y, z| {
                if !(0..CHUNK_SIZE as i32).contains(&x)
                    || !(0..CHUNK_SIZE as i32).contains(&y)
                    || !(0..CHUNK_SIZE as i32).contains(&z)
                {
                    return crate::blocks::VoxelShape::EMPTY.boxes();
                }
                let block = chunk.get(x as usize, y as usize, z as usize).unwrap();
                registry.visual_boxes(
                    block,
                    chunk.get_state(x as usize, y as usize, z as usize).unwrap(),
                )
            },
        );

        assert_eq!(mesh.triangle_count(), 22);
    }

    #[test]
    fn shape_mesher_splits_full_cube_faces_around_partial_neighbors() {
        let mut chunk = Chunk::empty();
        chunk.set(0, 0, 0, BlockId(1)).unwrap();
        chunk.set(1, 0, 0, BlockId(4)).unwrap();
        let registry = crate::blocks::BlockRegistry::new();
        let mesh = mesh_chunk_with_shapes(
            &chunk,
            |x, y, z| {
                if !(0..CHUNK_SIZE as i32).contains(&x)
                    || !(0..CHUNK_SIZE as i32).contains(&y)
                    || !(0..CHUNK_SIZE as i32).contains(&z)
                {
                    return false;
                }
                let block = chunk.get(x as usize, y as usize, z as usize).unwrap();
                registry.is_full_cube(
                    block,
                    chunk.get_state(x as usize, y as usize, z as usize).unwrap(),
                )
            },
            |x, y, z| {
                if !(0..CHUNK_SIZE as i32).contains(&x)
                    || !(0..CHUNK_SIZE as i32).contains(&y)
                    || !(0..CHUNK_SIZE as i32).contains(&z)
                {
                    return crate::blocks::VoxelShape::EMPTY.boxes();
                }
                let block = chunk.get(x as usize, y as usize, z as usize).unwrap();
                registry.visual_boxes(
                    block,
                    chunk.get_state(x as usize, y as usize, z as usize).unwrap(),
                )
            },
        );

        let full_block_east_face: Vec<_> = mesh
            .vertices
            .iter()
            .filter(|vertex| {
                vertex.normal.x > 0.5 && (vertex.position.x - 1.0).abs() <= f32::EPSILON
            })
            .collect();
        assert_eq!(full_block_east_face.len(), 4);
        assert!(full_block_east_face
            .iter()
            .all(|vertex| vertex.position.y >= 0.5));
    }

    #[test]
    fn camera_fov_is_explicit_and_configurable() {
        let mut camera = Camera::default();
        assert_eq!(camera.fov_degrees, 60.0);
        camera.fov_degrees = 90.0;
        assert!(camera.view_projection(16.0 / 9.0).is_ok());
    }

    #[test]
    fn ppm_resources_validate_header_and_payload() {
        let image =
            load_ppm(&b"P6\n# authored test texture\n2 1\n255\n\x01\x02\x03\x04\x05\x06"[..])
                .unwrap();
        assert_eq!(image.pixels, vec![[1, 2, 3], [4, 5, 6]]);
        assert!(load_ppm(&b"P6\n2 2\n255\n\0"[..]).is_err());
    }

    #[test]
    fn renderer_writes_visible_pixels_and_depth() {
        let mut chunk = Chunk::empty();
        chunk.set(0, 0, 0, BlockId(1)).unwrap();
        let mesh = mesh_chunk(&chunk, |_, _, _| false);
        let camera = Camera {
            position: Vec3::new(2.0, 2.0, 3.0),
            target: Vec3::new(0.5, 0.5, 0.5),
            ..Camera::default()
        };
        let mut materials = MaterialRegistry::default();
        materials
            .register(
                BlockId(1),
                Material {
                    color: [240, 40, 20],
                    texture: None,
                },
            )
            .unwrap();
        let frame = CpuRenderer {
            clear_color: [5, 7, 11],
        }
        .render(
            &[&mesh],
            &camera,
            &materials,
            DirectionalLight::default(),
            96,
            64,
        )
        .unwrap();
        assert!(frame.color.iter().any(|pixel| *pixel != [5, 7, 11]));
        assert!(frame.depth.iter().any(|depth| depth.is_finite()));
    }

    #[test]
    fn renderer_samples_registered_block_texture() {
        let mut chunk = Chunk::empty();
        chunk.set(0, 0, 0, BlockId(1)).unwrap();
        let mesh = mesh_chunk(&chunk, |_, _, _| false);
        let camera = Camera {
            position: Vec3::new(2.0, 2.0, 3.0),
            target: Vec3::new(0.5, 0.5, 0.5),
            ..Camera::default()
        };
        let mut materials = MaterialRegistry::default();
        let texture = materials
            .register_texture(RgbImage {
                width: 1,
                height: 1,
                pixels: vec![[30, 210, 90]],
            })
            .unwrap();
        materials
            .register(
                BlockId(1),
                Material {
                    color: [240, 40, 20],
                    texture: Some(texture),
                },
            )
            .unwrap();
        let frame = CpuRenderer {
            clear_color: [5, 7, 11],
        }
        .render(
            &[&mesh],
            &camera,
            &materials,
            DirectionalLight {
                direction: Vec3::new(1.0, 1.0, 1.0),
                ambient: 1.0,
                intensity: 0.0,
            },
            96,
            64,
        )
        .unwrap();
        assert!(frame.color.iter().any(|pixel| *pixel == [30, 210, 90]));
    }

    #[test]
    fn frame_profiler_tracks_a_bounded_window() {
        let mut profiler = FrameProfiler::new(2);
        profiler.record(Duration::from_millis(1));
        profiler.record(Duration::from_millis(2));
        profiler.record(Duration::from_millis(3));
        let summary = profiler.summary();
        assert_eq!(summary.samples, 2);
        assert_eq!(summary.mean, Duration::from_micros(2_500));
        assert_eq!(summary.max, Duration::from_millis(3));
    }
}
