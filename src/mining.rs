//! Fixed-tick, data-driven block mining progress.
//!
//! The included profile is an engine prototype, not a Java Edition baseline.
//! A pinned compatibility profile must supply measured values and fixtures.

use std::collections::BTreeMap;

use crate::interaction::BlockRayHit;
use crate::world::BlockId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MiningRule {
    /// Number of held simulation ticks with the reference speed tool.
    /// Zero means the block breaks on the first held tick.
    pub base_ticks: u16,
    /// Lowest tool tier allowed to harvest this block's configured loot.
    pub minimum_harvest_tier: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MiningTool {
    /// Tool speed in thousandths; 1000 is the unmodified hand speed.
    pub speed_permille: u16,
    pub harvest_tier: u8,
}

impl MiningTool {
    pub const HAND: Self = Self {
        speed_permille: 1000,
        harvest_tier: 0,
    };
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MiningCatalog {
    rules: BTreeMap<BlockId, MiningRule>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MiningCatalogError {
    DuplicateBlock,
    InvalidSpeed,
}

impl MiningCatalog {
    pub fn register(&mut self, block: BlockId, rule: MiningRule) -> Result<(), MiningCatalogError> {
        if self.rules.contains_key(&block) {
            return Err(MiningCatalogError::DuplicateBlock);
        }
        self.rules.insert(block, rule);
        Ok(())
    }

    pub fn rule(&self, block: BlockId) -> Option<MiningRule> {
        self.rules.get(&block).copied()
    }

    pub fn prototype() -> Self {
        let mut catalog = Self::default();
        for (id, base_ticks, minimum_harvest_tier) in [(1, 10, 0), (2, 15, 0), (3, 60, 0)] {
            catalog
                .register(
                    BlockId(id),
                    MiningRule {
                        base_ticks,
                        minimum_harvest_tier,
                    },
                )
                .expect("prototype block IDs are unique");
        }
        catalog
    }

    pub fn ticks_to_break(
        &self,
        block: BlockId,
        tool: MiningTool,
    ) -> Result<u16, MiningCatalogError> {
        if tool.speed_permille == 0 {
            return Err(MiningCatalogError::InvalidSpeed);
        }
        let Some(rule) = self.rule(block) else {
            return Ok(u16::MAX);
        };
        if rule.base_ticks == 0 {
            return Ok(0);
        }
        let ticks = (u32::from(rule.base_ticks) * 1000)
            .div_ceil(u32::from(tool.speed_permille))
            .max(1);
        Ok(ticks.min(u32::from(u16::MAX)) as u16)
    }

    pub fn can_harvest(&self, block: BlockId, tool: MiningTool) -> bool {
        self.rule(block)
            .is_some_and(|rule| tool.harvest_tier >= rule.minimum_harvest_tier)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MiningEvent {
    Idle,
    Cancelled,
    Progress {
        hit: BlockRayHit,
        progress_permille: u16,
    },
    Completed(BlockRayHit),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MiningSession {
    target: Option<BlockRayHit>,
    required_ticks: u16,
    elapsed_ticks: u16,
}

impl MiningSession {
    pub fn progress_permille(&self) -> u16 {
        if self.required_ticks == 0 {
            return 0;
        }
        ((u32::from(self.elapsed_ticks) * 1000) / u32::from(self.required_ticks)).min(999) as u16
    }

    pub fn tick(
        &mut self,
        held: bool,
        target: Option<BlockRayHit>,
        required_ticks: u16,
    ) -> MiningEvent {
        if !held || target.is_none() {
            let was_active = self.target.take().is_some();
            self.required_ticks = 0;
            self.elapsed_ticks = 0;
            return if was_active {
                MiningEvent::Cancelled
            } else {
                MiningEvent::Idle
            };
        }
        let hit = target.expect("target was checked");
        let same_target = self
            .target
            .is_some_and(|active| active.position == hit.position && active.block == hit.block);
        if !same_target || self.required_ticks != required_ticks {
            self.target = Some(hit);
            self.required_ticks = required_ticks;
            self.elapsed_ticks = 0;
        } else {
            // Update the hit distance while retaining progress on the same voxel.
            self.target = Some(hit);
        }
        if required_ticks == 0 {
            self.reset();
            return MiningEvent::Completed(hit);
        }
        self.elapsed_ticks = self.elapsed_ticks.saturating_add(1);
        if self.elapsed_ticks >= required_ticks {
            self.reset();
            MiningEvent::Completed(hit)
        } else {
            MiningEvent::Progress {
                hit,
                progress_permille: self.progress_permille(),
            }
        }
    }

    pub fn reset(&mut self) {
        self.target = None;
        self.required_ticks = 0;
        self.elapsed_ticks = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::BlockPos;

    fn hit(position: [i32; 3], block: u16) -> BlockRayHit {
        BlockRayHit {
            position: BlockPos(position),
            face_normal: [0, 1, 0],
            block: BlockId(block),
            distance: 2.0,
        }
    }

    #[test]
    fn progress_advances_in_fixed_ticks_and_completes_once() {
        let mut session = MiningSession::default();
        let target = hit([2, 3, 4], 3);
        assert!(matches!(
            session.tick(true, Some(target), 3),
            MiningEvent::Progress {
                progress_permille: 333,
                ..
            }
        ));
        assert!(matches!(
            session.tick(true, Some(target), 3),
            MiningEvent::Progress {
                progress_permille: 666,
                ..
            }
        ));
        assert_eq!(
            session.tick(true, Some(target), 3),
            MiningEvent::Completed(target)
        );
        assert_eq!(
            session.tick(true, Some(target), 3),
            MiningEvent::Progress {
                hit: target,
                progress_permille: 333,
            }
        );
    }

    #[test]
    fn releasing_or_changing_target_cancels_and_restarts_progress() {
        let mut session = MiningSession::default();
        let first = hit([1, 1, 1], 1);
        let second = hit([2, 1, 1], 1);
        session.tick(true, Some(first), 10);
        assert_eq!(session.tick(false, Some(first), 10), MiningEvent::Cancelled);
        assert_eq!(session.tick(false, None, 10), MiningEvent::Idle);
        session.tick(true, Some(first), 10);
        assert!(matches!(
            session.tick(true, Some(second), 10),
            MiningEvent::Progress {
                hit: BlockRayHit {
                    position: BlockPos([2, 1, 1]),
                    ..
                },
                progress_permille: 100,
            }
        ));
    }

    #[test]
    fn mining_catalog_scales_ticks_and_checks_harvest_tiers() {
        let mut catalog = MiningCatalog::default();
        catalog
            .register(
                BlockId(9),
                MiningRule {
                    base_ticks: 20,
                    minimum_harvest_tier: 2,
                },
            )
            .unwrap();
        assert_eq!(catalog.ticks_to_break(BlockId(9), MiningTool::HAND), Ok(20));
        assert_eq!(
            catalog.ticks_to_break(
                BlockId(9),
                MiningTool {
                    speed_permille: 2500,
                    harvest_tier: 1,
                }
            ),
            Ok(8)
        );
        assert!(!catalog.can_harvest(BlockId(9), MiningTool::HAND));
        assert!(catalog.can_harvest(
            BlockId(9),
            MiningTool {
                speed_permille: 1000,
                harvest_tier: 2,
            }
        ));
        assert_eq!(
            catalog.ticks_to_break(
                BlockId(9),
                MiningTool {
                    speed_permille: 0,
                    harvest_tier: 2,
                }
            ),
            Err(MiningCatalogError::InvalidSpeed)
        );
    }
}
