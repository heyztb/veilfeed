use crate::AppState;
use tauri::http::{Response, StatusCode, header};
use veilfeed_core::error::AppError;
pub async fn serve(state: AppState, token: &str, range: Option<&str>) -> Response<Vec<u8>> {
    match veilfeed_core::media::load(&state, token).await {
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
}
