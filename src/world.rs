use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};

use crate::blocks::{default_block_state, BlockState};

pub const CHUNK_SIZE: usize = 16;
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;
const WORLD_MAGIC_V1: &[u8; 8] = b"VOXEL01\0";
const WORLD_MAGIC_V2: &[u8; 8] = b"VOXEL02\0";
const WORLD_MAGIC_V3: &[u8; 8] = b"VOXEL03\0";
const WORLD_MAGIC_V4: &[u8; 8] = b"VOXEL04\0";
/// Bound persistent world chunks and the current generated chunk cache.
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
    states: HashMap<u16, BlockState>,
}

impl Chunk {
    pub fn empty() -> Self {
        Self {
            blocks: Box::new([BlockId(0); CHUNK_VOLUME]),
            states: HashMap::new(),
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
        let block_index = index(x, y, z)?;
        if self.blocks[block_index] != block {
            self.states.remove(&(block_index as u16));
        }
        self.blocks[block_index] = block;
        Ok(())
    }

    pub fn get_state(&self, x: usize, y: usize, z: usize) -> Result<BlockState, &'static str> {
        let block_index = index(x, y, z)?;
        Ok(self
            .states
            .get(&(block_index as u16))
            .copied()
            .unwrap_or_else(|| default_block_state(self.blocks[block_index])))
    }

