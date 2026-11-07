//! Versioned local world storage with bounded decoding and recoverable writes.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::drops::{DropId, DroppedItem, ItemDrops, MAX_DROPPED_ITEMS};
use crate::inventory::{Inventory, ItemId, ItemKey, ItemStack, SAVED_SLOT_COUNT};
use crate::world::{BlockId, World, CHUNK_VOLUME, MAX_SERIALIZED_CHUNKS};

const SAVE_MAGIC_V1: &[u8; 8] = b"VXSAVE01";
const SAVE_MAGIC_V2: &[u8; 8] = b"VXSAVE02";
const SAVE_MAGIC_V3: &[u8; 8] = b"VXSAVE03";
const SAVE_MAGIC_V4: &[u8; 8] = b"VXSAVE04";
const SAVE_MAGIC_V5: &[u8; 8] = b"VXSAVE05";
const SAVE_MAGIC_V6: &[u8; 8] = b"VXSAVE06";
const SAVE_VERSION: u32 = 6;
const SAVE_FILE: &str = "world.vxs";
const BACKUP_DIR: &str = "backups";
const MAX_NAME_BYTES: usize = 128;
const MAX_BACKUPS: usize = 5;
const MAX_SAVE_BYTES: usize = 4096
    + MAX_DROPPED_ITEMS * 72
    + MAX_SERIALIZED_CHUNKS
        * (12
            + 1
            + CHUNK_VOLUME * std::mem::size_of::<u16>()
            + 2
            + CHUNK_VOLUME * (2 * std::mem::size_of::<u16>()));
static NEXT_FILE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum StoredGameMode {
    Survival = 0,
    Creative = 1,
    Adventure = 2,
    Spectator = 3,
}

impl TryFrom<u8> for StoredGameMode {
    type Error = io::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Survival),
            1 => Ok(Self::Creative),
            2 => Ok(Self::Adventure),
            3 => Ok(Self::Spectator),
            _ => Err(invalid_data("invalid saved game mode")),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum StoredDifficulty {
    Peaceful = 0,
    Easy = 1,
    #[default]
    Normal = 2,
    Hard = 3,
}

