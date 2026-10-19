//! The surface the webview can call. Kept thin: validate, delegate, return.

use std::sync::Arc;

use tauri::State;

use crate::{
    account::{Account, AccountError, AccountState},
    network::{Connection, Network},
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
