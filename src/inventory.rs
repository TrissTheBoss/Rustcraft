//! Transactional player inventory primitives.
//!
//! The model separates item identity and stack rules from the window layer so
//! inventory UI, saves, crafting, and multiplayer can share the same checks.
//! It intentionally contains no client artwork or copied game implementation.

use std::collections::BTreeMap;
use std::fmt;

use crate::drops::{DropError, DropId, ItemDrops};

pub const HOTBAR_SLOT_COUNT: usize = 9;
pub const MAIN_SLOT_COUNT: usize = 36;
pub(crate) const SAVED_SLOT_COUNT: usize = MAIN_SLOT_COUNT + 5;
pub const MAX_CATALOG_STACK_SIZE: u16 = 64;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ItemId(pub u16);

/// An item key distinguishes otherwise identical item IDs that carry a stable
/// variant value, such as a future durability or data component identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ItemKey {
    pub id: ItemId,
    pub variant: u32,
}

impl ItemKey {
    pub const fn new(id: ItemId) -> Self {
        Self { id, variant: 0 }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ItemStack {
    pub key: ItemKey,
    pub count: u16,
}

impl ItemStack {
    pub fn new(key: ItemKey, count: u16, catalog: &ItemCatalog) -> Result<Self, InventoryError> {
        let definition = catalog.definition(key.id)?;
        if count == 0 || count > definition.max_stack_size {
            return Err(InventoryError::InvalidCount);
        }
        Ok(Self { key, count })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArmorSlot {
    Head,
    Chest,
    Legs,
    Feet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EquipmentSlot {
    Armor(ArmorSlot),
    Offhand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ItemDefinition {
    pub max_stack_size: u16,
    pub equipment_slot: Option<EquipmentSlot>,
}

impl ItemDefinition {
    pub const fn stackable(max_stack_size: u16) -> Self {
        Self {
            max_stack_size,
            equipment_slot: None,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ItemCatalog {
    definitions: BTreeMap<ItemId, ItemDefinition>,
}

impl ItemCatalog {
    pub fn register(
        &mut self,
        id: ItemId,
        definition: ItemDefinition,
    ) -> Result<(), InventoryError> {
        if definition.max_stack_size == 0
            || definition.max_stack_size > MAX_CATALOG_STACK_SIZE
            || (definition.equipment_slot.is_some() && definition.max_stack_size != 1)
            || self.definitions.contains_key(&id)
        {
            return Err(InventoryError::InvalidDefinition);
        }
        self.definitions.insert(id, definition);
        Ok(())
    }

    pub fn definition(&self, id: ItemId) -> Result<ItemDefinition, InventoryError> {
        self.definitions
            .get(&id)
            .copied()
            .ok_or(InventoryError::UnknownItem)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventorySlot {
    Main(usize),
    Armor(ArmorSlot),
    Offhand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClickButton {
    Primary,
    Secondary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryError {
    UnknownItem,
    InvalidDefinition,
    InvalidCount,
    InvalidSlot,
    IncompatibleItem,
    NotEnoughItems,
    NoCapacity,
}

impl fmt::Display for InventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnknownItem => "item is not in the catalog",
            Self::InvalidDefinition => "item definition is invalid or duplicated",
            Self::InvalidCount => "item stack count is invalid",
            Self::InvalidSlot => "inventory slot is out of range",
            Self::IncompatibleItem => "item cannot occupy that equipment slot",
            Self::NotEnoughItems => "source stack does not contain that many items",
            Self::NoCapacity => "inventory operation would exceed stack capacity",
        })
    }
}

impl std::error::Error for InventoryError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Inventory {
    main: [Option<ItemStack>; MAIN_SLOT_COUNT],
    armor: [Option<ItemStack>; 4],
    offhand: Option<ItemStack>,
    selected_hotbar: u8,
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            main: [None; MAIN_SLOT_COUNT],
            armor: [None; 4],
            offhand: None,
            selected_hotbar: 0,
        }
    }
}

impl Inventory {
    pub fn selected_hotbar(&self) -> u8 {
        self.selected_hotbar
    }

    pub fn select_hotbar(&mut self, selected: usize) -> Result<(), InventoryError> {
        if selected >= HOTBAR_SLOT_COUNT {
            return Err(InventoryError::InvalidSlot);
        }
        self.selected_hotbar = selected as u8;
        Ok(())
    }

    pub fn selected_stack(&self) -> Option<ItemStack> {
        self.main[self.selected_hotbar as usize]
    }

    pub(crate) fn saved_slots(&self) -> [Option<ItemStack>; SAVED_SLOT_COUNT] {
        let mut slots = [None; SAVED_SLOT_COUNT];
        slots[..MAIN_SLOT_COUNT].copy_from_slice(&self.main);
        slots[MAIN_SLOT_COUNT..MAIN_SLOT_COUNT + 4].copy_from_slice(&self.armor);
        slots[MAIN_SLOT_COUNT + 4] = self.offhand;
        slots
    }

    pub(crate) fn from_saved_slots(
        slots: [Option<ItemStack>; SAVED_SLOT_COUNT],
        selected_hotbar: u8,
    ) -> Result<Self, InventoryError> {
        if usize::from(selected_hotbar) >= HOTBAR_SLOT_COUNT {
            return Err(InventoryError::InvalidSlot);
        }
        for (index, stack) in slots.iter().enumerate() {
            let Some(stack) = stack else {
                continue;
            };
            if stack.count == 0
                || stack.count > MAX_CATALOG_STACK_SIZE
                || ((MAIN_SLOT_COUNT..MAIN_SLOT_COUNT + 4).contains(&index) && stack.count != 1)
            {
                return Err(InventoryError::InvalidCount);
            }
        }
        let main = std::array::from_fn(|index| slots[index]);
        let armor = std::array::from_fn(|index| slots[MAIN_SLOT_COUNT + index]);
        Ok(Self {
            main,
            armor,
            offhand: slots[MAIN_SLOT_COUNT + 4],
            selected_hotbar,
        })
    }

    pub fn stack(&self, slot: InventorySlot) -> Result<Option<ItemStack>, InventoryError> {
        match slot {
            InventorySlot::Main(index) => self
                .main
                .get(index)
                .copied()
                .ok_or(InventoryError::InvalidSlot),
            InventorySlot::Armor(slot) => Ok(self.armor[armor_index(slot)]),
            InventorySlot::Offhand => Ok(self.offhand),
        }
    }

    pub fn count(&self, key: ItemKey) -> u32 {
        self.main
            .iter()
            .chain(self.armor.iter())
            .chain(std::iter::once(&self.offhand))
            .flatten()
            .filter(|stack| stack.key == key)
            .map(|stack| u32::from(stack.count))
            .sum()
    }

    /// Insert as many items as fit, merging identical keys before using empty
    /// main-inventory slots. Returns the quantity that did not fit.
    pub fn insert(
        &mut self,
        key: ItemKey,
        mut count: u32,
        catalog: &ItemCatalog,
    ) -> Result<u32, InventoryError> {
        let max_stack = catalog.definition(key.id)?.max_stack_size;
        if count == 0 {
            return Ok(0);
        }
        for stack in self
            .main
            .iter_mut()
            .flatten()
            .filter(|stack| stack.key == key)
        {
            let room = u32::from(max_stack - stack.count);
            let inserted = count.min(room);
            stack.count += inserted as u16;
            count -= inserted;
            if count == 0 {
                return Ok(0);
            }
        }
        for slot in &mut self.main {
            if slot.is_none() {
                let inserted = count.min(u32::from(max_stack));
                *slot = Some(ItemStack {
                    key,
                    count: inserted as u16,
                });
                count -= inserted;
                if count == 0 {
                    return Ok(0);
                }
            }
        }
        Ok(count)
    }

    /// Move an exact quantity. Failure leaves both slots unchanged.
    pub fn transfer(
        &mut self,
        source: InventorySlot,
        destination: InventorySlot,
        count: u16,
        catalog: &ItemCatalog,
    ) -> Result<(), InventoryError> {
        if count == 0 || source == destination {
            return Err(InventoryError::InvalidCount);
        }
        let source_stack = self.stack(source)?.ok_or(InventoryError::NotEnoughItems)?;
        if source_stack.count < count {
            return Err(InventoryError::NotEnoughItems);
        }
        self.validate_slot(destination, source_stack.key, catalog)?;
        let destination_stack = self.stack(destination)?;
        let max_stack = catalog.definition(source_stack.key.id)?.max_stack_size;
        match destination_stack {
            None if count <= max_stack => {}
            Some(target) if target.key == source_stack.key => {
                if u32::from(target.count) + u32::from(count) > u32::from(max_stack) {
                    return Err(InventoryError::NoCapacity);
                }
            }
            Some(_) => return Err(InventoryError::NoCapacity),
            None => return Err(InventoryError::NoCapacity),
        }

        self.set_stack(
            source,
            (source_stack.count > count).then_some(ItemStack {
                count: source_stack.count - count,
                ..source_stack
            }),
        )?;
        let new_target = match destination_stack {
            Some(target) => Some(ItemStack {
                count: target.count + count,
                ..target
            }),
            None => Some(ItemStack {
                key: source_stack.key,
                count,
            }),
        };
        self.set_stack(destination, new_target)
    }

    pub fn swap(
        &mut self,
        first: InventorySlot,
        second: InventorySlot,
        catalog: &ItemCatalog,
    ) -> Result<(), InventoryError> {
        let first_stack = self.stack(first)?;
        let second_stack = self.stack(second)?;
        if let Some(stack) = second_stack {
            self.validate_slot(first, stack.key, catalog)?;
        }
        if let Some(stack) = first_stack {
            self.validate_slot(second, stack.key, catalog)?;
        }
        self.set_stack(first, second_stack)?;
        self.set_stack(second, first_stack)
    }

    pub fn consume(
        &mut self,
        slot: InventorySlot,
        count: u16,
    ) -> Result<ItemStack, InventoryError> {
        if count == 0 {
            return Err(InventoryError::InvalidCount);
        }
        let stack = self.stack(slot)?.ok_or(InventoryError::NotEnoughItems)?;
        if stack.count < count {
            return Err(InventoryError::NotEnoughItems);
        }
        self.set_stack(
            slot,
            (stack.count > count).then_some(ItemStack {
                count: stack.count - count,
                ..stack
            }),
        )?;
        Ok(ItemStack { count, ..stack })
    }

    /// Drop part of a slot stack into the world. Spawn validation happens
    /// before the slot changes; a failed spawn restores the exact prior stack.
    pub fn drop_from_slot(
        &mut self,
        slot: InventorySlot,
        count: u16,
        drops: &mut ItemDrops,
        position: [f64; 3],
        pickup_delay_ticks: u16,
        catalog: &ItemCatalog,
    ) -> Result<Option<DropId>, DropError> {
        if count == 0 {
            return Err(DropError::InvalidStack);
        }
        let stack = self.stack(slot).map_err(DropError::Inventory)?;
        let Some(stack) = stack else {
            return Ok(None);
        };
        if count > stack.count {
            return Err(DropError::Inventory(InventoryError::NotEnoughItems));
        }
        let dropped = ItemStack { count, ..stack };
        drops.validate_spawn(dropped, position, catalog)?;
        let remaining = (stack.count > count).then_some(ItemStack {
            count: stack.count - count,
            ..stack
        });
        self.set_stack(slot, remaining)
            .map_err(DropError::Inventory)?;
        match drops.spawn(dropped, position, pickup_delay_ticks, catalog) {
            Ok(id) => Ok(Some(id)),
            Err(error) => {
                let _ = self.set_stack(slot, Some(stack));
                Err(error)
            }
        }
    }

    fn validate_slot(
        &self,
        slot: InventorySlot,
        key: ItemKey,
        catalog: &ItemCatalog,
    ) -> Result<(), InventoryError> {
        let definition = catalog.definition(key.id)?;
        let required = match slot {
            InventorySlot::Main(index) if index < MAIN_SLOT_COUNT => return Ok(()),
            InventorySlot::Main(_) => return Err(InventoryError::InvalidSlot),
            InventorySlot::Armor(armor) => Some(EquipmentSlot::Armor(armor)),
            InventorySlot::Offhand => None,
        };
        if required.is_some() && definition.equipment_slot != required {
            return Err(InventoryError::IncompatibleItem);
        }
        Ok(())
    }

    fn set_stack(
        &mut self,
        slot: InventorySlot,
        stack: Option<ItemStack>,
    ) -> Result<(), InventoryError> {
        match slot {
            InventorySlot::Main(index) => {
                *self
                    .main
                    .get_mut(index)
                    .ok_or(InventoryError::InvalidSlot)? = stack;
            }
            InventorySlot::Armor(armor) => self.armor[armor_index(armor)] = stack,
            InventorySlot::Offhand => self.offhand = stack,
        }
        Ok(())
    }
}

fn armor_index(slot: ArmorSlot) -> usize {
    match slot {
        ArmorSlot::Head => 0,
        ArmorSlot::Chest => 1,
        ArmorSlot::Legs => 2,
        ArmorSlot::Feet => 3,
    }
}

/// Cursor state and click semantics for a player inventory screen. Closing the
/// menu either returns the full cursor stack or leaves it untouched on failure.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InventoryMenu {
    cursor: Option<ItemStack>,
}

impl InventoryMenu {
    pub fn cursor(&self) -> Option<ItemStack> {
        self.cursor
    }

    pub fn click(
        &mut self,
        inventory: &mut Inventory,
        slot: InventorySlot,
        button: ClickButton,
        catalog: &ItemCatalog,
    ) -> Result<(), InventoryError> {
        let target = inventory.stack(slot)?;
        match (button, self.cursor, target) {
            (ClickButton::Primary, None, Some(stack)) => {
                inventory.set_stack(slot, None)?;
                self.cursor = Some(stack);
            }
            (ClickButton::Secondary, None, Some(stack)) => {
                let picked_up = stack.count.div_ceil(2);
                inventory.set_stack(
                    slot,
                    (stack.count > picked_up).then_some(ItemStack {
                        count: stack.count - picked_up,
                        ..stack
                    }),
                )?;
                self.cursor = Some(ItemStack {
                    count: picked_up,
                    ..stack
                });
            }
            (_, None, None) => {}
            (ClickButton::Primary, Some(cursor), None) => {
                inventory.validate_slot(slot, cursor.key, catalog)?;
                inventory.set_stack(slot, Some(cursor))?;
                self.cursor = None;
            }
            (ClickButton::Secondary, Some(cursor), None) => {
                inventory.validate_slot(slot, cursor.key, catalog)?;
                inventory.set_stack(slot, Some(ItemStack { count: 1, ..cursor }))?;
                self.cursor = (cursor.count > 1).then_some(ItemStack {
                    count: cursor.count - 1,
                    ..cursor
                });
            }
            (ClickButton::Primary, Some(cursor), Some(target)) if cursor.key == target.key => {
                let max_stack = catalog.definition(cursor.key.id)?.max_stack_size;
                let moved = cursor.count.min(max_stack - target.count);
                if moved > 0 {
                    inventory.set_stack(
                        slot,
                        Some(ItemStack {
                            count: target.count + moved,
                            ..target
                        }),
                    )?;
                    self.cursor = (cursor.count > moved).then_some(ItemStack {
                        count: cursor.count - moved,
                        ..cursor
                    });
                }
            }
            (ClickButton::Secondary, Some(cursor), Some(target)) if cursor.key == target.key => {
                let max_stack = catalog.definition(cursor.key.id)?.max_stack_size;
                if cursor.count > 0 && target.count < max_stack {
                    inventory.set_stack(
                        slot,
                        Some(ItemStack {
                            count: target.count + 1,
                            ..target
                        }),
                    )?;
                    self.cursor = (cursor.count > 1).then_some(ItemStack {
                        count: cursor.count - 1,
                        ..cursor
                    });
                }
            }
            (ClickButton::Primary, Some(cursor), Some(target)) => {
                inventory.validate_slot(slot, cursor.key, catalog)?;
                self.cursor = Some(target);
                inventory.set_stack(slot, Some(cursor))?;
            }
            (ClickButton::Secondary, Some(_), Some(_)) => {}
        }
        Ok(())
    }

    pub fn return_cursor(
        &mut self,
        inventory: &mut Inventory,
        catalog: &ItemCatalog,
    ) -> Result<(), InventoryError> {
        let Some(cursor) = self.cursor else {
            return Ok(());
        };
        let max_stack = catalog.definition(cursor.key.id)?.max_stack_size;
        let mut capacity = 0u32;
        for stack in inventory
            .main
            .iter()
            .flatten()
            .filter(|stack| stack.key == cursor.key)
        {
            capacity += u32::from(max_stack - stack.count);
        }
        capacity += inventory.main.iter().filter(|slot| slot.is_none()).count() as u32
            * u32::from(max_stack);
        if capacity < u32::from(cursor.count) {
            return Err(InventoryError::NoCapacity);
        }
        let remaining = inventory.insert(cursor.key, u32::from(cursor.count), catalog)?;
        debug_assert_eq!(remaining, 0);
        self.cursor = None;
        Ok(())
    }

    /// Spawn the carried stack into the world and clear the cursor only after
    /// the drop entity has been validated and created successfully.
    pub fn drop_cursor(
        &mut self,
        drops: &mut ItemDrops,
        position: [f64; 3],
        pickup_delay_ticks: u16,
        catalog: &ItemCatalog,
    ) -> Result<Option<DropId>, DropError> {
        let Some(stack) = self.cursor else {
            return Ok(None);
        };
        drops.validate_spawn(stack, position, catalog)?;
        let id = drops.spawn(stack, position, pickup_delay_ticks, catalog)?;
        self.cursor = None;
        Ok(Some(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> ItemCatalog {
        let mut catalog = ItemCatalog::default();
        catalog
            .register(ItemId(1), ItemDefinition::stackable(64))
            .unwrap();
        catalog
            .register(ItemId(2), ItemDefinition::stackable(16))
            .unwrap();
        catalog
            .register(
                ItemId(3),
                ItemDefinition {
                    max_stack_size: 1,
                    equipment_slot: Some(EquipmentSlot::Armor(ArmorSlot::Head)),
                },
            )
            .unwrap();
        catalog
    }

    fn key(id: u16) -> ItemKey {
        ItemKey::new(ItemId(id))
    }

    #[test]
    fn catalog_rejects_invalid_or_duplicate_stack_definitions() {
        let mut catalog = ItemCatalog::default();
        assert_eq!(
            catalog.register(ItemId(1), ItemDefinition::stackable(0)),
            Err(InventoryError::InvalidDefinition)
        );
        assert_eq!(
            catalog.register(ItemId(1), ItemDefinition::stackable(65)),
            Err(InventoryError::InvalidDefinition)
        );
        assert_eq!(
            catalog.register(
                ItemId(2),
                ItemDefinition {
                    max_stack_size: 2,
                    equipment_slot: Some(EquipmentSlot::Armor(ArmorSlot::Head)),
                },
            ),
            Err(InventoryError::InvalidDefinition)
        );
        catalog
            .register(ItemId(1), ItemDefinition::stackable(64))
            .unwrap();
        assert_eq!(
            catalog.register(ItemId(1), ItemDefinition::stackable(64)),
            Err(InventoryError::InvalidDefinition)
        );
    }

    #[test]
    fn insertion_merges_then_fills_slots_without_exceeding_limits() {
        let catalog = catalog();
        let mut inventory = Inventory::default();
        inventory.insert(key(1), 60, &catalog).unwrap();
        inventory.insert(key(1), 10, &catalog).unwrap();
        assert_eq!(
            inventory
                .stack(InventorySlot::Main(0))
                .unwrap()
                .unwrap()
                .count,
            64
        );
        assert_eq!(
            inventory
                .stack(InventorySlot::Main(1))
                .unwrap()
                .unwrap()
                .count,
            6
        );
        assert_eq!(inventory.count(key(1)), 70);
    }

    #[test]
    fn transfer_is_exact_atomic_and_conserves_item_count() {
        let catalog = catalog();
        let mut inventory = Inventory::default();
        inventory.insert(key(1), 20, &catalog).unwrap();
        inventory
            .transfer(InventorySlot::Main(0), InventorySlot::Main(1), 7, &catalog)
            .unwrap();
        assert_eq!(inventory.count(key(1)), 20);
        assert_eq!(
            inventory
                .stack(InventorySlot::Main(0))
                .unwrap()
                .unwrap()
                .count,
            13
        );
        assert_eq!(
            inventory
                .stack(InventorySlot::Main(1))
                .unwrap()
                .unwrap()
                .count,
            7
        );
        let before = inventory.clone();
        assert_eq!(
            inventory.transfer(InventorySlot::Main(0), InventorySlot::Main(1), 60, &catalog,),
            Err(InventoryError::NotEnoughItems)
        );
        assert_eq!(inventory, before);
        assert_eq!(
            inventory.transfer(InventorySlot::Main(0), InventorySlot::Main(1), 60, &catalog,),
            Err(InventoryError::NotEnoughItems)
        );
        assert_eq!(inventory, before);
    }

    #[test]
    fn full_destination_rejects_transfer_without_partial_mutation() {
        let catalog = catalog();
        let mut inventory = Inventory::default();
        inventory.insert(key(1), 128, &catalog).unwrap();
        let before = inventory.clone();
        assert_eq!(
            inventory.transfer(InventorySlot::Main(0), InventorySlot::Main(1), 1, &catalog,),
            Err(InventoryError::NoCapacity)
        );
        assert_eq!(inventory, before);
    }

    #[test]
    fn distinct_item_variants_do_not_merge() {
        let catalog = catalog();
        let mut inventory = Inventory::default();
        let first = key(1);
        let second = ItemKey {
            id: first.id,
            variant: 1,
        };
        inventory.insert(first, 4, &catalog).unwrap();
        inventory.insert(second, 5, &catalog).unwrap();
        assert_eq!(
            inventory
                .stack(InventorySlot::Main(0))
                .unwrap()
                .unwrap()
                .key,
            first
        );
        assert_eq!(
            inventory
                .stack(InventorySlot::Main(1))
                .unwrap()
                .unwrap()
                .key,
            second
        );
        assert_eq!(inventory.count(first), 4);
        assert_eq!(inventory.count(second), 5);
    }

    #[test]
    fn equipment_slots_enforce_item_compatibility_and_failed_swaps_are_atomic() {
        let catalog = catalog();
        let mut inventory = Inventory::default();
        inventory.insert(key(3), 1, &catalog).unwrap();
        inventory
            .transfer(
                InventorySlot::Main(0),
                InventorySlot::Armor(ArmorSlot::Head),
                1,
                &catalog,
            )
            .unwrap();
        inventory.insert(key(1), 3, &catalog).unwrap();
        let before = inventory.clone();
        assert_eq!(
            inventory.swap(
                InventorySlot::Main(0),
                InventorySlot::Armor(ArmorSlot::Head),
                &catalog,
            ),
            Err(InventoryError::IncompatibleItem)
        );
        assert_eq!(inventory, before);
    }

    #[test]
    fn consume_and_hotbar_selection_validate_without_losing_items() {
        let catalog = catalog();
        let mut inventory = Inventory::default();
        inventory.insert(key(1), 12, &catalog).unwrap();
        inventory.select_hotbar(8).unwrap();
        assert_eq!(inventory.selected_hotbar(), 8);
        assert_eq!(inventory.select_hotbar(9), Err(InventoryError::InvalidSlot));
        assert_eq!(
            inventory.consume(InventorySlot::Main(0), 5).unwrap().count,
            5
        );
        assert_eq!(inventory.count(key(1)), 7);
    }

    #[test]
    fn dropping_a_slot_preserves_items_on_validation_failure_and_supports_partial_stacks() {
        let catalog = catalog();
        let mut inventory = Inventory::default();
        inventory.insert(key(1), 3, &catalog).unwrap();
        let mut drops = ItemDrops::default();
        let slot = InventorySlot::Main(0);

        let id = inventory
            .drop_from_slot(slot, 1, &mut drops, [1.5, 2.0, 1.5], 4, &catalog)
            .unwrap()
            .unwrap();
        assert_eq!(drops.iter().next().unwrap().id, id);
        assert_eq!(drops.iter().next().unwrap().stack.count, 1);
        assert_eq!(inventory.stack(slot).unwrap().unwrap().count, 2);

        let before = inventory;
        assert_eq!(
            inventory.drop_from_slot(slot, 1, &mut drops, [f64::NAN, 2.0, 1.5], 4, &catalog),
            Err(DropError::InvalidPosition)
        );
        assert_eq!(inventory, before);

        inventory
            .drop_from_slot(slot, 2, &mut drops, [1.5, 2.0, 1.5], 4, &catalog)
            .unwrap()
            .unwrap();
        assert_eq!(inventory.stack(slot).unwrap(), None);
        assert_eq!(drops.iter().map(|item| item.stack.count).sum::<u16>(), 3);
    }

    #[test]
    fn swapping_the_selected_main_slot_with_offhand_is_transactional() {
        let catalog = catalog();
        let mut inventory = Inventory::default();
        inventory.insert(key(1), 5, &catalog).unwrap();
        inventory.insert(key(2), 2, &catalog).unwrap();
        inventory.select_hotbar(1).unwrap();

        inventory
            .swap(InventorySlot::Main(1), InventorySlot::Offhand, &catalog)
            .unwrap();
        assert_eq!(inventory.stack(InventorySlot::Main(1)).unwrap(), None);
        assert_eq!(
            inventory
                .stack(InventorySlot::Offhand)
                .unwrap()
                .unwrap()
                .key,
            key(2)
        );

        inventory
            .swap(InventorySlot::Main(0), InventorySlot::Offhand, &catalog)
            .unwrap();
        assert_eq!(
            inventory
                .stack(InventorySlot::Main(0))
                .unwrap()
                .unwrap()
                .key,
            key(2)
        );
        assert_eq!(
            inventory
                .stack(InventorySlot::Offhand)
                .unwrap()
                .unwrap()
                .key,
            key(1)
        );
        assert_eq!(inventory.selected_hotbar(), 1);
    }

    #[test]
    fn cursor_primary_and_secondary_clicks_split_merge_and_return_atomically() {
        let catalog = catalog();
        let mut inventory = Inventory::default();
        inventory.insert(key(1), 9, &catalog).unwrap();
        let mut menu = InventoryMenu::default();
        menu.click(
            &mut inventory,
            InventorySlot::Main(0),
            ClickButton::Secondary,
            &catalog,
        )
        .unwrap();
        assert_eq!(menu.cursor().unwrap().count, 5);
        assert_eq!(
            inventory
                .stack(InventorySlot::Main(0))
                .unwrap()
                .unwrap()
                .count,
            4
        );
        menu.click(
            &mut inventory,
            InventorySlot::Main(1),
            ClickButton::Secondary,
            &catalog,
        )
        .unwrap();
        assert_eq!(menu.cursor().unwrap().count, 4);
        assert_eq!(
            inventory
                .stack(InventorySlot::Main(1))
                .unwrap()
                .unwrap()
                .count,
            1
        );
        menu.return_cursor(&mut inventory, &catalog).unwrap();
        assert_eq!(menu.cursor(), None);
        assert_eq!(inventory.count(key(1)), 9);
    }

    #[test]
    fn cursor_cannot_be_lost_when_return_capacity_is_insufficient() {
        let catalog = catalog();
        let mut inventory = Inventory::default();
        let mut menu = InventoryMenu {
            cursor: Some(ItemStack {
                key: key(1),
                count: 5,
            }),
        };
        inventory
            .insert(key(1), (MAIN_SLOT_COUNT * 64) as u32, &catalog)
            .unwrap();
        assert_eq!(
            menu.return_cursor(&mut inventory, &catalog),
            Err(InventoryError::NoCapacity)
        );
        assert_eq!(menu.cursor().unwrap().count, 5);
    }
}
