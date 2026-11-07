//! Block targeting and edits for the voxel interaction layer.
//!
//! This module contains engine-owned ray and mutation primitives. The current
//! prototype uses immediate block edits and mapped Survival loot. Survival
//! mining progression/tool rules, advanced loot, placement rules, and full
//! inventory UI behavior remain separate requirements.

use crate::blocks::{Aabb, BlockRegistry};
use crate::drops::{BlockLootTable, DropError, ItemDrops};
use crate::inventory::ItemCatalog;
use crate::world::{BlockId, World};

pub const PROTOTYPE_INTERACTION_REACH: f64 = 5.0;
const MAX_BLOCK_RAY_DISTANCE: f64 = 4096.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockPos(pub [i32; 3]);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockRayHit {
    pub position: BlockPos,
    /// Unit axis normal pointing out of the face crossed by the ray.
    pub face_normal: [i32; 3],
    pub block: BlockId,
    /// Distance from ray origin in world units.
    pub distance: f64,
}

/// Find the first solid voxel on a finite ray through loaded world space.
/// Missing chunks behave as empty for traversal but can never be edited.
pub fn raycast_blocks(
    world: &World,
    registry: &BlockRegistry,
    origin: [f64; 3],
    direction: [f64; 3],
    max_distance: f64,
) -> Option<BlockRayHit> {
    if !max_distance.is_finite()
        || max_distance <= 0.0
        || max_distance > MAX_BLOCK_RAY_DISTANCE
        || !origin.iter().all(|value| value.is_finite())
        || !direction.iter().all(|value| value.is_finite())
    {
        return None;
    }

    let length = direction[0].hypot(direction[1]).hypot(direction[2]);
    if !length.is_finite() || length <= f64::EPSILON {
        return None;
    }
    let direction = [
        direction[0] / length,
        direction[1] / length,
        direction[2] / length,
    ];
    let mut cell = [0i32; 3];
    for axis in 0..3 {
        let floored = origin[axis].floor();
        if floored < i32::MIN as f64 || floored > i32::MAX as f64 {
            return None;
        }
        cell[axis] = floored as i32;
    }

    if let Some(block) = world.block_at(cell) {
        if registry.is_solid(block) {
            return Some(BlockRayHit {
                position: BlockPos(cell),
                face_normal: [0; 3],
                block,
                distance: 0.0,
            });
        }
    }

    let mut step = [0i32; 3];
    let mut t_delta = [f64::INFINITY; 3];
    let mut t_max = [f64::INFINITY; 3];
    for axis in 0..3 {
        if direction[axis] > 0.0 {
            step[axis] = 1;
            t_delta[axis] = 1.0 / direction[axis];
            t_max[axis] = (cell[axis] as f64 + 1.0 - origin[axis]) / direction[axis];
        } else if direction[axis] < 0.0 {
            step[axis] = -1;
            t_delta[axis] = -1.0 / direction[axis];
            t_max[axis] = (origin[axis] - cell[axis] as f64) / -direction[axis];
        }
    }

    // A unit ray crosses at most three voxel boundaries per world unit. The
    // saturating cap also avoids overflow when callers request extreme ranges.
    let max_steps = (max_distance.ceil() as usize)
        .saturating_mul(3)
        .saturating_add(3);
    for _ in 0..max_steps {
        // Stable X, then Y, then Z tie breaking makes corner hits reproducible.
        let axis = if t_max[0] <= t_max[1] && t_max[0] <= t_max[2] {
            0
        } else if t_max[1] <= t_max[2] {
            1
        } else {
            2
        };
        let distance = t_max[axis];
        if !distance.is_finite() || distance > max_distance {
            return None;
        }
        cell[axis] = cell[axis].checked_add(step[axis])?;
        t_max[axis] += t_delta[axis];
        if let Some(block) = world.block_at(cell) {
            if registry.is_solid(block) {
                let mut face_normal = [0; 3];
                face_normal[axis] = -step[axis];
                return Some(BlockRayHit {
                    position: BlockPos(cell),
                    face_normal,
                    block,
                    distance,
                });
            }
        }
    }
    None
}

pub fn break_block(world: &mut World, hit: BlockRayHit) -> Option<BlockId> {
    if hit.block.0 == 0 || world.block_at(hit.position.0) != Some(hit.block) {
        return None;
    }
    world.set_block(hit.position.0, BlockId(0))
}

/// Break a block and create its configured drop as one operation. Spawn
/// constraints are checked before the world mutates, so a full drop store or
/// exhausted ID allocator cannot destroy a block while losing its loot.
pub fn break_block_with_drop(
    world: &mut World,
    hit: BlockRayHit,
    drops: &mut ItemDrops,
    loot: &BlockLootTable,
    catalog: &ItemCatalog,
    drop_position: [f64; 3],
    pickup_delay_ticks: u16,
) -> Result<Option<BlockId>, DropError> {
    let Some(stack) = loot.drop_for(hit.block) else {
        return Ok(break_block(world, hit));
    };
    drops.validate_spawn(stack, drop_position, catalog)?;
    let Some(broken) = break_block(world, hit) else {
        return Ok(None);
    };
    drops.spawn(stack, drop_position, pickup_delay_ticks, catalog)?;
    Ok(Some(broken))
}

