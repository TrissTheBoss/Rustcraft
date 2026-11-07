//! Deterministic dropped-item entities and block-loot data.
//!
//! This is a small engine-owned simulation model. Its timings and loot are
//! prototype values until conformance data is pinned and independently tested.

use std::collections::BTreeMap;

use crate::blocks::BlockRegistry;
use crate::inventory::{Inventory, InventoryError, ItemCatalog, ItemKey, ItemStack};
use crate::world::{BlockId, World};

pub const MAX_DROPPED_ITEMS: usize = 1024;
pub const ITEM_DESPAWN_TICKS: u32 = 6000;
pub const ITEM_PICKUP_DELAY_TICKS: u16 = 10;
const ITEM_GRAVITY_PER_TICK: f64 = 0.04;
const ITEM_HALF_EXTENT: f64 = 0.125;
const ITEM_PICKUP_RADIUS: f64 = 1.25;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DropId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DroppedItem {
    pub id: DropId,
    pub stack: ItemStack,
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub pickup_delay_ticks: u16,
    pub age_ticks: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BlockLootTable {
    drops: BTreeMap<BlockId, (ItemKey, u16)>,
}

impl BlockLootTable {
    /// Starter content mapping. These original prototype blocks drop one
    /// matching prototype item; it is not a vanilla loot table.
    pub fn prototype() -> Self {
        let mut drops = BTreeMap::new();
        for id in 1..=3 {
            drops.insert(BlockId(id), (ItemKey::new(crate::inventory::ItemId(id)), 1));
        }
        Self { drops }
    }

    pub fn register(
        &mut self,
        block: BlockId,
        stack: ItemStack,
        catalog: &ItemCatalog,
    ) -> Result<(), DropError> {
        ItemStack::new(stack.key, stack.count, catalog).map_err(|_| DropError::InvalidStack)?;
        if self.drops.contains_key(&block) {
            return Err(DropError::DuplicateLoot);
        }
        self.drops.insert(block, (stack.key, stack.count));
        Ok(())
    }

    pub fn drop_for(&self, block: BlockId) -> Option<ItemStack> {
        let (key, count) = self.drops.get(&block).copied()?;
        Some(ItemStack { key, count })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemDrops {
    items: BTreeMap<DropId, DroppedItem>,
    next_id: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DropTickResult {
    pub picked_up_items: u32,
    pub removed_entities: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropError {
    InvalidPosition,
    InvalidStack,
    InvalidSavedState,
    DuplicateLoot,
    Capacity,
    IdExhausted,
    Inventory(InventoryError),
}

impl From<InventoryError> for DropError {
    fn from(value: InventoryError) -> Self {
        Self::Inventory(value)
    }
}

impl std::fmt::Display for DropError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPosition => f.write_str("dropped-item position is invalid"),
            Self::InvalidStack => f.write_str("dropped-item stack is invalid"),
            Self::InvalidSavedState => f.write_str("saved dropped-item state is invalid"),
            Self::DuplicateLoot => f.write_str("block already has a loot entry"),
            Self::Capacity => f.write_str("dropped-item entity capacity is full"),
            Self::IdExhausted => f.write_str("dropped-item ID space is exhausted"),
            Self::Inventory(error) => write!(f, "inventory pickup failed: {error}"),
        }
    }
}

impl std::error::Error for DropError {}

impl ItemDrops {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &DroppedItem> {
        self.items.values()
    }

    pub(crate) fn save_parts(&self) -> (u64, Vec<DroppedItem>) {
        (self.next_id, self.items.values().copied().collect())
    }

    pub(crate) fn from_save_parts(
        next_id: u64,
        items: Vec<DroppedItem>,
    ) -> Result<Self, DropError> {
        if items.len() > MAX_DROPPED_ITEMS {
            return Err(DropError::Capacity);
        }
        let mut stored = BTreeMap::new();
        for item in items {
            if item.id.0 >= next_id
                || item.stack.count == 0
                || item.stack.count > crate::inventory::MAX_CATALOG_STACK_SIZE
                || !valid_position(item.position)
                || !item
                    .velocity
                    .iter()
                    .all(|value| value.is_finite() && value.abs() <= 1_000_000.0)
                || item.age_ticks >= ITEM_DESPAWN_TICKS
                || stored.insert(item.id, item).is_some()
            {
                return Err(DropError::InvalidSavedState);
            }
        }
        Ok(Self {
            items: stored,
            next_id,
        })
    }

    pub fn spawn(
        &mut self,
        stack: ItemStack,
        position: [f64; 3],
        pickup_delay_ticks: u16,
        catalog: &ItemCatalog,
    ) -> Result<DropId, DropError> {
        if !valid_position(position) {
            return Err(DropError::InvalidPosition);
        }
        ItemStack::new(stack.key, stack.count, catalog).map_err(|_| DropError::InvalidStack)?;
        if self.items.len() >= MAX_DROPPED_ITEMS {
            return Err(DropError::Capacity);
        }
        let id = DropId(self.next_id);
        self.next_id = self.next_id.checked_add(1).ok_or(DropError::IdExhausted)?;
        self.items.insert(
            id,
            DroppedItem {
                id,
                stack,
                position,
                velocity: [0.0; 3],
                pickup_delay_ticks,
                age_ticks: 0,
            },
        );
        Ok(id)
    }

    /// Advance items in ascending stable ID order. A pickup may insert only a
    /// partial stack; any remainder stays in the world and can be picked up
    /// after inventory space becomes available.
    pub fn tick(
        &mut self,
        world: &World,
        player_position: [f64; 3],
        inventory: &mut Inventory,
        catalog: &ItemCatalog,
        blocks: &BlockRegistry,
    ) -> Result<DropTickResult, DropError> {
        if !valid_position(player_position) {
            return Err(DropError::InvalidPosition);
        }
        let mut result = DropTickResult::default();
        let mut expired = Vec::new();
        for dropped in self.items.values_mut() {
            dropped.age_ticks = dropped.age_ticks.saturating_add(1);
            if dropped.age_ticks >= ITEM_DESPAWN_TICKS {
                expired.push(dropped.id);
                continue;
            }
            dropped.pickup_delay_ticks = dropped.pickup_delay_ticks.saturating_sub(1);
            dropped.velocity[1] -= ITEM_GRAVITY_PER_TICK;
            for axis in 0..3 {
                dropped.position[axis] += dropped.velocity[axis];
            }
            settle_on_full_block(dropped, world, blocks);
            if dropped.pickup_delay_ticks > 0
                || squared_distance(dropped.position, player_position)
                    > ITEM_PICKUP_RADIUS * ITEM_PICKUP_RADIUS
            {
                continue;
            }
            let old_count = dropped.stack.count;
            let remainder = inventory.insert(dropped.stack.key, u32::from(old_count), catalog)?;
            if remainder < u32::from(old_count) {
                let picked_up = (u32::from(old_count) - remainder) as u16;
                result.picked_up_items += u32::from(picked_up);
                if remainder == 0 {
                    expired.push(dropped.id);
                } else {
                    dropped.stack.count = remainder as u16;
                }
            }
        }
        result.removed_entities = expired.len() as u32;
        for id in expired {
            self.items.remove(&id);
        }
        Ok(result)
    }
}

fn settle_on_full_block(dropped: &mut DroppedItem, world: &World, blocks: &BlockRegistry) {
    let candidate = [
        dropped.position[0].floor() as i32,
        (dropped.position[1] - ITEM_HALF_EXTENT).floor() as i32,
        dropped.position[2].floor() as i32,
    ];
    let Some(block) = world.block_at(candidate) else {
        return;
    };
    if blocks.is_solid(block) && dropped.velocity[1] <= 0.0 {
        dropped.position[1] = candidate[1] as f64 + 1.0 + ITEM_HALF_EXTENT;
        dropped.velocity[1] = 0.0;
    }
}

fn squared_distance(first: [f64; 3], second: [f64; 3]) -> f64 {
    (0..3)
        .map(|axis| (first[axis] - second[axis]).powi(2))
        .sum()
}

fn valid_position(position: [f64; 3]) -> bool {
    position
        .iter()
        .all(|value| value.is_finite() && value.abs() <= i32::MAX as f64 - 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{ItemDefinition, ItemId};
    use crate::world::ChunkPos;

    fn catalog() -> ItemCatalog {
        let mut catalog = ItemCatalog::default();
        for id in 1..=3 {
            catalog
                .register(ItemId(id), ItemDefinition::stackable(64))
                .unwrap();
        }
        catalog
    }

    fn floor_world() -> World {
        let mut world = World::new();
        let chunk = world.get_or_create_chunk(ChunkPos { x: 0, y: 0, z: 0 });
        for z in 0..16 {
            for x in 0..16 {
                chunk.set(x, 0, z, BlockId(3)).unwrap();
            }
        }
        world
    }

    #[test]
    fn prototype_loot_maps_only_known_prototype_blocks() {
        let loot = BlockLootTable::prototype();
        assert_eq!(
            loot.drop_for(BlockId(1)).unwrap().key,
            ItemKey::new(ItemId(1))
        );
        assert_eq!(loot.drop_for(BlockId(2)).unwrap().count, 1);
        assert_eq!(loot.drop_for(BlockId(99)), None);
        assert_eq!(loot.drop_for(BlockId(0)), None);

        let catalog = catalog();
        let mut custom = BlockLootTable::default();
        let stack = ItemStack {
            key: ItemKey::new(ItemId(1)),
            count: 2,
        };
        custom.register(BlockId(10), stack, &catalog).unwrap();
        assert_eq!(custom.drop_for(BlockId(10)), Some(stack));
        assert_eq!(
            custom.register(BlockId(10), stack, &catalog),
            Err(DropError::DuplicateLoot)
        );
    }

    #[test]
    fn pickup_waits_for_delay_then_transfers_once_to_inventory() {
        let catalog = catalog();
        let world = floor_world();
        let key = ItemKey::new(ItemId(1));
        let mut drops = ItemDrops::default();
        drops
            .spawn(ItemStack { key, count: 4 }, [2.5, 1.125, 2.5], 2, &catalog)
            .unwrap();
        let mut inventory = Inventory::default();
        let first = drops
            .tick(
                &world,
                [2.5, 1.125, 2.5],
                &mut inventory,
                &catalog,
                &BlockRegistry::new(),
            )
            .unwrap();
        assert_eq!(first.picked_up_items, 0);
        assert_eq!(drops.len(), 1);
        let second = drops
            .tick(
                &world,
                [2.5, 1.125, 2.5],
                &mut inventory,
                &catalog,
                &BlockRegistry::new(),
            )
            .unwrap();
        assert_eq!(second.picked_up_items, 4);
        assert_eq!(second.removed_entities, 1);
        assert_eq!(drops.len(), 0);
        assert_eq!(inventory.count(key), 4);
    }

    #[test]
    fn full_inventory_keeps_uncollected_drop_without_losing_count() {
        let catalog = catalog();
        let world = floor_world();
        let key = ItemKey::new(ItemId(1));
        let mut inventory = Inventory::default();
        inventory.insert(key, 36 * 64, &catalog).unwrap();
        let mut drops = ItemDrops::default();
        drops
            .spawn(ItemStack { key, count: 12 }, [3.5, 1.125, 3.5], 0, &catalog)
            .unwrap();
        let result = drops
            .tick(
                &world,
                [3.5, 1.125, 3.5],
                &mut inventory,
                &catalog,
                &BlockRegistry::new(),
            )
            .unwrap();
        assert_eq!(result.picked_up_items, 0);
        assert_eq!(drops.iter().next().unwrap().stack.count, 12);
        assert_eq!(inventory.count(key), 36 * 64);
    }

    #[test]
    fn partial_capacity_pickup_leaves_only_the_remainder_in_world() {
        let catalog = catalog();
        let world = floor_world();
        let key = ItemKey::new(ItemId(1));
        let mut inventory = Inventory::default();
        inventory.insert(key, 36 * 64 - 3, &catalog).unwrap();
        let mut drops = ItemDrops::default();
        drops
            .spawn(ItemStack { key, count: 8 }, [3.5, 1.125, 3.5], 0, &catalog)
            .unwrap();
        let result = drops
            .tick(
                &world,
                [3.5, 1.125, 3.5],
                &mut inventory,
                &catalog,
                &BlockRegistry::new(),
            )
            .unwrap();
        assert_eq!(result.picked_up_items, 3);
        assert_eq!(drops.iter().next().unwrap().stack.count, 5);
        assert_eq!(inventory.count(key), 36 * 64);
    }

    #[test]
    fn drop_spawn_validates_catalog_position_and_world_entity_cap() {
        let catalog = catalog();
        let mut drops = ItemDrops::default();
        let valid = ItemStack {
            key: ItemKey::new(ItemId(1)),
            count: 1,
        };
        assert_eq!(
            drops.spawn(valid, [f64::NAN, 0.0, 0.0], 0, &catalog),
            Err(DropError::InvalidPosition)
        );
        assert_eq!(drops.len(), 0);
        for _ in 0..MAX_DROPPED_ITEMS {
            drops.spawn(valid, [0.5, 2.0, 0.5], 0, &catalog).unwrap();
        }
        assert_eq!(
            drops.spawn(valid, [0.5, 2.0, 0.5], 0, &catalog),
            Err(DropError::Capacity)
        );
        assert_eq!(drops.len(), MAX_DROPPED_ITEMS);
    }

    #[test]
    fn dropped_item_lands_on_a_full_block_without_sinking() {
        let catalog = catalog();
        let world = floor_world();
        let mut drops = ItemDrops::default();
        drops
            .spawn(
                ItemStack {
                    key: ItemKey::new(ItemId(1)),
                    count: 1,
                },
                [4.5, 1.2, 4.5],
                u16::MAX,
                &catalog,
            )
            .unwrap();
        let mut inventory = Inventory::default();
        for _ in 0..40 {
            drops
                .tick(
                    &world,
                    [12.5, 12.0, 12.5],
                    &mut inventory,
                    &catalog,
                    &BlockRegistry::new(),
                )
                .unwrap();
        }
        let item = drops.iter().next().unwrap();
        assert!((item.position[1] - 1.125).abs() < 1e-9);
        assert_eq!(item.velocity[1], 0.0);
    }

    #[test]
    fn expired_items_are_removed_without_being_added_to_inventory() {
        let catalog = catalog();
        let world = floor_world();
        let key = ItemKey::new(ItemId(1));
        let mut drops = ItemDrops::default();
        let id = drops
            .spawn(
                ItemStack { key, count: 2 },
                [2.5, 1.125, 2.5],
                u16::MAX,
                &catalog,
            )
            .unwrap();
        drops.items.get_mut(&id).unwrap().age_ticks = ITEM_DESPAWN_TICKS - 1;
        let mut inventory = Inventory::default();
        let result = drops
            .tick(
                &world,
                [2.5, 1.125, 2.5],
                &mut inventory,
                &catalog,
                &BlockRegistry::new(),
            )
            .unwrap();
        assert_eq!(result.removed_entities, 1);
        assert_eq!(result.picked_up_items, 0);
        assert!(drops.is_empty());
        assert_eq!(inventory.count(key), 0);
    }

    #[test]
    fn exhausted_ids_and_out_of_range_positions_fail_without_spawning() {
        let catalog = catalog();
        let stack = ItemStack {
            key: ItemKey::new(ItemId(1)),
            count: 1,
        };
        let mut drops = ItemDrops {
            next_id: u64::MAX,
            ..ItemDrops::default()
        };
        assert_eq!(
            drops.spawn(stack, [0.0, 1.0, 0.0], 0, &catalog),
            Err(DropError::IdExhausted)
        );
        assert_eq!(drops.len(), 0);
        assert_eq!(
            drops.spawn(stack, [i32::MAX as f64, 1.0, 0.0], 0, &catalog),
            Err(DropError::InvalidPosition)
        );
        assert_eq!(drops.len(), 0);
    }
}
