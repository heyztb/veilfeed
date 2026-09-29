use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::Row;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::{
    AppState,
    content::{decode_media_token, validate_remote_url},
    error::{AppError, AppResult},
};

const MEDIA_URL_PREFIX: &str = "reader-media://localhost/";

pub async fn load(state: &AppState, token: &str) -> AppResult<(Vec<u8>, String)> {
    let decoded = decode_media_token(token)?;
    // Article media must have been emitted by the sanitizer for this feed. A
    // caller cannot turn this protocol into an arbitrary network fetcher by
    // supplying its own base64-encoded URL.
    if !decoded.cache_only
        && !state
            .db
            .article_media_authorized(&decoded.feed_id, token)
            .await?
    {
        return Err(AppError::NotFound("article media".into()));
    }
    load_source(state, &decoded.url, &decoded.feed_id, decoded.cache_only).await
}

/// Yields every media token the sanitizer emitted into `html`. Tokens are
/// unpadded URL-safe base64, so they end at the first other character.
pub(crate) fn article_media_tokens(html: &str) -> impl Iterator<Item = &str> {
    html.match_indices(MEDIA_URL_PREFIX)
        .filter_map(move |(start, _)| {
            let rest = &html[start + MEDIA_URL_PREFIX.len()..];
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                .unwrap_or(rest.len());
            (end > 0).then(|| &rest[..end])
        })
}

async fn load_source(
    state: &AppState,
    url: &str,
    feed_id: &str,
    cache_only: bool,
) -> AppResult<(Vec<u8>, String)> {
    validate_remote_url(url)?;
    let feed = sqlx::query("SELECT proxy_profile_id FROM feeds WHERE id=? AND deleted_at IS NULL")
        .bind(feed_id)
        .fetch_optional(&state.db.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("feed".into()))?;
    let proxy_id: Option<String> = feed.try_get("proxy_profile_id")?;
    let profile = state.db.proxy(proxy_id.as_deref()).await?;
    let resolved_profile_id = profile
        .as_ref()
        .map(|value| value.id.as_str())
        .unwrap_or("direct");
    let mut hash = Sha256::new();
    hash.update(url.as_bytes());
    hash.update([0]);
    hash.update(resolved_profile_id.as_bytes());
    let key = hex::encode(hash.finalize());
    let file = state.cache_dir.join(format!("{key}.bin"));
    if let Some(row) = sqlx::query("SELECT mime_type FROM media_cache WHERE cache_key=?")
        .bind(&key)
        .fetch_optional(&state.db.pool)
        .await?
        && let Ok(bytes) = tokio::fs::read(&file).await
    {
        if cache_only {
            let owned: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM media_cache_feeds WHERE cache_key=? AND feed_id=?)",
            )
            .bind(&key)
            .bind(feed_id)
            .fetch_one(&state.db.pool)
            .await?;
            if !owned {
                return Err(AppError::NotFound("cached media".into()));
            }
        }
        associate_with_feed(state, &key, feed_id).await?;
        sqlx::query("UPDATE media_cache SET last_accessed_at=? WHERE cache_key=?")
            .bind(Utc::now().to_rfc3339())
            .bind(&key)
            .execute(&state.db.pool)
            .await?;
        return Ok((
            bytes,
            row.try_get::<String, _>("mime_type")
                .unwrap_or_else(|_| "application/octet-stream".into()),
        ));
    }
    if cache_only {
        return Err(AppError::NotFound("cached media".into()));
    }
    let (bytes, mime, etag, modified) = state.network.fetch_media(url, profile.as_ref()).await?;
    tokio::fs::create_dir_all(&state.cache_dir).await?;
    #[cfg(unix)]
    tokio::fs::set_permissions(&state.cache_dir, std::fs::Permissions::from_mode(0o700)).await?;
    tokio::fs::write(&file, &bytes).await?;
    #[cfg(unix)]
    tokio::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).await?;
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO media_cache(cache_key,source_url,proxy_profile_id,mime_type,etag,last_modified,byte_size,file_name,last_accessed_at,created_at) VALUES(?,?,?,?,?,?,?,?,?,?) ON CONFLICT(cache_key) DO UPDATE SET source_url=excluded.source_url,proxy_profile_id=excluded.proxy_profile_id,mime_type=excluded.mime_type,etag=excluded.etag,last_modified=excluded.last_modified,byte_size=excluded.byte_size,file_name=excluded.file_name,last_accessed_at=excluded.last_accessed_at")
        .bind(&key).bind(url).bind(resolved_profile_id).bind(&mime).bind(etag).bind(modified).bind(bytes.len() as i64).bind(file.file_name().and_then(|v| v.to_str()).unwrap_or_default()).bind(&now).bind(&now).execute(&state.db.pool).await?;
    associate_with_feed(state, &key, feed_id).await?;
    evict(state).await?;
    Ok((bytes, mime))
}

async fn associate_with_feed(state: &AppState, cache_key: &str, feed_id: &str) -> AppResult<()> {
    sqlx::query("INSERT OR IGNORE INTO media_cache_feeds(cache_key,feed_id) VALUES(?,?)")
        .bind(cache_key)
        .bind(feed_id)
        .execute(&state.db.pool)
        .await?;
    Ok(())
}

