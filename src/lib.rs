//! Clean-room voxel engine foundation. No Minecraft code or assets are included.
pub mod asset_extract;
pub mod entity;
pub mod logger;
pub mod movement;
pub mod networking;
pub mod render;
pub mod resource;
pub mod simulation;
pub mod terrain;
#[cfg(windows)]
pub mod window;
pub mod world;

pub const ENGINE_NAME: &str = "Voxel Engine";

#[cfg(test)]
mod tests {
    use super::world::{BlockId, Chunk, ChunkPos, World, CHUNK_SIZE};

    #[test]
    fn chunks_have_stable_bounds() {
        let mut chunk = Chunk::empty();
        chunk.set(0, 0, 0, BlockId(1)).unwrap();
        assert_eq!(chunk.get(0, 0, 0), Ok(BlockId(1)));
        assert!(chunk.get(CHUNK_SIZE, 0, 0).is_err());
    }

    #[test]
    fn world_round_trips_deterministically() {
        let mut world = World::new();
        world
            .get_or_create_chunk(ChunkPos { x: 2, y: -1, z: 4 })
            .set(3, 5, 7, BlockId(42))
            .unwrap();
        let mut first = Vec::new();
        world.save(&mut first).unwrap();
        let restored = World::load(first.as_slice()).unwrap();
        assert_eq!(restored.chunk_count(), 1);
        assert_eq!(
            restored
                .get_chunk(ChunkPos { x: 2, y: -1, z: 4 })
                .unwrap()
                .get(3, 5, 7),
            Ok(BlockId(42))
        );
        let mut second = Vec::new();
        restored.save(&mut second).unwrap();
        assert_eq!(first, second);
    }
}
