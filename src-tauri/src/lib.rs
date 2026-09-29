mod commands;
mod media;

use tauri::Manager;
use veilfeed_core::{AppService, lifecycle::BackgroundTasks};
type AppState = AppService;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("could not start background runtime");
    tauri::async_runtime::set(runtime.handle().clone());
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
            let service = tauri::async_runtime::block_on(AppService::open(&data_dir))?;
            let tasks =
                tauri::async_runtime::block_on(async { BackgroundTasks::start(service.clone()) });
            app.manage(std::sync::Mutex::new(tasks));
            app.manage(service);
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
            commands::get_automatic_blocklist_updates,
            commands::set_automatic_blocklist_updates,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Veilfeed");
}
