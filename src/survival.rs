//! Player survival status primitives.
//!
//! Values here are an unverified prototype profile. The pinned Java Edition
//! baseline must supply black-box fixtures before these values can be treated
//! as conformance behavior.

pub const MAX_HEALTH: u8 = 20;
pub const SAFE_FALL_DISTANCE: f64 = 3.0;
pub const MAX_FOOD_LEVEL: u8 = 20;
pub const MAX_FOOD_SATURATION_MILLI: u16 = 20_000;
pub const EXHAUSTION_PER_FOOD_POINT_MILLI: u16 = 4_000;

/// Provisional activity rates. Fractional milli-exhaustion is kept between
/// ticks so a slow rendering cadence cannot change distance-based costs.
#[derive(Default)]
pub struct ActivityExhaustion {
    fractional_milli: f64,
}

impl ActivityExhaustion {
    pub fn record(&mut self, sprint_distance: f64, jumped: bool, sprinting: bool) -> u32 {
        let distance = if sprint_distance.is_finite() && (0.0..=16.0).contains(&sprint_distance) {
            sprint_distance
        } else {
            0.0
        };
        let total = self.fractional_milli + distance * 100.0;
        let distance_cost = (total + 1e-9).floor() as u32;
        self.fractional_milli = (total - f64::from(distance_cost)).max(0.0);
        distance_cost
            + if jumped {
                if sprinting {
                    200
                } else {
                    50
                }
            } else {
                0
            }
    }
}

/// Saved hunger state. Exact exhaustion sources and thresholds must be checked
/// against the pinned Java Edition runtime before this is treated as parity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoodState {
    food_level: u8,
    saturation_milli: u16,
    exhaustion_milli: u16,
}

impl Default for FoodState {
    fn default() -> Self {
        Self {
            food_level: MAX_FOOD_LEVEL,
            saturation_milli: MAX_FOOD_SATURATION_MILLI,
            exhaustion_milli: 0,
        }
    }
}

impl FoodState {
    pub const fn from_saved(
        food_level: u8,
        saturation_milli: u16,
        exhaustion_milli: u16,
    ) -> Option<Self> {
        if food_level <= MAX_FOOD_LEVEL
            && saturation_milli <= MAX_FOOD_SATURATION_MILLI
            && saturation_milli <= food_level as u16 * 1_000
            && exhaustion_milli < EXHAUSTION_PER_FOOD_POINT_MILLI
        {
            Some(Self {
                food_level,
                saturation_milli,
                exhaustion_milli,
            })
        } else {
            None
        }
    }

    pub const fn food_level(self) -> u8 {
        self.food_level
    }

    pub const fn can_sprint(self) -> bool {
        self.food_level > 6
    }

    pub const fn saturation_milli(self) -> u16 {
        self.saturation_milli
    }

    pub const fn exhaustion_milli(self) -> u16 {
        self.exhaustion_milli
    }

    pub fn eat(&mut self, nutrition: u8, saturation_milli: u16) -> bool {
        if nutrition == 0 || self.food_level == MAX_FOOD_LEVEL {
            return false;
        }
        self.food_level = self
            .food_level
            .saturating_add(nutrition)
            .min(MAX_FOOD_LEVEL);
        self.saturation_milli = self
            .saturation_milli
            .saturating_add(saturation_milli)
            .min(u16::from(self.food_level) * 1_000);
        true
    }

