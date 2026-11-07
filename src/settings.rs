//! Versioned local preferences, stored separately from world and session data.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::world_store::{atomic_write, WorldStore};

const MAGIC: &[u8; 8] = b"VOXSET01";
const FORMAT_VERSION: u16 = 1;
const ENCODED_LEN: usize = 19;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Settings {
    /// Vertical field of view in degrees. This range is a current engine
    /// capability, not a claim about the pinned reference defaults.
    pub field_of_view_degrees: u16,
    /// Mouse look multiplier in thousandths; 1000 preserves current behavior.
    pub mouse_sensitivity_permille: u16,
    pub invert_mouse_y: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            field_of_view_degrees: 60,
            mouse_sensitivity_permille: 1000,
            invert_mouse_y: false,
        }
    }
}

impl Settings {
    pub fn validate(self) -> Result<(), SettingsValidationError> {
        if !(30..=110).contains(&self.field_of_view_degrees) {
            return Err(SettingsValidationError::FieldOfViewOutOfRange);
        }
        if !(100..=2000).contains(&self.mouse_sensitivity_permille) {
            return Err(SettingsValidationError::MouseSensitivityOutOfRange);
        }
        Ok(())
    }

    pub fn encode(self) -> Result<[u8; ENCODED_LEN], SettingsValidationError> {
        self.validate()?;
        let mut bytes = [0; ENCODED_LEN];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..10].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        bytes[10..12].copy_from_slice(&self.field_of_view_degrees.to_le_bytes());
        bytes[12..14].copy_from_slice(&self.mouse_sensitivity_permille.to_le_bytes());
        bytes[14] = u8::from(self.invert_mouse_y);
        let digest = checksum(&bytes[..15]);
        bytes[15..19].copy_from_slice(&digest.to_le_bytes());
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> SettingsLoad {
        let fallback = || SettingsLoad {
            settings: Self::default(),
            warnings: vec![SettingsWarning::CorruptFile],
        };
        if bytes.len() != ENCODED_LEN || bytes.get(..8) != Some(MAGIC.as_slice()) {
            return fallback();
        }
        let version = u16::from_le_bytes([bytes[8], bytes[9]]);
        if version != FORMAT_VERSION {
            return SettingsLoad {
                settings: Self::default(),
                warnings: vec![SettingsWarning::UnsupportedVersion(version)],
            };
        }
        let stored_checksum = u32::from_le_bytes(bytes[15..19].try_into().unwrap());
        if checksum(&bytes[..15]) != stored_checksum {
            return fallback();
        }

        let defaults = Self::default();
        let mut warnings = Vec::new();
        let field_of_view_degrees = u16::from_le_bytes([bytes[10], bytes[11]]);
        let field_of_view_degrees = if (30..=110).contains(&field_of_view_degrees) {
            field_of_view_degrees
        } else {
            warnings.push(SettingsWarning::InvalidFieldOfView);
            defaults.field_of_view_degrees
        };
        let mouse_sensitivity_permille = u16::from_le_bytes([bytes[12], bytes[13]]);
        let mouse_sensitivity_permille = if (100..=2000).contains(&mouse_sensitivity_permille) {
            mouse_sensitivity_permille
        } else {
            warnings.push(SettingsWarning::InvalidMouseSensitivity);
            defaults.mouse_sensitivity_permille
        };
        let invert_mouse_y = match bytes[14] {
            0 => false,
            1 => true,
            _ => {
                warnings.push(SettingsWarning::InvalidInvertMouseY);
                defaults.invert_mouse_y
            }
        };
        SettingsLoad {
            settings: Self {
                field_of_view_degrees,
                mouse_sensitivity_permille,
                invert_mouse_y,
            },
            warnings,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsValidationError {
    FieldOfViewOutOfRange,
    MouseSensitivityOutOfRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsLoad {
    pub settings: Settings,
    pub warnings: Vec<SettingsWarning>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsWarning {
    CorruptFile,
    UnsupportedVersion(u16),
    InvalidFieldOfView,
    InvalidMouseSensitivity,
    InvalidInvertMouseY,
}

impl std::fmt::Display for SettingsWarning {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CorruptFile => formatter.write_str("settings file is corrupt; defaults loaded"),
            Self::UnsupportedVersion(version) => write!(
                formatter,
                "settings version {version} is unsupported; defaults loaded"
            ),
            Self::InvalidFieldOfView => {
                formatter.write_str("invalid field of view; default loaded")
            }
            Self::InvalidMouseSensitivity => {
                formatter.write_str("invalid mouse sensitivity; default loaded")
            }
            Self::InvalidInvertMouseY => {
                formatter.write_str("invalid mouse inversion value; default loaded")
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn default_path() -> io::Result<PathBuf> {
        let worlds = WorldStore::default_root()?;
        let app_data = worlds.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "world path has no parent")
        })?;
        Ok(app_data.join("settings.bin"))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> io::Result<SettingsLoad> {
        let metadata = match fs::symlink_metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(SettingsLoad {
                    settings: Settings::default(),
                    warnings: Vec::new(),
                });
            }
            Err(error) => return Err(error),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "settings path is not a regular file",
            ));
        }
        let mut bytes = Vec::with_capacity(ENCODED_LEN + 1);
        File::open(&self.path)?
            .take((ENCODED_LEN + 1) as u64)
            .read_to_end(&mut bytes)?;
        Ok(Settings::decode(&bytes))
    }

