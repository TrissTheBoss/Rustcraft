//! Data-driven block definitions and collision geometry.
//!
//! Shapes are authored in block-local coordinates `[0, 1]`. This module is
//! intentionally independent of Minecraft implementation code or assets. The
//! registry can later be populated from independently authored versioned data.

use crate::world::BlockId;

const SNOW_SHAPES: [[Aabb; 1]; 8] = [
    [Aabb {
        min: [0.0, 0.0, 0.0],
        max: [1.0, 1.0 / 8.0, 1.0],
    }],
    [Aabb {
        min: [0.0, 0.0, 0.0],
        max: [1.0, 2.0 / 8.0, 1.0],
    }],
    [Aabb {
        min: [0.0, 0.0, 0.0],
        max: [1.0, 3.0 / 8.0, 1.0],
    }],
    [Aabb {
        min: [0.0, 0.0, 0.0],
        max: [1.0, 4.0 / 8.0, 1.0],
    }],
    [Aabb {
        min: [0.0, 0.0, 0.0],
        max: [1.0, 5.0 / 8.0, 1.0],
    }],
    [Aabb {
        min: [0.0, 0.0, 0.0],
        max: [1.0, 6.0 / 8.0, 1.0],
    }],
    [Aabb {
        min: [0.0, 0.0, 0.0],
        max: [1.0, 7.0 / 8.0, 1.0],
    }],
    [Aabb {
        min: [0.0, 0.0, 0.0],
        max: [1.0, 1.0, 1.0],
    }],
];

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

    /// Strict overlap; touching faces do not count as intersection.
    pub fn intersects(self, other: Self) -> bool {
        (0..3).all(|axis| self.min[axis] < other.max[axis] && self.max[axis] > other.min[axis])
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionShape {
    boxes: &'static [Aabb],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockKind {
    Air,
    FullBlock,
    Slab,
    Carpet,
    Snow,
    Farmland,
    Path,
    Fence,
    Pane,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockState {
    pub kind: BlockKind,
    /// Slab/fence/pane orientation or variant flag. For slabs, `true` means
    /// top slab; for fences/panes it is reserved for future connection data.
    pub upper: bool,
    /// Snow layer count, clamped to the Java block-state range 1..=8.
    pub layers: u8,
}

impl BlockState {
    pub const fn full_block() -> Self {
        Self {
            kind: BlockKind::FullBlock,
            upper: false,
            layers: 1,
        }
    }

    pub const fn shape(self) -> CollisionShape {
        match self.kind {
            BlockKind::Air => CollisionShape::EMPTY,
            BlockKind::FullBlock => CollisionShape::FULL_BLOCK,
            BlockKind::Slab => {
                if self.upper {
                    CollisionShape::from_boxes(&[Aabb {
                        min: [0.0, 0.5, 0.0],
                        max: [1.0, 1.0, 1.0],
                    }])
                } else {
                    CollisionShape::from_boxes(&[Aabb {
                        min: [0.0, 0.0, 0.0],
                        max: [1.0, 0.5, 1.0],
                    }])
                }
            }
            BlockKind::Carpet => CollisionShape::from_boxes(&[Aabb {
                min: [0.0, 0.0, 0.0],
                max: [1.0, 1.0 / 16.0, 1.0],
            }]),
            BlockKind::Snow => {
                let layers = if self.layers < 1 {
                    1
                } else if self.layers > 8 {
                    8
                } else {
                    self.layers
                };
                CollisionShape::from_boxes(&SNOW_SHAPES[layers as usize - 1])
            }
            BlockKind::Farmland => CollisionShape::from_boxes(&[Aabb {
                min: [0.0, 0.0, 0.0],
                max: [1.0, 15.0 / 16.0, 1.0],
            }]),
            BlockKind::Path => CollisionShape::from_boxes(&[Aabb {
                min: [0.0, 0.0, 0.0],
                max: [1.0, 15.0 / 16.0, 1.0],
            }]),
            BlockKind::Fence => CollisionShape::from_boxes(&[Aabb {
                min: [6.0 / 16.0, 0.0, 6.0 / 16.0],
                max: [10.0 / 16.0, 1.5, 10.0 / 16.0],
            }]),
            BlockKind::Pane => CollisionShape::from_boxes(&[Aabb {
                min: [7.0 / 16.0, 0.0, 7.0 / 16.0],
                max: [9.0 / 16.0, 1.0, 9.0 / 16.0],
            }]),
        }
    }
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
            4 => BlockDefinition {
                id,
                name: "slab",
                collision: BlockState {
                    kind: BlockKind::Slab,
                    upper: false,
                    layers: 1,
                }
                .shape(),
                opaque: true,
            },
            5 => BlockDefinition {
                id,
                name: "carpet",
                collision: BlockState {
                    kind: BlockKind::Carpet,
                    upper: false,
                    layers: 1,
                }
                .shape(),
                opaque: false,
            },
            6 => BlockDefinition {
                id,
                name: "snow layer",
                collision: BlockState {
                    kind: BlockKind::Snow,
                    upper: false,
                    layers: 1,
                }
                .shape(),
                opaque: false,
            },
            7 => BlockDefinition {
                id,
                name: "farmland",
                collision: BlockState {
                    kind: BlockKind::Farmland,
                    upper: false,
                    layers: 1,
                }
                .shape(),
                opaque: true,
            },
            8 => BlockDefinition {
                id,
                name: "path",
                collision: BlockState {
                    kind: BlockKind::Path,
                    upper: false,
                    layers: 1,
                }
                .shape(),
                opaque: true,
            },
            9 => BlockDefinition {
                id,
                name: "fence",
                collision: BlockState {
                    kind: BlockKind::Fence,
                    upper: false,
                    layers: 1,
                }
                .shape(),
                opaque: false,
            },
            10 => BlockDefinition {
                id,
                name: "pane",
                collision: BlockState {
                    kind: BlockKind::Pane,
                    upper: false,
                    layers: 1,
                }
                .shape(),
                opaque: false,
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

    #[test]
    fn state_shapes_match_documented_voxel_dimensions() {
        let bottom_slab = BlockState {
            kind: BlockKind::Slab,
            upper: false,
            layers: 1,
        };
        assert_eq!(bottom_slab.shape().boxes()[0].max[1], 0.5);
        assert_eq!(
            BlockState {
                kind: BlockKind::Snow,
                upper: false,
                layers: 8,
            }
            .shape()
            .boxes()[0]
                .max[1],
            1.0
        );
        assert_eq!(
            BlockState {
                kind: BlockKind::Carpet,
                upper: false,
                layers: 1,
            }
            .shape()
            .boxes()[0]
                .max[1],
            1.0 / 16.0
        );
    }

    #[test]
    fn prototype_registry_exposes_non_full_collision_shapes() {
        let registry = BlockRegistry::new();
        assert_eq!(
            registry.definition(BlockId(4)).collision.boxes()[0].max[1],
            0.5
        );
        assert_eq!(
            registry.definition(BlockId(5)).collision.boxes()[0].max[1],
            1.0 / 16.0
        );
        assert_eq!(
            registry.definition(BlockId(6)).collision.boxes()[0].max[1],
            1.0 / 8.0
        );
        assert_eq!(
            registry.definition(BlockId(9)).collision.boxes()[0].max[1],
            1.5
        );
    }
}
