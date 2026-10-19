mod commands;
mod media;
pub mod network;
pub mod secrets;
pub mod tunnel;

use std::sync::Arc;

use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use network::Network;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::connection_status])
        .register_asynchronous_uri_scheme_protocol(media::SCHEME, |context, request, responder| {
            media::handle(context.app_handle(), request, responder)
        })
        .setup(|app| {
            secrets::init()?;
            let network = Arc::new(tauri::async_runtime::block_on(Network::start())?);
            build_main_window(app)?;
            app.manage(Arc::clone(&network));

            let handle = app.handle().clone();
            let mut changes = network.watch_connection();
            tauri::async_runtime::spawn(async move {
                while changes.changed().await.is_ok() {
                    let connection = changes.borrow_and_update().clone();
                    if let Err(error) = handle.emit("connection-changed", connection) {
                        tracing::warn!("could not notify the UI of a connection change: {error}");
                    }
                }
            });
            tauri::async_runtime::spawn(async move { network.connect_saved().await });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Tauri application");
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
