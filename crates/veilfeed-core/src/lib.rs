pub mod ad_filter;
pub mod content;
pub mod db;
pub mod error;
pub mod lifecycle;
pub mod media;
pub mod models;
pub mod network;
pub mod opml;
pub mod release;
mod service;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone)]
pub struct AppService {
    pub(crate) db: db::Database,
    pub(crate) network: network::Network,
    pub(crate) ad_filter: ad_filter::AdFilter,
    pub(crate) refresh_guard: Arc<tokio::sync::Mutex<()>>,
    // Wakes the background blocker-list updater, the only caller of `AdFilter::refresh`.
    pub(crate) blocklist_wake: Arc<tokio::sync::Notify>,
    pub(crate) cache_dir: PathBuf,
}

// Compatibility name for the existing media tests and the Tauri adapter.
pub type AppState = AppService;

impl AppService {
    pub async fn open(data_dir: &Path) -> error::AppResult<Self> {
        let db = db::Database::open(&data_dir.join("veilfeed.db")).await?;
        let cache_dir = data_dir.join("media-cache");
        db.prune_unowned_media(&cache_dir).await?;
        let automatic_updates = db.automatic_blocklist_updates().await?;
        let filter_dir = data_dir.to_owned();
        let ad_filter = tokio::task::spawn_blocking(move || {
            ad_filter::AdFilter::load(&filter_dir, automatic_updates)
        })
        .await
        .map_err(|error| error::AppError::Io(std::io::Error::other(error)))?;
        Ok(Self {
            db,
            network: network::Network::new(),
            ad_filter,
            refresh_guard: Arc::new(tokio::sync::Mutex::new(())),
            blocklist_wake: Arc::new(tokio::sync::Notify::new()),
            cache_dir,
        })
    }
}

#[cfg(test)]
mod migration_tests;
