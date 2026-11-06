//! Clean-room voxel engine foundation. No Minecraft code or assets are included.
pub mod entity;
pub mod networking;
pub mod resource;
pub mod world;

pub const ENGINE_NAME: &str = "Voxel Engine";

#[cfg(test)]
mod tests {
    use super::world::{BlockId, Chunk, CHUNK_SIZE};

    #[test]
    fn chunks_have_stable_bounds() {
        let mut chunk = Chunk::empty();
        chunk.set(0, 0, 0, BlockId(1)).unwrap();
        assert_eq!(chunk.get(0, 0, 0), Ok(BlockId(1)));
        assert!(chunk.get(CHUNK_SIZE, 0, 0).is_err());
    }
}
