//! Version-pinned Java 26.3-style player movement primitives.
//!
//! Values are expressed in blocks per game tick (20 ticks per second) and are
//! intentionally centralized so later conformance measurements can adjust them.

use crate::entity::Transform;
use crate::grid::{PLAYER_HALF_HEIGHT, PLAYER_HALF_WIDTH, PLAYER_STEP_HEIGHT};

pub const TICKS_PER_SECOND: f64 = 20.0;
pub const WALK_ACCELERATION: f64 = 0.098;
pub const SPRINT_ACCELERATION: f64 = WALK_ACCELERATION * 1.30;
pub const GROUND_FRICTION: f64 = 0.546;
pub const AIR_FRICTION: f64 = 0.91;
pub const GRAVITY: f64 = 0.08;
pub const JUMP_IMPULSE: f64 = 0.42;
pub const FLIGHT_ACCELERATION: f64 = 0.049;
pub const FLIGHT_SPEED_MULTIPLIER: f64 = 2.0;
pub const STEP_HEIGHT: f64 = PLAYER_STEP_HEIGHT;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameMode {
    Survival,
    Creative,
    Spectator,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MovementInput {
    pub forward: f64,
    pub strafe: f64,
    pub jump: bool,
    pub sneak: bool,
    pub sprint: bool,
    pub toggle_flight: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerController {
    pub transform: Transform,
    pub velocity: [f64; 3],
    pub half_extents: [f64; 3],
    pub mode: GameMode,
    pub flying: bool,
    pub grounded: bool,
}

impl PlayerController {
    pub fn new(transform: Transform) -> Self {
        Self {
            transform,
            velocity: [0.0; 3],
            half_extents: [PLAYER_HALF_WIDTH, PLAYER_HALF_HEIGHT, PLAYER_HALF_WIDTH],
            mode: GameMode::Survival,
            flying: false,
            grounded: false,
        }
    }

    pub fn tick(
        &mut self,
        input: MovementInput,
        yaw_radians: f64,
        is_solid: impl Fn(i32, i32, i32) -> bool,
    ) {
        if input.toggle_flight && self.mode == GameMode::Creative {
            self.flying = !self.flying;
            self.velocity[1] = 0.0;
        }
        let flying = self.flying || self.mode == GameMode::Spectator;
        let mut forward = [yaw_radians.cos(), 0.0, yaw_radians.sin()];
        let mut right = [-forward[2], 0.0, forward[0]];
        let length = (input.forward * input.forward + input.strafe * input.strafe)
            .sqrt()
            .max(1.0);
        forward = [
            forward[0] * input.forward / length,
            0.0,
            forward[2] * input.forward / length,
        ];
        right = [
            right[0] * input.strafe / length,
            0.0,
            right[2] * input.strafe / length,
        ];
        let acceleration = if flying {
            FLIGHT_ACCELERATION
        } else if input.sprint {
            SPRINT_ACCELERATION
        } else {
            WALK_ACCELERATION
        };
        self.velocity[0] += (forward[0] + right[0]) * acceleration;
        self.velocity[2] += (forward[2] + right[2]) * acceleration;
        if flying {
            self.velocity[0] *= AIR_FRICTION;
            self.velocity[2] *= AIR_FRICTION;
            self.velocity[1] = if input.jump {
                acceleration * FLIGHT_SPEED_MULTIPLIER
            } else if input.sneak {
                -acceleration * FLIGHT_SPEED_MULTIPLIER
            } else {
                0.0
            };
        } else {
            if input.jump && self.grounded {
                self.velocity[1] = JUMP_IMPULSE;
                self.grounded = false;
            }
            self.velocity[1] -= GRAVITY;
            let friction = if self.grounded {
                GROUND_FRICTION
            } else {
                AIR_FRICTION
            };
            self.velocity[0] *= friction;
            self.velocity[2] *= friction;
        }
        move_axis(self, 0, &is_solid);
        move_axis(self, 2, &is_solid);
        move_axis(self, 1, &is_solid);
        if self.transform.y < self.half_extents[1] {
            self.transform.y = self.half_extents[1];
            self.velocity[1] = 0.0;
            self.grounded = true;
        }
    }
}

fn move_axis(
    player: &mut PlayerController,
    axis: usize,
    is_solid: &impl Fn(i32, i32, i32) -> bool,
) {
    let mut candidate = player.transform;
    let delta = player.velocity[axis] / TICKS_PER_SECOND;
    match axis {
        0 => candidate.x += delta,
        1 => candidate.y += delta,
        _ => candidate.z += delta,
    }
    if !collides(candidate, player.half_extents, &is_solid) {
        player.transform = candidate;
        return;
    }
    if axis != 1 && (player.grounded || player.velocity[1] > 0.0) {
        let mut stepped = candidate;
        stepped.y += if player.velocity[1] > 0.0 {
            1.0
        } else {
            STEP_HEIGHT
        };
        if !collides(stepped, player.half_extents, &is_solid) {
            player.transform = stepped;
            return;
        }
    }
    player.velocity[axis] = 0.0;
    if axis == 1 && delta < 0.0 {
        player.grounded = true;
    }
}

fn collides(center: Transform, half: [f64; 3], is_solid: &impl Fn(i32, i32, i32) -> bool) -> bool {
    let ranges = [
        (center.x - half[0]).floor() as i32..(center.x + half[0]).ceil() as i32,
        (center.y - half[1]).floor() as i32..(center.y + half[1]).ceil() as i32,
        (center.z - half[2]).floor() as i32..(center.z + half[2]).ceil() as i32,
    ];
    ranges[0].clone().any(|x| {
        ranges[1]
            .clone()
            .any(|y| ranges[2].clone().any(|z| is_solid(x, y, z)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn walking_and_sprinting_have_versioned_acceleration() {
        let mut player = PlayerController::new(Transform {
            x: 0.0,
            y: 2.0,
            z: 0.0,
        });
        player.grounded = true;
        for _ in 0..40 {
            player.tick(
                MovementInput {
                    forward: 1.0,
                    ..Default::default()
                },
                0.0,
                |_, y, _| y < 0,
            );
        }
        let walking = player.transform.x;
        let mut sprinting = PlayerController::new(Transform {
            x: 0.0,
            y: 2.0,
            z: 0.0,
        });
        sprinting.grounded = true;
        for _ in 0..40 {
            sprinting.tick(
                MovementInput {
                    forward: 1.0,
                    sprint: true,
                    ..Default::default()
                },
                0.0,
                |_, y, _| y < 0,
            );
        }
        assert!(sprinting.transform.x > walking);
    }
    #[test]
    fn jump_reaches_air_then_returns_to_ground() {
        let mut player = PlayerController::new(Transform {
            x: 0.0,
            y: 0.9,
            z: 0.0,
        });
        player.grounded = true;
        player.tick(
            MovementInput {
                jump: true,
                ..Default::default()
            },
            0.0,
            |_, y, _| y < 0,
        );
        assert!(player.velocity[1] > 0.0);
        for _ in 0..40 {
            player.tick(Default::default(), 0.0, |_, y, _| y < 0);
        }
        assert!(player.grounded);
    }
    #[test]
    fn creative_flight_ignores_gravity() {
        let mut player = PlayerController::new(Transform {
            x: 0.0,
            y: 10.0,
            z: 0.0,
        });
        player.mode = GameMode::Creative;
        player.tick(
            MovementInput {
                toggle_flight: true,
                ..Default::default()
            },
            0.0,
            |_, _, _| true,
        );
        player.tick(
            MovementInput {
                jump: true,
                ..Default::default()
            },
            0.0,
            |_, _, _| false,
        );
        assert!(player.transform.y > 10.0);
    }

    #[test]
    fn jumping_can_clear_a_single_block_grid_cell() {
        let mut player = PlayerController::new(Transform {
            x: 0.5,
            y: 0.9,
            z: 0.5,
        });
        player.grounded = true;
        let solid = |x: i32, y: i32, z: i32| x == 1 && (0..1).contains(&y) && z == 0;
        for tick in 0..20 {
            player.tick(
                MovementInput {
                    forward: 1.0,
                    jump: tick == 0,
                    ..Default::default()
                },
                0.0,
                solid,
            );
        }
        assert!(player.transform.x > 0.5);
    }
}
