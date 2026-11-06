//! Shared world-space units for terrain, collision, movement, and rendering.

pub const BLOCK_EDGE: f64 = 1.0;
pub const CHUNK_EDGE: usize = 16;
pub const PLAYER_WIDTH: f64 = 0.6;
pub const PLAYER_HEIGHT: f64 = 1.8;
pub const PLAYER_HALF_WIDTH: f64 = PLAYER_WIDTH / 2.0;
pub const PLAYER_HALF_HEIGHT: f64 = PLAYER_HEIGHT / 2.0;
pub const PLAYER_EYE_HEIGHT: f64 = 1.62;
/// Vertical distance from the player's AABB center to the survival camera.
pub const PLAYER_EYE_OFFSET: f64 = PLAYER_EYE_HEIGHT - PLAYER_HALF_HEIGHT;
pub const PLAYER_STEP_HEIGHT: f64 = 0.6;
