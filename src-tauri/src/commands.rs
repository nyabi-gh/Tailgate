//! The surface the webview can call. Kept thin: validate, delegate, return.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, ipc::Channel, path::BaseDirectory};

use crate::{
    account::{Account, AccountError, AccountState},
    api::{
        Api, ApiError,
        models::{Favorited, TagSuggestion, Voted},
    },
    download::{
        DownloadDetails, DownloadError, DownloadRequest, Downloads, NameRuleError, Saved,
        TemplateError, example_name,
    },
    media::{self, CacheUsage, ClearError, Media},
    network::{Connection, ConnectionDetails, Network, SettingsChangeError},
    relays::LocationChoice,
    search::{SearchError, SearchPage, Searches},
    settings::{Browse, CacheSettings, ConnectionMode, General, Settings, UpdateSettings},
    update::{Available, Progress, UpdateError, Updates},
};

#[tauri::command]
pub fn connection_status(network: State<'_, Arc<Network>>) -> Connection {
    network.connection()
}

#[tauri::command]
pub fn connection_details(network: State<'_, Arc<Network>>) -> ConnectionDetails {
    network.details()
}

#[tauri::command]
pub fn reconnect(network: State<'_, Arc<Network>>) {
    network.reconnect();
}

#[tauri::command]
pub fn set_connection_mode(
    network: State<'_, Arc<Network>>,
    mode: ConnectionMode,
) -> Result<(), SettingsChangeError> {
    network.set_mode(mode)
}

/// Resolves to `false` when the user cancels the file dialog.
#[tauri::command]
pub async fn import_wireguard_config(
    app: AppHandle,
    network: State<'_, Arc<Network>>,
) -> Result<bool, SettingsChangeError> {
    use tauri_plugin_dialog::DialogExt;
    let (picked, chosen) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("WireGuard", &["conf"])
        .pick_file(move |path| {
            // The receiver only goes away if the command was dropped; nobody waits then.
            let _ = picked.send(path);
        });
    let Some(path) = chosen.await.ok().flatten() else {
        return Ok(false);
    };
    let path = path.into_path().map_err(|error| {
        tracing::warn!("the picked file has no path: {error}");
        SettingsChangeError::UnreadableFile
    })?;
    network.import_wireguard(path).await?;
    Ok(true)
}

#[tauri::command]
pub async fn mullvad_sign_in(
    network: State<'_, Arc<Network>>,
    account_number: String,
) -> Result<(), SettingsChangeError> {
    network.mullvad_sign_in(&account_number).await
}

#[tauri::command]
pub async fn mullvad_sign_out(network: State<'_, Arc<Network>>) -> Result<(), SettingsChangeError> {
    network.mullvad_sign_out().await
}

#[tauri::command]
pub async fn mullvad_expiry(
    network: State<'_, Arc<Network>>,
) -> Result<Option<String>, SettingsChangeError> {
    network.mullvad_expiry().await
}

#[tauri::command]
pub async fn mullvad_locations(
    network: State<'_, Arc<Network>>,
) -> Result<Vec<LocationChoice>, SettingsChangeError> {
    network.locations().await
}

#[tauri::command]
pub async fn set_mullvad_location(
    network: State<'_, Arc<Network>>,
    code: String,
) -> Result<(), SettingsChangeError> {
    network.set_location(&code).await
}

#[tauri::command]
pub fn account_status(account: State<'_, Arc<Account>>) -> AccountState {
    account.state()
}

#[tauri::command]
pub async fn sign_in(
    account: State<'_, Arc<Account>>,
    username: String,
    api_key: String,
) -> Result<(), AccountError> {
    account.sign_in(&username, &api_key).await
}

#[tauri::command]
pub async fn sign_out(account: State<'_, Arc<Account>>) -> Result<(), AccountError> {
    account.sign_out().await
}

