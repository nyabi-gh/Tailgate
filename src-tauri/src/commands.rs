//! The surface the webview can call. Kept thin: validate, delegate, return.

use std::sync::Arc;

use tauri::State;

use crate::network::{Connection, Network};

#[tauri::command]
pub fn connection_status(network: State<'_, Arc<Network>>) -> Connection {
    network.connection()
}
