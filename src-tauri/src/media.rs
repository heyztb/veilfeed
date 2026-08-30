use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::Row;
use tauri::http::{Response, StatusCode, header};

use crate::{
    AppState,
    content::{decode_media_token, validate_remote_url},
    error::{AppError, AppResult},
};

pub async fn serve(state: AppState, token: &str, range: Option<&str>) -> Response<Vec<u8>> {
    match load(&state, token).await {
        Ok((bytes, mime)) => ranged_response(bytes, &mime, range),
        Err(error) => Response::builder()
            .status(match error {
                AppError::NotFound(_) => StatusCode::NOT_FOUND,
                AppError::UnsafeTarget(_) | AppError::InvalidInput(_) => StatusCode::BAD_REQUEST,
                _ => StatusCode::BAD_GATEWAY,
            })
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-store")
            .body(error.to_string().into_bytes())
            .unwrap(),
    }
}

async fn load(state: &AppState, token: &str) -> AppResult<(Vec<u8>, String)> {
    let decoded = decode_media_token(token)?;
    load_source(state, &decoded.url, &decoded.feed_id, decoded.cache_only).await
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
    tokio::fs::write(&file, &bytes).await?;
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

fn ranged_response(bytes: Vec<u8>, mime: &str, requested: Option<&str>) -> Response<Vec<u8>> {
    let len = bytes.len();
    let range = requested.and_then(|value| parse_range(value, len));
    let (status, body, content_range) = match range {
        Some(Ok((start, end))) => (
            StatusCode::PARTIAL_CONTENT,
            bytes[start..=end].to_vec(),
            Some(format!("bytes {start}-{end}/{len}")),
        ),
        Some(Err(())) => (
            StatusCode::RANGE_NOT_SATISFIABLE,
            Vec::new(),
            Some(format!("bytes */{len}")),
        ),
        None => (StatusCode::OK, bytes, None),
    };
    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "private, max-age=86400")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::CONTENT_LENGTH, body.len().to_string());
    if let Some(content_range) = content_range {
        builder = builder.header(header::CONTENT_RANGE, content_range);
    }
    builder.body(body).unwrap()
}

/// Parses a single byte range. `None` means the header is malformed or uses an
/// unsupported range unit and should be ignored; `Err` is a valid but
/// unsatisfiable byte range.
fn parse_range(value: &str, len: usize) -> Option<Result<(usize, usize), ()>> {
    let spec = value.strip_prefix("bytes=")?;
    if spec.contains(',') {
        return None;
    }
    let (first, last) = spec.split_once('-')?;
    if first.is_empty() {
        let suffix = last.parse::<usize>().ok()?;
        if suffix == 0 || len == 0 {
            return Some(Err(()));
        }
        return Some(Ok((len.saturating_sub(suffix), len - 1)));
    }

    let start = first.parse::<usize>().ok()?;
    if start >= len {
        return Some(Err(()));
    }
    let end = if last.is_empty() {
        len - 1
    } else {
        last.parse::<usize>().ok()?.min(len - 1)
    };
    if start > end {
        return Some(Err(()));
    }
    Some(Ok((start, end)))
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

    #[test]
    fn serves_ranges() {
        let r = ranged_response(vec![0, 1, 2, 3, 4], "video/mp4", Some("bytes=1-3"));
        assert_eq!(r.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(r.body(), &vec![1, 2, 3]);
        assert_eq!(r.headers()[header::CONTENT_RANGE], "bytes 1-3/5");
    }

    #[test]
    fn serves_open_ended_and_suffix_ranges() {
        let open = ranged_response(vec![0, 1, 2, 3, 4], "video/mp4", Some("bytes=3-"));
        assert_eq!(open.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(open.body(), &vec![3, 4]);

        let suffix = ranged_response(vec![0, 1, 2, 3, 4], "video/mp4", Some("bytes=-2"));
        assert_eq!(suffix.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(suffix.body(), &vec![3, 4]);
        assert_eq!(suffix.headers()[header::CONTENT_RANGE], "bytes 3-4/5");
    }

    #[test]
    fn returns_416_for_unsatisfiable_ranges() {
        for range in ["bytes=5-", "bytes=4-2", "bytes=-0"] {
            let r = ranged_response(vec![0, 1, 2, 3, 4], "video/mp4", Some(range));
            assert_eq!(r.status(), StatusCode::RANGE_NOT_SATISFIABLE, "{range}");
            assert!(r.body().is_empty());
            assert_eq!(r.headers()[header::CONTENT_RANGE], "bytes */5");
        }

        let empty = ranged_response(vec![], "video/mp4", Some("bytes=0-"));
        assert_eq!(empty.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(empty.headers()[header::CONTENT_RANGE], "bytes */0");
    }

    #[test]
    fn ignores_malformed_or_unsupported_ranges() {
        for range in ["items=1-2", "bytes=abc-2", "bytes=1-2,4-5"] {
            let r = ranged_response(vec![0, 1, 2, 3, 4], "video/mp4", Some(range));
            assert_eq!(r.status(), StatusCode::OK, "{range}");
            assert_eq!(r.body(), &vec![0, 1, 2, 3, 4]);
            assert!(!r.headers().contains_key(header::CONTENT_RANGE));
        }
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
            ad_filter: crate::ad_filter::AdFilter::load(directory.path()),
            refresh_guard: std::sync::Arc::new(tokio::sync::Mutex::new(())),
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
}