#[tauri::command]
pub fn search_start(searches: State<'_, Arc<Searches>>, tags: String) -> u64 {
    searches.start(&tags)
}

#[tauri::command]
pub async fn search_next(
    searches: State<'_, Arc<Searches>>,
    api: State<'_, Arc<Api>>,
    id: u64,
) -> Result<SearchPage, SearchError> {
    searches.next(&api, id).await
}

#[tauri::command]
pub fn search_close(searches: State<'_, Arc<Searches>>, id: u64) {
    searches.close(id);
}

#[tauri::command]
pub async fn autocomplete(
    api: State<'_, Arc<Api>>,
    text: String,
) -> Result<Vec<TagSuggestion>, ApiError> {
    api.autocomplete(&text).await
}

#[tauri::command]
pub fn search_start_favorites(searches: State<'_, Arc<Searches>>) -> u64 {
    searches.start_favorites()
}

#[tauri::command]
pub async fn favorite(
    api: State<'_, Arc<Api>>,
    id: u64,
    favorited: bool,
) -> Result<Favorited, ApiError> {
    if favorited {
        api.favorite(id).await
    } else {
        api.unfavorite(id).await
    }
}

#[tauri::command]
pub async fn vote(api: State<'_, Arc<Api>>, id: u64, up: bool) -> Result<Voted, ApiError> {
    api.vote(id, up).await
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    id: u64,
    received: u64,
    total: u64,
}

#[tauri::command]
pub async fn download(
    app: AppHandle,
    downloads: State<'_, Arc<Downloads>>,
    request: DownloadRequest,
) -> Result<Saved, DownloadError> {
    let id = request.id;
    downloads
        .save(&request, |received, total| {
            let progress = DownloadProgress {
                id,
                received,
                total,
            };
            if let Err(error) = app.emit("download-progress", progress) {
                tracing::warn!("could not send download progress to the UI: {error}");
            }
        })
        .await
}

#[tauri::command]
pub fn download_settings(downloads: State<'_, Arc<Downloads>>) -> DownloadDetails {
    downloads.details()
}

/// Resolves to `false` when the user cancels the folder dialog.
#[tauri::command]
pub async fn choose_download_folder(
    app: AppHandle,
    downloads: State<'_, Arc<Downloads>>,
) -> Result<bool, SettingsChangeError> {
    use tauri_plugin_dialog::DialogExt;
    let (picked, chosen) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_directory(downloads.folder())
        .pick_folder(move |path| {
            // The receiver only goes away if the command was dropped; nobody waits then.
            let _ = picked.send(path);
        });
    let Some(path) = chosen.await.ok().flatten() else {
        return Ok(false);
    };
    let path = path.into_path().map_err(|error| {
        tracing::warn!("the picked folder has no path: {error}");
        SettingsChangeError::UnreadableFile
    })?;
    downloads.set_folder(path)?;
    Ok(true)
}

/// Created first: the default folder only exists once something has been saved to it.
#[tauri::command]
pub async fn open_download_folder(
    app: AppHandle,
    downloads: State<'_, Arc<Downloads>>,
) -> Result<(), DownloadError> {
    use tauri_plugin_opener::OpenerExt;
    let folder = downloads.folder();
    tokio::fs::create_dir_all(&folder).await.map_err(|error| {
        tracing::warn!("could not create {}: {error}", folder.display());
        DownloadError::Storage
    })?;
    app.opener()
        .open_path(folder.to_string_lossy(), None::<&str>)
        .map_err(|error| {
            tracing::warn!("could not open {}: {error}", folder.display());
            DownloadError::Storage
        })
}

#[tauri::command]
pub fn download_name_example(name: String) -> Result<String, TemplateError> {
    example_name(&name)
}

#[tauri::command]
pub fn set_download_name(
    downloads: State<'_, Arc<Downloads>>,
    name: String,
) -> Result<(), NameRuleError> {
    downloads.set_name_rule(&name)
}