impl TryFrom<u8> for StoredDifficulty {
    type Error = io::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Peaceful),
            1 => Ok(Self::Easy),
            2 => Ok(Self::Normal),
            3 => Ok(Self::Hard),
            _ => Err(invalid_data("invalid saved world difficulty")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorldMetadata {
    pub id: String,
    pub name: String,
    pub seed: u64,
    pub terrain_seed: u64,
    pub terrain_generator_version: u32,
    pub created_at: u64,
    pub updated_at: u64,
    pub game_mode: StoredGameMode,
    pub difficulty: StoredDifficulty,
    pub difficulty_locked: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerSnapshot {
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub flying: bool,
    pub grounded: bool,
    pub selected_block: BlockId,
    pub inventory: Inventory,
    pub drops: ItemDrops,
}

impl Default for PlayerSnapshot {
    fn default() -> Self {
        Self {
            position: [8.0, 12.0, 8.0],
            velocity: [0.0; 3],
            yaw: -2.16,
            pitch: -0.4,
            flying: false,
            grounded: false,
            selected_block: BlockId(1),
            inventory: Inventory::default(),
            drops: ItemDrops::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredWorld {
    pub metadata: WorldMetadata,
    pub world: World,
    pub player: PlayerSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldHealth {
    Ready,
    Corrupt,
    UnsupportedVersion,
    MissingSave,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorldSummary {
    pub id: String,
    pub name: Option<String>,
    pub seed: Option<u64>,
    pub updated_at: Option<u64>,
    pub health: WorldHealth,
}

#[derive(Clone, Debug)]
pub struct WorldStore {
    root: PathBuf,
}

impl WorldStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn default_root() -> io::Result<PathBuf> {
        #[cfg(windows)]
        {
            let base = std::env::var_os("LOCALAPPDATA")
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is unset"))?;
            Ok(PathBuf::from(base).join("VoxelEngine").join("worlds"))
        }
        #[cfg(target_os = "macos")]
        {
            let home = std::env::var_os("HOME")
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is unset"))?;
            Ok(PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("VoxelEngine")
                .join("worlds"))
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            if let Some(data) = std::env::var_os("XDG_DATA_HOME") {
                return Ok(PathBuf::from(data).join("voxel-engine").join("worlds"));
            }
            let home = std::env::var_os("HOME")
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is unset"))?;
            Ok(PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("voxel-engine")
                .join("worlds"))
        }
    }

    /// Open the fixed development world, creating it once when absent. This
    /// keeps the current prototype playable while the PRD's world menu is built.
    pub fn open_or_create_prototype(&self, world: World) -> io::Result<StoredWorld> {
        let id = "world-0-0";
        if self.save_path(id)?.exists() {
            return self.load(id);
        }
        self.create_with_id(
            id,
            "Creative Prototype".to_owned(),
            0xC0FFEE_1234_5678,
            StoredGameMode::Creative,
            StoredDifficulty::Normal,
            false,
            world,
            PlayerSnapshot::default(),
        )
    }

    pub fn create(
        &self,
        name: &str,
        seed: u64,
        game_mode: StoredGameMode,
        world: World,
        player: PlayerSnapshot,
    ) -> io::Result<StoredWorld> {
        self.create_with_options(
            name,
            seed,
            game_mode,
            StoredDifficulty::Normal,
            false,
            world,
            player,
        )
    }

    pub fn create_with_options(
        &self,
        name: &str,
        seed: u64,
        game_mode: StoredGameMode,
        difficulty: StoredDifficulty,
        difficulty_locked: bool,
        world: World,
        player: PlayerSnapshot,
    ) -> io::Result<StoredWorld> {
        self.create_with_terrain_seed(
            name,
            seed,
            seed,
            crate::terrain::CURRENT_TERRAIN_GENERATOR_VERSION,
            game_mode,
            difficulty,
            difficulty_locked,
            world,
            player,
        )
    }

    fn create_with_terrain_seed(
        &self,
        name: &str,
        seed: u64,
        terrain_seed: u64,
        terrain_generator_version: u32,
        game_mode: StoredGameMode,
        difficulty: StoredDifficulty,
        difficulty_locked: bool,
        world: World,
        player: PlayerSnapshot,
    ) -> io::Result<StoredWorld> {
        let name = validate_name(name)?;
        fs::create_dir_all(&self.root)?;
        for _ in 0..100 {
            let counter = NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed);
            let id = format!("world-{}-{counter}", unix_seconds()?);
            let directory = self.world_dir(&id)?;
            match fs::create_dir(&directory) {
                Ok(()) => {
                    let now = unix_seconds()?;
                    let metadata = WorldMetadata {
                        id,
                        name,
                        seed,
                        terrain_seed,
                        terrain_generator_version,
                        created_at: now,
                        updated_at: now,
                        game_mode,
                        difficulty,
                        difficulty_locked,
                    };
                    let mut saved = StoredWorld {
                        metadata,
                        world,
                        player,
                    };
                    if let Err(error) = self.write_snapshot(
                        &mut saved.metadata,
                        &saved.world,
                        saved.player.clone(),
                        false,
                    ) {
                        let _ = fs::remove_dir_all(directory);
                        return Err(error);
                    }
                    return Ok(saved);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique world directory",
        ))
    }

    pub fn list(&self) -> io::Result<Vec<WorldSummary>> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let mut worlds = Vec::new();
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if validate_id(&id).is_err() {
                continue;
            }
            let path = entry.path().join(SAVE_FILE);
            let summary = match read_summary(&path, &id) {
                Ok(summary) => summary,
                Err(error) if error.kind() == io::ErrorKind::NotFound => WorldSummary {
                    id,
                    name: None,
                    seed: None,
                    updated_at: None,
                    health: WorldHealth::MissingSave,
                },
                Err(error) if error.kind() == io::ErrorKind::Unsupported => WorldSummary {
                    id,
                    name: None,
                    seed: None,
                    updated_at: None,
                    health: WorldHealth::UnsupportedVersion,
                },
                Err(_) => WorldSummary {
                    id,
                    name: None,
                    seed: None,
                    updated_at: None,
                    health: WorldHealth::Corrupt,
                },
            };
            worlds.push(summary);
        }
        worlds.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(worlds)
    }

    pub fn load(&self, id: &str) -> io::Result<StoredWorld> {
        self.ensure_world_dir(id)?;
        let path = self.save_path(id)?;
        let bytes = read_bounded(&path)?;
        decode_save(&bytes, id)
    }

    pub fn rename(&self, id: &str, name: &str) -> io::Result<StoredWorld> {
        let mut saved = self.load(id)?;
        saved.metadata.name = validate_name(name)?;
        self.save(&mut saved.metadata, &saved.world, saved.player.clone())?;
        Ok(saved)
    }

    pub fn update_options(
        &self,
        id: &str,
        name: &str,
        seed: u64,
        game_mode: StoredGameMode,
        difficulty: StoredDifficulty,
        difficulty_locked: bool,
    ) -> io::Result<StoredWorld> {
        let mut saved = self.load(id)?;
        if saved.metadata.difficulty_locked
            && difficulty_locked
            && saved.metadata.difficulty != difficulty
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "world difficulty is locked",
            ));
        }
        saved.metadata.name = validate_name(name)?;
        saved.metadata.seed = seed;
        saved.metadata.game_mode = game_mode;
        saved.metadata.difficulty = difficulty;
        saved.metadata.difficulty_locked = difficulty_locked;
        self.save(&mut saved.metadata, &saved.world, saved.player.clone())?;
        Ok(saved)
    }

    pub fn copy(&self, id: &str, name: &str) -> io::Result<StoredWorld> {
        let source = self.load(id)?;
        self.create_with_terrain_seed(
            name,
            source.metadata.seed,
            source.metadata.terrain_seed,
            source.metadata.terrain_generator_version,
            source.metadata.game_mode,
            source.metadata.difficulty,
            source.metadata.difficulty_locked,
            source.world,
            source.player,
        )
    }

    pub fn delete(&self, id: &str) -> io::Result<()> {
        let directory = self.ensure_world_dir(id)?;
        fs::remove_dir_all(directory)
    }

    pub fn save(
        &self,
        metadata: &mut WorldMetadata,
        world: &World,
        player: PlayerSnapshot,
    ) -> io::Result<()> {
        validate_id(&metadata.id)?;
        validate_name(&metadata.name)?;
        self.write_snapshot(metadata, world, player, true)
    }

    pub fn list_backups(&self, id: &str) -> io::Result<Vec<String>> {
        let directory = self.ensure_world_dir(id)?.join(BACKUP_DIR);
        match fs::symlink_metadata(&directory) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => return Err(invalid_data("backup path is not a real directory")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        }
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let mut backups = Vec::new();
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if valid_backup_name(&name)
                && read_bounded(&entry.path())
                    .ok()
                    .is_some_and(|bytes| decode_save(&bytes, id).is_ok())
            {
                backups.push(name);
            }
        }
        backups.sort();
        Ok(backups)
    }

    pub fn restore_backup(&self, id: &str, backup_name: &str) -> io::Result<StoredWorld> {
        self.restore_backup_with_snapshot(id, backup_name, None)
    }

    pub fn restore_backup_with_snapshot(
        &self,
        id: &str,
        backup_name: &str,
        current: Option<(&mut WorldMetadata, &World, PlayerSnapshot)>,
    ) -> io::Result<StoredWorld> {
        let world_dir = self.ensure_world_dir(id)?;
        let backup_dir = world_dir.join(BACKUP_DIR);
        ensure_real_directory(&backup_dir)?;
        let backup_path = self
            .world_dir(id)?
            .join(BACKUP_DIR)
            .join(validate_backup_name(backup_name)?);
        let bytes = read_bounded(&backup_path)?;
        let mut restored = decode_save(&bytes, id)?;
        restored.metadata.updated_at = unix_seconds()?;
        let mut world_bytes = Vec::new();
        restored.world.save(&mut world_bytes)?;
        let restored_bytes =
            encode_save(&restored.metadata, restored.player.clone(), &world_bytes)?;
        let current_path = self.save_path(id)?;
        if let Some((metadata, world, player)) = current {
            self.write_snapshot_preserving(metadata, world, player, false, None)?;
        }
        self.backup_current(id, &current_path, Some(backup_name))?;
        atomic_write(&current_path, &restored_bytes)?;
        Ok(restored)
    }

    fn create_with_id(
        &self,
        id: &str,
        name: String,
        seed: u64,
        game_mode: StoredGameMode,
        difficulty: StoredDifficulty,
        difficulty_locked: bool,
        world: World,
        player: PlayerSnapshot,
    ) -> io::Result<StoredWorld> {
        validate_id(id)?;
        let name = validate_name(&name)?;
        fs::create_dir_all(&self.root)?;
        let directory = self.world_dir(id)?;
        fs::create_dir(&directory)?;
        let now = unix_seconds()?;
        let mut saved = StoredWorld {
            metadata: WorldMetadata {
                id: id.to_owned(),
                name,
                seed,
                terrain_seed: seed,
                terrain_generator_version: crate::terrain::CURRENT_TERRAIN_GENERATOR_VERSION,
                created_at: now,
                updated_at: now,
                game_mode,
                difficulty,
                difficulty_locked,
            },
            world,
            player,
        };
        self.write_snapshot(
            &mut saved.metadata,
            &saved.world,
            saved.player.clone(),
            false,
        )?;
        Ok(saved)
    }

    fn write_snapshot(
        &self,
        metadata: &mut WorldMetadata,
        world: &World,
        player: PlayerSnapshot,
        backup: bool,
    ) -> io::Result<()> {
        self.write_snapshot_preserving(metadata, world, player, backup, None)
    }

    fn write_snapshot_preserving(
        &self,
        metadata: &mut WorldMetadata,
        world: &World,
        player: PlayerSnapshot,
        backup: bool,
        preserved_backup: Option<&str>,
    ) -> io::Result<()> {
        validate_player(&player)?;
        self.ensure_world_dir(&metadata.id)?;
        let path = self.save_path(&metadata.id)?;
        let mut world_bytes = Vec::new();
        world.save(&mut world_bytes)?;
        let updated_at = unix_seconds()?;
        let mut next_metadata = metadata.clone();
        next_metadata.updated_at = updated_at;
        let bytes = encode_save(&next_metadata, player, &world_bytes)?;
        fs::create_dir_all(path.parent().expect("world save path has parent"))?;
        if backup && path.exists() {
            self.backup_current(&next_metadata.id, &path, preserved_backup)
                .map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!("could not preserve previous world save: {error}"),
                    )
                })?;
        }
        atomic_write(&path, &bytes).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("could not replace world save: {error}"),
            )
        })?;
        *metadata = next_metadata;
        Ok(())
    }

    fn backup_current(
        &self,
        id: &str,
        save_path: &Path,
        preserved_backup: Option<&str>,
    ) -> io::Result<()> {
        self.ensure_world_dir(id)?;
        let metadata = match fs::symlink_metadata(save_path) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => metadata,
            Ok(_) => return Err(invalid_data("existing save is not a regular file")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        if metadata.len() > MAX_SAVE_BYTES as u64 {
            return Err(invalid_data("existing save exceeds the backup size limit"));
        }
        let directory = self.world_dir(id)?.join(BACKUP_DIR);
        fs::create_dir_all(&directory)?;
        ensure_real_directory(&directory)?;
        let counter = NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let name = format!("backup-{}-{counter}.sav", unix_seconds()?);
        let backup = directory.join(name);
        atomic_copy(save_path, &backup)?;
        prune_backups(&directory, MAX_BACKUPS, preserved_backup)
    }

    fn world_dir(&self, id: &str) -> io::Result<PathBuf> {
        validate_id(id)?;
        Ok(self.root.join(id))
    }

    fn save_path(&self, id: &str) -> io::Result<PathBuf> {
        Ok(self.world_dir(id)?.join(SAVE_FILE))
    }

    fn ensure_world_dir(&self, id: &str) -> io::Result<PathBuf> {
        let directory = self.world_dir(id)?;
        ensure_real_directory(&directory)?;
        Ok(directory)
    }
}

fn ensure_real_directory(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        Ok(())
    } else {
        Err(invalid_data("world storage path is not a real directory"))
    }
}

fn validate_id(id: &str) -> io::Result<()> {
    let Some(suffix) = id.strip_prefix("world-") else {
        return Err(invalid_input("invalid world ID"));
    };
    if suffix.len() > 40
        || suffix.is_empty()
        || suffix
            .split('-')
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(invalid_input("invalid world ID"));
    }
    Ok(())
}

