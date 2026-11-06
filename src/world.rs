use std::collections::HashMap;
use std::io::{self, Read, Write};

pub const CHUNK_SIZE: usize = 16;
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

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

#[derive(Default)]
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
    pub fn unload_chunk(&mut self, position: ChunkPos) -> Option<Chunk> {
        self.chunks.remove(&position)
    }

    pub fn save<W: Write>(&self, mut output: W) -> io::Result<()> {
        output.write_all(b"VOXEL01\0")?;
        output.write_all(&(self.chunks.len() as u32).to_le_bytes())?;
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
        let mut header = [0; 8];
        input.read_exact(&mut header)?;
        if &header != b"VOXEL01\0" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid world header",
            ));
        }
        let count = read_u32(&mut input)? as usize;
        let mut world = Self::new();
        for _ in 0..count {
            let position = ChunkPos {
                x: read_i32(&mut input)?,
                y: read_i32(&mut input)?,
                z: read_i32(&mut input)?,
            };
            world
                .get_or_create_chunk(position)
                .read_blocks(&mut input)?;
        }
        Ok(world)
    }
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
