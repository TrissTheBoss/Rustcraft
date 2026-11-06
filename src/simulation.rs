//! Deterministic fixed-step simulation primitives for the original engine.

use std::collections::BTreeMap;

use crate::entity::{Entity, EntityId, Transform};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Velocity {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Default for Velocity {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub entity: Entity,
    pub velocity: Velocity,
    pub half_extents: [f64; 3],
    pub grounded: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FixedClock {
    step: f64,
    accumulator: f64,
}

impl FixedClock {
    pub fn new(step_seconds: f64) -> Result<Self, &'static str> {
        if !step_seconds.is_finite() || step_seconds <= 0.0 {
            return Err("fixed simulation step must be finite and positive");
        }
        Ok(Self {
            step: step_seconds,
            accumulator: 0.0,
        })
    }

    pub fn step_seconds(&self) -> f64 {
        self.step
    }

    pub fn advance(&mut self, elapsed_seconds: f64, mut tick: impl FnMut(f64)) -> usize {
        if !elapsed_seconds.is_finite() || elapsed_seconds < 0.0 {
            return 0;
        }
        self.accumulator += elapsed_seconds.min(self.step * 8.0);
        let mut ticks = 0;
        while self.accumulator >= self.step {
            self.accumulator -= self.step;
            tick(self.step);
            ticks += 1;
        }
        ticks
    }
}

#[derive(Debug)]
pub struct Simulation {
    pub gravity: f64,
    next_id: u64,
    bodies: BTreeMap<EntityId, Body>,
}

impl Default for Simulation {
    fn default() -> Self {
        Self {
            gravity: -9.81,
            next_id: 1,
            bodies: BTreeMap::new(),
        }
    }
}

impl Simulation {
    pub fn spawn(&mut self, transform: Transform, half_extents: [f64; 3]) -> EntityId {
        let id = EntityId(self.next_id);
        self.next_id += 1;
        self.bodies.insert(
            id,
            Body {
                entity: Entity { id, transform },
                velocity: Velocity::default(),
                half_extents,
                grounded: false,
            },
        );
        id
    }

    pub fn despawn(&mut self, id: EntityId) -> Option<Body> {
        self.bodies.remove(&id)
    }
    pub fn body(&self, id: EntityId) -> Option<&Body> {
        self.bodies.get(&id)
    }
    pub fn body_mut(&mut self, id: EntityId) -> Option<&mut Body> {
        self.bodies.get_mut(&id)
    }
    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }

    /// Integrates bodies in stable EntityId order and clamps their feet to y=0.
    pub fn tick(&mut self, seconds: f64) {
        self.tick_with_collision(seconds, |_, y, _| y < 0);
    }

    /// Integrates bodies against a deterministic voxel occupancy callback.
    /// Solid cells are unit cubes addressed by integer world coordinates.
    pub fn tick_with_collision(
        &mut self,
        seconds: f64,
        mut is_solid: impl FnMut(i32, i32, i32) -> bool,
    ) {
        if !seconds.is_finite() || seconds <= 0.0 {
            return;
        }
        for body in self.bodies.values_mut() {
            body.velocity.y += self.gravity * seconds;
            body.grounded = false;
            move_axis(body, 0, body.velocity.x * seconds, &mut is_solid);
            move_axis(body, 2, body.velocity.z * seconds, &mut is_solid);
            let before_y = body.entity.transform.y;
            move_axis(body, 1, body.velocity.y * seconds, &mut is_solid);
            if body.entity.transform.y == before_y && body.velocity.y <= 0.0 {
                body.grounded = true;
            }
            if body.entity.transform.y < body.half_extents[1].max(0.0) {
                body.entity.transform.y = body.half_extents[1].max(0.0);
                body.velocity.y = 0.0;
                body.grounded = true;
            }
        }
    }

    pub fn snapshot(&self) -> Vec<(EntityId, Transform, Velocity)> {
        self.bodies
            .values()
            .map(|body| (body.entity.id, body.entity.transform, body.velocity))
            .collect()
    }
}