fn validate_name(name: &str) -> io::Result<String> {
    let name = name.trim();
    if name.is_empty() || name.len() > MAX_NAME_BYTES || name.chars().any(char::is_control) {
        return Err(invalid_input(
            "world name must be 1 to 128 UTF-8 bytes without control characters",
        ));
    }
    Ok(name.to_owned())
}

fn validate_backup_name(name: &str) -> io::Result<&str> {
    if !valid_backup_name(name) {
        return Err(invalid_input("invalid backup filename"));
    }
    Ok(name)
}

fn valid_backup_name(name: &str) -> bool {
    name.starts_with("backup-")
        && name.ends_with(".sav")
        && name.len() <= 80
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'.')
        && !name.contains("..")
}

fn write_inventory<W: Write>(output: &mut W, inventory: &Inventory) -> io::Result<()> {
    output.write_all(&[inventory.selected_hotbar()])?;
    for slot in inventory.saved_slots() {
        output.write_all(&[u8::from(slot.is_some())])?;
        if let Some(stack) = slot {
            output.write_all(&stack.key.id.0.to_le_bytes())?;
            output.write_all(&stack.key.variant.to_le_bytes())?;
            output.write_all(&stack.count.to_le_bytes())?;
        }
    }
    Ok(())
}

fn read_inventory<R: Read>(input: &mut R) -> io::Result<Inventory> {
    let mut selected = [0];
    input.read_exact(&mut selected)?;
    let mut slots = [None; SAVED_SLOT_COUNT];
    for slot in &mut slots {
        if !read_bool(input)? {
            continue;
        }
        let id = ItemId(read_u16(input)?);
        let variant = read_u32(input)?;
        let count = read_u16(input)?;
        *slot = Some(ItemStack {
            key: ItemKey { id, variant },
            count,
        });
    }
    Inventory::from_saved_slots(slots, selected[0]).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid saved inventory: {error}"),
        )
    })
}

fn write_drops<W: Write>(output: &mut W, drops: &ItemDrops) -> io::Result<()> {
    let (next_id, items) = drops.save_parts();
    if items.len() > MAX_DROPPED_ITEMS {
        return Err(invalid_input("too many dropped-item entities"));
    }
    let count = u32::try_from(items.len())
        .map_err(|_| invalid_input("dropped-item entity count is invalid"))?;
    output.write_all(&next_id.to_le_bytes())?;
    output.write_all(&count.to_le_bytes())?;
    for item in items {
        output.write_all(&item.id.0.to_le_bytes())?;
        output.write_all(&item.stack.key.id.0.to_le_bytes())?;
        output.write_all(&item.stack.key.variant.to_le_bytes())?;
        output.write_all(&item.stack.count.to_le_bytes())?;
        for value in item.position.into_iter().chain(item.velocity) {
            output.write_all(&value.to_le_bytes())?;
        }
        output.write_all(&item.pickup_delay_ticks.to_le_bytes())?;
        output.write_all(&item.age_ticks.to_le_bytes())?;
    }
    Ok(())
}

fn read_drops<R: Read>(input: &mut R) -> io::Result<ItemDrops> {
    let next_id = read_u64(input)?;
    let count = read_u32(input)? as usize;
    if count > MAX_DROPPED_ITEMS {
        return Err(invalid_data("too many saved dropped-item entities"));
    }
    let mut items = Vec::with_capacity(count);
    for _ in 0..count {
        let id = DropId(read_u64(input)?);
        let item_id = ItemId(read_u16(input)?);
        let variant = read_u32(input)?;
        let stack_count = read_u16(input)?;
        let mut position = [0.0; 3];
        let mut velocity = [0.0; 3];
        for value in &mut position {
            *value = read_f64(input)?;
        }
        for value in &mut velocity {
            *value = read_f64(input)?;
        }
        let pickup_delay_ticks = read_u16(input)?;
        let age_ticks = read_u32(input)?;
        items.push(DroppedItem {
            id,
            stack: ItemStack {
                key: ItemKey {
                    id: item_id,
                    variant,
                },
                count: stack_count,
            },
            position,
            velocity,
            pickup_delay_ticks,
            age_ticks,
        });
    }
    ItemDrops::from_save_parts(next_id, items).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid saved dropped-item state: {error}"),
        )
    })
}

fn validate_player(player: &PlayerSnapshot) -> io::Result<()> {
    if !player
        .position
        .iter()
        .all(|value| value.is_finite() && value.abs() <= i32::MAX as f64 - 2.0)
        || !player
            .velocity
            .iter()
            .all(|value| value.is_finite() && value.abs() <= 1_000_000.0)
        || !player.yaw.is_finite()
        || !player.pitch.is_finite()
        || player.pitch.abs() > std::f32::consts::FRAC_PI_2
    {
        return Err(invalid_input("invalid player snapshot values"));
    }
    Ok(())
}

fn encode_save(
    metadata: &WorldMetadata,
    player: PlayerSnapshot,
    world_bytes: &[u8],
) -> io::Result<Vec<u8>> {
    validate_name(&metadata.name)?;
    validate_player(&player)?;
    if metadata.terrain_generator_version != crate::terrain::CURRENT_TERRAIN_GENERATOR_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "terrain generator version is unsupported",
        ));
    }
    if world_bytes.len() > MAX_SAVE_BYTES {
        return Err(invalid_input("world save exceeds the current size limit"));
    }
    let name = metadata.name.as_bytes();
    let name_len =
        u16::try_from(name.len()).map_err(|_| invalid_input("world name is too long"))?;
    let world_len = u64::try_from(world_bytes.len())
        .map_err(|_| invalid_input("world payload length is invalid"))?;
    let mut output = Vec::with_capacity(world_bytes.len() + 96 + name.len());
    output.write_all(SAVE_MAGIC_V6)?;
    output.write_all(&SAVE_VERSION.to_le_bytes())?;
    output.write_all(&name_len.to_le_bytes())?;
    output.write_all(name)?;
    output.write_all(&metadata.seed.to_le_bytes())?;
    output.write_all(&metadata.terrain_seed.to_le_bytes())?;
    output.write_all(&metadata.terrain_generator_version.to_le_bytes())?;
    output.write_all(&metadata.created_at.to_le_bytes())?;
    output.write_all(&metadata.updated_at.to_le_bytes())?;
    output.write_all(&[metadata.game_mode as u8])?;
    output.write_all(&[metadata.difficulty as u8])?;
    output.write_all(&[u8::from(metadata.difficulty_locked)])?;
    for value in player.position.into_iter().chain(player.velocity) {
        output.write_all(&value.to_le_bytes())?;
    }
    output.write_all(&player.yaw.to_le_bytes())?;
    output.write_all(&player.pitch.to_le_bytes())?;
    output.write_all(&[u8::from(player.flying), u8::from(player.grounded)])?;
    output.write_all(&player.selected_block.0.to_le_bytes())?;
    write_inventory(&mut output, &player.inventory)?;
    write_drops(&mut output, &player.drops)?;
    output.write_all(&world_len.to_le_bytes())?;
    output.write_all(world_bytes)?;
    output.write_all(&checksum64(&output).to_le_bytes())?;
    Ok(output)
}

fn decode_save(bytes: &[u8], id: &str) -> io::Result<StoredWorld> {
    validate_id(id)?;
    if bytes.len() > MAX_SAVE_BYTES {
        return Err(invalid_data("world save exceeds the current size limit"));
    }
    if bytes.len() < 8 {
        return Err(invalid_data("world save is truncated"));
    }
    let payload_len = bytes.len() - 8;
    let expected_checksum = u64::from_le_bytes(bytes[payload_len..].try_into().unwrap());
    if checksum64(&bytes[..payload_len]) != expected_checksum {
        return Err(invalid_data("world save checksum mismatch"));
    }
    let mut input = Cursor::new(&bytes[..payload_len]);
    let (metadata, version) = read_metadata(&mut input, id)?;
    let mut player = PlayerSnapshot::default();
    for axis in 0..3 {
        player.position[axis] = read_f64(&mut input)?;
    }
    for axis in 0..3 {
        player.velocity[axis] = read_f64(&mut input)?;
    }
    player.yaw = read_f32(&mut input)?;
    player.pitch = read_f32(&mut input)?;
    player.flying = read_bool(&mut input)?;
    player.grounded = read_bool(&mut input)?;
    player.selected_block = BlockId(read_u16(&mut input)?);
    if version >= 3 {
        player.inventory = read_inventory(&mut input)?;
    }
    if version >= 4 {
        player.drops = read_drops(&mut input)?;
    }
    validate_player(&player).map_err(|_| invalid_data("invalid player snapshot values"))?;
    let world_len = read_u64(&mut input)?;
    let remaining = payload_len.saturating_sub(input.position() as usize);
    if world_len > MAX_SAVE_BYTES as u64 || world_len != remaining as u64 {
        return Err(invalid_data("invalid world payload length"));
    }
    let start = input.position() as usize;
    let world_bytes = &bytes[start..payload_len];
    let mut world_input = Cursor::new(world_bytes);
    let world = World::load_with_limit(&mut world_input, MAX_SERIALIZED_CHUNKS)?;
    if world_input.position() != world_len {
        return Err(invalid_data("world payload contains trailing bytes"));
    }
    Ok(StoredWorld {
        metadata,
        world,
        player,
    })
}

