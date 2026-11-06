//! Data-driven block definitions and collision geometry.
//!
//! Shapes are authored in block-local coordinates `[0, 1]`. This module is
//! intentionally independent of Minecraft implementation code or assets. The
//! registry can later be populated from independently authored versioned data.

use crate::world::BlockId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Aabb {
    pub const FULL_BLOCK: Self = Self {
        min: [0.0, 0.0, 0.0],
        max: [1.0, 1.0, 1.0],
    };

    pub fn translated(self, block: [i32; 3]) -> Self {
        let offset = [block[0] as f64, block[1] as f64, block[2] as f64];
        Self {
            min: [
                self.min[0] + offset[0],
                self.min[1] + offset[1],
                self.min[2] + offset[2],
            ],
            max: [
                self.max[0] + offset[0],
                self.max[1] + offset[1],
                self.max[2] + offset[2],
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionShape {
    boxes: &'static [Aabb],
}

impl CollisionShape {
    pub const EMPTY: Self = Self { boxes: &[] };
    pub const FULL_BLOCK: Self = Self {
        boxes: &[Aabb::FULL_BLOCK],
    };

    pub const fn from_boxes(boxes: &'static [Aabb]) -> Self {
        Self { boxes }
    }

    pub fn boxes(&self) -> &'static [Aabb] {
        self.boxes
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockDefinition {
    pub id: BlockId,
    pub name: &'static str,
    pub collision: CollisionShape,
    pub opaque: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BlockRegistry;

impl BlockRegistry {
    pub const fn new() -> Self {
        Self
    }

    /// Returns the independently authored definition for a known prototype
    /// block. Unknown IDs intentionally use a conservative full cube so they
    /// cannot create accidental walk-through geometry.
    pub fn definition(&self, id: BlockId) -> BlockDefinition {
        match id.0 {
            0 => BlockDefinition {
                id,
                name: "air",
                collision: CollisionShape::EMPTY,
                opaque: false,
            },
            1 => BlockDefinition {
                id,
                name: "soil",
                collision: CollisionShape::FULL_BLOCK,
                opaque: true,
            },
            2 => BlockDefinition {
                id,
                name: "surface",
                collision: CollisionShape::FULL_BLOCK,
                opaque: true,
            },
            3 => BlockDefinition {
                id,
                name: "rock",
                collision: CollisionShape::FULL_BLOCK,
                opaque: true,
            },
            _ => BlockDefinition {
                id,
                name: "unknown",
                collision: CollisionShape::FULL_BLOCK,
                opaque: true,
            },
        }
    }

    pub fn is_solid(&self, id: BlockId) -> bool {
        !self.definition(id).collision.boxes().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prototype_registry_distinguishes_air_and_full_blocks() {
        let registry = BlockRegistry::new();
        assert!(!registry.is_solid(BlockId(0)));
        assert!(registry.is_solid(BlockId(1)));
        assert_eq!(registry.definition(BlockId(2)).name, "surface");
    }

    #[test]
    fn full_block_shape_is_one_world_block() {
        assert_eq!(CollisionShape::FULL_BLOCK.boxes()[0], Aabb::FULL_BLOCK);
        assert_eq!(
            Aabb::FULL_BLOCK.translated([2, -1, 4]).min,
            [2.0, -1.0, 4.0]
        );
    }
}
