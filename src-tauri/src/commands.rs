//! The surface the webview can call. Kept thin: validate, delegate, return.

use std::sync::Arc;

use tauri::State;

use crate::{
    account::{Account, AccountError, AccountState},
    api::{Api, ApiError, models::TagSuggestion},
    network::{Connection, Network},
    search::{SearchError, SearchPage, Searches},
};

#[tauri::command]
pub fn connection_status(network: State<'_, Arc<Network>>) -> Connection {
    network.connection()
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
