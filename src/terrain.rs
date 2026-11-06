//! Independently authored deterministic terrain generation.

use crate::world::{BlockId, Chunk, ChunkPos, CHUNK_SIZE};

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
                let height = self.height_at(position.x * 16 + x as i32, position.z * 16 + z as i32);
                for y in 0..CHUNK_SIZE {
                    let world_y = position.y * 16 + y as i32;
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
}