#[tauri::command]
pub fn general_settings(settings: State<'_, Arc<Settings>>) -> General {
    settings.general()
}

#[tauri::command]
pub fn set_general_settings(
    settings: State<'_, Arc<Settings>>,
    general: General,
) -> Result<(), SettingsChangeError> {
    settings.set_general(general).map_err(|error| {
        tracing::error!("{error}");
        SettingsChangeError::Settings
    })
}

#[tauri::command]
pub fn browse_settings(settings: State<'_, Arc<Settings>>) -> Browse {
    settings.browse()
}

#[tauri::command]
pub fn set_browse_settings(
    settings: State<'_, Arc<Settings>>,
    browse: Browse,
) -> Result<(), SettingsChangeError> {
    settings.set_browse(browse).map_err(|error| {
        tracing::error!("{error}");
        SettingsChangeError::Settings
    })
}

/// Takes the app rather than borrowed state: an async command that borrows must return a
/// `Result`, and this one cannot fail.
#[tauri::command]
pub async fn cache_status(app: AppHandle) -> CacheUsage {
    let media = Arc::clone(app.state::<Arc<Media>>().inner());
    media.disk().status().await
}

#[tauri::command]
pub async fn set_cache_limit(
    settings: State<'_, Arc<Settings>>,
    media: State<'_, Arc<Media>>,
    megabytes: u32,
) -> Result<CacheUsage, SettingsChangeError> {
    settings
        .set_cache(CacheSettings {
            limit_mb: Some(megabytes),
        })
        .map_err(|error| {
            tracing::error!("{error}");
            SettingsChangeError::Settings
        })?;
    media.disk().set_limit(media::megabytes(megabytes)).await;
    Ok(media.disk().status().await)
}

#[tauri::command]
pub async fn clear_cache(media: State<'_, Arc<Media>>) -> Result<CacheUsage, ClearError> {
    media.disk().clear().await?;
    Ok(media.disk().status().await)
}

#[tauri::command]
pub fn update_settings(settings: State<'_, Arc<Settings>>) -> UpdateSettings {
    settings.updates()
}

#[tauri::command]
pub fn set_update_settings(
    settings: State<'_, Arc<Settings>>,
    updates: UpdateSettings,
) -> Result<(), SettingsChangeError> {
    settings.set_updates(updates).map_err(|error| {
        tracing::error!("{error}");
        SettingsChangeError::Settings
    })
}

#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    network: State<'_, Arc<Network>>,
    updates: State<'_, Arc<Updates>>,
) -> Result<Option<Available>, UpdateError> {
    updates.check(&app, &network.proxy_url()).await
}

#[tauri::command]
pub async fn install_update(
    updates: State<'_, Arc<Updates>>,
    progress: Channel<Progress>,
) -> Result<(), UpdateError> {
    updates
        .install(|step| {
            if let Err(error) = progress.send(step) {
                tracing::warn!("could not send update progress to the UI: {error}");
            }
        })
        .await
}

/// Into the version an update has just installed.
#[tauri::command]
pub fn restart(app: AppHandle) {
    app.restart();
}

#[derive(Debug, Serialize, thiserror::Error)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum OpenNoticesError {
    #[error("the third-party notices could not be opened")]
    Unavailable,
}

/// In the default browser, from the copy shipped in the app's resources.
#[tauri::command]
pub fn open_notices(app: AppHandle) -> Result<(), OpenNoticesError> {
    use tauri_plugin_opener::OpenerExt;
    let path = app
        .path()
        .resolve("THIRD-PARTY-NOTICES.html", BaseDirectory::Resource)
        .map_err(|error| {
            tracing::warn!("could not find the third-party notices: {error}");
            OpenNoticesError::Unavailable
        })?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|error| {
            tracing::warn!("could not open {}: {error}", path.display());
            OpenNoticesError::Unavailable
        })
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    app.exit(0);
}
