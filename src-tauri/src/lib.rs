pub mod account;
pub mod api;
mod commands;
mod media;
pub mod network;
pub mod secrets;
pub mod tunnel;

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::watch;

use account::Account;
use api::Api;
use network::{Network, UserAgent};

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::connection_status,
            commands::account_status,
            commands::sign_in,
            commands::sign_out
        ])
        .register_asynchronous_uri_scheme_protocol(media::SCHEME, |context, request, responder| {
            media::handle(context.app_handle(), request, responder)
        })
        .setup(|app| {
            secrets::init()?;
            let network = Arc::new(tauri::async_runtime::block_on(Network::start())?);
            let user_agent = UserAgent::new();
            let api = Arc::new(Api::new(network.http().clone(), user_agent.clone()));
            let account = Arc::new(Account::new(Arc::clone(&api)));
            app.manage(Arc::clone(&network));
            app.manage(Arc::clone(&account));
            app.manage(Arc::new(media::Media::new(
                network.http().clone(),
                user_agent,
            )));
            build_main_window(app)?;

            forward(
                app.handle(),
                "connection-changed",
                network.watch_connection(),
            );
            forward(app.handle(), "account-changed", account.watch());
            let connection = network.watch_connection();
            tauri::async_runtime::spawn(async move { account.run(connection).await });
            tauri::async_runtime::spawn(async move { network.connect_saved().await });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Tauri application");
}

/// Sends every new value of `changes` to the UI as `event`.
fn forward<T: Clone + serde::Serialize + Send + Sync + 'static>(
    app: &AppHandle,
    event: &'static str,
    mut changes: watch::Receiver<T>,
) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        while changes.changed().await.is_ok() {
            let value = changes.borrow_and_update().clone();
            if let Err(error) = app.emit(event, value) {
                tracing::warn!("could not send {event} to the UI: {error}");
            }
        }
    });
}

/// Built in code rather than tauri.conf.json so platform-specific window options live in one place.
fn build_main_window(app: &tauri::App) -> tauri::Result<()> {
    let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
        .title("Tailgate")
        .inner_size(1280.0, 820.0)
        .min_inner_size(720.0, 480.0);

    #[cfg(target_os = "macos")]
    let builder = {
        use tauri::{
            LogicalPosition, TitleBarStyle,
            utils::config::WindowEffectsConfig,
            window::{Effect, EffectState},
        };
        builder
            .title_bar_style(TitleBarStyle::Overlay)
            .hidden_title(true)
            .traffic_light_position(LogicalPosition::new(20.0, 33.0))
            .transparent(true)
            .effects(WindowEffectsConfig {
                effects: vec![Effect::Sidebar],
                state: Some(EffectState::FollowsWindowActiveState),
                ..Default::default()
            })
    };

    let window = builder.build()?;
    #[cfg(debug_assertions)]
    if std::env::var_os("TAILGATE_DEVTOOLS").is_some() {
        window.open_devtools();
    }
    Ok(())
}