/// Place a full-block prototype item against the hit face if its destination
/// chunk is loaded and the block would not overlap the player's body.
pub fn place_full_block(
    world: &mut World,
    registry: &BlockRegistry,
    hit: BlockRayHit,
    block: BlockId,
    player_bounds: Aabb,
) -> bool {
    if block.0 == 0
        || !registry.is_solid(block)
        || world.block_at(hit.position.0) != Some(hit.block)
        || !registry.is_solid(hit.block)
        || hit
            .face_normal
            .iter()
            .any(|value| !(-1..=1).contains(value))
        || hit.face_normal.iter().map(|value| value.abs()).sum::<i32>() != 1
        || !(0..3).all(|axis| {
            player_bounds.min[axis].is_finite()
                && player_bounds.max[axis].is_finite()
                && player_bounds.min[axis] < player_bounds.max[axis]
        })
    {
        return false;
    }
    let mut target = [0; 3];
    for axis in 0..3 {
        let Some(coordinate) = hit.position.0[axis].checked_add(hit.face_normal[axis]) else {
            return false;
        };
        target[axis] = coordinate;
    }
    let Some(existing) = world.block_at(target) else {
        return false;
    };
    if registry.is_solid(existing) {
        return false;
    }
    let shape = registry.definition(block).collision;
    if shape
        .boxes()
        .iter()
        .copied()
        .map(|bounds| bounds.translated(target))
        .any(|bounds| bounds.intersects(player_bounds))
    {
        return false;
    }
    world.set_block(target, block).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drops::{ItemDrops, MAX_DROPPED_ITEMS};
    use crate::inventory::{ItemDefinition, ItemId, ItemKey};
    use crate::world::ChunkPos;

    fn world_with_chunks(positions: &[ChunkPos]) -> World {
        let mut world = World::new();
        for position in positions {
            world.get_or_create_chunk(*position);
        }
        world
    }

    #[test]
    fn raycast_hits_first_block_and_reports_entered_face() {
        let mut world = world_with_chunks(&[ChunkPos { x: 0, y: 0, z: 0 }]);
        world.set_block([3, 1, 0], BlockId(3));
        world.set_block([5, 1, 0], BlockId(2));

        let hit = raycast_blocks(
            &world,
            &BlockRegistry::new(),
            [0.5, 1.5, 0.5],
            [8.0, 0.0, 0.0],
            5.0,
        )
        .unwrap();

        assert_eq!(hit.position, BlockPos([3, 1, 0]));
        assert_eq!(hit.face_normal, [-1, 0, 0]);
        assert_eq!(hit.block, BlockId(3));
        assert!((hit.distance - 2.5).abs() < 1.0e-9);
    }

    #[test]
    fn raycast_traverses_negative_world_coordinates() {
        let mut world = world_with_chunks(&[ChunkPos { x: -1, y: 0, z: 0 }]);
        world.set_block([-1, 0, 0], BlockId(1));
        let hit = raycast_blocks(
            &world,
            &BlockRegistry::new(),
            [0.1, 0.5, 0.5],
            [-1.0, 0.0, 0.0],
            2.0,
        )
        .unwrap();
        assert_eq!(hit.position, BlockPos([-1, 0, 0]));
        assert_eq!(hit.face_normal, [1, 0, 0]);
        assert!((hit.distance - 0.1).abs() < 1.0e-9);
    }

    #[test]
    fn raycast_rejects_invalid_rays_and_respects_reach() {
        let mut world = world_with_chunks(&[ChunkPos { x: 0, y: 0, z: 0 }]);
        world.set_block([3, 0, 0], BlockId(3));
        let registry = BlockRegistry::new();
        assert!(raycast_blocks(&world, &registry, [0.0; 3], [0.0; 3], 5.0).is_none());
        assert!(raycast_blocks(&world, &registry, [0.0; 3], [1.0, 0.0, 0.0], 2.0).is_none());
        assert!(raycast_blocks(&world, &registry, [0.0; 3], [1.0, 0.0, 0.0], f64::MAX,).is_none());
        assert!(raycast_blocks(
            &world,
            &registry,
            [f64::NAN, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            5.0,
        )
        .is_none());
    }

    #[test]
    fn edits_only_loaded_blocks_and_prevent_player_overlap() {
        let mut world = world_with_chunks(&[ChunkPos { x: 0, y: 0, z: 0 }]);
        world.set_block([2, 1, 1], BlockId(3));
        let hit = raycast_blocks(
            &world,
            &BlockRegistry::new(),
            [0.5, 1.5, 1.5],
            [1.0, 0.0, 0.0],
            5.0,
        )
        .unwrap();
        let registry = BlockRegistry::new();
        assert!(!place_full_block(
            &mut world,
            &registry,
            hit,
            BlockId(1),
            Aabb {
                min: [1.1, 1.0, 1.0],
                max: [1.9, 2.0, 2.0],
            },
        ));
        assert!(place_full_block(
            &mut world,
            &registry,
            hit,
            BlockId(1),
            Aabb {
                min: [10.0, 10.0, 10.0],
                max: [11.0, 11.0, 11.0],
            },
        ));
        assert_eq!(world.block_at([1, 1, 1]), Some(BlockId(1)));
        assert_eq!(break_block(&mut world, hit), Some(BlockId(3)));
        assert_eq!(world.block_at([2, 1, 1]), Some(BlockId(0)));
    }

    #[test]
    fn edits_do_not_cross_into_unloaded_chunks() {
        let mut world = world_with_chunks(&[ChunkPos { x: 0, y: 0, z: 0 }]);
        world.set_block([15, 1, 0], BlockId(2));
        let hit = raycast_blocks(
            &world,
            &BlockRegistry::new(),
            [17.5, 1.5, 0.5],
            [-1.0, 0.0, 0.0],
            2.0,
        )
        .unwrap();
        assert!(!place_full_block(
            &mut world,
            &BlockRegistry::new(),
            hit,
            BlockId(3),
            Aabb {
                min: [20.0, 20.0, 20.0],
                max: [21.0, 21.0, 21.0],
            },
        ));
        assert_eq!(world.chunk_count(), 1);
    }

    #[test]
    fn stale_or_invalid_hits_cannot_mutate_the_world() {
        let mut world = world_with_chunks(&[ChunkPos { x: 0, y: 0, z: 0 }]);
        world.set_block([2, 1, 1], BlockId(3));
        let hit = raycast_blocks(
            &world,
            &BlockRegistry::new(),
            [0.5, 1.5, 1.5],
            [1.0, 0.0, 0.0],
            5.0,
        )
        .unwrap();
        assert_eq!(break_block(&mut world, hit), Some(BlockId(3)));
        assert_eq!(break_block(&mut world, hit), None);

        world.set_block([2, 1, 1], BlockId(3));
        let forged_face = BlockRayHit {
            face_normal: [1, 1, 0],
            ..hit
        };
        assert!(!place_full_block(
            &mut world,
            &BlockRegistry::new(),
            forged_face,
            BlockId(1),
            Aabb {
                min: [10.0, 10.0, 10.0],
                max: [11.0, 11.0, 11.0],
            },
        ));
    }

    #[test]
    fn survival_break_creates_loot_and_capacity_failure_preserves_block() {
        let mut world = world_with_chunks(&[ChunkPos { x: 0, y: 0, z: 0 }]);
        world.set_block([2, 1, 1], BlockId(3));
        let hit = raycast_blocks(
            &world,
            &BlockRegistry::new(),
            [0.5, 1.5, 1.5],
            [1.0, 0.0, 0.0],
            5.0,
        )
        .unwrap();
        let mut catalog = ItemCatalog::default();
        for id in 1..=3 {
            catalog
                .register(ItemId(id), ItemDefinition::stackable(64))
                .unwrap();
        }
        let loot = BlockLootTable::prototype();
        let mut drops = ItemDrops::default();
        assert_eq!(
            break_block_with_drop(
                &mut world,
                hit,
                &mut drops,
                &loot,
                &catalog,
                [2.5, 1.5, 1.5],
                10,
            ),
            Ok(Some(BlockId(3)))
        );
        assert_eq!(world.block_at([2, 1, 1]), Some(BlockId(0)));
        let drop = drops.iter().next().unwrap();
        assert_eq!(drop.stack.key.id, ItemId(3));
        assert_eq!(drop.position, [2.5, 1.5, 1.5]);

        let mut full_drops = ItemDrops::default();
        for _ in 0..MAX_DROPPED_ITEMS {
            full_drops
                .spawn(
                    crate::inventory::ItemStack {
                        key: ItemKey::new(ItemId(1)),
                        count: 1,
                    },
                    [0.5, 5.0, 0.5],
                    u16::MAX,
                    &catalog,
                )
                .unwrap();
        }
        let mut full_world = world_with_chunks(&[ChunkPos { x: 0, y: 0, z: 0 }]);
        full_world.set_block([2, 1, 1], BlockId(3));
        let intact_hit = raycast_blocks(
            &full_world,
            &BlockRegistry::new(),
            [0.5, 1.5, 1.5],
            [1.0, 0.0, 0.0],
            5.0,
        )
        .unwrap();
        assert_eq!(
            break_block_with_drop(
                &mut full_world,
                intact_hit,
                &mut full_drops,
                &loot,
                &catalog,
                [2.5, 1.5, 1.5],
                10,
            ),
            Err(DropError::Capacity)
        );
        assert_eq!(full_world.block_at([2, 1, 1]), Some(BlockId(3)));
    }
}
