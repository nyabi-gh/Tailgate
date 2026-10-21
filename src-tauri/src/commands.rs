//! The surface the webview can call. Kept thin: validate, delegate, return.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::{
    account::{Account, AccountError, AccountState},
    api::{
        Api, ApiError,
        models::{Favorited, TagSuggestion, Voted},
    },
    download::{DownloadError, DownloadRequest, Downloads, Saved},
    network::{Connection, ConnectionDetails, Network, SettingsChangeError},
    relays::LocationChoice,
    search::{SearchError, SearchPage, Searches},
    settings::ConnectionMode,
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