fn read_summary(path: &Path, id: &str) -> io::Result<WorldSummary> {
    let bytes = read_bounded(path)?;
    let saved = decode_save(&bytes, id)?;
    Ok(WorldSummary {
        id: id.to_owned(),
        name: Some(saved.metadata.name),
        seed: Some(saved.metadata.seed),
        updated_at: Some(saved.metadata.updated_at),
        health: WorldHealth::Ready,
    })
}

fn checksum64(bytes: &[u8]) -> u64 {
    let mut checksum = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        checksum ^= *byte as u64;
        checksum = checksum.wrapping_mul(0x0000_0100_0000_01b3);
    }
    checksum
}

fn read_metadata<R: Read>(input: &mut R, id: &str) -> io::Result<(WorldMetadata, u32)> {
    let mut magic = [0; 8];
    input.read_exact(&mut magic)?;
    let version = read_u32(input)?;
    let version_matches_magic = match version {
        1 => &magic == SAVE_MAGIC_V1,
        2 => &magic == SAVE_MAGIC_V2,
        3 => &magic == SAVE_MAGIC_V3,
        4 => &magic == SAVE_MAGIC_V4,
        5 => &magic == SAVE_MAGIC_V5,
        6 => &magic == SAVE_MAGIC_V6,
        _ => false,
    };
    if !version_matches_magic {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unsupported world save version",
        ));
    }
    let name_len = read_u16(input)? as usize;
    if name_len == 0 || name_len > MAX_NAME_BYTES {
        return Err(invalid_data("invalid world name length"));
    }
    let mut name = vec![0; name_len];
    input.read_exact(&mut name)?;
    let name = String::from_utf8(name).map_err(|_| invalid_data("world name is not UTF-8"))?;
    let name = validate_name(&name).map_err(|_| invalid_data("invalid saved world name"))?;
    let seed = read_u64(input)?;
    let terrain_seed = if version >= 5 { read_u64(input)? } else { seed };
    let terrain_generator_version = if version >= 6 {
        read_u32(input)?
    } else {
        crate::terrain::CURRENT_TERRAIN_GENERATOR_VERSION
    };
    if terrain_generator_version != crate::terrain::CURRENT_TERRAIN_GENERATOR_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "terrain generator version is unsupported",
        ));
    }
    let created_at = read_u64(input)?;
    let updated_at = read_u64(input)?;
    let mut game_mode = [0];
    input.read_exact(&mut game_mode)?;
    let game_mode = StoredGameMode::try_from(game_mode[0])?;
    let (difficulty, difficulty_locked) = if version >= 2 {
        let mut difficulty = [0];
        input.read_exact(&mut difficulty)?;
        (
            StoredDifficulty::try_from(difficulty[0])?,
            read_bool(input)?,
        )
    } else {
        (StoredDifficulty::Normal, false)
    };
    Ok((
        WorldMetadata {
            id: id.to_owned(),
            name,
            seed,
            terrain_seed,
            terrain_generator_version,
            created_at,
            updated_at,
            game_mode,
            difficulty,
            difficulty_locked,
        },
        version,
    ))
}

fn read_bounded(path: &Path) -> io::Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(invalid_data("save path is not a regular file"));
    }
    let file = File::open(path)?;
    let length = metadata.len();
    if length > MAX_SAVE_BYTES as u64 {
        return Err(invalid_data("world save exceeds the current size limit"));
    }
    let mut bytes = Vec::with_capacity(length as usize);
    file.take(MAX_SAVE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_SAVE_BYTES {
        return Err(invalid_data("world save exceeds the current size limit"));
    }
    Ok(bytes)
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid_input("save path has no parent directory"))?;
    fs::create_dir_all(parent)?;
    let temp = temporary_path(path);
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        replace_file(&temp, path).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("atomic save replacement failed: {error}"),
            )
        })?;
        sync_directory(parent)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn atomic_copy(source: &Path, destination: &Path) -> io::Result<()> {
    let temp = temporary_path(destination);
    let result = (|| {
        fs::copy(source, &temp)?;
        OpenOptions::new().write(true).open(&temp)?.sync_all()?;
        replace_file(&temp, destination).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("atomic backup replacement failed: {error}"),
            )
        })?;
        sync_directory(destination.parent().expect("backup path has parent"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "Kernel32")]
    extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    let source = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

fn temporary_path(path: &Path) -> PathBuf {
    let counter = NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("save");
    path.with_file_name(format!(".{name}.{}.{}.tmp", std::process::id(), counter))
}

fn prune_backups(directory: &Path, keep: usize, preserved: Option<&str>) -> io::Result<()> {
    let mut entries = fs::read_dir(directory)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            if !file_type.is_file() {
                return None;
            }
            let name = entry.file_name().to_str()?.to_owned();
            valid_backup_name(&name).then_some((name, entry.path()))
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| {
        backup_order(&a.0)
            .cmp(&backup_order(&b.0))
            .then_with(|| a.0.cmp(&b.0))
    });
    while entries.len() > keep {
        let Some(index) = entries
            .iter()
            .position(|(name, _)| Some(name.as_str()) != preserved)
        else {
            break;
        };
        let (_, path) = entries.remove(index);
        fs::remove_file(path)?;
    }
    Ok(())
}

fn backup_order(name: &str) -> Option<(u64, u64)> {
    let name = name.strip_prefix("backup-")?.strip_suffix(".sav")?;
    let (timestamp, sequence) = name.split_once('-')?;
    Some((timestamp.parse().ok()?, sequence.parse().ok()?))
}

fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        File::open(path)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

fn unix_seconds() -> io::Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))
}

