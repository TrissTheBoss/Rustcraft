//! Version-pinned Java 26.3-style player movement primitives.
//!
//! Values are expressed in blocks per game tick (20 ticks per second) and are
//! intentionally centralized so later conformance measurements can adjust them.

use crate::blocks::{Aabb, CollisionShape};
use crate::entity::Transform;
use crate::grid::{PLAYER_HALF_HEIGHT, PLAYER_HALF_WIDTH};
use std::time::Duration;

pub const TICKS_PER_SECOND: f64 = 20.0;
pub const WALK_ACCELERATION: f64 = 0.098;
pub const SPRINT_ACCELERATION: f64 = WALK_ACCELERATION * 1.30;
pub const GROUND_FRICTION: f64 = 0.546;
pub const AIR_FRICTION: f64 = 0.91;
pub const GRAVITY: f64 = 0.08;
// Tuned for the 20 Hz discrete integrator: approximately 1.25 blocks of
// vertical rise, enough to clear one full block without an auto-step.
pub const JUMP_IMPULSE: f64 = 0.45;
pub const FLIGHT_ACCELERATION: f64 = 0.049;
pub const FLIGHT_SPEED_MULTIPLIER: f64 = 2.0;
pub const FLIGHT_SPRINT_MULTIPLIER: f64 = 2.0;
pub const WALK_SPEED_PER_TICK: f64 = 4.317 / TICKS_PER_SECOND;
pub const SPRINT_SPEED_PER_TICK: f64 = 5.612 / TICKS_PER_SECOND;
const FIXED_TICK: Duration = Duration::from_millis(50);
const MAX_CATCH_UP_TICKS: u32 = 5;

/// Keeps simulation time at 20 Hz even when the window/render loop is delayed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FixedStepClock {
    accumulated: Duration,
}

impl FixedStepClock {
    pub fn advance(&mut self, elapsed: Duration) -> u32 {
        self.accumulated = self.accumulated.saturating_add(elapsed);
        let steps = (self.accumulated.as_nanos() / FIXED_TICK.as_nanos()) as u32;
        if steps > MAX_CATCH_UP_TICKS {
            self.accumulated =
                Duration::from_nanos((self.accumulated.as_nanos() % FIXED_TICK.as_nanos()) as u64);
            return MAX_CATCH_UP_TICKS;
        }
        self.accumulated -= FIXED_TICK * steps;
        steps
    }

    pub fn interpolation_alpha(self) -> f32 {
        (self.accumulated.as_secs_f32() / FIXED_TICK.as_secs_f32()).clamp(0.0, 1.0)
    }

