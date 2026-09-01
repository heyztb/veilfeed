use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use chrono::Utc;
use sqlx::{
    QueryBuilder, Row, Sqlite, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    models::{
        ArticleDetail, ArticlePage, ArticleQuery, ArticleSummary, FeedSummary, FolderSummary,
        OpmlEntry, ProxyProfile, SidebarData, UpdateFeedInput,
    },
    network::ParsedFeed,
};

#[derive(Clone)]
pub struct Database {
    pub pool: SqlitePool,
}

impl Database {
    pub async fn open(path: &Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        sqlx::migrate!().run(&pool).await?;
        let db = Self { pool };
        db.seed().await?;
        Ok(db)
    }

    async fn seed(&self) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT OR IGNORE INTO proxy_profiles(id,name,kind,remote_dns,created_at,updated_at) VALUES('direct','Direct','direct',0,?,?)")
            .bind(&now).bind(&now).execute(&self.pool).await?;
        sqlx::query("INSERT OR IGNORE INTO proxy_profiles(id,name,kind,endpoint,remote_dns,created_at,updated_at) VALUES('tor','Tor','socks5','socks5h://127.0.0.1:9050',1,?,?)")
            .bind(&now).bind(&now).execute(&self.pool).await?;
        sqlx::query("INSERT OR IGNORE INTO proxy_profiles(id,name,kind,endpoint,remote_dns,created_at,updated_at) VALUES('tor-browser','Tor Browser','socks5','socks5h://127.0.0.1:9150',1,?,?)")
            .bind(&now).bind(&now).execute(&self.pool).await?;
        sqlx::query("INSERT OR IGNORE INTO settings(key,value) VALUES('default_proxy_profile_id','direct'),('media_cache_limit_bytes','524288000'),('refresh_interval_minutes','60')")
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn proxies(&self) -> AppResult<Vec<ProxyProfile>> {
        Ok(sqlx::query_as("SELECT id,name,kind,endpoint,username,remote_dns FROM proxy_profiles WHERE deleted_at IS NULL ORDER BY CASE id WHEN 'direct' THEN 0 WHEN 'tor' THEN 1 WHEN 'tor-browser' THEN 2 ELSE 3 END,name")
            .fetch_all(&self.pool).await?)
    }

    pub async fn default_proxy_profile_id(&self) -> AppResult<String> {
        Ok(
            sqlx::query_scalar("SELECT value FROM settings WHERE key='default_proxy_profile_id'")
                .fetch_one(&self.pool)
                .await?,
        )
    }

