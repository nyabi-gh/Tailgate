pub mod account;
pub mod api;
mod blacklist;
mod commands;
mod download;
mod media;
pub mod network;
mod relays;
mod search;
pub mod secrets;
pub mod settings;
pub mod tunnel;
mod update;

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tokio::sync::watch;

use account::Account;
use api::Api;
use network::{Network, UserAgent};
use settings::Settings;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    tauri::Builder::default()
        // First, so a second launch exits before it opens a tunnel: two tunnels with the same
        // WireGuard key cut each other off at the server (docs/report.md §6.6).
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            focus_main_window(app)
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::connection_status,
            commands::connection_details,
            commands::reconnect,
            commands::set_connection_mode,
            commands::import_wireguard_config,
            commands::mullvad_sign_in,
            commands::mullvad_sign_out,
            commands::mullvad_expiry,
            commands::mullvad_locations,
            commands::set_mullvad_location,
            commands::account_status,
            commands::sign_in,
            commands::sign_out,
            commands::search_start,
            commands::search_next,
            commands::search_close,
            commands::autocomplete,
            commands::search_start_favorites,
            commands::favorite,
            commands::vote,
            commands::download,
            commands::general_settings,
            commands::set_general_settings,
            commands::browse_settings,
            commands::set_browse_settings,
            commands::download_settings,
            commands::choose_download_folder,
            commands::open_download_folder,
            commands::download_name_example,
            commands::set_download_name,
            commands::cache_status,
            commands::set_cache_limit,
            commands::clear_cache,
            commands::update_settings,
            commands::set_update_settings,
            commands::check_for_update,
            commands::install_update,
            commands::restart,
            commands::open_notices,
            commands::quit
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::Focused(true) = event {
                refresh_blacklist(window.app_handle());
            }
        })
        .register_asynchronous_uri_scheme_protocol(media::SCHEME, |context, request, responder| {
            media::handle(context.app_handle(), request, responder)
        })
        .setup(|app| {
            secrets::init()?;
            let settings = Arc::new(Settings::load(
                app.path().app_config_dir()?.join("settings.json"),
            ));
            let network = Arc::new(tauri::async_runtime::block_on(Network::start(
                Arc::clone(&settings),
                app.path().app_cache_dir()?.join("relays.json"),
            ))?);
            app.manage(Arc::clone(&settings));
            let user_agent = UserAgent::new();
            let api = Arc::new(Api::new(network.http().clone(), user_agent.clone()));
            let account = Arc::new(Account::new(Arc::clone(&api)));
            app.manage(Arc::clone(&network));
            app.manage(Arc::clone(&account));
            app.manage(Arc::clone(&api));
            app.manage(Arc::new(search::Searches::default()));
            app.manage(Arc::new(update::Updates::default()));
            app.manage(Arc::new(download::Downloads::new(
                network.http().clone(),
                user_agent.clone(),
                Arc::clone(&settings),
                app.path().download_dir()?.join("Tailgate"),
            )));
            let disk = Arc::new(media::DiskCache::new(
                app.path().app_cache_dir()?.join("media"),
                media::megabytes(
                    settings
                        .cache()
                        .limit_mb
                        .unwrap_or(media::DEFAULT_DISK_LIMIT_MB),
                ),
            ));
            // Earlier runs may have kept many files; counting them should not hold up the window.
            let scanning = Arc::clone(&disk);
            tauri::async_runtime::spawn(async move { scanning.scan().await });
            app.manage(Arc::new(media::Media::new(
                network.http().clone(),
                user_agent,
                disk,
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
            tauri::async_runtime::spawn(network.run());
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

/// The user may have changed the blacklist on the site while the window was in the background.
fn refresh_blacklist(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let searches = Arc::clone(app.state::<Arc<search::Searches>>().inner());
        let api = Arc::clone(app.state::<Arc<Api>>().inner());
        match searches.refresh_blacklist(&api).await {
            Ok(None) => {}
            Ok(Some(change)) => {
                tracing::info!("the blacklist changed on e621");
                if let Err(error) = app.emit("blacklist-changed", change) {
                    tracing::warn!("could not send blacklist-changed to the UI: {error}");
                }
            }
            Err(error) => tracing::warn!("could not refresh the blacklist: {error}"),
        }
    });
}

fn focus_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let shown = window
        .unminimize()
        .and_then(|()| window.show())
        .and_then(|()| window.set_focus());
    if let Err(error) = shown {
        tracing::warn!("could not bring the window forward: {error}");
    }
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
