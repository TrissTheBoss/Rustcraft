//! Original provisional consumable profile and fixed-tick item-use transaction.
use crate::inventory::{Inventory, InventorySlot, ItemKey};
use crate::survival::{FoodState, MAX_FOOD_LEVEL};

pub const RATION_ITEM_ID: u16 = 11;
pub const RATION_USE_TICKS: u16 = 32;

pub fn is_food(key: ItemKey) -> bool {
    key.id.0 == RATION_ITEM_ID && key.variant == 0
}

#[derive(Default)]
pub struct FoodUseSession {
    target: Option<(u8, ItemKey)>,
    ticks: u16,
}

impl FoodUseSession {
    pub fn reset(&mut self) {
        self.target = None;
        self.ticks = 0;
    }

    /// Returns true only after both inventory and hunger commit together.
    pub fn tick(&mut self, held: bool, inventory: &mut Inventory, food: &mut FoodState) -> bool {
        let target = inventory
            .selected_stack()
            .filter(|stack| is_food(stack.key))
            .map(|stack| (inventory.selected_hotbar(), stack.key));
        if !held || target.is_none() || food.food_level() == MAX_FOOD_LEVEL {
            self.reset();
            return false;
        }
        if self.target != target {
            self.reset();
            self.target = target;
        }
        self.ticks += 1;
        if self.ticks < RATION_USE_TICKS {
            return false;
        }
        self.reset();
        let mut restored = *food;
        if !restored.eat(4, 2_400) {
            return false;
        }
        let slot = InventorySlot::Main(usize::from(inventory.selected_hotbar()));
        if inventory.consume(slot, 1).is_err() {
            return false;
        }
        *food = restored;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{ItemCatalog, ItemDefinition, ItemId};

    fn setup() -> (Inventory, FoodState) {
        let mut catalog = ItemCatalog::default();
        catalog
            .register(ItemId(RATION_ITEM_ID), ItemDefinition::stackable(64))
            .unwrap();
        let mut inventory = Inventory::default();
        inventory
            .insert(ItemKey::new(ItemId(RATION_ITEM_ID)), 2, &catalog)
            .unwrap();
        (inventory, FoodState::from_saved(18, 17_000, 200).unwrap())
    }

    #[test]
    fn held_use_consumes_once_and_caps_food_and_saturation() {
        let (mut inventory, mut food) = setup();
        let mut session = FoodUseSession::default();
        for _ in 1..RATION_USE_TICKS {
            assert!(!session.tick(true, &mut inventory, &mut food));
        }
        assert_eq!(inventory.selected_stack().unwrap().count, 2);
        assert!(session.tick(true, &mut inventory, &mut food));
        assert_eq!(inventory.selected_stack().unwrap().count, 1);
        assert_eq!(food.food_level(), 20);
        assert_eq!(food.saturation_milli(), 19_400);
        assert_eq!(food.exhaustion_milli(), 200);
        for _ in 0..64 {
            assert!(!session.tick(true, &mut inventory, &mut food));
        }
        assert_eq!(inventory.selected_stack().unwrap().count, 1);
    }

    #[test]
    fn release_and_slot_change_cancel_progress_without_consumption() {
        let (mut inventory, mut food) = setup();
        let mut session = FoodUseSession::default();
        for _ in 0..31 {
            session.tick(true, &mut inventory, &mut food);
        }
        session.tick(false, &mut inventory, &mut food);
        assert!(!session.tick(true, &mut inventory, &mut food));
        inventory.select_hotbar(1).unwrap();
        assert!(!session.tick(true, &mut inventory, &mut food));
        inventory.select_hotbar(0).unwrap();
        assert!(!session.tick(true, &mut inventory, &mut food));
        assert_eq!(inventory.selected_stack().unwrap().count, 2);
        assert_eq!(food.food_level(), 18);
    }

    #[test]
    fn missing_or_modified_items_cannot_complete_use_and_restoration_stays_bounded() {
        let (mut inventory, mut food) = setup();
        let mut session = FoodUseSession::default();
        for _ in 0..31 {
            session.tick(true, &mut inventory, &mut food);
        }
        inventory.consume(InventorySlot::Main(0), 2).unwrap();
        assert!(!session.tick(true, &mut inventory, &mut food));
        assert_eq!(food.food_level(), 18);
        assert!(!is_food(ItemKey {
            id: crate::inventory::ItemId(RATION_ITEM_ID),
            variant: 1
        }));
        let mut low_food = FoodState::from_saved(1, 500, 0).unwrap();
        assert!(low_food.eat(1, u16::MAX));
        assert_eq!(low_food.saturation_milli(), 2_000);
        assert_eq!(low_food.food_level(), 2);
    }
}
