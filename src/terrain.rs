//! Independently authored deterministic terrain generation.

use crate::world::{BlockId, Chunk, ChunkPos, World, CHUNK_SIZE, MAX_SERIALIZED_CHUNKS};

pub const AIR: BlockId = BlockId(0);
pub const SOIL: BlockId = BlockId(1);
pub const SURFACE: BlockId = BlockId(2);
pub const ROCK: BlockId = BlockId(3);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerrainGenerator {
    pub seed: u64,
    pub base_height: i32,
    pub amplitude: i32,
}

impl Default for TerrainGenerator {
    fn default() -> Self {
        Self {
            seed: 0xC0FFEE_1234_5678,
            base_height: 6,
            amplitude: 5,
        }
    }
}

impl TerrainGenerator {
    pub fn height_at(&self, x: i32, z: i32) -> i32 {
        let coarse = value_noise(self.seed, x.div_euclid(8), z.div_euclid(8));
        let detail = value_noise(self.seed ^ 0x9E37_79B9, x.div_euclid(3), z.div_euclid(3));
        self.base_height + ((coarse * 0.7 + detail * 0.3) * self.amplitude as f32).round() as i32
    }

    pub fn generate_chunk(&self, position: ChunkPos) -> Chunk {
        let mut chunk = Chunk::empty();
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let height = self.height_at(
                    position
                        .x
                        .saturating_mul(CHUNK_SIZE as i32)
                        .saturating_add(x as i32),
                    position
                        .z
                        .saturating_mul(CHUNK_SIZE as i32)
                        .saturating_add(z as i32),
                );
                for y in 0..CHUNK_SIZE {
                    let world_y = position
                        .y
                        .saturating_mul(CHUNK_SIZE as i32)
                        .saturating_add(y as i32);
                    let block = if world_y < 0 || world_y > height {
                        AIR
                    } else if world_y == height {
                        SURFACE
                    } else if world_y >= height - 3 {
                        SOIL
                    } else {
                        ROCK
                    };
                    chunk.set(x, y, z, block).unwrap();
                }
            }
        }
        chunk
    }

    pub fn generate_region(&self, radius: i32) -> World {
        let mut world = World::new();
        for z in -radius..=radius {
            for x in -radius..=radius {
                let position = ChunkPos { x, y: 0, z };
                *world.get_or_create_chunk(position) = self.generate_chunk(position);
            }
        }
        world
    }

    /// Ensure a bounded square of horizontal terrain chunks around `center`.
    /// Existing chunks are preserved so edits and imported terrain are never
    /// replaced by generated content.
    pub fn ensure_region(
        &self,
        world: &mut World,
        center: ChunkPos,
        radius: i32,
    ) -> Result<Vec<ChunkPos>, &'static str> {
        if !(0..=16).contains(&radius) {
            return Err("terrain region radius is outside the supported bound");
        }
        let mut requested = Vec::with_capacity(((radius * 2 + 1).pow(2)) as usize);
        for z_offset in -radius..=radius {
            for x_offset in -radius..=radius {
                requested.push(ChunkPos {
                    x: center
                        .x
                        .checked_add(x_offset)
                        .ok_or("terrain chunk coordinate overflow")?,
                    y: 0,
                    z: center
                        .z
                        .checked_add(z_offset)
                        .ok_or("terrain chunk coordinate overflow")?,
                });
            }
        }
        let missing: Vec<_> = requested
            .into_iter()
            .filter(|position| world.get_chunk(*position).is_none())
            .collect();
        let min_chunk = i32::MIN.div_euclid(CHUNK_SIZE as i32);
        let max_chunk = i32::MAX.div_euclid(CHUNK_SIZE as i32);
        if missing.iter().any(|position| {
            !(min_chunk..=max_chunk).contains(&position.x)
                || !(min_chunk..=max_chunk).contains(&position.z)
        }) {
            return Err("terrain chunk exceeds the world coordinate limit");
        }
        if world.chunk_count().saturating_add(missing.len()) > MAX_SERIALIZED_CHUNKS {
            return Err("terrain region would exceed the world chunk limit");
        }
        for position in &missing {
            *world.get_or_create_chunk(*position) = self.generate_chunk(*position);
        }
        Ok(missing)
    }
}

fn value_noise(seed: u64, x: i32, z: i32) -> f32 {
    let mut value = seed ^ (x as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    value ^= (z as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((value ^ (value >> 31)) as f64 / u64::MAX as f64 * 2.0 - 1.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generation_is_deterministic() {
        let generator = TerrainGenerator::default();
        assert_eq!(
            generator.generate_chunk(ChunkPos { x: 2, y: 0, z: -3 }),
            generator.generate_chunk(ChunkPos { x: 2, y: 0, z: -3 })
        );
    }
    #[test]
    fn terrain_has_layers_and_varied_heights() {
        let generator = TerrainGenerator::default();
        let chunk = generator.generate_chunk(ChunkPos { x: 0, y: 0, z: 0 });
        let mut kinds = std::collections::BTreeSet::new();
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    kinds.insert(chunk.get(x, y, z).unwrap());
                }
            }
        }
        assert!(kinds.contains(&ROCK) && kinds.contains(&SURFACE));
        assert_ne!(generator.height_at(0, 0), generator.height_at(15, 15));
    }

    #[test]
    fn region_generation_fills_only_missing_chunks_and_respects_negative_coordinates() {
        let generator = TerrainGenerator::default();
        let mut world = World::new();
        let preserved = ChunkPos { x: -3, y: 0, z: 4 };
        world
            .get_or_create_chunk(preserved)
            .set(0, 0, 0, ROCK)
            .unwrap();

        let generated = generator
            .ensure_region(&mut world, ChunkPos { x: -3, y: 0, z: 4 }, 1)
            .unwrap();
        assert_eq!(generated.len(), 8);
        assert_eq!(world.chunk_count(), 9);
        assert_eq!(world.get_chunk(preserved).unwrap().get(0, 0, 0), Ok(ROCK));
        assert!(generated.contains(&ChunkPos { x: -4, y: 0, z: 4 }));
        assert!(generated.contains(&ChunkPos { x: -2, y: 0, z: 3 }));
        assert!(generator
            .ensure_region(&mut world, ChunkPos { x: -3, y: 0, z: 4 }, 1)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn region_generation_rejects_invalid_or_overflowing_requests_before_mutation() {
        let generator = TerrainGenerator::default();
        let mut world = World::new();
        assert!(generator
            .ensure_region(&mut world, ChunkPos::default(), 17)
            .is_err());
        assert!(generator
            .ensure_region(
                &mut world,
                ChunkPos {
                    x: i32::MAX,
                    y: 0,
                    z: 0,
                },
                1,
            )
            .is_err());
        assert!(generator
            .ensure_region(
                &mut world,
                ChunkPos {
                    x: i32::MAX / CHUNK_SIZE as i32 + 1,
                    y: 0,
                    z: 0,
                },
                0,
            )
            .is_err());
        assert_eq!(world.chunk_count(), 0);
    }
}
