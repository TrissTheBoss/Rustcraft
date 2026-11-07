//! Versioned local preferences, stored separately from world and session data.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::world_store::{atomic_write, WorldStore};

const MAGIC: &[u8; 8] = b"VOXSET01";
const LEGACY_VERSION: u16 = 1;
const BINDINGS_VERSION: u16 = 2;
const FORMAT_VERSION: u16 = 3;
const LEGACY_ENCODED_LEN: usize = 19;
const BINDINGS_ENCODED_LEN: usize = 33;
const ENCODED_LEN: usize = 35;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum HoldToggle {
    #[default]
    Hold,
    Toggle,
}

impl HoldToggle {
    fn encoded(self) -> u8 {
        match self {
            Self::Hold => 0,
            Self::Toggle => 1,
        }
    }

    fn decode(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Hold),
            1 => Some(Self::Toggle),
            _ => None,
        }
    }

    pub fn active(self, held: bool, toggled: bool) -> bool {
        match self {
            Self::Hold => held,
            Self::Toggle => toggled,
        }
    }

    pub fn after_key_down(self, active: bool, initial_press: bool) -> bool {
        match self {
            Self::Hold => true,
            Self::Toggle if initial_press => !active,
            Self::Toggle => active,
        }
    }

    pub fn after_key_up(self, active: bool) -> bool {
        match self {
            Self::Hold => false,
            Self::Toggle => active,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingAction {
    Forward,
    Backward,
    Left,
    Right,
    Jump,
    Sneak,
    Sprint,
}

impl BindingAction {
    pub const ALL: [Self; 7] = [
        Self::Forward,
        Self::Backward,
        Self::Left,
        Self::Right,
        Self::Jump,
        Self::Sneak,
        Self::Sprint,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Forward => "Move Forward",
            Self::Backward => "Move Backward",
            Self::Left => "Strafe Left",
            Self::Right => "Strafe Right",
            Self::Jump => "Jump / Fly Up",
            Self::Sneak => "Sneak / Fly Down",
            Self::Sprint => "Sprint",
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Forward => 0,
            Self::Backward => 1,
            Self::Left => 2,
            Self::Right => 3,
            Self::Jump => 4,
            Self::Sneak => 5,
            Self::Sprint => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyBindings {
    keys: [u16; BindingAction::ALL.len()],
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            keys: [0x57, 0x53, 0x41, 0x44, 0x20, 0x11, 0x10],
        }
    }
}

impl KeyBindings {
    pub fn key(self, action: BindingAction) -> u16 {
        self.keys[action.index()]
    }

    pub fn conflicts(self, action: BindingAction, key: u16) -> Vec<BindingAction> {
        BindingAction::ALL
            .into_iter()
            .filter(|other| *other != action && self.key(*other) == key)
            .collect()
    }

    pub fn actions(self, key: u16) -> Vec<BindingAction> {
        BindingAction::ALL
            .into_iter()
            .filter(|action| self.key(*action) == key)
            .collect()
    }

    pub fn assign(
        &mut self,
        action: BindingAction,
        key: u16,
        allow_shared: bool,
    ) -> Result<(), KeyBindingError> {
        if !valid_bindable_key(key) {
            return Err(KeyBindingError::ReservedOrInvalid);
        }
        if !allow_shared && !self.conflicts(action, key).is_empty() {
            return Err(KeyBindingError::Conflict);
        }
        self.keys[action.index()] = key;
        Ok(())
    }

    fn validate(self) -> Result<(), KeyBindingError> {
        for action in BindingAction::ALL {
            if !valid_bindable_key(self.key(action)) {
                return Err(KeyBindingError::ReservedOrInvalid);
            }
        }
        Ok(())
    }
}

fn valid_bindable_key(key: u16) -> bool {
    let supported = (0x30..=0x39).contains(&key)
        || (0x41..=0x5a).contains(&key)
        || (0x70..=0x87).contains(&key)
        || matches!(key, 0x09 | 0x0d | 0x10 | 0x11 | 0x20 | 0x25..=0x28);
    supported
        && key != 0x1b
        && key != 0x45
        && key != 0x46
        && key != 0x51
        && !(0x31..=0x39).contains(&key)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyBindingError {
    ReservedOrInvalid,
    Conflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Settings {
    /// Vertical field of view in degrees. This range is a current engine
    /// capability, not a claim about the pinned reference defaults.
    pub field_of_view_degrees: u16,
    /// Mouse look multiplier in thousandths; 1000 preserves current behavior.
    pub mouse_sensitivity_permille: u16,
    pub invert_mouse_y: bool,
    pub key_bindings: KeyBindings,
    pub sprint_behavior: HoldToggle,
    pub sneak_behavior: HoldToggle,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            field_of_view_degrees: 60,
            mouse_sensitivity_permille: 1000,
            invert_mouse_y: false,
            key_bindings: KeyBindings::default(),
            sprint_behavior: HoldToggle::Hold,
            sneak_behavior: HoldToggle::Hold,
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
        self.key_bindings
            .validate()
            .map_err(|_| SettingsValidationError::InvalidKeyBindings)?;
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
        for (index, action) in BindingAction::ALL.into_iter().enumerate() {
            let offset = 15 + index * 2;
            bytes[offset..offset + 2].copy_from_slice(&self.key_bindings.key(action).to_le_bytes());
        }
        bytes[29] = self.sprint_behavior.encoded();
        bytes[30] = self.sneak_behavior.encoded();
        let digest = checksum(&bytes[..31]);
        bytes[31..35].copy_from_slice(&digest.to_le_bytes());
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> SettingsLoad {
        let fallback = || SettingsLoad {
            settings: Self::default(),
            warnings: vec![SettingsWarning::CorruptFile],
        };
        if bytes.get(..8) != Some(MAGIC.as_slice()) || bytes.len() < 10 {
            return fallback();
        }
        let version = u16::from_le_bytes([bytes[8], bytes[9]]);
        let (expected_len, checksum_offset) = match version {
            LEGACY_VERSION => (LEGACY_ENCODED_LEN, 15),
            BINDINGS_VERSION => (BINDINGS_ENCODED_LEN, 29),
            FORMAT_VERSION => (ENCODED_LEN, 31),
            _ => {
                return SettingsLoad {
                    settings: Self::default(),
                    warnings: vec![SettingsWarning::UnsupportedVersion(version)],
                }
            }
        };
        if bytes.len() != expected_len {
            return fallback();
        }
        let stored_checksum = u32::from_le_bytes(
            bytes[checksum_offset..checksum_offset + 4]
                .try_into()
                .unwrap(),
        );
        if checksum(&bytes[..checksum_offset]) != stored_checksum {
            return SettingsLoad {
                settings: Self::default(),
                warnings: vec![SettingsWarning::CorruptFile],
            };
        }

        let defaults = Self::default();
        let mut warnings = Vec::new();
        if version < FORMAT_VERSION {
            warnings.push(SettingsWarning::MigratedVersion(version));
        }
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
        let key_bindings = if version == LEGACY_VERSION {
            KeyBindings::default()
        } else {
            let mut bindings = defaults.key_bindings;
            let mut valid = true;
            for action in BindingAction::ALL {
                let offset = 15 + action.index() * 2;
                let key = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
                bindings.keys[action.index()] = key;
                valid &= valid_bindable_key(key);
            }
            if !valid {
                warnings.push(SettingsWarning::InvalidKeyBindings);
                bindings = defaults.key_bindings;
            }
            bindings
        };
        let sprint_behavior = if version < FORMAT_VERSION {
            HoldToggle::Hold
        } else if let Some(value) = HoldToggle::decode(bytes[29]) {
            value
        } else {
            warnings.push(SettingsWarning::InvalidSprintBehavior);
            HoldToggle::Hold
        };
        let sneak_behavior = if version < FORMAT_VERSION {
            HoldToggle::Hold
        } else if let Some(value) = HoldToggle::decode(bytes[30]) {
            value
        } else {
            warnings.push(SettingsWarning::InvalidSneakBehavior);
            HoldToggle::Hold
        };
        SettingsLoad {
            settings: Self {
                field_of_view_degrees,
                mouse_sensitivity_permille,
                invert_mouse_y,
                key_bindings,
                sprint_behavior,
                sneak_behavior,
            },
            warnings,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsValidationError {
    FieldOfViewOutOfRange,
    MouseSensitivityOutOfRange,
    InvalidKeyBindings,
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
    InvalidKeyBindings,
    InvalidSprintBehavior,
    InvalidSneakBehavior,
    MigratedVersion(u16),
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
            Self::InvalidKeyBindings => {
                formatter.write_str("invalid or conflicting key bindings; defaults loaded")
            }
            Self::InvalidSprintBehavior => {
                formatter.write_str("invalid sprint mode; hold mode loaded")
            }
            Self::InvalidSneakBehavior => {
                formatter.write_str("invalid sneak mode; hold mode loaded")
            }
            Self::MigratedVersion(version) => {
                write!(
                    formatter,
                    "settings version {version} migrated to current version"
                )
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
        let value = checksum(&bytes[..31]);
        bytes[31..35].copy_from_slice(&value.to_le_bytes());
    }

    fn legacy_v1_bytes(settings: Settings) -> [u8; LEGACY_ENCODED_LEN] {
        let mut bytes = [0; LEGACY_ENCODED_LEN];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..10].copy_from_slice(&LEGACY_VERSION.to_le_bytes());
        bytes[10..12].copy_from_slice(&settings.field_of_view_degrees.to_le_bytes());
        bytes[12..14].copy_from_slice(&settings.mouse_sensitivity_permille.to_le_bytes());
        bytes[14] = u8::from(settings.invert_mouse_y);
        let digest = checksum(&bytes[..15]);
        bytes[15..19].copy_from_slice(&digest.to_le_bytes());
        bytes
    }

    fn bindings_v2_bytes(settings: Settings) -> [u8; BINDINGS_ENCODED_LEN] {
        let mut bytes = [0; BINDINGS_ENCODED_LEN];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..10].copy_from_slice(&BINDINGS_VERSION.to_le_bytes());
        bytes[10..12].copy_from_slice(&settings.field_of_view_degrees.to_le_bytes());
        bytes[12..14].copy_from_slice(&settings.mouse_sensitivity_permille.to_le_bytes());
        bytes[14] = u8::from(settings.invert_mouse_y);
        for (index, action) in BindingAction::ALL.into_iter().enumerate() {
            let offset = 15 + index * 2;
            bytes[offset..offset + 2]
                .copy_from_slice(&settings.key_bindings.key(action).to_le_bytes());
        }
        let digest = checksum(&bytes[..29]);
        bytes[29..33].copy_from_slice(&digest.to_le_bytes());
        bytes
    }

    #[test]
    fn settings_round_trip_and_validate_ranges() {
        let settings = Settings {
            field_of_view_degrees: 90,
            mouse_sensitivity_permille: 1250,
            invert_mouse_y: true,
            ..Settings::default()
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
    fn version_one_settings_migrate_with_default_movement_bindings() {
        let old = Settings {
            field_of_view_degrees: 82,
            mouse_sensitivity_permille: 700,
            invert_mouse_y: true,
            ..Settings::default()
        };
        let loaded = Settings::decode(&legacy_v1_bytes(old));
        assert_eq!(loaded.settings.field_of_view_degrees, 82);
        assert_eq!(loaded.settings.mouse_sensitivity_permille, 700);
        assert!(loaded.settings.invert_mouse_y);
        assert_eq!(loaded.settings.key_bindings, KeyBindings::default());
        assert_eq!(loaded.warnings, vec![SettingsWarning::MigratedVersion(1)]);
    }

    #[test]
    fn version_two_settings_migrate_with_hold_behaviors() {
        let mut bindings = KeyBindings::default();
        bindings.assign(BindingAction::Sprint, 0x47, false).unwrap();
        let old = Settings {
            field_of_view_degrees: 82,
            mouse_sensitivity_permille: 700,
            invert_mouse_y: true,
            key_bindings: bindings,
            sprint_behavior: HoldToggle::Toggle,
            sneak_behavior: HoldToggle::Toggle,
        };
        let loaded = Settings::decode(&bindings_v2_bytes(old));
        assert_eq!(loaded.settings.field_of_view_degrees, 82);
        assert_eq!(loaded.settings.mouse_sensitivity_permille, 700);
        assert!(loaded.settings.invert_mouse_y);
        assert_eq!(loaded.settings.key_bindings, bindings);
        assert_eq!(loaded.settings.sprint_behavior, HoldToggle::Hold);
        assert_eq!(loaded.settings.sneak_behavior, HoldToggle::Hold);
        assert_eq!(loaded.warnings, vec![SettingsWarning::MigratedVersion(2)]);
    }

    #[test]
    fn hold_and_toggle_modes_apply_key_press_and_release_rules() {
        assert!(HoldToggle::Hold.after_key_down(false, true));
        assert!(!HoldToggle::Hold.after_key_up(true));
        assert!(HoldToggle::Hold.active(true, false));
        assert!(!HoldToggle::Toggle.after_key_down(false, false));
        assert!(HoldToggle::Toggle.after_key_down(false, true));
        assert!(HoldToggle::Toggle.after_key_down(true, false));
        assert!(HoldToggle::Toggle.after_key_up(true));
        assert!(HoldToggle::Toggle.active(false, true));
        assert!(!HoldToggle::Toggle.after_key_down(true, true));
    }

    #[test]
    fn invalid_toggle_modes_fall_back_individually() {
        let mut bytes = Settings {
            sprint_behavior: HoldToggle::Toggle,
            sneak_behavior: HoldToggle::Toggle,
            ..Settings::default()
        }
        .encode()
        .unwrap();
        bytes[29] = 9;
        refresh_checksum(&mut bytes);
        let loaded = Settings::decode(&bytes);
        assert_eq!(loaded.settings.sprint_behavior, HoldToggle::Hold);
        assert_eq!(loaded.settings.sneak_behavior, HoldToggle::Toggle);
        assert_eq!(
            loaded.warnings,
            vec![SettingsWarning::InvalidSprintBehavior]
        );
    }

    #[test]
    fn key_binding_conflicts_are_reported_and_shared_bindings_are_explicit() {
        let mut bindings = KeyBindings::default();
        assert_eq!(
            bindings.assign(BindingAction::Forward, 0x41, false),
            Err(KeyBindingError::Conflict)
        );
        assert_eq!(
            bindings.assign(BindingAction::Forward, 0x1b, true),
            Err(KeyBindingError::ReservedOrInvalid)
        );
        assert_eq!(
            bindings.assign(BindingAction::Forward, 0x46, true),
            Err(KeyBindingError::ReservedOrInvalid)
        );
        assert_eq!(
            bindings.assign(BindingAction::Forward, 0x51, true),
            Err(KeyBindingError::ReservedOrInvalid)
        );
        bindings.assign(BindingAction::Forward, 0x41, true).unwrap();
        let settings = Settings {
            key_bindings: bindings,
            ..Settings::default()
        };
        assert_eq!(
            Settings::decode(&settings.encode().unwrap()).settings,
            settings
        );
    }

    #[test]
    fn invalid_setting_fields_fall_back_individually() {
        let mut bytes = Settings::default().encode().unwrap();
        bytes[10..12].copy_from_slice(&u16::MAX.to_le_bytes());
        bytes[12..14].copy_from_slice(&1u16.to_le_bytes());
        bytes[14] = 2;
        bytes[15..17].copy_from_slice(&0x1bu16.to_le_bytes());
        refresh_checksum(&mut bytes);
        let loaded = Settings::decode(&bytes);
        assert_eq!(loaded.settings, Settings::default());
        assert_eq!(loaded.warnings.len(), 4);
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
        future[8..10].copy_from_slice(&4u16.to_le_bytes());
        assert_eq!(
            Settings::decode(&future).warnings,
            vec![SettingsWarning::UnsupportedVersion(4)]
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
            key_bindings: KeyBindings::default(),
            ..Settings::default()
        };
        store.save(saved).unwrap();
        assert_eq!(store.load().unwrap().settings, saved);
        fs::remove_dir_all(root).unwrap();
    }
}