    pub fn save(&self, settings: Settings) -> io::Result<()> {
        let bytes = settings.encode().map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid settings: {error:?}"),
            )
        })?;
        atomic_write(&self.path, &bytes)
    }
}

fn checksum(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(0x0100_0193)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir() -> PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("voxel-engine-settings-{id}"))
    }

    fn refresh_checksum(bytes: &mut [u8; ENCODED_LEN]) {
        let value = checksum(&bytes[..15]);
        bytes[15..19].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn settings_round_trip_and_validate_ranges() {
        let settings = Settings {
            field_of_view_degrees: 90,
            mouse_sensitivity_permille: 1250,
            invert_mouse_y: true,
        };
        assert_eq!(
            Settings::decode(&settings.encode().unwrap()).settings,
            settings
        );
        assert_eq!(
            Settings {
                field_of_view_degrees: 29,
                ..Settings::default()
            }
            .validate(),
            Err(SettingsValidationError::FieldOfViewOutOfRange)
        );
        assert_eq!(
            Settings {
                mouse_sensitivity_permille: 2001,
                ..Settings::default()
            }
            .validate(),
            Err(SettingsValidationError::MouseSensitivityOutOfRange)
        );
    }

    #[test]
    fn invalid_setting_fields_fall_back_individually() {
        let mut bytes = Settings::default().encode().unwrap();
        bytes[10..12].copy_from_slice(&u16::MAX.to_le_bytes());
        bytes[12..14].copy_from_slice(&1u16.to_le_bytes());
        bytes[14] = 2;
        refresh_checksum(&mut bytes);
        let loaded = Settings::decode(&bytes);
        assert_eq!(loaded.settings, Settings::default());
        assert_eq!(loaded.warnings.len(), 3);
    }

    #[test]
    fn corrupt_and_future_files_fall_back_with_diagnostics() {
        assert_eq!(
            Settings::decode(b"bad").warnings,
            vec![SettingsWarning::CorruptFile]
        );
        let mut corrupted = Settings::default().encode().unwrap();
        corrupted[10] ^= 1;
        assert_eq!(
            Settings::decode(&corrupted).warnings,
            vec![SettingsWarning::CorruptFile]
        );
        let mut future = Settings::default().encode().unwrap();
        future[8..10].copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(
            Settings::decode(&future).warnings,
            vec![SettingsWarning::UnsupportedVersion(2)]
        );
    }

    #[test]
    fn store_loads_defaults_then_atomically_round_trips_settings() {
        let root = unique_temp_dir();
        let store = SettingsStore::new(root.join("settings.bin"));
        assert_eq!(store.load().unwrap().settings, Settings::default());
        let saved = Settings {
            field_of_view_degrees: 80,
            mouse_sensitivity_permille: 750,
            invert_mouse_y: true,
        };
        store.save(saved).unwrap();
        assert_eq!(store.load().unwrap().settings, saved);
        fs::remove_dir_all(root).unwrap();
    }
}