pub async fn prefetch(state: &AppState, url: &str, feed_id: &str) -> AppResult<()> {
    load_source(state, url, feed_id, false).await.map(|_| ())
}

pub fn favicon_candidates(
    icon_url: Option<&str>,
    site_url: Option<&str>,
    feed_url: &str,
) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(icon) = icon_url.and_then(valid_http_url) {
        candidates.push(icon);
    }
    if let Some(site) = site_url
        .and_then(valid_http_url)
        .or_else(|| valid_http_url(feed_url))
        && let Ok(url) = url::Url::parse(&site).and_then(|url| url.join("/favicon.ico"))
        && !candidates.contains(&url.to_string())
    {
        candidates.push(url.to_string());
    }
    candidates
}

fn valid_http_url(value: &str) -> Option<String> {
    url::Url::parse(value)
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https"))
        .map(|url| url.to_string())
}

async fn evict(state: &AppState) -> AppResult<()> {
    let limit: i64 = sqlx::query_scalar(
        "SELECT cast(value AS INTEGER) FROM settings WHERE key='media_cache_limit_bytes'",
    )
    .fetch_one(&state.db.pool)
    .await?;
    let mut total: i64 = sqlx::query_scalar("SELECT coalesce(sum(byte_size),0) FROM media_cache")
        .fetch_one(&state.db.pool)
        .await?;
    if total <= limit {
        return Ok(());
    }
    let rows = sqlx::query(
        "SELECT cache_key,file_name,byte_size FROM media_cache ORDER BY last_accessed_at ASC",
    )
    .fetch_all(&state.db.pool)
    .await?;
    for row in rows {
        if total <= limit {
            break;
        }
        let key: String = row.get("cache_key");
        let file_name: String = row.get("file_name");
        let size: i64 = row.get("byte_size");
        let _ = tokio::fs::remove_file(state.cache_dir.join(file_name)).await;
        sqlx::query("DELETE FROM media_cache WHERE cache_key=?")
            .bind(key)
            .execute(&state.db.pool)
            .await?;
        total -= size;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn favicon_candidates_prefer_published_icon_then_site_default() {
        assert_eq!(
            favicon_candidates(
                Some("https://cdn.example.com/icon.png"),
                Some("https://example.com/news/"),
                "https://example.com/feed.xml",
            ),
            vec![
                "https://cdn.example.com/icon.png",
                "https://example.com/favicon.ico",
            ]
        );
    }

    #[test]
    fn favicon_candidates_reject_non_http_urls() {
        assert!(favicon_candidates(Some("file:///secret"), None, "data:text/plain,no").is_empty());
    }

    #[tokio::test]
    async fn rejects_media_tokens_for_missing_feeds_before_fetching() {
        let directory = tempfile::tempdir().unwrap();
        let db = crate::db::Database::open(&directory.path().join("missing-feed.db"))
            .await
            .unwrap();
        let state = AppState {
            db,
            network: crate::network::Network::new(),
            ad_filter: crate::ad_filter::AdFilter::load(directory.path(), false),
            refresh_guard: std::sync::Arc::new(tokio::sync::Mutex::new(())),
            blocklist_wake: std::sync::Arc::new(tokio::sync::Notify::new()),
            cache_dir: directory.path().join("media-cache"),
        };

        let error = load_source(
            &state,
            "https://example.com/image.png",
            "missing-feed",
            true,
        )
        .await
        .unwrap_err();

        assert!(matches!(error, AppError::NotFound(ref value) if value == "feed"));
    }

    #[tokio::test]
    async fn rejects_unreferenced_article_media_before_fetching() {
        let directory = tempfile::tempdir().unwrap();
        let db = crate::db::Database::open(&directory.path().join("media.db"))
            .await
            .unwrap();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO feeds(id,proxy_profile_id,title,feed_url,created_at,updated_at) VALUES('feed','direct','Feed','https://example.com/feed.xml',?,?)")
            .bind(&now).bind(&now).execute(&db.pool).await.unwrap();
        let state = AppState {
            db,
            network: crate::network::Network::new(),
            ad_filter: crate::ad_filter::AdFilter::load(directory.path(), false),
            refresh_guard: std::sync::Arc::new(tokio::sync::Mutex::new(())),
            blocklist_wake: std::sync::Arc::new(tokio::sync::Notify::new()),
            cache_dir: directory.path().join("media-cache"),
        };
        let url = crate::content::media_url("https://example.com/image.png", "feed");
        let token = url.rsplit('/').next().unwrap();
        let error = load(&state, token).await.unwrap_err();
        assert!(matches!(error, AppError::NotFound(ref value) if value == "article media"));
    }

    #[test]
    fn extracts_every_token_the_sanitizer_emits() {
        let html = crate::content::sanitize_html(
            "<p>Text</p><img src='https://example.com/a.png'><video poster='/b.jpg'></video>",
            Some("https://example.com/post"),
            "feed",
        );
        let tokens = article_media_tokens(&html).collect::<Vec<_>>();
        let urls = tokens
            .iter()
            .map(|token| decode_media_token(token).unwrap().url)
            .collect::<Vec<_>>();
        assert_eq!(
            urls,
            vec!["https://example.com/a.png", "https://example.com/b.jpg"]
        );
        assert!(
            article_media_tokens("reader-media://localhost/\"")
                .next()
                .is_none()
        );
    }
}
