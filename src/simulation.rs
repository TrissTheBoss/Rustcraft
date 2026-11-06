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
    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }

    /// Integrates bodies in stable EntityId order and clamps their feet to y=0.
    pub fn tick(&mut self, seconds: f64) {
        if !seconds.is_finite() || seconds <= 0.0 {
            return;
        }
        for body in self.bodies.values_mut() {
            body.velocity.y += self.gravity * seconds;
            body.entity.transform.x += body.velocity.x * seconds;
            body.entity.transform.y += body.velocity.y * seconds;
            body.entity.transform.z += body.velocity.z * seconds;
            let floor = body.half_extents[1].max(0.0);
            if body.entity.transform.y <= floor {
                body.entity.transform.y = floor;
                body.velocity.y = 0.0;
                body.grounded = true;
            } else {
                body.grounded = false;
            }
        }
    }
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
        assert_eq!(result.entity.transform.y, 1.0);
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
}