    pub fn set_state(
        &mut self,
        x: usize,
        y: usize,
        z: usize,
        state: BlockState,
    ) -> Result<BlockState, &'static str> {
        let block_index = index(x, y, z)?;
        let block = self.blocks[block_index];
        let default_state = default_block_state(block);
        if !state.is_valid() || state.kind != default_state.kind {
            return Err("block state is invalid for this block");
        }
        let previous = self
            .states
            .get(&(block_index as u16))
            .copied()
            .unwrap_or(default_state);
        if state == default_state {
            self.states.remove(&(block_index as u16));
        } else {
            self.states.insert(block_index as u16, state);
        }
        Ok(previous)
    }

    fn write_block_ids<W: Write>(&self, output: &mut W) -> io::Result<()> {
        for block in self.blocks.iter() {
            output.write_all(&block.0.to_le_bytes())?;
        }
        Ok(())
    }

    fn write_blocks_v4<W: Write>(&self, output: &mut W) -> io::Result<()> {
        self.write_block_ids(output)?;
        let count = u16::try_from(self.states.len())
            .map_err(|_| invalid_data("chunk contains too many block states"))?;
        output.write_all(&count.to_le_bytes())?;
        let mut states: Vec<_> = self.states.iter().collect();
        states.sort_by_key(|(index, _)| **index);
        for (index, state) in states {
            let code = state
                .encode()
                .ok_or_else(|| invalid_data("chunk contains an invalid block state"))?;
            output.write_all(&index.to_le_bytes())?;
            output.write_all(&code.to_le_bytes())?;
        }
        Ok(())
    }

    fn read_blocks<R: Read>(&mut self, input: &mut R, version: u8) -> io::Result<()> {
        for block in self.blocks.iter_mut() {
            *block = BlockId(read_u16(input)?);
        }
        if version < 3 {
            return Ok(());
        }
        let count = usize::from(read_u16(input)?);
        if count > CHUNK_VOLUME {
            return Err(invalid_data(
                "chunk block-state count exceeds the chunk volume",
            ));
        }
        for _ in 0..count {
            let block_index = read_u16(input)?;
            if usize::from(block_index) >= CHUNK_VOLUME || self.states.contains_key(&block_index) {
                return Err(invalid_data(
                    "chunk contains an invalid or duplicate state index",
                ));
            }
            let state = BlockState::decode_with_connections(read_u16(input)?, version >= 4)
                .ok_or_else(|| invalid_data("chunk contains an invalid block-state value"))?;
            let block = self.blocks[usize::from(block_index)];
            let default_state = default_block_state(block);
            if state.kind != default_state.kind || state == default_state {
                return Err(invalid_data("chunk block state does not match its block"));
            }
            self.states.insert(block_index, state);
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
    generated_chunks: HashSet<ChunkPos>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
    pub fn get_or_create_chunk(&mut self, position: ChunkPos) -> &mut Chunk {
        self.generated_chunks.remove(&position);
        self.chunks.entry(position).or_insert_with(Chunk::empty)
    }

    pub fn insert_generated_chunk(&mut self, position: ChunkPos, chunk: Chunk) {
        if let std::collections::hash_map::Entry::Vacant(entry) = self.chunks.entry(position) {
            entry.insert(chunk);
            self.generated_chunks.insert(position);
        }
    }

    pub fn is_generated_chunk(&self, position: ChunkPos) -> bool {
        self.generated_chunks.contains(&position)
    }

    /// Discard only generated chunks outside the retained render region.
    /// Edited and legacy-loaded chunks are never eligible for eviction.
    pub fn evict_generated_chunks_outside(&mut self, retained: &[ChunkPos]) -> Vec<ChunkPos> {
        let evicted: Vec<_> = self
            .generated_chunks
            .iter()
            .filter(|position| !retained.contains(position))
            .copied()
            .collect();
        for position in &evicted {
            self.generated_chunks.remove(position);
            self.chunks.remove(position);
        }
        evicted
    }
    pub fn get_chunk(&self, position: ChunkPos) -> Option<&Chunk> {
        self.chunks.get(&position)
    }
    pub fn get_chunk_mut(&mut self, position: ChunkPos) -> Option<&mut Chunk> {
        self.generated_chunks.remove(&position);
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

    /// Read the persisted state associated with a loaded block position.
    pub fn block_state_at(&self, position: [i32; 3]) -> Option<BlockState> {
        let chunk_position = chunk_position(position);
        let local = local_position(position);
        self.get_chunk(chunk_position)?
            .get_state(local[0], local[1], local[2])
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

    /// Change a loaded block's state without creating a chunk or changing its
    /// block ID. Invalid states for the current block leave the world intact.
    pub fn set_block_state(&mut self, position: [i32; 3], state: BlockState) -> Option<BlockState> {
        let chunk_position = chunk_position(position);
        let local = local_position(position);
        self.get_chunk_mut(chunk_position)?
            .set_state(local[0], local[1], local[2], state)
            .ok()
    }
    pub fn unload_chunk(&mut self, position: ChunkPos) -> Option<Chunk> {
        self.generated_chunks.remove(&position);
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
        output.write_all(WORLD_MAGIC_V4)?;
        output.write_all(&count.to_le_bytes())?;
        let mut positions: Vec<_> = self.chunks.keys().copied().collect();
        positions.sort_by_key(|p| (p.x, p.y, p.z));
        for position in positions {
            output.write_all(&position.x.to_le_bytes())?;
            output.write_all(&position.y.to_le_bytes())?;
            output.write_all(&position.z.to_le_bytes())?;
            output.write_all(&[u8::from(self.generated_chunks.contains(&position))])?;
            self.chunks[&position].write_blocks_v4(&mut output)?;
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
        let version = if &header == WORLD_MAGIC_V1 {
            1
        } else if &header == WORLD_MAGIC_V2 {
            2
        } else if &header == WORLD_MAGIC_V3 {
            3
        } else if &header == WORLD_MAGIC_V4 {
            4
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid world header",
            ));
        };
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
            let generated = if version >= 2 {
                read_bool(&mut input)?
            } else {
                false
            };
            world
                .chunks
                .entry(position)
                .or_insert_with(Chunk::empty)
                .read_blocks(&mut input, version)?;
            if generated {
                world.generated_chunks.insert(position);
            }
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

fn invalid_data(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn read_u32<R: Read>(input: &mut R) -> io::Result<u32> {
    let mut bytes = [0; 4];
    input.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}
fn read_i32<R: Read>(input: &mut R) -> io::Result<i32> {
    Ok(read_u32(input)? as i32)
}

fn read_bool<R: Read>(input: &mut R) -> io::Result<bool> {
    let mut byte = [0];
    input.read_exact(&mut byte)?;
    match byte[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid generated-chunk marker",
        )),
    }
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

    #[test]
    fn generated_chunk_provenance_survives_save_and_edits_make_chunks_persistent() {
        let generated = ChunkPos { x: -2, y: 0, z: 3 };
        let edited = ChunkPos { x: -1, y: 0, z: 3 };
        let mut world = World::new();
        world.insert_generated_chunk(generated, Chunk::empty());
        world.insert_generated_chunk(edited, Chunk::empty());
        assert!(world.is_generated_chunk(generated));
        assert!(world.is_generated_chunk(edited));

        world.set_block([-1, 1, 48], BlockId(5));
        assert!(!world.is_generated_chunk(edited));
        let mut bytes = Vec::new();
        world.save(&mut bytes).unwrap();
        let mut loaded = World::load(bytes.as_slice()).unwrap();
        assert!(loaded.is_generated_chunk(generated));
        assert!(!loaded.is_generated_chunk(edited));

        assert_eq!(loaded.evict_generated_chunks_outside(&[]), vec![generated]);
        assert_eq!(loaded.get_chunk(generated), None);
        assert_eq!(loaded.block_at([-1, 1, 48]), Some(BlockId(5)));
        assert_eq!(loaded.chunk_count(), 1);
    }

    #[test]
    fn legacy_world_payload_chunks_load_as_persistent() {
        let position = ChunkPos { x: -1, y: 0, z: 2 };
        let mut bytes = WORLD_MAGIC_V1.to_vec();
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(position.x.to_le_bytes());
        bytes.extend(position.y.to_le_bytes());
        bytes.extend(position.z.to_le_bytes());
        Chunk::empty().write_block_ids(&mut bytes).unwrap();

        let mut world = World::load(bytes.as_slice()).unwrap();
        assert_eq!(world.chunk_count(), 1);
        assert!(!world.is_generated_chunk(position));
        assert!(world.evict_generated_chunks_outside(&[]).is_empty());
        assert!(world.get_chunk(position).is_some());
    }

    #[test]
    fn generated_chunk_marker_rejects_unknown_values() {
        let mut world = World::new();
        world.insert_generated_chunk(ChunkPos::default(), Chunk::empty());
        let mut bytes = Vec::new();
        world.save(&mut bytes).unwrap();
        bytes[24] = 2;
        assert_eq!(
            World::load(bytes.as_slice()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn block_states_round_trip_and_drive_registered_collision_shapes() {
        let mut world = World::new();
        world.get_or_create_chunk(ChunkPos::default());
        let slab_position = [1, 1, 1];
        world.set_block(slab_position, BlockId(4));
        let bottom_slab = world.block_state_at(slab_position).unwrap();
        let top_slab = BlockState {
            kind: crate::blocks::BlockKind::Slab,
            upper: true,
            layers: 1,
            connections: 0,
        };
        assert_eq!(
            world.set_block_state(slab_position, top_slab),
            Some(bottom_slab)
        );
        assert_eq!(
            world.set_block_state(
                slab_position,
                BlockState {
                    kind: crate::blocks::BlockKind::Snow,
                    upper: false,
                    layers: 4,
                    connections: 0,
                }
            ),
            None
        );

        let snow_position = [2, 1, 1];
        world.set_block(snow_position, BlockId(6));
        let eight_layers = BlockState {
            kind: crate::blocks::BlockKind::Snow,
            upper: false,
            layers: 8,
            connections: 0,
        };
        world.set_block_state(snow_position, eight_layers);

        let fence_position = [3, 1, 1];
        world.set_block(fence_position, BlockId(9));
        let connected_fence = BlockState {
            kind: crate::blocks::BlockKind::Fence,
            upper: false,
            layers: 1,
            connections: 0b0101,
        };
        world.set_block_state(fence_position, connected_fence);

        let mut bytes = Vec::new();
        world.save(&mut bytes).unwrap();
        assert_eq!(&bytes[..8], WORLD_MAGIC_V4);
        let loaded = World::load(bytes.as_slice()).unwrap();
        assert_eq!(loaded.block_state_at(slab_position), Some(top_slab));
        assert_eq!(loaded.block_state_at(snow_position), Some(eight_layers));
        assert_eq!(loaded.block_state_at(fence_position), Some(connected_fence));
        let registry = crate::blocks::BlockRegistry::new();
        assert_eq!(
            registry
                .definition_with_state(BlockId(4), loaded.block_state_at(slab_position).unwrap())
                .collision
                .boxes()[0]
                .min[1],
            0.5
        );
        assert_eq!(
            registry
                .definition_with_state(BlockId(6), loaded.block_state_at(snow_position).unwrap())
                .collision
                .boxes()[0]
                .max[1],
            1.0
        );
    }

    #[test]
    fn version_three_payloads_remain_readable_and_reject_new_state_bits() {
        let position = [0, 0, 0];
        let mut old_world = World::new();
        old_world.get_or_create_chunk(ChunkPos::default());
        old_world.set_block(position, BlockId(4));
        old_world.set_block_state(
            position,
            BlockState {
                kind: crate::blocks::BlockKind::Slab,
                upper: true,
                layers: 1,
                connections: 0,
            },
        );
        let mut legacy_bytes = Vec::new();
        old_world.save(&mut legacy_bytes).unwrap();
        legacy_bytes[..8].copy_from_slice(WORLD_MAGIC_V3);
        assert_eq!(
            World::load(legacy_bytes.as_slice())
                .unwrap()
                .block_state_at(position)
                .unwrap()
                .upper,
            true
        );

        let mut connected_world = World::new();
        connected_world.get_or_create_chunk(ChunkPos::default());
        connected_world.set_block(position, BlockId(9));
        connected_world.set_block_state(
            position,
            BlockState {
                kind: crate::blocks::BlockKind::Fence,
                upper: false,
                layers: 1,
                connections: 1,
            },
        );
        let mut invalid_legacy_bytes = Vec::new();
        connected_world.save(&mut invalid_legacy_bytes).unwrap();
        invalid_legacy_bytes[..8].copy_from_slice(WORLD_MAGIC_V3);
        assert_eq!(
            World::load(invalid_legacy_bytes.as_slice())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn block_states_reset_when_replacing_blocks_and_legacy_v2_gets_defaults() {
        let position = [0, 0, 0];
        let mut chunk = Chunk::empty();
        chunk.set(0, 0, 0, BlockId(4)).unwrap();
        let mut legacy = WORLD_MAGIC_V2.to_vec();
        legacy.extend(1u32.to_le_bytes());
        legacy.extend(0i32.to_le_bytes());
        legacy.extend(0i32.to_le_bytes());
        legacy.extend(0i32.to_le_bytes());
        legacy.push(0);
        chunk.write_block_ids(&mut legacy).unwrap();
        let mut loaded = World::load(legacy.as_slice()).unwrap();
        assert_eq!(
            loaded.block_state_at(position),
            Some(default_block_state(BlockId(4)))
        );
        let top_slab = BlockState {
            kind: crate::blocks::BlockKind::Slab,
            upper: true,
            layers: 1,
            connections: 0,
        };
        loaded.set_block_state(position, top_slab);
        assert_eq!(loaded.set_block(position, BlockId(3)), Some(BlockId(4)));
        assert_eq!(
            loaded.block_state_at(position),
            Some(default_block_state(BlockId(3)))
        );
    }

    #[test]
    fn block_state_decoder_rejects_invalid_state_values() {
        let mut world = World::new();
        world.get_or_create_chunk(ChunkPos::default());
        world.set_block([0, 0, 0], BlockId(6));
        world.set_block_state(
            [0, 0, 0],
            BlockState {
                kind: crate::blocks::BlockKind::Snow,
                upper: false,
                layers: 2,
                connections: 0,
            },
        );
        let mut bytes = Vec::new();
        world.save(&mut bytes).unwrap();
        let code_offset = bytes.len() - 2;
        bytes[code_offset..].copy_from_slice(&21u16.to_le_bytes()); // snow cannot be upper
        assert_eq!(
            World::load(bytes.as_slice()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn block_state_decoder_rejects_duplicate_and_out_of_range_indices() {
        let mut world = World::new();
        world.get_or_create_chunk(ChunkPos::default());
        world.set_block([0, 0, 0], BlockId(4));
        world.set_block_state(
            [0, 0, 0],
            BlockState {
                kind: crate::blocks::BlockKind::Slab,
                upper: true,
                layers: 1,
                connections: 0,
            },
        );
        let mut bytes = Vec::new();
        world.save(&mut bytes).unwrap();
        let state_count_offset = 8 + 4 + 12 + 1 + CHUNK_VOLUME * 2;
        let first_state_record = bytes[state_count_offset + 2..state_count_offset + 6].to_vec();

        let mut duplicate = bytes.clone();
        duplicate[state_count_offset..state_count_offset + 2].copy_from_slice(&2u16.to_le_bytes());
        duplicate.extend(first_state_record);
        assert_eq!(
            World::load(duplicate.as_slice()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );

        bytes[state_count_offset + 2..state_count_offset + 4]
            .copy_from_slice(&(CHUNK_VOLUME as u16).to_le_bytes());
        assert_eq!(
            World::load(bytes.as_slice()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
