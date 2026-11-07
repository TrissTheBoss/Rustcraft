use std::collections::HashMap;
use std::io::{self, Read, Write};

pub const CHUNK_SIZE: usize = 16;
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;
/// Bound file-driven allocation until chunk streaming and region paging exist.
pub const MAX_SERIALIZED_CHUNKS: usize = 16_384;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct BlockId(pub u16);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct ChunkPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Chunk {
    blocks: Box<[BlockId; CHUNK_VOLUME]>,
}

impl Chunk {
    pub fn empty() -> Self {
        Self {
            blocks: Box::new([BlockId(0); CHUNK_VOLUME]),
        }
    }
    pub fn get(&self, x: usize, y: usize, z: usize) -> Result<BlockId, &'static str> {
        Ok(self.blocks[index(x, y, z)?])
    }
    pub fn set(
        &mut self,
        x: usize,
        y: usize,
        z: usize,
        block: BlockId,
    ) -> Result<(), &'static str> {
        self.blocks[index(x, y, z)?] = block;
        Ok(())
    }

    fn write_blocks<W: Write>(&self, output: &mut W) -> io::Result<()> {
        for block in self.blocks.iter() {
            output.write_all(&block.0.to_le_bytes())?;
        }
        Ok(())
    }

    fn read_blocks<R: Read>(&mut self, input: &mut R) -> io::Result<()> {
        for block in self.blocks.iter_mut() {
            *block = BlockId(read_u16(input)?);
        }
        Ok(())
    }
}

fn index(x: usize, y: usize, z: usize) -> Result<usize, &'static str> {
    if x >= CHUNK_SIZE || y >= CHUNK_SIZE || z >= CHUNK_SIZE {
        return Err("chunk coordinate out of bounds");
    }
    Ok((y * CHUNK_SIZE * CHUNK_SIZE) + (z * CHUNK_SIZE) + x)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct World {
    chunks: HashMap<ChunkPos, Chunk>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
    pub fn get_or_create_chunk(&mut self, position: ChunkPos) -> &mut Chunk {
        self.chunks.entry(position).or_insert_with(Chunk::empty)
    }
    pub fn get_chunk(&self, position: ChunkPos) -> Option<&Chunk> {
        self.chunks.get(&position)
    }
    pub fn get_chunk_mut(&mut self, position: ChunkPos) -> Option<&mut Chunk> {
        self.chunks.get_mut(&position)
    }

    /// Read a block by world coordinate. Unloaded chunks return `None` so
    /// callers can distinguish unknown space from loaded air.
    pub fn block_at(&self, position: [i32; 3]) -> Option<BlockId> {
        let chunk_position = chunk_position(position);
        let local = local_position(position);
        self.get_chunk(chunk_position)?
            .get(local[0], local[1], local[2])
            .ok()
    }

    /// Change a block in an already loaded chunk and return its previous ID.
    /// This never creates a chunk as a side effect of a placement or edit.
    pub fn set_block(&mut self, position: [i32; 3], block: BlockId) -> Option<BlockId> {
        let chunk_position = chunk_position(position);
        let local = local_position(position);
        let chunk = self.get_chunk_mut(chunk_position)?;
        let previous = chunk.get(local[0], local[1], local[2]).ok()?;
        chunk.set(local[0], local[1], local[2], block).ok()?;
        Some(previous)
    }
    pub fn unload_chunk(&mut self, position: ChunkPos) -> Option<Chunk> {
        self.chunks.remove(&position)
    }

    pub fn save<W: Write>(&self, mut output: W) -> io::Result<()> {
        if self.chunks.len() > MAX_SERIALIZED_CHUNKS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "world exceeds the current serialized chunk limit",
            ));
        }
        let count = u32::try_from(self.chunks.len()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "too many chunks to serialize")
        })?;
        output.write_all(b"VOXEL01\0")?;
        output.write_all(&count.to_le_bytes())?;
        let mut positions: Vec<_> = self.chunks.keys().copied().collect();
        positions.sort_by_key(|p| (p.x, p.y, p.z));
        for position in positions {
            output.write_all(&position.x.to_le_bytes())?;
            output.write_all(&position.y.to_le_bytes())?;
            output.write_all(&position.z.to_le_bytes())?;
            self.chunks[&position].write_blocks(&mut output)?;
        }
        Ok(())
    }

    pub fn load<R: Read>(mut input: R) -> io::Result<Self> {
        Self::load_with_limit(&mut input, MAX_SERIALIZED_CHUNKS)
    }

    pub fn load_with_limit<R: Read>(input: R, max_chunks: usize) -> io::Result<Self> {
        let mut input = input;
        let mut header = [0; 8];
        input.read_exact(&mut header)?;
        if &header != b"VOXEL01\0" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid world header",
            ));
        }
        let count = read_u32(&mut input)? as usize;
        if count > max_chunks.min(MAX_SERIALIZED_CHUNKS) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "world chunk count exceeds the configured limit",
            ));
        }
        let mut world = Self::new();
        for _ in 0..count {
            let position = ChunkPos {
                x: read_i32(&mut input)?,
                y: read_i32(&mut input)?,
                z: read_i32(&mut input)?,
            };
            if world.chunks.contains_key(&position) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "world contains a duplicate chunk coordinate",
                ));
            }
            world
                .get_or_create_chunk(position)
                .read_blocks(&mut input)?;
        }
        Ok(world)
    }
}