    pub fn reset(&mut self) {
        self.accumulated = Duration::ZERO;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameMode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameMode {
    /// Adventure block permissions require item and block-tag data that is not
    /// present yet, so the current prototype denies edits in that mode.
    pub const fn can_edit_blocks(self) -> bool {
        matches!(self, Self::Survival | Self::Creative)
    }
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
    pub previous_transform: Transform,
    pub velocity: [f64; 3],
    pub half_extents: [f64; 3],
    pub mode: GameMode,
    pub flying: bool,
    pub grounded: bool,
    pub fall_distance: f64,
    pub fall_damage_armed: bool,
}

impl PlayerController {
    pub fn new(transform: Transform) -> Self {
        Self {
            transform,
            previous_transform: transform,
            velocity: [0.0; 3],
            half_extents: [PLAYER_HALF_WIDTH, PLAYER_HALF_HEIGHT, PLAYER_HALF_WIDTH],
            mode: GameMode::Survival,
            flying: false,
            grounded: false,
            fall_distance: 0.0,
            fall_damage_armed: false,
        }
    }

    pub fn tick(
        &mut self,
        input: MovementInput,
        yaw_radians: f64,
        is_solid: impl Fn(i32, i32, i32) -> bool,
    ) -> Option<f64> {
        self.tick_with_collision_shapes(input, yaw_radians, |x, y, z| {
            if is_solid(x, y, z) {
                CollisionShape::FULL_BLOCK
            } else {
                CollisionShape::EMPTY
            }
        })
    }

    /// Advance one tick against block-local collision shapes. Empty shapes
    /// represent non-colliding cells; multiple boxes are tested independently.
    pub fn tick_with_collision_shapes(
        &mut self,
        input: MovementInput,
        yaw_radians: f64,
        collision_shape: impl Fn(i32, i32, i32) -> CollisionShape,
    ) -> Option<f64> {
        let was_grounded = self.grounded;
        self.previous_transform = self.transform;
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
                * if input.sprint {
                    FLIGHT_SPRINT_MULTIPLIER
                } else {
                    1.0
                }
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
            let has_horizontal_input = input.forward != 0.0 || input.strafe != 0.0;
            let friction = if self.grounded {
                if has_horizontal_input {
                    1.0
                } else {
                    GROUND_FRICTION
                }
            } else {
                AIR_FRICTION
            };
            self.velocity[0] *= friction;
            self.velocity[2] *= friction;
            let maximum = if input.sprint {
                SPRINT_SPEED_PER_TICK
            } else {
                WALK_SPEED_PER_TICK
            };
            let horizontal_speed = self.velocity[0].hypot(self.velocity[2]);
            if horizontal_speed > maximum {
                let scale = maximum / horizontal_speed;
                self.velocity[0] *= scale;
                self.velocity[2] *= scale;
            }
        }
        let starting_y = self.transform.y;
        self.grounded = false;
        move_axis(self, 1, &collision_shape);
        // Resolve vertical motion first. This lets a jump raise the player's
        // AABB above a one-block obstacle before horizontal collision checks,
        // instead of treating the obstacle as an impenetrable wall.
        move_axis(self, 0, &collision_shape);
        move_axis(self, 2, &collision_shape);
        if self.transform.y < self.half_extents[1] {
            self.transform.y = self.half_extents[1];
            self.velocity[1] = 0.0;
            self.grounded = true;
        }
        if flying {
            self.fall_distance = 0.0;
            self.fall_damage_armed = true;
            return None;
        }
        self.fall_distance += (starting_y - self.transform.y).max(0.0);
        if self.grounded {
            let landing_distance = if was_grounded || !self.fall_damage_armed {
                None
            } else {
                Some(self.fall_distance)
            };
            self.fall_distance = 0.0;
            self.fall_damage_armed = true;
            landing_distance
        } else {
            if was_grounded {
                self.fall_damage_armed = true;
            }
            None
        }
    }
}

fn move_axis(
    player: &mut PlayerController,
    axis: usize,
    collision_shape: &impl Fn(i32, i32, i32) -> CollisionShape,
) {
    let mut candidate = player.transform;
    // Velocity is stored in blocks per simulation tick, so do not divide it
    // again when applying the 20 Hz tick displacement.
    let delta = player.velocity[axis];
    match axis {
        0 => candidate.x += delta,
        1 => candidate.y += delta,
        _ => candidate.z += delta,
    }
    if !collides(candidate, player.half_extents, collision_shape) {
        player.transform = candidate;
        return;
    }
    player.velocity[axis] = 0.0;
    if axis == 1 && delta < 0.0 {
        player.grounded = true;
    }
}

fn collides(
    center: Transform,
    half: [f64; 3],
    collision_shape: &impl Fn(i32, i32, i32) -> CollisionShape,
) -> bool {
    let epsilon = 1.0e-9;
    let ranges = [
        (center.x - half[0] + epsilon).floor() as i32..(center.x + half[0] - epsilon).ceil() as i32,
        (center.y - half[1] + epsilon).floor() as i32..(center.y + half[1] - epsilon).ceil() as i32,
        (center.z - half[2] + epsilon).floor() as i32..(center.z + half[2] - epsilon).ceil() as i32,
    ];
    let player_bounds = Aabb {
        min: [
            center.x - half[0] + epsilon,
            center.y - half[1] + epsilon,
            center.z - half[2] + epsilon,
        ],
        max: [
            center.x + half[0] - epsilon,
            center.y + half[1] - epsilon,
            center.z + half[2] - epsilon,
        ],
    };
    ranges[0].clone().any(|x| {
        ranges[1].clone().any(|y| {
            ranges[2].clone().any(|z| {
                (-1..=1).any(|dx| {
                    (-1..=1).any(|dy| {
                        (-1..=1).any(|dz| {
                            let owner = [
                                x.saturating_add(dx),
                                y.saturating_add(dy),
                                z.saturating_add(dz),
                            ];
                            collision_shape(owner[0], owner[1], owner[2])
                                .boxes()
                                .iter()
                                .copied()
                                .map(|bounds| bounds.translated(owner))
                                .any(|bounds| bounds.intersects(player_bounds))
                        })
                    })
                })
            })
        })
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
    fn fixed_step_clock_catches_up_and_reports_interpolation_remainder() {
        let mut clock = FixedStepClock::default();
        assert_eq!(clock.advance(Duration::from_millis(20)), 0);
        assert!((clock.interpolation_alpha() - 0.4).abs() < 1.0e-6);
        assert_eq!(clock.advance(Duration::from_millis(130)), 3);
        assert!(clock.interpolation_alpha() < 1.0e-6);
        assert_eq!(clock.advance(Duration::from_secs(10)), MAX_CATCH_UP_TICKS);
        assert!(clock.interpolation_alpha() < 1.0);
    }

    #[test]
    fn creative_flight_sprint_increases_horizontal_and_vertical_speed() {
        let mut flying = PlayerController::new(Transform {
            x: 0.0,
            y: 10.0,
            z: 0.0,
        });
        flying.mode = GameMode::Creative;
        flying.flying = true;
        let mut sprint_flying = flying;
        for _ in 0..20 {
            flying.tick(
                MovementInput {
                    forward: 1.0,
                    jump: true,
                    ..Default::default()
                },
                0.0,
                |_, _, _| false,
            );
            sprint_flying.tick(
                MovementInput {
                    forward: 1.0,
                    jump: true,
                    sprint: true,
                    ..Default::default()
                },
                0.0,
                |_, _, _| false,
            );
        }
        assert!(sprint_flying.transform.x > flying.transform.x);
        assert!(sprint_flying.transform.y > flying.transform.y);
    }

    #[test]
    fn movement_uses_partial_block_collision_boxes() {
        let slab = crate::blocks::BlockRegistry::new()
            .definition(crate::world::BlockId(4))
            .collision;
        let mut player = PlayerController::new(Transform {
            x: 0.5,
            y: 2.5,
            z: 0.5,
        });
        player.mode = GameMode::Creative;
        player.flying = true;
        for _ in 0..8 {
            player.tick_with_collision_shapes(
                MovementInput {
                    forward: 1.0,
                    ..Default::default()
                },
                0.0,
                |x, y, z| {
                    if [x, y, z] == [1, 1, 0] {
                        slab
                    } else {
                        crate::blocks::CollisionShape::EMPTY
                    }
                },
            );
        }
        assert!(player.transform.x > 0.7);
    }

    #[test]
    fn movement_collides_with_a_tall_shape_above_its_owner_cell() {
        let fence = crate::blocks::BlockRegistry::new()
            .definition(crate::world::BlockId(9))
            .collision;
        let mut player = PlayerController::new(Transform {
            x: 0.5,
            y: 2.35,
            z: 0.5,
        });
        player.mode = GameMode::Creative;
        player.flying = true;
        for _ in 0..12 {
            player.tick_with_collision_shapes(
                MovementInput {
                    forward: 1.0,
                    ..Default::default()
                },
                0.0,
                |x, y, z| {
                    if [x, y, z] == [1, 0, 0] {
                        fence
                    } else {
                        crate::blocks::CollisionShape::EMPTY
                    }
                },
            );
        }
        assert!(player.transform.x < 1.2);
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
    fn fall_distance_is_reported_on_armed_landing_but_not_initial_spawn_drop() {
        let mut player = PlayerController::new(Transform {
            x: 0.0,
            y: 8.0,
            z: 0.0,
        });
        let mut landing = None;
        for _ in 0..80 {
            landing = player.tick(Default::default(), 0.0, |_, y, _| y < 0);
            if player.grounded {
                break;
            }
        }
        assert!(player.grounded);
        assert!(landing.is_none());
        assert!(player.fall_damage_armed);

        player.transform.y = 8.0;
        player.previous_transform = player.transform;
        player.grounded = false;
        player.velocity = [0.0; 3];
        for _ in 0..80 {
            landing = player.tick(Default::default(), 0.0, |_, y, _| y < 0);
            if landing.is_some() {
                break;
            }
        }
        assert!(landing.is_some_and(|distance| distance > 6.0));
        assert!(player.grounded);
    }

    #[test]
    fn creative_flight_resets_fall_tracking_without_reporting_damage() {
        let mut player = PlayerController::new(Transform {
            x: 0.0,
            y: 8.0,
            z: 0.0,
        });
        player.mode = GameMode::Creative;
        player.flying = true;
        player.fall_distance = 20.0;
        let landing = player.tick(Default::default(), 0.0, |_, _, _| false);
        assert!(landing.is_none());
        assert_eq!(player.fall_distance, 0.0);
    }

    #[test]
    fn adventure_mode_is_preserved_as_a_distinct_non_flying_mode() {
        let mut player = PlayerController::new(Transform {
            x: 0.0,
            y: 10.0,
            z: 0.0,
        });
        player.mode = GameMode::Adventure;
        player.tick(
            MovementInput {
                toggle_flight: true,
                ..Default::default()
            },
            0.0,
            |_, _, _| false,
        );
        assert!(!player.flying);
        assert!(player.velocity[1] < 0.0);
        assert!(!player.mode.can_edit_blocks());
    }

    #[test]
    fn survival_and_creative_allow_prototype_block_edits() {
        assert!(GameMode::Survival.can_edit_blocks());
        assert!(GameMode::Creative.can_edit_blocks());
        assert!(!GameMode::Adventure.can_edit_blocks());
        assert!(!GameMode::Spectator.can_edit_blocks());
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
        assert!(player.transform.x > 1.0);
    }

    #[test]
    fn ground_contact_does_not_block_horizontal_speed() {
        let mut player = PlayerController::new(Transform {
            x: 0.5,
            y: 1.9,
            z: 0.5,
        });
        player.grounded = true;
        for _ in 0..40 {
            player.tick(
                MovementInput {
                    forward: 1.0,
                    ..Default::default()
                },
                0.0,
                |_, y, _| y == 0,
            );
        }
        assert!(player.transform.x > 2.0);
    }

    #[test]
    fn walking_input_reaches_configured_ground_speed() {
        let mut player = PlayerController::new(Transform {
            x: 0.0,
            y: 0.9,
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
                |_, _, _| false,
            );
        }
        assert!((player.velocity[0] * TICKS_PER_SECOND - 4.317).abs() < 1.0e-9);
    }

    #[test]
    fn airborne_horizontal_speed_does_not_accumulate_unboundedly() {
        let mut player = PlayerController::new(Transform {
            x: 0.5,
            y: 1.9,
            z: 0.5,
        });
        player.grounded = true;
        for tick in 0..80 {
            player.tick(
                MovementInput {
                    forward: 1.0,
                    jump: tick == 0,
                    ..Default::default()
                },
                0.0,
                |_, y, _| y == 0,
            );
        }
        assert!(player.velocity[0] <= WALK_SPEED_PER_TICK + 1.0e-9);
    }

    #[test]
    fn horizontal_collision_never_steps_player_onto_a_block() {
        let mut player = PlayerController::new(Transform {
            x: 0.5,
            y: 0.9,
            z: 0.5,
        });
        player.grounded = true;
        let solid = |x: i32, y: i32, z: i32| x == 1 && y == 0 && z == 0;
        for _ in 0..40 {
            player.tick(
                MovementInput {
                    forward: 1.0,
                    ..Default::default()
                },
                0.0,
                solid,
            );
        }
        assert!(player.transform.x <= 0.7 + 1.0e-9);
        assert!((player.transform.y - 0.9).abs() < 1.0e-9);
    }
}
