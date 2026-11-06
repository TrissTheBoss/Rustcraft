pub const CHUNK_SIZE: usize = 16;
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BlockId(pub u16);

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
}

fn index(x: usize, y: usize, z: usize) -> Result<usize, &'static str> {
    if x >= CHUNK_SIZE || y >= CHUNK_SIZE || z >= CHUNK_SIZE {
        return Err("chunk coordinate out of bounds");
    }
    Ok((y * CHUNK_SIZE * CHUNK_SIZE) + (z * CHUNK_SIZE) + x)
}
