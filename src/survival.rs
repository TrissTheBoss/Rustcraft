//! Player survival status primitives.
//!
//! Values here are an unverified prototype profile. The pinned Java Edition
//! baseline must supply black-box fixtures before these values can be treated
//! as conformance behavior.

pub const MAX_HEALTH: u8 = 20;
pub const SAFE_FALL_DISTANCE: f64 = 3.0;

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
}