    /// Apply accumulated exhaustion in fixed point, preserving fractional
    /// progress and consuming saturation before food. Activity sources are
    /// deliberately supplied by the caller so their reference rates can be
    /// calibrated independently.
    pub fn apply_exhaustion(&mut self, amount_milli: u32) -> u8 {
        let threshold = u32::from(EXHAUSTION_PER_FOOD_POINT_MILLI);
        let total = u32::from(self.exhaustion_milli) + amount_milli;
        let depletion_cycles = total / threshold;
        self.exhaustion_milli = (total % threshold) as u16;

        let saturation_cycles = (u32::from(self.saturation_milli) + 999) / 1_000;
        let spent_saturation = depletion_cycles.min(saturation_cycles);
        self.saturation_milli = self
            .saturation_milli
            .saturating_sub((spent_saturation * 1_000) as u16);

        let food_cycles = depletion_cycles - spent_saturation;
        let consumed_food = food_cycles.min(u32::from(self.food_level)) as u8;
        self.food_level -= consumed_food;
        consumed_food
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DamageOutcome {
    pub applied: u8,
    pub became_dead: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlayerVitals {
    health: u8,
}

impl Default for PlayerVitals {
    fn default() -> Self {
        Self { health: MAX_HEALTH }
    }
}

impl PlayerVitals {
    pub const fn from_saved_health(health: u8) -> Option<Self> {
        if health <= MAX_HEALTH {
            Some(Self { health })
        } else {
            None
        }
    }

    pub const fn health(self) -> u8 {
        self.health
    }

    pub const fn is_dead(self) -> bool {
        self.health == 0
    }

    pub fn damage(&mut self, amount: u8) -> DamageOutcome {
        let before = self.health;
        self.health = self.health.saturating_sub(amount);
        DamageOutcome {
            applied: before - self.health,
            became_dead: before > 0 && self.health == 0,
        }
    }

    pub fn apply_fall_damage(&mut self, distance: f64) -> DamageOutcome {
        if !distance.is_finite() || distance <= SAFE_FALL_DISTANCE {
            return DamageOutcome {
                applied: 0,
                became_dead: false,
            };
        }
        let amount = (distance - SAFE_FALL_DISTANCE)
            .floor()
            .min(f64::from(MAX_HEALTH)) as u8;
        self.damage(amount)
    }

    pub fn heal(&mut self, amount: u8) -> u8 {
        let before = self.health;
        self.health = self.health.saturating_add(amount).min(MAX_HEALTH);
        self.health - before
    }

    pub fn respawn(&mut self) {
        self.health = MAX_HEALTH;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn activity_cost_preserves_fractional_distance_and_jump_events() {
        let mut costs = super::ActivityExhaustion::default();
        let total: u32 = (0..1000).map(|_| costs.record(0.001, false, true)).sum();
        assert_eq!(total, 100);
        assert_eq!(costs.record(0.0, true, false), 50);
        assert_eq!(costs.record(0.0, true, true), 200);
        assert_eq!(costs.record(f64::NAN, false, false), 0);
        let mut food = super::FoodState::from_saved(7, 0, 3950).unwrap();
        assert!(food.can_sprint());
        food.apply_exhaustion(50);
        assert_eq!(food.food_level(), 6);
        assert!(!food.can_sprint());
    }
    use super::*;

    #[test]
    fn health_damage_clamps_at_zero_and_reports_the_death_transition_once() {
        let mut vitals = PlayerVitals::default();
        assert_eq!(
            vitals.damage(19),
            DamageOutcome {
                applied: 19,
                became_dead: false
            }
        );
        assert_eq!(
            vitals.damage(4),
            DamageOutcome {
                applied: 1,
                became_dead: true
            }
        );
        assert_eq!(
            vitals.damage(1),
            DamageOutcome {
                applied: 0,
                became_dead: false
            }
        );
        assert!(vitals.is_dead());
    }

    #[test]
    fn prototype_fall_profile_uses_a_safe_distance_and_healing_is_bounded() {
        let mut vitals = PlayerVitals::default();
        assert_eq!(vitals.apply_fall_damage(3.0).applied, 0);
        assert_eq!(vitals.apply_fall_damage(3.99).applied, 0);
        assert_eq!(vitals.apply_fall_damage(5.0).applied, 2);
        assert_eq!(vitals.heal(50), 2);
        assert_eq!(vitals.health(), MAX_HEALTH);
        assert_eq!(vitals.apply_fall_damage(f64::NAN).applied, 0);
    }

    #[test]
    fn respawn_restores_prototype_health() {
        let mut vitals = PlayerVitals::default();
        vitals.damage(MAX_HEALTH);
        vitals.respawn();
        assert_eq!(vitals.health(), MAX_HEALTH);
        assert!(!vitals.is_dead());
    }

    #[test]
    fn food_state_preserves_fixed_point_exhaustion_and_spends_saturation_first() {
        let mut food = FoodState::default();
        assert_eq!(food.apply_exhaustion(2_500), 0);
        assert_eq!(food.exhaustion_milli(), 2_500);
        assert_eq!(food.apply_exhaustion(1_500), 0);
        assert_eq!(food.saturation_milli(), 19_000);
        assert_eq!(food.exhaustion_milli(), 0);
        assert_eq!(food.food_level(), 20);

        let mut empty_saturation = FoodState::from_saved(3, 0, 3_500).unwrap();
        assert_eq!(empty_saturation.apply_exhaustion(8_500), 3);
        assert_eq!(empty_saturation.food_level(), 0);
        assert_eq!(empty_saturation.exhaustion_milli(), 0);
    }

    #[test]
    fn food_state_rejects_out_of_range_or_inconsistent_saved_values() {
        assert_eq!(
            FoodState::from_saved(20, 20_000, 3_999),
            Some(FoodState {
                exhaustion_milli: 3_999,
                ..FoodState::default()
            })
        );
        assert!(FoodState::from_saved(21, 0, 0).is_none());
        assert!(FoodState::from_saved(10, 10_001, 0).is_none());
        assert!(FoodState::from_saved(1, 1_000, 4_000).is_none());
    }
}
