//! The settings file: one JSON document in the app's config directory. Secrets never go here;
//! they live in the credential store (docs/report.md §7.3).

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

/// The Mullvad location used until the user picks one.
pub const DEFAULT_LOCATION: &str = "jp-tyo";

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsFile {
    #[serde(default)]
    pub connection: Option<ConnectionSettings>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSettings {
    pub mode: ConnectionMode,
    #[serde(default = "default_location")]
    pub location: String,
    /// What the imported `.conf` was, for the settings screen; the file itself is a secret.
    #[serde(default)]
    pub wireguard: Option<WireGuardSummary>,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionMode {
    Direct,
    WireGuard,
    Mullvad,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireGuardSummary {
    pub file_name: String,
    pub endpoint: String,
}

impl ConnectionSettings {
    pub fn new(mode: ConnectionMode) -> Self {
        Self {
            mode,
            location: default_location(),
            wireguard: None,
        }
    }
}

fn default_location() -> String {
    DEFAULT_LOCATION.to_owned()
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("could not read the settings file: {0}")]
    Read(io::Error),
    #[error("the settings file is damaged: {0}")]
    Damaged(serde_json::Error),
    #[error("could not write the settings file: {0}")]
    Write(io::Error),
}

pub struct Settings {
    path: PathBuf,
    /// `Err` holds why the file could not be loaded. It stays until something is saved, so a
    /// damaged file keeps the connection blocked instead of passing for "no settings".
    file: Mutex<Result<Option<SettingsFile>, String>>,
}

impl Settings {
    pub fn load(path: PathBuf) -> Self {
        let file = read(&path).map_err(|error| {
            tracing::error!("{error}");
            error.to_string()
        });
        Self {
            path,
            file: Mutex::new(file),
        }
    }

    /// `Ok(None)` when nothing has been chosen yet: no file, or no connection in it.
    pub fn connection(&self) -> Result<Option<ConnectionSettings>, String> {
        self.lock()
            .clone()
            .map(|file| file.and_then(|file| file.connection))
    }

    /// Saved before it takes effect, so what runs is always what the next launch reads. A file
    /// that could not be loaded is replaced: the user has just said what they want.
    pub fn set_connection(&self, connection: ConnectionSettings) -> Result<(), SettingsError> {
        let mut file = self.lock();
        let mut next = file.clone().ok().flatten().unwrap_or_default();
        next.connection = Some(connection);
        write(&self.path, &next).map_err(SettingsError::Write)?;
        *file = Ok(Some(next));
        Ok(())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Result<Option<SettingsFile>, String>> {
        self.file.lock().expect("settings lock poisoned")
    }
}

fn read(path: &Path) -> Result<Option<SettingsFile>, SettingsError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(SettingsError::Read(error)),
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(SettingsError::Damaged)
}

/// Through a temporary file and a rename, so a crash mid-write cannot leave half a file.
fn write(path: &Path, file: &SettingsFile) -> io::Result<()> {
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)?;
    }
    let temporary = path.with_extension("json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(file).expect("settings serialize"),
    )?;
    fs::rename(&temporary, path)
}

#[cfg(test)]
pub(crate) mod testing {
    use std::path::PathBuf;

    /// A fresh directory under the system temp directory, removed when dropped.
    pub(crate) struct TempDir(pub PathBuf);

    impl TempDir {
        pub(crate) fn new() -> Self {
            let mut name = [0u8; 8];
            getrandom::fill(&mut name).unwrap();
            let path =
                std::env::temp_dir().join(format!("tailgate-test-{}", u64::from_le_bytes(name)));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        pub(crate) fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{testing::TempDir, *};

    #[test]
    fn a_missing_file_means_nothing_chosen() {
        let dir = TempDir::new();
        let settings = Settings::load(dir.join("settings.json"));
        assert_eq!(settings.connection(), Ok(None));
    }

    #[test]
    fn saved_settings_load_again() {
        let dir = TempDir::new();
        let path = dir.join("nested/settings.json");
        let mut connection = ConnectionSettings::new(ConnectionMode::Mullvad);
        connection.location = "se-sto".into();
        Settings::load(path.clone())
            .set_connection(connection.clone())
            .unwrap();
        assert_eq!(Settings::load(path).connection(), Ok(Some(connection)));
    }

    #[test]
    fn a_damaged_file_is_an_error_until_replaced() {
        let dir = TempDir::new();
        let path = dir.join("settings.json");
        fs::write(&path, "{ not json").unwrap();
        let settings = Settings::load(path.clone());
        assert!(settings.connection().is_err());

        settings
            .set_connection(ConnectionSettings::new(ConnectionMode::Direct))
            .unwrap();
        assert_eq!(
            Settings::load(path).connection().unwrap().unwrap().mode,
            ConnectionMode::Direct
        );
    }

    #[test]
    fn missing_fields_take_their_defaults() {
        let dir = TempDir::new();
        let path = dir.join("settings.json");
        fs::write(&path, r#"{"connection":{"mode":"mullvad"}}"#).unwrap();
        let connection = Settings::load(path).connection().unwrap().unwrap();
        assert_eq!(connection, ConnectionSettings::new(ConnectionMode::Mullvad));
    }
}
