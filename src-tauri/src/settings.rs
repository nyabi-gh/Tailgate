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
    #[serde(default)]
    pub general: General,
    #[serde(default)]
    pub browse: Browse,
    #[serde(default)]
    pub downloads: DownloadSettings,
    #[serde(default)]
    pub cache: CacheSettings,
    #[serde(default)]
    pub updates: UpdateSettings,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct General {
    pub language: Language,
    /// The user has said they are 18 or older, which e621's terms require; asked on the first
    /// run only.
    pub adult: bool,
}

/// What the UI keeps between runs; it owns the limits of these values.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Browse {
    /// The grid's cell width in CSS pixels, once the user has changed it.
    pub cell_width: Option<u16>,
    /// Newest first.
    pub recent_searches: Vec<String>,
    /// The whole page's zoom in percent, once the user has changed it.
    pub page_zoom: Option<u16>,
}

/// `None` follows the app's default, so a later default reaches users who never chose.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DownloadSettings {
    pub folder: Option<PathBuf>,
    /// A file name rule such as `{id}_{artist}`; see `download::name`.
    pub name: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CacheSettings {
    /// The most the image cache keeps on disk; `None` follows the app's default.
    pub limit_mb: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateSettings {
    /// Look for a new version once a run, when the connection first opens.
    pub check_automatically: bool,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            check_automatically: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Language {
    #[default]
    System,
    En,
    Ko,
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
    /// `None` when the configuration was found in the credential store on a first run, with no
    /// settings file left to name it.
    pub file_name: Option<String>,
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
    #[error("the settings file could not be loaded, so it is left as it is")]
    NotLoaded,
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

    pub fn general(&self) -> General {
        self.section(|file| file.general.clone())
    }

    pub fn set_general(&self, general: General) -> Result<(), SettingsError> {
        self.change_section(|file| file.general = general)
    }

    pub fn browse(&self) -> Browse {
        self.section(|file| file.browse.clone())
    }

    pub fn set_browse(&self, browse: Browse) -> Result<(), SettingsError> {
        self.change_section(|file| file.browse = browse)
    }

    pub fn downloads(&self) -> DownloadSettings {
        self.section(|file| file.downloads.clone())
    }

    pub fn set_downloads(&self, downloads: DownloadSettings) -> Result<(), SettingsError> {
        self.change_section(|file| file.downloads = downloads)
    }

    pub fn cache(&self) -> CacheSettings {
        self.section(|file| file.cache.clone())
    }

    pub fn set_cache(&self, cache: CacheSettings) -> Result<(), SettingsError> {
        self.change_section(|file| file.cache = cache)
    }

    pub fn updates(&self) -> UpdateSettings {
        self.section(|file| file.updates.clone())
    }

    pub fn set_updates(&self, updates: UpdateSettings) -> Result<(), SettingsError> {
        self.change_section(|file| file.updates = updates)
    }

    /// The defaults while nothing is saved or the file cannot be loaded.
    fn section<T: Default>(&self, read: impl FnOnce(&SettingsFile) -> T) -> T {
        self.lock()
            .as_ref()
            .ok()
            .and_then(Option::as_ref)
            .map(read)
            .unwrap_or_default()
    }

    /// Unlike the connection, the other sections never replace a file that could not be
    /// loaded: the connection saved in it would be lost, and a damaged file keeps the
    /// connection blocked (docs/report.md §6.3, rule 1).
    fn change_section(&self, change: impl FnOnce(&mut SettingsFile)) -> Result<(), SettingsError> {
        let mut file = self.lock();
        let Ok(current) = &*file else {
            return Err(SettingsError::NotLoaded);
        };
        let mut next = current.clone().unwrap_or_default();
        change(&mut next);
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
    fn general_settings_are_saved_beside_the_connection() {
        let dir = TempDir::new();
        let path = dir.join("settings.json");
        let settings = Settings::load(path.clone());
        assert_eq!(settings.general(), General::default());
        settings
            .set_connection(ConnectionSettings::new(ConnectionMode::Mullvad))
            .unwrap();
        let general = General {
            language: Language::Ko,
            adult: true,
        };
        settings.set_general(general.clone()).unwrap();

        let loaded = Settings::load(path);
        assert_eq!(loaded.general(), general);
        assert_eq!(
            loaded.connection().unwrap().unwrap().mode,
            ConnectionMode::Mullvad
        );
    }

    #[test]
    fn each_section_keeps_the_others() {
        let dir = TempDir::new();
        let path = dir.join("settings.json");
        let settings = Settings::load(path.clone());
        let general = General {
            language: Language::En,
            adult: true,
        };
        let browse = Browse {
            cell_width: Some(240),
            recent_searches: vec!["wolf rating:s".into(), "fox".into()],
            page_zoom: Some(150),
        };
        let downloads = DownloadSettings {
            folder: Some(PathBuf::from("/tmp/elsewhere")),
            name: Some("{md5}".into()),
        };
        settings.set_general(general.clone()).unwrap();
        settings.set_browse(browse.clone()).unwrap();
        settings.set_downloads(downloads.clone()).unwrap();
        let cache = CacheSettings {
            limit_mb: Some(512),
        };
        settings.set_cache(cache.clone()).unwrap();
        let updates = UpdateSettings {
            check_automatically: false,
        };
        settings.set_updates(updates.clone()).unwrap();
        settings
            .set_connection(ConnectionSettings::new(ConnectionMode::Direct))
            .unwrap();

        let loaded = Settings::load(path);
        assert_eq!(loaded.general(), general);
        assert_eq!(loaded.browse(), browse);
        assert_eq!(loaded.downloads(), downloads);
        assert_eq!(loaded.cache(), cache);
        assert_eq!(loaded.updates(), updates);
        assert_eq!(
            loaded.connection().unwrap().unwrap().mode,
            ConnectionMode::Direct
        );
    }

    #[test]
    fn general_settings_leave_a_damaged_file_alone() {
        let dir = TempDir::new();
        let path = dir.join("settings.json");
        fs::write(&path, "{ not json").unwrap();
        let settings = Settings::load(path.clone());
        assert_eq!(settings.general(), General::default());

        let saved = settings.set_general(General {
            language: Language::En,
            adult: true,
        });
        assert!(matches!(saved, Err(SettingsError::NotLoaded)));
        let saved = settings.set_browse(Browse {
            cell_width: Some(200),
            recent_searches: Vec::new(),
            page_zoom: None,
        });
        assert!(matches!(saved, Err(SettingsError::NotLoaded)));
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");
        assert!(settings.connection().is_err());
    }

    #[test]
    fn missing_fields_take_their_defaults() {
        let dir = TempDir::new();
        let path = dir.join("settings.json");
        fs::write(&path, r#"{"connection":{"mode":"mullvad"}}"#).unwrap();
        let settings = Settings::load(path);
        let connection = settings.connection().unwrap().unwrap();
        assert_eq!(connection, ConnectionSettings::new(ConnectionMode::Mullvad));
        assert_eq!(settings.general(), General::default());
        assert_eq!(settings.browse(), Browse::default());
        assert!(settings.updates().check_automatically);
    }
}