fn invalid_data(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn invalid_input(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
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

fn read_u64<R: Read>(input: &mut R) -> io::Result<u64> {
    let mut bytes = [0; 8];
    input.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_f32<R: Read>(input: &mut R) -> io::Result<f32> {
    Ok(f32::from_le_bytes(read_array(input)?))
}

fn read_f64<R: Read>(input: &mut R) -> io::Result<f64> {
    Ok(f64::from_le_bytes(read_array(input)?))
}

fn read_array<const N: usize, R: Read>(input: &mut R) -> io::Result<[u8; N]> {
    let mut bytes = [0; N];
    input.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn read_bool<R: Read>(input: &mut R) -> io::Result<bool> {
    let mut byte = [0];
    input.read_exact(&mut byte)?;
    match byte[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(invalid_data("invalid boolean in player snapshot")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::ChunkPos;

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new() -> Self {
            let counter = NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "voxel-engine-world-store-{}-{counter}",
                std::process::id()
            ));
            Self(path)
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn test_world() -> World {
        let mut world = World::new();
        world.get_or_create_chunk(ChunkPos { x: -1, y: 0, z: 2 });
        world
    }

    fn encode_v1_fixture(
        metadata: &WorldMetadata,
        player: PlayerSnapshot,
        world_bytes: &[u8],
    ) -> Vec<u8> {
        let mut output = Vec::new();
        output.write_all(SAVE_MAGIC_V1).unwrap();
        output.write_all(&1u32.to_le_bytes()).unwrap();
        output
            .write_all(&(metadata.name.len() as u16).to_le_bytes())
            .unwrap();
        output.write_all(metadata.name.as_bytes()).unwrap();
        output.write_all(&metadata.seed.to_le_bytes()).unwrap();
        output
            .write_all(&metadata.created_at.to_le_bytes())
            .unwrap();
        output
            .write_all(&metadata.updated_at.to_le_bytes())
            .unwrap();
        output.write_all(&[metadata.game_mode as u8]).unwrap();
        for value in player.position.into_iter().chain(player.velocity) {
            output.write_all(&value.to_le_bytes()).unwrap();
        }
        output.write_all(&player.yaw.to_le_bytes()).unwrap();
        output.write_all(&player.pitch.to_le_bytes()).unwrap();
        output
            .write_all(&[u8::from(player.flying), u8::from(player.grounded)])
            .unwrap();
        output
            .write_all(&player.selected_block.0.to_le_bytes())
            .unwrap();
        output
            .write_all(&(world_bytes.len() as u64).to_le_bytes())
            .unwrap();
        output.write_all(world_bytes).unwrap();
        let checksum = checksum64(&output);
        output.write_all(&checksum.to_le_bytes()).unwrap();
        output
    }

    fn encode_v2_fixture(
        metadata: &WorldMetadata,
        player: PlayerSnapshot,
        world_bytes: &[u8],
    ) -> Vec<u8> {
        let mut v1 = encode_v1_fixture(metadata, player, world_bytes);
        let name_len = u16::from_le_bytes([v1[12], v1[13]]) as usize;
        let mode_end = 14 + name_len + 24 + 1;
        v1.splice(mode_end..mode_end, [StoredDifficulty::Normal as u8, 0]);
        v1[..8].copy_from_slice(SAVE_MAGIC_V2);
        v1[8..12].copy_from_slice(&2u32.to_le_bytes());
        let payload_len = v1.len() - 8;
        let checksum = checksum64(&v1[..payload_len]);
        v1[payload_len..].copy_from_slice(&checksum.to_le_bytes());
        v1
    }

    fn encode_v3_fixture(
        metadata: &WorldMetadata,
        player: PlayerSnapshot,
        world_bytes: &[u8],
    ) -> Vec<u8> {
        let mut v2 = encode_v2_fixture(metadata, player.clone(), world_bytes);
        let name_len = u16::from_le_bytes([v2[12], v2[13]]) as usize;
        let inventory_offset = 101 + name_len;
        let mut inventory_bytes = Vec::new();
        write_inventory(&mut inventory_bytes, &player.inventory).unwrap();
        v2.splice(inventory_offset..inventory_offset, inventory_bytes);
        v2[..8].copy_from_slice(SAVE_MAGIC_V3);
        v2[8..12].copy_from_slice(&3u32.to_le_bytes());
        let payload_len = v2.len() - 8;
        let checksum = checksum64(&v2[..payload_len]);
        v2[payload_len..].copy_from_slice(&checksum.to_le_bytes());
        v2
    }

    fn encode_v4_fixture(
        metadata: &WorldMetadata,
        player: PlayerSnapshot,
        world_bytes: &[u8],
    ) -> Vec<u8> {
        let mut v5 = encode_save(metadata, player, world_bytes).unwrap();
        let name_len = u16::from_le_bytes([v5[12], v5[13]]) as usize;
        let generator_version_offset = 14 + name_len + 16;
        v5.drain(generator_version_offset..generator_version_offset + 4);
        v5[..8].copy_from_slice(SAVE_MAGIC_V5);
        v5[8..12].copy_from_slice(&5u32.to_le_bytes());
        let payload_len = v5.len() - 8;
        let checksum = checksum64(&v5[..payload_len]);
        v5[payload_len..].copy_from_slice(&checksum.to_le_bytes());
        let terrain_seed_offset = 14 + name_len + 8;
        v5.drain(terrain_seed_offset..terrain_seed_offset + 8);
        v5[..8].copy_from_slice(SAVE_MAGIC_V4);
        v5[8..12].copy_from_slice(&4u32.to_le_bytes());
        let payload_len = v5.len() - 8;
        let checksum = checksum64(&v5[..payload_len]);
        v5[payload_len..].copy_from_slice(&checksum.to_le_bytes());
        v5
    }

    #[test]
    fn create_save_load_round_trips_world_and_player_state() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let mut catalog = crate::inventory::ItemCatalog::default();
        catalog
            .register(
                crate::inventory::ItemId(42),
                crate::inventory::ItemDefinition::stackable(64),
            )
            .unwrap();
        let mut inventory = Inventory::default();
        inventory
            .insert(ItemKey::new(ItemId(42)), 23, &catalog)
            .unwrap();
        inventory.select_hotbar(3).unwrap();
        let mut drops = ItemDrops::default();
        drops
            .spawn(
                ItemStack {
                    key: ItemKey::new(ItemId(42)),
                    count: 5,
                },
                [4.5, 8.125, -1.5],
                7,
                &catalog,
            )
            .unwrap();
        let player = PlayerSnapshot {
            position: [-2.5, 9.25, 33.75],
            velocity: [0.1, -0.2, 0.3],
            yaw: 1.25,
            pitch: -0.5,
            flying: true,
            grounded: false,
            selected_block: BlockId(3),
            inventory,
            drops,
        };
        let mut saved = store
            .create(
                "My World",
                42,
                StoredGameMode::Creative,
                test_world(),
                player.clone(),
            )
            .unwrap();
        saved.world.set_block([-1, 3, 32], BlockId(2));
        store
            .save(&mut saved.metadata, &saved.world, saved.player.clone())
            .unwrap();

        let loaded = store.load(&saved.metadata.id).unwrap();
        assert_eq!(loaded.metadata.name, "My World");
        assert_eq!(loaded.metadata.seed, 42);
        assert_eq!(loaded.metadata.game_mode, StoredGameMode::Creative);
        assert_eq!(loaded.player, player);
        assert_eq!(loaded.player.inventory.selected_hotbar(), 3);
        assert_eq!(loaded.player.inventory.count(ItemKey::new(ItemId(42))), 23);
        assert_eq!(loaded.player.drops.len(), 1);
        assert_eq!(loaded.player.drops.iter().next().unwrap().stack.count, 5);
        assert_eq!(loaded.world.block_at([-1, 3, 32]), Some(BlockId(2)));
        assert_eq!(store.list().unwrap()[0].health, WorldHealth::Ready);
    }

    #[test]
    fn difficulty_and_lock_options_round_trip_and_edit_with_a_backup() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let created = store
            .create_with_options(
                "Options Test",
                55,
                StoredGameMode::Survival,
                StoredDifficulty::Easy,
                false,
                test_world(),
                PlayerSnapshot::default(),
            )
            .unwrap();
        assert_eq!(created.metadata.difficulty, StoredDifficulty::Easy);
        assert!(!created.metadata.difficulty_locked);

        let updated = store
            .update_options(
                &created.metadata.id,
                "Options Updated",
                77,
                StoredGameMode::Creative,
                StoredDifficulty::Hard,
                true,
            )
            .unwrap();
        assert_eq!(updated.metadata.name, "Options Updated");
        assert_eq!(updated.metadata.seed, 77);
        assert_eq!(updated.metadata.terrain_seed, created.metadata.terrain_seed);
        assert_eq!(updated.metadata.game_mode, StoredGameMode::Creative);
        assert_eq!(updated.metadata.difficulty, StoredDifficulty::Hard);
        assert!(updated.metadata.difficulty_locked);
        assert_eq!(updated.world, created.world);
        assert_eq!(updated.player, created.player);
        assert_eq!(store.list_backups(&created.metadata.id).unwrap().len(), 1);
        assert_eq!(store.load(&created.metadata.id).unwrap(), updated);
        assert_eq!(
            store
                .update_options(
                    &created.metadata.id,
                    "Blocked Change",
                    77,
                    StoredGameMode::Creative,
                    StoredDifficulty::Easy,
                    true,
                )
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        let unlocked = store
            .update_options(
                &created.metadata.id,
                "Unlocked",
                77,
                StoredGameMode::Creative,
                StoredDifficulty::Easy,
                false,
            )
            .unwrap();
        assert_eq!(unlocked.metadata.difficulty, StoredDifficulty::Easy);
        assert!(!unlocked.metadata.difficulty_locked);
    }

    #[test]
    fn version_one_worlds_load_with_defaults_and_upgrade_on_next_save() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let created = store
            .create(
                "Legacy World",
                23,
                StoredGameMode::Creative,
                test_world(),
                PlayerSnapshot::default(),
            )
            .unwrap();
        let mut world_bytes = Vec::new();
        created.world.save(&mut world_bytes).unwrap();
        let legacy = encode_v1_fixture(&created.metadata, created.player, &world_bytes);
        let save_path = store.save_path(&created.metadata.id).unwrap();
        atomic_write(&save_path, &legacy).unwrap();

        let mut loaded = store.load(&created.metadata.id).unwrap();
        assert_eq!(loaded.metadata.difficulty, StoredDifficulty::Normal);
        assert!(!loaded.metadata.difficulty_locked);
        store
            .save(&mut loaded.metadata, &loaded.world, loaded.player.clone())
            .unwrap();

        let upgraded = read_bounded(&save_path).unwrap();
        assert_eq!(&upgraded[..8], SAVE_MAGIC_V6);
        assert_eq!(u32::from_le_bytes(upgraded[8..12].try_into().unwrap()), 6);
        assert_eq!(store.load(&created.metadata.id).unwrap(), loaded);
    }

    #[test]
    fn version_two_worlds_load_with_empty_player_items_and_upgrade_to_version_six() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let created = store
            .create(
                "Version Two World",
                24,
                StoredGameMode::Survival,
                test_world(),
                PlayerSnapshot::default(),
            )
            .unwrap();
        let mut world_bytes = Vec::new();
        created.world.save(&mut world_bytes).unwrap();
        let legacy = encode_v2_fixture(&created.metadata, created.player, &world_bytes);
        let save_path = store.save_path(&created.metadata.id).unwrap();
        atomic_write(&save_path, &legacy).unwrap();

        let mut loaded = store.load(&created.metadata.id).unwrap();
        assert_eq!(loaded.metadata.difficulty, StoredDifficulty::Normal);
        assert_eq!(loaded.player.inventory, Inventory::default());
        store
            .save(&mut loaded.metadata, &loaded.world, loaded.player.clone())
            .unwrap();

        let upgraded = read_bounded(&save_path).unwrap();
        assert_eq!(&upgraded[..8], SAVE_MAGIC_V6);
        assert_eq!(u32::from_le_bytes(upgraded[8..12].try_into().unwrap()), 6);
        assert_eq!(store.load(&created.metadata.id).unwrap(), loaded);
    }

    #[test]
    fn version_three_worlds_keep_inventory_and_upgrade_with_empty_drops() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let mut catalog = crate::inventory::ItemCatalog::default();
        catalog
            .register(ItemId(9), crate::inventory::ItemDefinition::stackable(64))
            .unwrap();
        let mut inventory = Inventory::default();
        inventory
            .insert(ItemKey::new(ItemId(9)), 13, &catalog)
            .unwrap();
        let player = PlayerSnapshot {
            inventory,
            ..PlayerSnapshot::default()
        };
        let created = store
            .create(
                "Version Three World",
                25,
                StoredGameMode::Creative,
                test_world(),
                player.clone(),
            )
            .unwrap();
        let mut world_bytes = Vec::new();
        created.world.save(&mut world_bytes).unwrap();
        let legacy = encode_v3_fixture(&created.metadata, player, &world_bytes);
        let save_path = store.save_path(&created.metadata.id).unwrap();
        atomic_write(&save_path, &legacy).unwrap();

        let mut loaded = store.load(&created.metadata.id).unwrap();
        assert_eq!(loaded.player.inventory, inventory);
        assert!(loaded.player.drops.is_empty());
        store
            .save(&mut loaded.metadata, &loaded.world, loaded.player.clone())
            .unwrap();
        let upgraded = read_bounded(&save_path).unwrap();
        assert_eq!(&upgraded[..8], SAVE_MAGIC_V6);
        assert_eq!(store.load(&created.metadata.id).unwrap(), loaded);
    }

    #[test]
    fn version_four_worlds_migrate_terrain_seed_from_seed_metadata() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let mut catalog = crate::inventory::ItemCatalog::default();
        catalog
            .register(ItemId(12), crate::inventory::ItemDefinition::stackable(64))
            .unwrap();
        let mut drops = ItemDrops::default();
        drops
            .spawn(
                ItemStack {
                    key: ItemKey::new(ItemId(12)),
                    count: 3,
                },
                [2.0, 3.0, 4.0],
                5,
                &catalog,
            )
            .unwrap();
        let player = PlayerSnapshot {
            drops,
            ..PlayerSnapshot::default()
        };
        let created = store
            .create(
                "Version Four World",
                42,
                StoredGameMode::Creative,
                test_world(),
                player.clone(),
            )
            .unwrap();
        let mut metadata = created.metadata.clone();
        metadata.terrain_seed = 99;
        let mut world_bytes = Vec::new();
        created.world.save(&mut world_bytes).unwrap();
        let legacy = encode_v4_fixture(&metadata, player.clone(), &world_bytes);
        let save_path = store.save_path(&created.metadata.id).unwrap();
        atomic_write(&save_path, &legacy).unwrap();

        let mut loaded = store.load(&created.metadata.id).unwrap();
        assert_eq!(loaded.metadata.seed, 42);
        assert_eq!(loaded.metadata.terrain_seed, 42);
        assert_eq!(loaded.player.drops, player.drops);
        store
            .save(&mut loaded.metadata, &loaded.world, loaded.player.clone())
            .unwrap();

        let upgraded = read_bounded(&save_path).unwrap();
        assert_eq!(&upgraded[..8], SAVE_MAGIC_V6);
        assert_eq!(store.load(&created.metadata.id).unwrap(), loaded);
    }

    #[test]
    fn editing_public_seed_keeps_terrain_seed_through_copy_and_reload() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let created = store
            .create(
                "Seeded World",
                19,
                StoredGameMode::Creative,
                test_world(),
                PlayerSnapshot::default(),
            )
            .unwrap();
        let edited = store
            .update_options(
                &created.metadata.id,
                "Seeded World",
                91,
                StoredGameMode::Creative,
                StoredDifficulty::Normal,
                false,
            )
            .unwrap();
        assert_eq!(edited.metadata.seed, 91);
        assert_eq!(edited.metadata.terrain_seed, 19);

        let loaded = store.load(&created.metadata.id).unwrap();
        assert_eq!(loaded.metadata.terrain_seed, 19);
        let copied = store.copy(&created.metadata.id, "Seeded Copy").unwrap();
        assert_eq!(copied.metadata.seed, 91);
        assert_eq!(copied.metadata.terrain_seed, 19);
        assert_eq!(store.load(&copied.metadata.id).unwrap(), copied);
    }

    #[test]
    fn evicted_generated_chunks_regenerate_after_reload_without_losing_edits() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let generator = crate::terrain::TerrainGenerator::default();
        let edited_position = [-31, 15, 1];
        let mut world = generator.generate_region(2);
        world.set_block(edited_position, BlockId(5));

        let active_region: Vec<_> = (-1..=3)
            .flat_map(|z| (-1..=3).map(move |x| ChunkPos { x, y: 0, z }))
            .collect();
        generator
            .ensure_region(&mut world, ChunkPos { x: 1, y: 0, z: 1 }, 2)
            .unwrap();
        world.evict_generated_chunks_outside(&active_region);
        assert_eq!(world.chunk_count(), 26);

        let saved = store
            .create(
                "Streamed World",
                generator.seed,
                StoredGameMode::Creative,
                world,
                PlayerSnapshot::default(),
            )
            .unwrap();
        let mut loaded = store.load(&saved.metadata.id).unwrap();
        assert_eq!(loaded.world.chunk_count(), 26);
        assert_eq!(loaded.world.block_at(edited_position), Some(BlockId(5)));
        assert!(!loaded
            .world
            .is_generated_chunk(ChunkPos { x: -2, y: 0, z: 0 }));
        assert!(loaded
            .world
            .is_generated_chunk(ChunkPos { x: 3, y: 0, z: 0 }));

        let origin_region: Vec<_> = (-2..=2)
            .flat_map(|z| (-2..=2).map(move |x| ChunkPos { x, y: 0, z }))
            .collect();
        generator
            .ensure_region(&mut loaded.world, ChunkPos::default(), 2)
            .unwrap();
        loaded.world.evict_generated_chunks_outside(&origin_region);
        assert_eq!(loaded.world.chunk_count(), 25);
        assert_eq!(loaded.world.block_at(edited_position), Some(BlockId(5)));
        assert_eq!(
            loaded.world.get_chunk(ChunkPos { x: 0, y: 0, z: 0 }),
            Some(&generator.generate_chunk(ChunkPos { x: 0, y: 0, z: 0 }))
        );
    }

    #[test]
    fn block_state_shapes_survive_world_store_save_and_reload() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let position = [2, 1, 1];
        let state = crate::blocks::BlockState {
            kind: crate::blocks::BlockKind::Slab,
            upper: true,
            layers: 1,
            connections: 0,
        };
        let mut world = World::new();
        world.get_or_create_chunk(ChunkPos::default());
        world.set_block(position, BlockId(4));
        world.set_block_state(position, state);
        let created = store
            .create(
                "Block State World",
                12,
                StoredGameMode::Creative,
                world,
                PlayerSnapshot::default(),
            )
            .unwrap();

        let loaded = store.load(&created.metadata.id).unwrap();
        assert_eq!(loaded.world.block_at(position), Some(BlockId(4)));
        assert_eq!(loaded.world.block_state_at(position), Some(state));
        let hit = crate::interaction::raycast_blocks(
            &loaded.world,
            &crate::blocks::BlockRegistry::new(),
            [0.5, 1.75, 1.5],
            [1.0, 0.0, 0.0],
            3.0,
        )
        .unwrap();
        assert_eq!(hit.position, crate::interaction::BlockPos(position));
    }

    #[test]
    fn unsupported_terrain_generator_versions_are_rejected_without_downgrade() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let created = store
            .create(
                "Generator Version World",
                43,
                StoredGameMode::Creative,
                test_world(),
                PlayerSnapshot::default(),
            )
            .unwrap();
        let path = store.save_path(&created.metadata.id).unwrap();
        let mut bytes = read_bounded(&path).unwrap();
        let name_len = u16::from_le_bytes([bytes[12], bytes[13]]) as usize;
        let generator_version_offset = 14 + name_len + 16;
        bytes[generator_version_offset..generator_version_offset + 4]
            .copy_from_slice(&99u32.to_le_bytes());
        let checksum_offset = bytes.len() - 8;
        let checksum = checksum64(&bytes[..checksum_offset]);
        bytes[checksum_offset..].copy_from_slice(&checksum.to_le_bytes());
        atomic_write(&path, &bytes).unwrap();

        assert_eq!(
            store.load(&created.metadata.id).unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
        assert_eq!(
            store.list().unwrap()[0].health,
            WorldHealth::UnsupportedVersion
        );
        assert_eq!(read_bounded(&path).unwrap(), bytes);
    }

    #[test]
    fn dropped_item_decoder_enforces_count_id_and_state_bounds() {
        let mut excessive = Vec::new();
        excessive.write_all(&0u64.to_le_bytes()).unwrap();
        excessive
            .write_all(&((MAX_DROPPED_ITEMS as u32) + 1).to_le_bytes())
            .unwrap();
        assert!(read_drops(&mut Cursor::new(excessive)).is_err());

        let mut invalid_id = Vec::new();
        invalid_id.write_all(&1u64.to_le_bytes()).unwrap();
        invalid_id.write_all(&1u32.to_le_bytes()).unwrap();
        invalid_id.write_all(&1u64.to_le_bytes()).unwrap(); // id must be less than next_id
        invalid_id.write_all(&1u16.to_le_bytes()).unwrap();
        invalid_id.write_all(&0u32.to_le_bytes()).unwrap();
        invalid_id.write_all(&1u16.to_le_bytes()).unwrap();
        for value in [0.0f64, 1.0, 0.0, 0.0, 0.0, 0.0] {
            invalid_id.write_all(&value.to_le_bytes()).unwrap();
        }
        invalid_id.write_all(&0u16.to_le_bytes()).unwrap();
        invalid_id.write_all(&0u32.to_le_bytes()).unwrap();
        assert!(read_drops(&mut Cursor::new(invalid_id)).is_err());
    }

    #[test]
    fn inventory_decoder_rejects_invalid_selection_counts_and_armor_stacks() {
        let mut invalid_selection = vec![9];
        invalid_selection.extend([0; SAVED_SLOT_COUNT]);
        assert!(read_inventory(&mut Cursor::new(invalid_selection)).is_err());

        let mut zero_count = vec![0, 1];
        zero_count.extend(7u16.to_le_bytes());
        zero_count.extend(0u32.to_le_bytes());
        zero_count.extend(0u16.to_le_bytes());
        zero_count.extend([0; SAVED_SLOT_COUNT - 1]);
        assert!(read_inventory(&mut Cursor::new(zero_count)).is_err());

        let mut over_capacity = vec![0, 1];
        over_capacity.extend(7u16.to_le_bytes());
        over_capacity.extend(0u32.to_le_bytes());
        over_capacity.extend(65u16.to_le_bytes());
        over_capacity.extend([0; SAVED_SLOT_COUNT - 1]);
        assert!(read_inventory(&mut Cursor::new(over_capacity)).is_err());

        let mut armor_stack = vec![0];
        armor_stack.extend([0; crate::inventory::MAIN_SLOT_COUNT]);
        armor_stack.push(1);
        armor_stack.extend(8u16.to_le_bytes());
        armor_stack.extend(0u32.to_le_bytes());
        armor_stack.extend(2u16.to_le_bytes());
        armor_stack.extend([0; SAVED_SLOT_COUNT - crate::inventory::MAIN_SLOT_COUNT - 1]);
        assert!(read_inventory(&mut Cursor::new(armor_stack)).is_err());
    }

    #[test]
    fn prototype_open_resumes_existing_world_instead_of_regenerating_it() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let mut first = store.open_or_create_prototype(test_world()).unwrap();
        first.world.set_block([-1, 2, 32], BlockId(3));
        store
            .save(&mut first.metadata, &first.world, first.player.clone())
            .unwrap();

        let reopened = store
            .open_or_create_prototype(World::new())
            .expect("existing prototype world should load");
        assert_eq!(reopened.metadata.id, first.metadata.id);
        assert_eq!(reopened.world.block_at([-1, 2, 32]), Some(BlockId(3)));
        assert_eq!(reopened.metadata.name, "Creative Prototype");
    }

    #[test]
    fn save_creates_bounded_backup_and_restore_recovers_previous_state() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let mut saved = store
            .create(
                "Backup World",
                7,
                StoredGameMode::Survival,
                test_world(),
                PlayerSnapshot::default(),
            )
            .unwrap();
        saved.world.set_block([-1, 0, 32], BlockId(3));
        store
            .save(&mut saved.metadata, &saved.world, saved.player.clone())
            .unwrap();
        let backups = store.list_backups(&saved.metadata.id).unwrap();
        assert_eq!(backups.len(), 1);

        let backup_dir = store
            .world_dir(&saved.metadata.id)
            .unwrap()
            .join(BACKUP_DIR);
        let invalid_backup = "backup-9999999999-999.sav";
        fs::write(backup_dir.join(invalid_backup), b"corrupt backup").unwrap();
        assert_eq!(store.list_backups(&saved.metadata.id).unwrap(), backups);
        assert!(store
            .restore_backup(&saved.metadata.id, invalid_backup)
            .is_err());

        let restored = store
            .restore_backup(&saved.metadata.id, &backups[0])
            .unwrap();
        assert_eq!(restored.world.block_at([-1, 0, 32]), Some(BlockId(0)));
        assert_eq!(
            store
                .load(&saved.metadata.id)
                .unwrap()
                .world
                .block_at([-1, 0, 32]),
            Some(BlockId(0))
        );
        assert!(store
            .restore_backup(&saved.metadata.id, "..\\outside.sav")
            .is_err());
    }

    #[test]
    fn backup_retention_orders_numeric_sequences_not_lexicographic_names() {
        let root = TestRoot::new();
        fs::create_dir_all(&root.0).unwrap();
        for name in ["backup-100-2.sav", "backup-100-10.sav", "backup-100-3.sav"] {
            fs::write(root.0.join(name), b"fixture").unwrap();
        }

        prune_backups(&root.0, 2, None).unwrap();
        let mut names: Vec<_> = fs::read_dir(&root.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, vec!["backup-100-10.sav", "backup-100-3.sav"]);
    }

    #[test]
    fn restoring_backup_preserves_selected_snapshot_and_dirty_current_world() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let mut saved = store
            .create(
                "Recovery World",
                11,
                StoredGameMode::Creative,
                test_world(),
                PlayerSnapshot::default(),
            )
            .unwrap();
        for block in 1..=6 {
            saved.world.set_block([-1, 1, 32], BlockId(block));
            store
                .save(&mut saved.metadata, &saved.world, saved.player.clone())
                .unwrap();
        }
        let backups = store.list_backups(&saved.metadata.id).unwrap();
        assert_eq!(backups.len(), MAX_BACKUPS);
        let selected_backup = backups[0].clone();
        let selected_bytes = read_bounded(
            &store
                .world_dir(&saved.metadata.id)
                .unwrap()
                .join(BACKUP_DIR)
                .join(&selected_backup),
        )
        .unwrap();
        let expected = decode_save(&selected_bytes, &saved.metadata.id).unwrap();

        let dirty_marker = [-1, 2, 32];
        saved.world.set_block(dirty_marker, BlockId(3));
        saved.player.position = [4.0, 12.0, 5.0];
        let world_id = saved.metadata.id.clone();
        let restored = store
            .restore_backup_with_snapshot(
                &world_id,
                &selected_backup,
                Some((&mut saved.metadata, &saved.world, saved.player.clone())),
            )
            .unwrap();

        assert_eq!(restored.world, expected.world);
        assert_eq!(restored.player, expected.player);
        assert!(store
            .list_backups(&saved.metadata.id)
            .unwrap()
            .contains(&selected_backup));
        let backup_snapshots: Vec<_> = store
            .list_backups(&saved.metadata.id)
            .unwrap()
            .into_iter()
            .filter_map(|name| {
                let path = store
                    .world_dir(&saved.metadata.id)
                    .ok()?
                    .join(BACKUP_DIR)
                    .join(&name);
                let snapshot = read_bounded(&path)
                    .ok()
                    .and_then(|bytes| decode_save(&bytes, &saved.metadata.id).ok())?;
                Some((name, snapshot))
            })
            .collect();
        let retained_dirty_snapshot = backup_snapshots.iter().any(|(_, snapshot)| {
            snapshot.world.block_at(dirty_marker) == Some(BlockId(3))
                && snapshot.player.position == [4.0, 12.0, 5.0]
        });
        let backup_states: Vec<_> = backup_snapshots
            .iter()
            .map(|(name, snapshot)| {
                (
                    name,
                    snapshot.world.block_at(dirty_marker),
                    snapshot.player.position,
                    snapshot.metadata.terrain_seed,
                )
            })
            .collect();
        assert!(retained_dirty_snapshot, "backup states: {backup_states:?}");
        assert_eq!(store.load(&saved.metadata.id).unwrap(), restored);
    }

    #[test]
    fn rename_keeps_world_and_player_data_and_preserves_previous_snapshot() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let player = PlayerSnapshot {
            position: [3.0, 12.0, -4.0],
            selected_block: BlockId(3),
            ..PlayerSnapshot::default()
        };
        let saved = store
            .create(
                "Before Rename",
                81,
                StoredGameMode::Survival,
                test_world(),
                player.clone(),
            )
            .unwrap();

        let renamed = store.rename(&saved.metadata.id, "After Rename").unwrap();

        assert_eq!(renamed.metadata.name, "After Rename");
        assert_eq!(renamed.metadata.seed, saved.metadata.seed);
        assert_eq!(renamed.player, player);
        assert_eq!(renamed.world, saved.world);
        assert_eq!(store.list_backups(&saved.metadata.id).unwrap().len(), 1);
        assert_eq!(
            store.load(&saved.metadata.id).unwrap().metadata.name,
            "After Rename"
        );
    }

    #[test]
    fn copy_creates_independent_world_and_delete_removes_only_the_target() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let source = store
            .create(
                "Source",
                19,
                StoredGameMode::Creative,
                test_world(),
                PlayerSnapshot::default(),
            )
            .unwrap();
        let copied = store.copy(&source.metadata.id, "Copy").unwrap();
        assert_ne!(copied.metadata.id, source.metadata.id);
        assert_eq!(copied.metadata.name, "Copy");
        assert_eq!(copied.metadata.seed, source.metadata.seed);
        assert_eq!(copied.world, source.world);

        let mut edited_copy = store.load(&copied.metadata.id).unwrap();
        edited_copy.world.set_block([-1, 1, 32], BlockId(3));
        store
            .save(
                &mut edited_copy.metadata,
                &edited_copy.world,
                edited_copy.player,
            )
            .unwrap();
        assert_eq!(
            store
                .load(&source.metadata.id)
                .unwrap()
                .world
                .block_at([-1, 1, 32]),
            Some(BlockId(0))
        );
        assert_eq!(
            store
                .load(&copied.metadata.id)
                .unwrap()
                .world
                .block_at([-1, 1, 32]),
            Some(BlockId(3))
        );

        store.delete(&source.metadata.id).unwrap();
        assert!(store.load(&source.metadata.id).is_err());
        assert_eq!(
            store
                .load(&copied.metadata.id)
                .unwrap()
                .world
                .block_at([-1, 1, 32]),
            Some(BlockId(3))
        );
    }

    #[test]
    fn non_directory_world_paths_and_non_file_saves_are_rejected() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        fs::create_dir_all(&root.0).unwrap();
        fs::write(root.0.join("world-1-4"), b"not a directory").unwrap();
        assert_eq!(
            store.load("world-1-4").unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );

        fs::create_dir_all(root.0.join("world-1-5").join(SAVE_FILE)).unwrap();
        assert_eq!(
            store.load("world-1-5").unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert!(store
            .list()
            .unwrap()
            .iter()
            .any(|entry| entry.id == "world-1-5" && entry.health == WorldHealth::Corrupt));
    }

    #[cfg(unix)]
    #[test]
    fn world_symlinks_cannot_read_or_delete_data_outside_the_store() {
        use std::os::unix::fs::symlink;

        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let outside = root.0.with_file_name(format!(
            "voxel-engine-world-outside-{}-{}",
            std::process::id(),
            NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root.0).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join(SAVE_FILE), b"outside save").unwrap();
        symlink(&outside, root.0.join("world-1-2")).unwrap();

        assert_eq!(
            store.load("world-1-2").unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            store.delete("world-1-2").unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(fs::read(outside.join(SAVE_FILE)).unwrap(), b"outside save");
        fs::remove_dir_all(outside).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn save_file_symlinks_are_reported_as_corrupt_without_following_them() {
        use std::os::unix::fs::symlink;

        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let outside = root.0.with_file_name(format!(
            "voxel-engine-save-outside-{}-{}",
            std::process::id(),
            NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.0.join("world-1-3")).unwrap();
        fs::write(&outside, b"outside save").unwrap();
        symlink(&outside, root.0.join("world-1-3").join(SAVE_FILE)).unwrap();

        let entry = store
            .list()
            .unwrap()
            .into_iter()
            .find(|entry| entry.id == "world-1-3")
            .unwrap();
        assert_eq!(entry.health, WorldHealth::Corrupt);
        assert_eq!(
            store.load("world-1-3").unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(fs::read(&outside).unwrap(), b"outside save");
        fs::remove_file(outside).unwrap();
    }

    #[test]
    fn world_names_and_ids_are_validated() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        assert!(store
            .create(
                "  ",
                1,
                StoredGameMode::Creative,
                World::new(),
                PlayerSnapshot::default(),
            )
            .is_err());
        assert!(store.load("..\\outside").is_err());
        assert!(validate_name(&("x".repeat(MAX_NAME_BYTES + 1))).is_err());
    }

    #[test]
    fn list_marks_corrupt_or_missing_world_files_without_overwriting_them() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        fs::create_dir_all(root.0.join("world-123-1")).unwrap();
        fs::create_dir_all(root.0.join("world-123-2")).unwrap();
        fs::write(root.0.join("world-123-1").join(SAVE_FILE), b"bad").unwrap();
        let list = store.list().unwrap();
        assert_eq!(list.len(), 2);
        assert!(list
            .iter()
            .any(|world| world.health == WorldHealth::Corrupt));
        assert!(list
            .iter()
            .any(|world| world.health == WorldHealth::MissingSave));
        assert_eq!(
            fs::read(root.0.join("world-123-1").join(SAVE_FILE)).unwrap(),
            b"bad"
        );
    }

    #[test]
    fn decoder_rejects_truncated_and_trailing_payloads() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let saved = store
            .create(
                "Decode Test",
                1,
                StoredGameMode::Creative,
                test_world(),
                PlayerSnapshot::default(),
            )
            .unwrap();
        let path = store.save_path(&saved.metadata.id).unwrap();
        let bytes = fs::read(path).unwrap();
        assert!(decode_save(&bytes[..bytes.len() - 1], &saved.metadata.id).is_err());
        let mut trailing = bytes;
        trailing.push(0);
        assert!(decode_save(&trailing, &saved.metadata.id).is_err());
    }

    #[test]
    fn decoder_detects_save_corruption() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let saved = store
            .create(
                "Corruption Test",
                1,
                StoredGameMode::Creative,
                test_world(),
                PlayerSnapshot::default(),
            )
            .unwrap();
        let mut bytes = fs::read(store.save_path(&saved.metadata.id).unwrap()).unwrap();
        bytes[16] ^= 0xff;
        assert_eq!(
            decode_save(&bytes, &saved.metadata.id).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