fn chunk_position(position: [i32; 3]) -> ChunkPos {
    ChunkPos {
        x: position[0].div_euclid(CHUNK_SIZE as i32),
        y: position[1].div_euclid(CHUNK_SIZE as i32),
        z: position[2].div_euclid(CHUNK_SIZE as i32),
    }
}

fn local_position(position: [i32; 3]) -> [usize; 3] {
    [
        position[0].rem_euclid(CHUNK_SIZE as i32) as usize,
        position[1].rem_euclid(CHUNK_SIZE as i32) as usize,
        position[2].rem_euclid(CHUNK_SIZE as i32) as usize,
    ]
}

fn read_u16<R: Read>(input: &mut R) -> io::Result<u16> {
    let mut bytes = [0; 2];
    input.read_exact(&mut bytes)?;
    Ok(u16::from_le_bytes(bytes))
}
fn read_u32<R: Read>(input: &mut R) -> io::Result<u32> {
    let mut bytes = [0; 4];
    input.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}
fn read_i32<R: Read>(input: &mut R) -> io::Result<i32> {
    Ok(read_u32(input)? as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_coordinate_access_handles_negative_chunk_boundaries() {
        let mut world = World::new();
        world
            .get_or_create_chunk(ChunkPos { x: -1, y: 0, z: 1 })
            .set(15, 0, 0, BlockId(3))
            .unwrap();

        assert_eq!(world.block_at([-1, 0, 16]), Some(BlockId(3)));
        assert_eq!(world.block_at([0, 0, 16]), None);
        assert_eq!(world.set_block([-1, 0, 16], BlockId(2)), Some(BlockId(3)));
        assert_eq!(world.block_at([-1, 0, 16]), Some(BlockId(2)));
    }

    #[test]
    fn setting_an_unloaded_position_does_not_create_a_chunk() {
        let mut world = World::new();
        assert_eq!(world.set_block([32, -1, -17], BlockId(1)), None);
        assert_eq!(world.chunk_count(), 0);
    }

    #[test]
    fn load_rejects_chunk_counts_above_the_allocation_limit() {
        let mut bytes = b"VOXEL01\0".to_vec();
        bytes.extend(((MAX_SERIALIZED_CHUNKS + 1) as u32).to_le_bytes());
        assert_eq!(
            World::load(bytes.as_slice()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn load_rejects_duplicate_chunk_coordinates() {
        let position = ChunkPos { x: 4, y: -1, z: 7 };
        let mut world = World::new();
        world.get_or_create_chunk(position);
        let mut bytes = Vec::new();
        world.save(&mut bytes).unwrap();
        bytes[8..12].copy_from_slice(&2u32.to_le_bytes());
        let chunk_record = bytes[12..].to_vec();
        bytes.extend(chunk_record);
        assert_eq!(
            World::load(bytes.as_slice()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