fn move_axis(
    body: &mut Body,
    axis: usize,
    delta: f64,
    is_solid: &mut impl FnMut(i32, i32, i32) -> bool,
) {
    if delta == 0.0 {
        return;
    }
    let mut candidate = body.entity.transform;
    match axis {
        0 => candidate.x += delta,
        1 => candidate.y += delta,
        _ => candidate.z += delta,
    }
    if !collides(candidate, body.half_extents, is_solid) {
        body.entity.transform = candidate;
        return;
    }
    if axis == 1 {
        body.velocity.y = 0.0;
    }
}

fn collides(
    center: Transform,
    half: [f64; 3],
    is_solid: &mut impl FnMut(i32, i32, i32) -> bool,
) -> bool {
    let min_x = (center.x - half[0]).floor() as i32;
    let max_x = (center.x + half[0]).ceil() as i32;
    let min_y = (center.y - half[1]).floor() as i32;
    let max_y = (center.y + half[1]).ceil() as i32;
    let min_z = (center.z - half[2]).floor() as i32;
    let max_z = (center.z + half[2]).ceil() as i32;
    (min_x..max_x).any(|x| (min_y..max_y).any(|y| (min_z..max_z).any(|z| is_solid(x, y, z))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_emits_only_complete_fixed_steps() {
        let mut clock = FixedClock::new(0.1).unwrap();
        let mut ticks = 0;
        assert_eq!(clock.advance(0.25, |_| ticks += 1), 2);
        assert_eq!(ticks, 2);
        assert_eq!(clock.advance(0.04, |_| ticks += 1), 0);
        assert_eq!(clock.advance(0.06, |_| ticks += 1), 1);
    }

    #[test]
    fn gravity_is_deterministic_and_stops_at_floor() {
        let mut first = Simulation::default();
        let id = first.spawn(
            Transform {
                x: 0.0,
                y: 10.0,
                z: 0.0,
            },
            [0.5, 1.0, 0.5],
        );
        for _ in 0..120 {
            first.tick(1.0 / 60.0);
        }
        let result = first.body(id).unwrap();
        assert!((result.entity.transform.y - 1.0).abs() < 0.01);
        assert!(result.grounded);
        assert_eq!(result.velocity.y, 0.0);
    }

    #[test]
    fn lifecycle_is_explicit() {
        let mut simulation = Simulation::default();
        let id = simulation.spawn(Transform::default(), [0.5; 3]);
        assert_eq!(simulation.body_count(), 1);
        assert!(simulation.despawn(id).is_some());
        assert_eq!(simulation.body_count(), 0);
    }

    #[test]
    fn voxel_collision_stops_downward_motion() {
        let mut simulation = Simulation::default();
        let id = simulation.spawn(
            Transform {
                x: 0.5,
                y: 3.0,
                z: 0.5,
            },
            [0.4, 0.5, 0.4],
        );
        for _ in 0..120 {
            simulation.tick_with_collision(1.0 / 60.0, |x, y, z| y == 0 && x == 0 && z == 0);
        }
        let body = simulation.body(id).unwrap();
        assert!((body.entity.transform.y - 1.5).abs() < 0.01);
        assert!(body.grounded);
    }

    #[test]
    fn replay_snapshot_is_identical_for_identical_inputs() {
        let run = || {
            let mut simulation = Simulation::default();
            let id = simulation.spawn(
                Transform {
                    x: 0.0,
                    y: 4.0,
                    z: 0.0,
                },
                [0.5; 3],
            );
            for step in 0..180 {
                simulation.body_mut(id).unwrap().velocity.x = if step < 60 { 2.0 } else { 0.0 };
                simulation.tick(1.0 / 60.0);
            }
            simulation.snapshot()
        };
        assert_eq!(run(), run());
    }
}
