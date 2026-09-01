mod ad_filter;
mod commands;
mod content;
mod db;
mod error;
mod media;
mod models;
mod network;
mod opml;
mod release;

use std::{path::PathBuf, sync::Arc};

use db::Database;
use network::Network;
use tauri::Manager;
use tokio::time::{Duration, sleep};

#[derive(Clone)]
pub struct AppState {
    db: Database,
    network: Network,
    ad_filter: ad_filter::AdFilter,
    refresh_guard: Arc<tokio::sync::Mutex<()>>,
    cache_dir: PathBuf,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .register_asynchronous_uri_scheme_protocol("reader-media", |ctx, request, responder| {
            let state = ctx.app_handle().state::<AppState>().inner().clone();
            let token = request.uri().path().trim_start_matches('/').to_owned();
            let range = request
                .headers()
                .get(tauri::http::header::RANGE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            tauri::async_runtime::spawn(async move {
                responder.respond(media::serve(state, &token, range.as_deref()).await);
            });
        })
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let database =
                tauri::async_runtime::block_on(Database::open(&data_dir.join("veilfeed.db")))
                    .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;
            let cache_dir = data_dir.join("media-cache");
            tauri::async_runtime::block_on(database.prune_unowned_media(&cache_dir))
                .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;
            let ad_filter = ad_filter::AdFilter::load(&data_dir);
            app.manage(AppState {
                db: database,
                network: Network::new(),
                ad_filter,
                refresh_guard: Arc::new(tokio::sync::Mutex::new(())),
                cache_dir,
            });
            let state = app.state::<AppState>().inner().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    match state.db.proxy(None).await {
                        Ok(profile) => {
                            if let Err(error) = state
                                .ad_filter
                                .refresh(&state.network, profile.as_ref())
                                .await
                            {
                                eprintln!(
                                    "blocker-list update failed; retaining current rules: {error}"
                                );
                            }
                        }
                        Err(error) => {
                            eprintln!(
                                "blocker-list route unavailable; retaining current rules: {error}"
                            );
                        }
                    }
                    sleep(Duration::from_secs(60 * 60)).await;
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            commands::get_sidebar,
            commands::list_articles,
            commands::get_article,
            commands::set_article_state,
            commands::mark_all_read,
            commands::create_folder,
            commands::update_folder,
            commands::delete_folder,
            commands::update_feed,
            commands::move_feed,
            commands::delete_feed,
            commands::list_proxies,
            commands::get_default_proxy_profile,
            commands::set_default_proxy_profile,
            commands::save_proxy,
            commands::test_proxy,
            commands::discover_feed,
            commands::subscribe,
            commands::refresh_feeds,
            commands::fetch_full_content,
            commands::preview_opml,
            commands::import_opml,
            commands::export_opml,
            commands::check_for_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Veilfeed");
}