    pub async fn set_default_proxy_profile_id(&self, id: &str) -> AppResult<()> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM proxy_profiles WHERE id=? AND deleted_at IS NULL)",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        if !exists {
            return Err(AppError::InvalidInput(
                "default connection profile does not exist".into(),
            ));
        }
        sqlx::query("UPDATE settings SET value=? WHERE key='default_proxy_profile_id'")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn proxy(&self, id: Option<&str>) -> AppResult<Option<ProxyProfile>> {
        let id = if let Some(id) = id {
            id.to_owned()
        } else {
            self.default_proxy_profile_id().await?
        };
        Ok(sqlx::query_as("SELECT id,name,kind,endpoint,username,remote_dns FROM proxy_profiles WHERE id=? AND deleted_at IS NULL")
            .bind(id).fetch_optional(&self.pool).await?)
    }

    pub async fn sidebar(&self) -> AppResult<SidebarData> {
        let folders: Vec<FolderSummary> = sqlx::query_as("SELECT fo.id,fo.parent_id,fo.name,fo.sort_order,coalesce(sum(CASE WHEN a.is_unread=1 AND a.deleted_at IS NULL THEN 1 ELSE 0 END),0) unread_count FROM folders fo LEFT JOIN feeds f ON f.folder_id=fo.id AND f.deleted_at IS NULL LEFT JOIN articles a ON a.feed_id=f.id WHERE fo.deleted_at IS NULL GROUP BY fo.id ORDER BY fo.sort_order,fo.name")
            .fetch_all(&self.pool).await?;
        let feeds: Vec<FeedSummary> = sqlx::query_as("SELECT f.id,f.folder_id,f.proxy_profile_id,f.title,f.feed_url,f.site_url,f.icon_url,f.block_ads,f.refresh_interval_minutes,coalesce(sum(CASE WHEN a.is_unread=1 AND a.deleted_at IS NULL THEN 1 ELSE 0 END),0) unread_count,f.last_error_code,f.last_error_message,f.last_checked_at FROM feeds f LEFT JOIN articles a ON a.feed_id=f.id WHERE f.deleted_at IS NULL GROUP BY f.id ORDER BY f.sort_order,f.title")
            .fetch_all(&self.pool).await?;
        let counts = sqlx::query("SELECT count(*) all_count,coalesce(sum(is_unread),0) unread_count,coalesce(sum(is_starred),0) starred_count FROM articles WHERE deleted_at IS NULL")
            .fetch_one(&self.pool).await?;
        Ok(SidebarData {
            folders,
            feeds,
            all_count: counts.get("all_count"),
            unread_count: counts.get("unread_count"),
            starred_count: counts.get("starred_count"),
        })
    }

    pub async fn articles(&self, input: ArticleQuery) -> AppResult<ArticlePage> {
        let limit = input.limit.unwrap_or(80).clamp(1, 200) as i64;
        let offset = input
            .cursor
            .as_deref()
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0);
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT a.id,a.feed_id,f.title feed_title,a.title,a.author,a.published_at,a.received_at,a.is_unread,a.is_starred,substr(a.plain_text,1,240) excerpt FROM articles a JOIN feeds f ON f.id=a.feed_id WHERE a.deleted_at IS NULL AND f.deleted_at IS NULL",
        );
        if let Some(scope) = input.scope.as_deref() {
            match scope {
                "unread" => {
                    query.push(" AND a.is_unread=1");
                }
                "starred" => {
                    query.push(" AND a.is_starred=1");
                }
                _ => {}
            }
        }
        if let Some(feed_id) = input.feed_id {
            query.push(" AND a.feed_id=").push_bind(feed_id);
        }
        if let Some(folder_id) = input.folder_id {
            query.push(" AND f.folder_id=").push_bind(folder_id);
        }
        if let Some(search) = input.search.filter(|value| !value.trim().is_empty()) {
            query
                .push(" AND a.id IN (SELECT article_id FROM article_fts WHERE article_fts MATCH ")
                .push_bind(search)
                .push(")");
        }
        query
            .push(" ORDER BY coalesce(a.published_at,a.received_at) DESC,a.id DESC LIMIT ")
            .push_bind(limit + 1)
            .push(" OFFSET ")
            .push_bind(offset);
        let mut items: Vec<ArticleSummary> = query.build_query_as().fetch_all(&self.pool).await?;
        let has_more = items.len() > limit as usize;
        if has_more {
            items.pop();
        }
        let next_cursor = has_more.then(|| (offset + limit).to_string());
        Ok(ArticlePage { items, next_cursor })
    }

    pub async fn article(&self, id: &str) -> AppResult<ArticleDetail> {
        sqlx::query_as("SELECT a.id,a.feed_id,f.title feed_title,a.title,a.author,a.canonical_url,a.published_at,a.content_html,a.full_content_html,a.content_filter_revision,a.full_content_filter_revision,f.block_ads,a.is_unread,a.is_starred FROM articles a JOIN feeds f ON f.id=a.feed_id WHERE a.id=? AND a.deleted_at IS NULL")
            .bind(id).fetch_optional(&self.pool).await?.ok_or_else(|| AppError::NotFound("article".into()))
    }

    pub async fn set_article_state(
        &self,
        ids: &[String],
        unread: Option<bool>,
        starred: Option<bool>,
    ) -> AppResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let now = Utc::now().to_rfc3339();
        let mut query = QueryBuilder::<Sqlite>::new("UPDATE articles SET updated_at=");
        query.push_bind(now);
        if let Some(value) = unread {
            query.push(",is_unread=").push_bind(value);
        }
        if let Some(value) = starred {
            query.push(",is_starred=").push_bind(value);
        }
        query.push(" WHERE id IN (");
        let mut separated = query.separated(",");
        for id in ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");
        query.build().execute(&self.pool).await?;
        Ok(())
    }

    pub async fn mark_all_read(
        &self,
        feed_id: Option<&str>,
        folder_id: Option<&str>,
        scope: Option<&str>,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let mut query = QueryBuilder::<Sqlite>::new("UPDATE articles SET is_unread=0,updated_at=");
        query
            .push_bind(now)
            .push(" WHERE deleted_at IS NULL AND is_unread=1");
        if let Some(feed_id) = feed_id {
            query.push(" AND feed_id=").push_bind(feed_id);
        } else if let Some(folder_id) = folder_id {
            query
                .push(" AND feed_id IN (SELECT id FROM feeds WHERE folder_id=")
                .push_bind(folder_id)
                .push(" AND deleted_at IS NULL)");
        } else if scope == Some("starred") {
            query.push(" AND is_starred=1");
        }
        query.build().execute(&self.pool).await?;
        Ok(())
    }

    pub async fn create_folder(&self, name: &str, parent_id: Option<&str>) -> AppResult<String> {
        if name.trim().is_empty() {
            return Err(AppError::InvalidInput("folder name cannot be empty".into()));
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO folders(id,parent_id,name,created_at,updated_at) VALUES(?,?,?,?,?)",
        )
        .bind(&id)
        .bind(parent_id)
        .bind(name.trim())
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn create_folder_with_feed(
        &self,
        name: &str,
        parent_id: Option<&str>,
        feed_id: &str,
    ) -> AppResult<String> {
        if name.trim().is_empty() {
            return Err(AppError::InvalidInput("folder name cannot be empty".into()));
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO folders(id,parent_id,name,created_at,updated_at) VALUES(?,?,?,?,?)",
        )
        .bind(&id)
        .bind(parent_id)
        .bind(name.trim())
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        let old_folder: Option<String> =
            sqlx::query_scalar("SELECT folder_id FROM feeds WHERE id=? AND deleted_at IS NULL")
                .bind(feed_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::NotFound("feed".into()))?;
        sqlx::query("UPDATE feeds SET folder_id=?,updated_at=? WHERE id=?")
            .bind(&id)
            .bind(&now)
            .bind(feed_id)
            .execute(&mut *tx)
            .await?;
        cleanup_empty_folder(&mut tx, old_folder.as_deref(), &now).await?;
        tx.commit().await?;
        Ok(id)
    }

    pub async fn update_folder(&self, id: &str, name: &str) -> AppResult<()> {
        if name.trim().is_empty() {
            return Err(AppError::InvalidInput("folder name cannot be empty".into()));
        }
        let result =
            sqlx::query("UPDATE folders SET name=?,updated_at=? WHERE id=? AND deleted_at IS NULL")
                .bind(name.trim())
                .bind(Utc::now().to_rfc3339())
                .bind(id)
                .execute(&self.pool)
                .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("folder".into()));
        }
        Ok(())
    }

    pub async fn delete_folder(&self, id: &str) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let result = sqlx::query(
            "UPDATE folders SET deleted_at=?,updated_at=? WHERE id=? AND deleted_at IS NULL",
        )
        .bind(&now)
        .bind(&now)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("folder".into()));
        }
        sqlx::query(
            "UPDATE feeds SET folder_id=NULL,updated_at=? WHERE folder_id=? AND deleted_at IS NULL",
        )
        .bind(&now)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE folders SET parent_id=NULL,updated_at=? WHERE parent_id=? AND deleted_at IS NULL")
            .bind(&now).bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn update_feed(&self, input: &UpdateFeedInput) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        ensure_active_folder(&mut tx, input.folder_id.as_deref()).await?;
        let existing =
            sqlx::query("SELECT folder_id,feed_url FROM feeds WHERE id=? AND deleted_at IS NULL")
                .bind(&input.id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::NotFound("feed".into()))?;
        let old_folder: Option<String> = existing.try_get("folder_id")?;
        let old_url: String = existing.get("feed_url");
        let url_changed = old_url != input.feed_url;
        sqlx::query("UPDATE feeds SET title=?,feed_url=?,folder_id=?,proxy_profile_id=?,refresh_interval_minutes=?,block_ads=?,etag=CASE WHEN ? THEN NULL ELSE etag END,last_modified=CASE WHEN ? THEN NULL ELSE last_modified END,last_checked_at=CASE WHEN ? THEN NULL ELSE last_checked_at END,last_error_code=CASE WHEN ? THEN NULL ELSE last_error_code END,last_error_message=CASE WHEN ? THEN NULL ELSE last_error_message END,updated_at=? WHERE id=?")
            .bind(input.title.trim()).bind(&input.feed_url).bind(&input.folder_id).bind(&input.proxy_profile_id).bind(input.refresh_interval_minutes).bind(input.block_ads)
            .bind(url_changed).bind(url_changed).bind(url_changed).bind(url_changed).bind(url_changed).bind(&now).bind(&input.id)
            .execute(&mut *tx).await?;
        if old_folder.as_deref() != input.folder_id.as_deref() {
            cleanup_empty_folder(&mut tx, old_folder.as_deref(), &now).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn move_feed(&self, id: &str, folder_id: Option<&str>) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        ensure_active_folder(&mut tx, folder_id).await?;
        let old_folder: Option<String> =
            sqlx::query_scalar("SELECT folder_id FROM feeds WHERE id=? AND deleted_at IS NULL")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::NotFound("feed".into()))?;
        sqlx::query("UPDATE feeds SET folder_id=?,updated_at=? WHERE id=?")
            .bind(folder_id)
            .bind(&now)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if old_folder.as_deref() != folder_id {
            cleanup_empty_folder(&mut tx, old_folder.as_deref(), &now).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_feed(&self, id: &str, cache_dir: &Path) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let old_folder: Option<String> =
            sqlx::query_scalar("SELECT folder_id FROM feeds WHERE id=? AND deleted_at IS NULL")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| AppError::NotFound("feed".into()))?;
        let orphaned_cache_keys: Vec<String> = sqlx::query_scalar(
            "SELECT owned.cache_key FROM media_cache_feeds owned WHERE owned.feed_id=? AND NOT EXISTS(SELECT 1 FROM media_cache_feeds other WHERE other.cache_key=owned.cache_key AND other.feed_id<>?)",
        )
        .bind(id)
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM feeds WHERE id=?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        for cache_key in &orphaned_cache_keys {
            sqlx::query("DELETE FROM media_cache WHERE cache_key=?")
                .bind(cache_key)
                .execute(&mut *tx)
                .await?;
            match tokio::fs::remove_file(cache_dir.join(format!("{cache_key}.bin"))).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        cleanup_empty_folder(&mut tx, old_folder.as_deref(), &now).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn prune_unowned_media(&self, cache_dir: &Path) -> AppResult<()> {
        let rows = sqlx::query("SELECT cache_key FROM media_cache WHERE NOT EXISTS(SELECT 1 FROM media_cache_feeds WHERE media_cache_feeds.cache_key=media_cache.cache_key)")
            .fetch_all(&self.pool)
            .await?;
        for row in rows {
            let cache_key: String = row.get("cache_key");
            match tokio::fs::remove_file(cache_dir.join(format!("{cache_key}.bin"))).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            sqlx::query("DELETE FROM media_cache WHERE cache_key=?")
                .bind(cache_key)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    pub async fn insert_feed(
        &self,
        feed_url: &str,
        folder_id: Option<&str>,
        proxy_id: Option<&str>,
        parsed: &ParsedFeed,
        requested_id: Option<&str>,
    ) -> AppResult<(String, usize)> {
        let id = requested_id
            .map(str::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO feeds(id,folder_id,proxy_profile_id,title,feed_url,site_url,icon_url,description,etag,last_modified,last_checked_at,last_success_at,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
            .bind(&id).bind(folder_id).bind(proxy_id).bind(&parsed.title).bind(feed_url).bind(&parsed.site_url).bind(&parsed.icon_url).bind(&parsed.description)
            .bind(&parsed.etag).bind(&parsed.last_modified).bind(&now).bind(&now).bind(&now).bind(&now).execute(&mut *tx).await?;
        let count = insert_articles(&mut tx, &id, parsed, &now).await?;
        tx.commit().await?;
        Ok((id, count))
    }

    pub async fn feed_refresh_context(
        &self,
        only_id: Option<&str>,
        due_only: bool,
    ) -> AppResult<Vec<(FeedSummary, Option<ProxyProfile>, String, String)>> {
        let feeds: Vec<FeedSummary> = if let Some(id) = only_id {
            sqlx::query_as("SELECT f.id,f.folder_id,f.proxy_profile_id,f.title,f.feed_url,f.site_url,f.icon_url,f.block_ads,f.refresh_interval_minutes,0 unread_count,f.last_error_code,f.last_error_message,f.last_checked_at FROM feeds f WHERE f.id=? AND f.deleted_at IS NULL").bind(id).fetch_all(&self.pool).await?
        } else if due_only {
            sqlx::query_as("SELECT f.id,f.folder_id,f.proxy_profile_id,f.title,f.feed_url,f.site_url,f.icon_url,f.block_ads,f.refresh_interval_minutes,0 unread_count,f.last_error_code,f.last_error_message,f.last_checked_at FROM feeds f WHERE f.deleted_at IS NULL AND (f.last_checked_at IS NULL OR datetime(f.last_checked_at) <= datetime('now','-' || f.refresh_interval_minutes || ' minutes'))").fetch_all(&self.pool).await?
        } else {
            sqlx::query_as("SELECT f.id,f.folder_id,f.proxy_profile_id,f.title,f.feed_url,f.site_url,f.icon_url,f.block_ads,f.refresh_interval_minutes,0 unread_count,f.last_error_code,f.last_error_message,f.last_checked_at FROM feeds f WHERE f.deleted_at IS NULL").fetch_all(&self.pool).await?
        };
        let mut output = Vec::new();
        for feed in feeds {
            let row = sqlx::query("SELECT coalesce(etag,'') etag,coalesce(last_modified,'') last_modified FROM feeds WHERE id=?").bind(&feed.id).fetch_one(&self.pool).await?;
            let profile = self.proxy(feed.proxy_profile_id.as_deref()).await?;
            output.push((feed, profile, row.get("etag"), row.get("last_modified")));
        }
        Ok(output)
    }

    pub async fn apply_refresh(
        &self,
        feed_id: &str,
        parsed: Option<&ParsedFeed>,
    ) -> AppResult<usize> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let count = if let Some(parsed) = parsed {
            sqlx::query("UPDATE feeds SET site_url=?,icon_url=?,description=?,etag=?,last_modified=?,last_checked_at=?,last_success_at=?,last_error_code=NULL,last_error_message=NULL,updated_at=? WHERE id=?")
                .bind(&parsed.site_url).bind(&parsed.icon_url).bind(&parsed.description).bind(&parsed.etag).bind(&parsed.last_modified).bind(&now).bind(&now).bind(&now).bind(feed_id).execute(&mut *tx).await?;
            insert_articles(&mut tx, feed_id, parsed, &now).await?
        } else {
            sqlx::query("UPDATE feeds SET last_checked_at=?,last_success_at=?,last_error_code=NULL,last_error_message=NULL WHERE id=?").bind(&now).bind(&now).bind(feed_id).execute(&mut *tx).await?;
            0
        };
        tx.commit().await?;
        Ok(count)
    }

    pub async fn set_feed_icon_url(&self, feed_id: &str, icon_url: Option<&str>) -> AppResult<()> {
        sqlx::query("UPDATE feeds SET icon_url=? WHERE id=? AND deleted_at IS NULL")
            .bind(icon_url)
            .bind(feed_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn record_feed_error(
        &self,
        feed_id: &str,
        code: &str,
        message: &str,
    ) -> AppResult<()> {
        sqlx::query(
            "UPDATE feeds SET last_checked_at=?,last_error_code=?,last_error_message=? WHERE id=?",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(code)
        .bind(message)
        .bind(feed_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_full_content(
        &self,
        article_id: &str,
        html: &str,
        filter_revision: Option<&str>,
    ) -> AppResult<()> {
        sqlx::query("UPDATE articles SET full_content_html=?,full_content_filter_revision=?,updated_at=? WHERE id=?")
            .bind(html)
            .bind(filter_revision)
            .bind(Utc::now().to_rfc3339())
            .bind(article_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn persist_filtered_article(
        &self,
        article_id: &str,
        content_html: Option<(&str, &str)>,
        full_content_html: Option<(&str, &str)>,
    ) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;
        if let Some((html, revision)) = content_html {
            let text = crate::content::plain_text(html);
            sqlx::query("UPDATE articles SET content_html=?,plain_text=?,content_filter_revision=?,updated_at=? WHERE id=?")
                .bind(html)
                .bind(text)
                .bind(revision)
                .bind(Utc::now().to_rfc3339())
                .bind(article_id)
                .execute(&mut *tx)
                .await?;
        }
        if let Some((html, revision)) = full_content_html {
            sqlx::query("UPDATE articles SET full_content_html=?,full_content_filter_revision=?,updated_at=? WHERE id=?")
                .bind(html)
                .bind(revision)
                .bind(Utc::now().to_rfc3339())
                .bind(article_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn export_entries(&self) -> AppResult<Vec<OpmlEntry>> {
        let folder_rows =
            sqlx::query("SELECT id,parent_id,name FROM folders WHERE deleted_at IS NULL")
                .fetch_all(&self.pool)
                .await?;
        let folders = folder_rows
            .into_iter()
            .map(|row| {
                (
                    row.get::<String, _>("id"),
                    (
                        row.try_get::<String, _>("parent_id").ok(),
                        row.get::<String, _>("name"),
                    ),
                )
            })
            .collect::<HashMap<_, _>>();
        let rows = sqlx::query(
            "SELECT title,feed_url,site_url,folder_id FROM feeds WHERE deleted_at IS NULL",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut entries = rows
            .into_iter()
            .map(|row| OpmlEntry {
                title: row.get("title"),
                feed_url: row.get("feed_url"),
                site_url: row.get("site_url"),
                folder_path: folder_path(
                    row.try_get::<String, _>("folder_id").ok().as_deref(),
                    &folders,
                ),
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            left.folder_path
                .cmp(&right.folder_path)
                .then_with(|| left.title.cmp(&right.title))
        });
        Ok(entries)
    }

    pub async fn find_or_create_folder_path(&self, path: &[String]) -> AppResult<Option<String>> {
        let mut parent: Option<String> = None;
        for name in path {
            let existing: Option<String> = sqlx::query_scalar("SELECT id FROM folders WHERE name=? AND (parent_id=? OR (parent_id IS NULL AND ? IS NULL)) AND deleted_at IS NULL")
                .bind(name).bind(&parent).bind(&parent).fetch_optional(&self.pool).await?;
            parent = Some(if let Some(id) = existing {
                id
            } else {
                self.create_folder(name, parent.as_deref()).await?
            });
        }
        Ok(parent)
    }
}

fn folder_path(
    folder_id: Option<&str>,
    folders: &HashMap<String, (Option<String>, String)>,
) -> Vec<String> {
    let mut current = folder_id;
    let mut visited = HashSet::new();
    let mut path = Vec::new();
    while let Some(id) = current {
        if !visited.insert(id.to_owned()) {
            break;
        }
        let Some((parent_id, name)) = folders.get(id) else {
            break;
        };
        path.push(name.clone());
        current = parent_id.as_deref();
    }
    path.reverse();
    path
}

async fn ensure_active_folder(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    folder_id: Option<&str>,
) -> AppResult<()> {
    if let Some(folder_id) = folder_id {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM folders WHERE id=? AND deleted_at IS NULL)",
        )
        .bind(folder_id)
        .fetch_one(&mut **tx)
        .await?;
        if !exists {
            return Err(AppError::NotFound("folder".into()));
        }
    }
    Ok(())
}

async fn cleanup_empty_folder(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    folder_id: Option<&str>,
    now: &str,
) -> AppResult<()> {
    if let Some(folder_id) = folder_id {
        sqlx::query("UPDATE folders SET deleted_at=?,updated_at=? WHERE id=? AND deleted_at IS NULL AND NOT EXISTS(SELECT 1 FROM feeds WHERE folder_id=? AND deleted_at IS NULL)")
            .bind(now).bind(now).bind(folder_id).bind(folder_id).execute(&mut **tx).await?;
    }
    Ok(())
}

#[allow(clippy::items_after_test_module)]
#[cfg(test)]
mod tests {
    use super::*;

    async fn insert_test_feed(db: &Database, id: &str, folder_id: Option<&str>) {
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO feeds(id,folder_id,proxy_profile_id,title,feed_url,created_at,updated_at) VALUES(?,?,?,?,?,?,?)")
            .bind(id)
            .bind(folder_id)
            .bind("direct")
            .bind(format!("Feed {id}"))
            .bind(format!("https://example.com/{id}.xml"))
            .bind(&now)
            .bind(&now)
            .execute(&db.pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn default_connection_profile_follows_the_configured_setting() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::open(&directory.path().join("default-profile.db"))
            .await
            .unwrap();
        assert_eq!(db.proxy(None).await.unwrap().unwrap().id, "direct");

        db.set_default_proxy_profile_id("tor").await.unwrap();
        assert_eq!(db.default_proxy_profile_id().await.unwrap(), "tor");
        assert_eq!(db.proxy(None).await.unwrap().unwrap().id, "tor");

        assert!(db.set_default_proxy_profile_id("missing").await.is_err());
        assert_eq!(db.default_proxy_profile_id().await.unwrap(), "tor");
    }

    #[tokio::test]
    async fn seeds_tor_daemon_and_tor_browser_connection_profiles() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::open(&directory.path().join("tor-profiles.db"))
            .await
            .unwrap();

        let profiles = db.proxies().await.unwrap();
        assert_eq!(
            profiles
                .iter()
                .map(|profile| (
                    profile.id.as_str(),
                    profile.endpoint.as_deref(),
                    profile.remote_dns
                ))
                .collect::<Vec<_>>(),
            vec![
                ("direct", None, false),
                ("tor", Some("socks5h://127.0.0.1:9050"), true),
                ("tor-browser", Some("socks5h://127.0.0.1:9150"), true),
            ]
        );
    }

    #[tokio::test]
    async fn new_folders_can_be_empty_but_are_removed_when_the_last_feed_leaves() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::open(&directory.path().join("folders.db"))
            .await
            .unwrap();
        let source = db.create_folder("Source", None).await.unwrap();
        let destination = db.create_folder("Destination", None).await.unwrap();
        insert_test_feed(&db, "one", Some(&source)).await;
        insert_test_feed(&db, "two", Some(&source)).await;

        let sidebar = db.sidebar().await.unwrap();
        assert_eq!(sidebar.folders.len(), 2);
        assert!(sidebar.folders.iter().any(|folder| folder.id == source));
        assert!(
            sidebar
                .folders
                .iter()
                .any(|folder| folder.id == destination)
        );

        db.move_feed("one", None).await.unwrap();
        let source_deleted: Option<String> =
            sqlx::query_scalar("SELECT deleted_at FROM folders WHERE id=?")
                .bind(&source)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert!(source_deleted.is_none());

        db.move_feed("two", Some(&destination)).await.unwrap();
        let source_deleted: Option<String> =
            sqlx::query_scalar("SELECT deleted_at FROM folders WHERE id=?")
                .bind(&source)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert!(source_deleted.is_some());
        let sidebar = db.sidebar().await.unwrap();
        assert_eq!(sidebar.folders.len(), 1);
        assert_eq!(sidebar.folders[0].id, destination);

        db.delete_feed("two", directory.path()).await.unwrap();
        assert!(db.sidebar().await.unwrap().folders.is_empty());
        let destination_deleted: Option<String> =
            sqlx::query_scalar("SELECT deleted_at FROM folders WHERE id=?")
                .bind(&destination)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert!(destination_deleted.is_some());
    }

    #[tokio::test]
    async fn folders_can_be_renamed_and_deleted_without_deleting_their_feeds() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::open(&directory.path().join("folder-management.db"))
            .await
            .unwrap();
        let folder = db.create_folder("Old name", None).await.unwrap();
        insert_test_feed(&db, "kept", Some(&folder)).await;

        db.update_folder(&folder, "New name").await.unwrap();
        assert_eq!(db.sidebar().await.unwrap().folders[0].name, "New name");

        db.delete_folder(&folder).await.unwrap();
        let feed_folder: Option<String> =
            sqlx::query_scalar("SELECT folder_id FROM feeds WHERE id='kept'")
                .fetch_one(&db.pool)
                .await
                .unwrap();
        let feed_deleted: Option<String> =
            sqlx::query_scalar("SELECT deleted_at FROM feeds WHERE id='kept'")
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert!(feed_folder.is_none());
        assert!(feed_deleted.is_none());
        assert!(db.sidebar().await.unwrap().folders.is_empty());
    }

    #[tokio::test]
    async fn feed_edits_preserve_intervals_and_reset_validators_for_a_new_url() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::open(&directory.path().join("feed-edit.db"))
            .await
            .unwrap();
        insert_test_feed(&db, "edited", None).await;
        sqlx::query("UPDATE feeds SET etag='old-etag',last_modified='old-date',last_checked_at='2026-08-27T00:00:00Z',last_error_code='old-error',last_error_message='old message' WHERE id='edited'")
            .execute(&db.pool)
            .await
            .unwrap();

        db.update_feed(&UpdateFeedInput {
            id: "edited".into(),
            title: "My title".into(),
            feed_url: "https://example.com/replacement.xml".into(),
            folder_id: None,
            proxy_profile_id: Some("direct".into()),
            refresh_interval_minutes: 180,
            block_ads: false,
        })
        .await
        .unwrap();

        let feed = db.sidebar().await.unwrap().feeds.remove(0);
        assert_eq!(feed.title, "My title");
        assert_eq!(feed.refresh_interval_minutes, 180);
        let state = sqlx::query(
            "SELECT etag,last_modified,last_checked_at,last_error_code,last_error_message FROM feeds WHERE id='edited'",
        )
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert!(state.get::<Option<String>, _>("etag").is_none());
        assert!(state.get::<Option<String>, _>("last_modified").is_none());
        assert!(state.get::<Option<String>, _>("last_checked_at").is_none());
        assert!(state.get::<Option<String>, _>("last_error_code").is_none());
        assert!(
            state
                .get::<Option<String>, _>("last_error_message")
                .is_none()
        );
    }

    #[tokio::test]
    async fn refresh_preserves_the_user_configured_feed_title() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::open(&directory.path().join("feed-title.db"))
            .await
            .unwrap();
        insert_test_feed(&db, "renamed", None).await;
        sqlx::query("UPDATE feeds SET title='My custom title' WHERE id='renamed'")
            .execute(&db.pool)
            .await
            .unwrap();
        let parsed = ParsedFeed {
            title: "Publisher title".into(),
            site_url: Some("https://example.com".into()),
            icon_url: None,
            description: None,
            articles: vec![],
            etag: Some("new-etag".into()),
            last_modified: None,
            filter_revision: None,
        };

        db.apply_refresh("renamed", Some(&parsed)).await.unwrap();

        let title: String = sqlx::query_scalar("SELECT title FROM feeds WHERE id='renamed'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(title, "My custom title");
    }

    #[tokio::test]
    async fn opml_export_entries_preserve_nested_folder_paths() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::open(&directory.path().join("nested-export.db"))
            .await
            .unwrap();
        let parent = db.create_folder("News", None).await.unwrap();
        let child = db.create_folder("Technology", Some(&parent)).await.unwrap();
        insert_test_feed(&db, "nested", Some(&child)).await;

        let entries = db.export_entries().await.unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].folder_path, ["News", "Technology"]);
    }

    #[tokio::test]
    async fn deleting_a_feed_permanently_deletes_it_and_its_articles_and_search_rows() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::open(&directory.path().join("feed-deletion.db"))
            .await
            .unwrap();
        insert_test_feed(&db, "removed", None).await;
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO articles(id,feed_id,source_key,title,received_at,content_html,plain_text,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?)")
            .bind("article")
            .bind("removed")
            .bind("guid:article")
            .bind("Article")
            .bind(&now)
            .bind("<p>Body</p>")
            .bind("Body")
            .bind(&now)
            .bind(&now)
            .execute(&db.pool)
            .await
            .unwrap();

        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM article_fts")
                .fetch_one(&db.pool)
                .await
                .unwrap(),
            1
        );

        db.delete_feed("removed", directory.path()).await.unwrap();

        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM articles WHERE feed_id='removed'")
                .fetch_one(&db.pool)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM article_fts")
                .fetch_one(&db.pool)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM feeds WHERE id='removed'")
                .fetch_one(&db.pool)
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn deleting_feeds_removes_only_unreferenced_cached_media() {
        let directory = tempfile::tempdir().unwrap();
        let cache_dir = directory.path().join("media-cache");
        tokio::fs::create_dir_all(&cache_dir).await.unwrap();
        let db = Database::open(&directory.path().join("media-deletion.db"))
            .await
            .unwrap();
        insert_test_feed(&db, "removed", None).await;
        insert_test_feed(&db, "kept", None).await;
        let now = Utc::now().to_rfc3339();
        let cache_key = "shared";
        let cache_file = cache_dir.join(format!("{cache_key}.bin"));
        tokio::fs::write(&cache_file, b"cached media")
            .await
            .unwrap();
        sqlx::query("INSERT INTO media_cache(cache_key,source_url,proxy_profile_id,mime_type,byte_size,file_name,last_accessed_at,created_at) VALUES(?,?,?,?,?,?,?,?)")
            .bind(cache_key)
            .bind("https://example.com/image.png")
            .bind("direct")
            .bind("image/png")
            .bind(12_i64)
            .bind("shared.bin")
            .bind(&now)
            .bind(&now)
            .execute(&db.pool)
            .await
            .unwrap();
        for feed_id in ["removed", "kept"] {
            sqlx::query("INSERT INTO media_cache_feeds(cache_key,feed_id) VALUES(?,?)")
                .bind(cache_key)
                .bind(feed_id)
                .execute(&db.pool)
                .await
                .unwrap();
        }

        db.delete_feed("removed", &cache_dir).await.unwrap();
        assert!(cache_file.exists());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM media_cache")
                .fetch_one(&db.pool)
                .await
                .unwrap(),
            1
        );

        db.delete_feed("kept", &cache_dir).await.unwrap();
        assert!(!cache_file.exists());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM media_cache")
                .fetch_one(&db.pool)
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn startup_prunes_legacy_cache_entries_without_feed_owners() {
        let directory = tempfile::tempdir().unwrap();
        let cache_dir = directory.path().join("media-cache");
        tokio::fs::create_dir_all(&cache_dir).await.unwrap();
        let db = Database::open(&directory.path().join("legacy-media.db"))
            .await
            .unwrap();
        let now = Utc::now().to_rfc3339();
        let cache_file = cache_dir.join("legacy.bin");
        tokio::fs::write(&cache_file, b"legacy").await.unwrap();
        sqlx::query("INSERT INTO media_cache(cache_key,source_url,proxy_profile_id,mime_type,byte_size,file_name,last_accessed_at,created_at) VALUES('legacy','https://example.com/legacy.png','direct','image/png',6,'legacy.bin',?,?)")
            .bind(&now)
            .bind(&now)
            .execute(&db.pool)
            .await
            .unwrap();

        db.prune_unowned_media(&cache_dir).await.unwrap();

        assert!(!cache_file.exists());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM media_cache")
                .fetch_one(&db.pool)
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn persisting_filtered_content_updates_revision_plain_text_and_search() {
        let directory = tempfile::tempdir().unwrap();
        let db = Database::open(&directory.path().join("content-filter.db"))
            .await
            .unwrap();
        insert_test_feed(&db, "filtered", None).await;
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO articles(id,feed_id,source_key,title,received_at,content_html,full_content_html,plain_text,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?)")
            .bind("article")
            .bind("filtered")
            .bind("guid:article")
            .bind("Article")
            .bind(&now)
            .bind("<p>Old advertisement</p>")
            .bind("<p>Old full advertisement</p>")
            .bind("Old advertisement")
            .bind(&now)
            .bind(&now)
            .execute(&db.pool)
            .await
            .unwrap();

        db.persist_filtered_article(
            "article",
            Some(("<p>Clean body</p>", "revision-1")),
            Some(("<p>Clean full body</p>", "revision-1")),
        )
        .await
        .unwrap();

        let article = db.article("article").await.unwrap();
        assert_eq!(article.content_html, "<p>Clean body</p>");
        assert_eq!(
            article.full_content_html.as_deref(),
            Some("<p>Clean full body</p>")
        );
        assert_eq!(
            article.content_filter_revision.as_deref(),
            Some("revision-1")
        );
        assert_eq!(
            article.full_content_filter_revision.as_deref(),
            Some("revision-1")
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM article_fts WHERE article_fts MATCH 'Clean'"
            )
            .fetch_one(&db.pool)
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM article_fts WHERE article_fts MATCH 'advertisement'"
            )
            .fetch_one(&db.pool)
            .await
            .unwrap(),
            0
        );
    }
}

async fn insert_articles(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    feed_id: &str,
    parsed: &ParsedFeed,
    now: &str,
) -> AppResult<usize> {
    let mut count = 0;
    for article in &parsed.articles {
        let result = sqlx::query("INSERT OR IGNORE INTO articles(id,feed_id,source_key,guid,canonical_url,title,author,published_at,received_at,summary_html,content_html,plain_text,content_filter_revision,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
            .bind(Uuid::new_v4().to_string()).bind(feed_id).bind(&article.source_key).bind(&article.guid).bind(&article.canonical_url)
            .bind(&article.title).bind(&article.author).bind(&article.published_at).bind(now).bind(&article.summary_html).bind(&article.content_html)
            .bind(&article.plain_text).bind(parsed.filter_revision.as_deref()).bind(now).bind(now).execute(&mut **tx).await?;
        count += result.rows_affected() as usize;
    }
    Ok(count)
}
