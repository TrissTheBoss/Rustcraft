//! Versioned local world storage with bounded decoding and recoverable writes.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::world::{BlockId, World, CHUNK_VOLUME, MAX_SERIALIZED_CHUNKS};

const SAVE_MAGIC: &[u8; 8] = b"VXSAVE01";
const SAVE_VERSION: u32 = 1;
const SAVE_FILE: &str = "world.vxs";
const BACKUP_DIR: &str = "backups";
const MAX_NAME_BYTES: usize = 128;
const MAX_BACKUPS: usize = 5;
const MAX_SAVE_BYTES: usize =
    4096 + MAX_SERIALIZED_CHUNKS * (12 + CHUNK_VOLUME * std::mem::size_of::<u16>());
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorldMetadata {
    pub id: String,
    pub name: String,
    pub seed: u64,
    pub created_at: u64,
    pub updated_at: u64,
    pub game_mode: StoredGameMode,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerSnapshot {
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub flying: bool,
    pub grounded: bool,
    pub selected_block: BlockId,
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
                        created_at: now,
                        updated_at: now,
                        game_mode,
                    };
                    let mut saved = StoredWorld {
                        metadata,
                        world,
                        player,
                    };
                    if let Err(error) =
                        self.write_snapshot(&mut saved.metadata, &saved.world, saved.player, false)
                    {
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
        let path = self.save_path(id)?;
        let bytes = read_bounded(&path)?;
        decode_save(&bytes, id)
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
        let directory = self.world_dir(id)?.join(BACKUP_DIR);
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let mut backups = entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
            .filter(|name| valid_backup_name(name))
            .collect::<Vec<_>>();
        backups.sort();
        Ok(backups)
    }

    pub fn restore_backup(&self, id: &str, backup_name: &str) -> io::Result<StoredWorld> {
        let backup_path = self
            .world_dir(id)?
            .join(BACKUP_DIR)
            .join(validate_backup_name(backup_name)?);
        let bytes = read_bounded(&backup_path)?;
        let restored = decode_save(&bytes, id)?;
        let current_path = self.save_path(id)?;
        self.backup_current(id, &current_path)?;
        atomic_write(&current_path, &bytes)?;
        Ok(restored)
    }

    fn create_with_id(
        &self,
        id: &str,
        name: String,
        seed: u64,
        game_mode: StoredGameMode,
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
                created_at: now,
                updated_at: now,
                game_mode,
            },
            world,
            player,
        };
        self.write_snapshot(&mut saved.metadata, &saved.world, saved.player, false)?;
        Ok(saved)
    }

    fn write_snapshot(
        &self,
        metadata: &mut WorldMetadata,
        world: &World,
        player: PlayerSnapshot,
        backup: bool,
    ) -> io::Result<()> {
        validate_player(player)?;
        let path = self.save_path(&metadata.id)?;
        let mut world_bytes = Vec::new();
        world.save(&mut world_bytes)?;
        let updated_at = unix_seconds()?;
        let mut next_metadata = metadata.clone();
        next_metadata.updated_at = updated_at;
        let bytes = encode_save(&next_metadata, player, &world_bytes)?;
        fs::create_dir_all(path.parent().expect("world save path has parent"))?;
        if backup && path.exists() {
            self.backup_current(&next_metadata.id, &path)
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

    fn backup_current(&self, id: &str, save_path: &Path) -> io::Result<()> {
        if !save_path.exists() {
            return Ok(());
        }
        let metadata = fs::metadata(save_path)?;
        if metadata.len() > MAX_SAVE_BYTES as u64 {
            return Err(invalid_data("existing save exceeds the backup size limit"));
        }
        let directory = self.world_dir(id)?.join(BACKUP_DIR);
        fs::create_dir_all(&directory)?;
        let counter = NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let name = format!("backup-{}-{counter}.sav", unix_seconds()?);
        let backup = directory.join(name);
        atomic_copy(save_path, &backup)?;
        prune_backups(&directory, MAX_BACKUPS)
    }

    fn world_dir(&self, id: &str) -> io::Result<PathBuf> {
        validate_id(id)?;
        Ok(self.root.join(id))
    }

    fn save_path(&self, id: &str) -> io::Result<PathBuf> {
        Ok(self.world_dir(id)?.join(SAVE_FILE))
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

fn validate_player(player: PlayerSnapshot) -> io::Result<()> {
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
    validate_player(player)?;
    if world_bytes.len() > MAX_SAVE_BYTES {
        return Err(invalid_input("world save exceeds the current size limit"));
    }
    let name = metadata.name.as_bytes();
    let name_len =
        u16::try_from(name.len()).map_err(|_| invalid_input("world name is too long"))?;
    let world_len = u64::try_from(world_bytes.len())
        .map_err(|_| invalid_input("world payload length is invalid"))?;
    let mut output = Vec::with_capacity(world_bytes.len() + 96 + name.len());
    output.write_all(SAVE_MAGIC)?;
    output.write_all(&SAVE_VERSION.to_le_bytes())?;
    output.write_all(&name_len.to_le_bytes())?;
    output.write_all(name)?;
    output.write_all(&metadata.seed.to_le_bytes())?;
    output.write_all(&metadata.created_at.to_le_bytes())?;
    output.write_all(&metadata.updated_at.to_le_bytes())?;
    output.write_all(&[metadata.game_mode as u8])?;
    for value in player.position.into_iter().chain(player.velocity) {
        output.write_all(&value.to_le_bytes())?;
    }
    output.write_all(&player.yaw.to_le_bytes())?;
    output.write_all(&player.pitch.to_le_bytes())?;
    output.write_all(&[u8::from(player.flying), u8::from(player.grounded)])?;
    output.write_all(&player.selected_block.0.to_le_bytes())?;
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
    let metadata = read_metadata(&mut input, id)?;
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
    validate_player(player).map_err(|_| invalid_data("invalid player snapshot values"))?;
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

fn read_metadata<R: Read>(input: &mut R, id: &str) -> io::Result<WorldMetadata> {
    let mut magic = [0; 8];
    input.read_exact(&mut magic)?;
    if &magic != SAVE_MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unsupported world save signature",
        ));
    }
    let version = read_u32(input)?;
    if version != SAVE_VERSION {
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
    let created_at = read_u64(input)?;
    let updated_at = read_u64(input)?;
    let mut game_mode = [0];
    input.read_exact(&mut game_mode)?;
    Ok(WorldMetadata {
        id: id.to_owned(),
        name,
        seed,
        created_at,
        updated_at,
        game_mode: StoredGameMode::try_from(game_mode[0])?,
    })
}

fn read_bounded(path: &Path) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    let length = file.metadata()?.len();
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

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
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

fn prune_backups(directory: &Path, keep: usize) -> io::Result<()> {
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
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let remove_count = entries.len().saturating_sub(keep);
    for (_, path) in entries.into_iter().take(remove_count) {
        fs::remove_file(path)?;
    }
    Ok(())
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

    #[test]
    fn create_save_load_round_trips_world_and_player_state() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let player = PlayerSnapshot {
            position: [-2.5, 9.25, 33.75],
            velocity: [0.1, -0.2, 0.3],
            yaw: 1.25,
            pitch: -0.5,
            flying: true,
            grounded: false,
            selected_block: BlockId(3),
        };
        let mut saved = store
            .create(
                "My World",
                42,
                StoredGameMode::Creative,
                test_world(),
                player,
            )
            .unwrap();
        saved.world.set_block([-1, 3, 32], BlockId(2));
        store
            .save(&mut saved.metadata, &saved.world, saved.player)
            .unwrap();

        let loaded = store.load(&saved.metadata.id).unwrap();
        assert_eq!(loaded.metadata.name, "My World");
        assert_eq!(loaded.metadata.seed, 42);
        assert_eq!(loaded.metadata.game_mode, StoredGameMode::Creative);
        assert_eq!(loaded.player, player);
        assert_eq!(loaded.world.block_at([-1, 3, 32]), Some(BlockId(2)));
        assert_eq!(store.list().unwrap()[0].health, WorldHealth::Ready);
    }

    #[test]
    fn prototype_open_resumes_existing_world_instead_of_regenerating_it() {
        let root = TestRoot::new();
        let store = WorldStore::new(&root.0);
        let mut first = store.open_or_create_prototype(test_world()).unwrap();
        first.world.set_block([-1, 2, 32], BlockId(3));
        store
            .save(&mut first.metadata, &first.world, first.player)
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
            .save(&mut saved.metadata, &saved.world, saved.player)
            .unwrap();
        let backups = store.list_backups(&saved.metadata.id).unwrap();
        assert_eq!(backups.len(), 1);

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
