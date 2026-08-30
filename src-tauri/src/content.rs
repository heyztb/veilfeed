use std::{borrow::Cow, collections::HashSet, net::IpAddr};

use ammonia::Builder;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::{Host, Url};

use crate::error::{AppError, AppResult};

#[derive(Serialize)]
struct MediaToken<'a> {
    url: &'a str,
    feed_id: &'a str,
}

#[derive(Deserialize)]
pub struct DecodedMediaToken {
    pub url: String,
    pub feed_id: String,
    #[serde(default)]
    pub cache_only: bool,
}

pub fn validate_remote_url(value: &str) -> AppResult<Url> {
    let url = Url::parse(value)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::UnsafeTarget(
            "only HTTP and HTTPS URLs are allowed".into(),
        ));
    }
    let host = url
        .host()
        .ok_or_else(|| AppError::UnsafeTarget("URL has no hostname".into()))?;
    match host {
        Host::Domain(host)
            if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") =>
        {
            return Err(AppError::UnsafeTarget(host.into()));
        }
        Host::Ipv4(ip) if !is_public_ip(IpAddr::V4(ip)) => {
            return Err(AppError::UnsafeTarget(ip.to_string()));
        }
        Host::Ipv6(ip) if !is_public_ip(IpAddr::V6(ip)) => {
            return Err(AppError::UnsafeTarget(ip.to_string()));
        }
        _ => {}
    }
    Ok(url)
}

pub fn is_public_ip(ip: IpAddr) -> bool {
    // HTTP fetches only need globally reachable unicast destinations. Keep
    // special-purpose, documentation, multicast, and transition ranges out.
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, d] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && b == 168)
                || (a == 192 && b == 0 && c == 0 && d != 9 && d != 10)
                || (a == 192 && b == 0 && c == 2)
                || (a == 198 && (b == 18 || b == 19))
                || (a == 198 && b == 51 && c == 100)
                || (a == 203 && b == 0 && c == 113)
                || a >= 224)
        }
        IpAddr::V6(ip) => {
            let [a, b, c, _, _, _, _, _] = ip.segments();
            let value = u128::from_be_bytes(ip.octets());
            let ietf_exception = value == 0x2001_0001_0000_0000_0000_0000_0000_0001
                || value == 0x2001_0001_0000_0000_0000_0000_0000_0002
                || (a == 0x2001 && b == 3)
                || (a == 0x2001 && b == 4 && c == 0x112)
                || (a == 0x2001 && (0x20..=0x3f).contains(&b));
            a & 0xe000 == 0x2000
                && !(a == 0x2001 && b < 0x200 && !ietf_exception)
                && !(a == 0x2001 && b == 0x0db8)
                && a != 0x2002
                && a & 0xfff0 != 0x3ff0
        }
    }
}

pub fn media_url(source: &str, feed_id: &str) -> String {
    let token = serde_json::to_vec(&MediaToken {
        url: source,
        feed_id,
    })
    .expect("serializable token");
    format!("reader-media://localhost/{}", URL_SAFE_NO_PAD.encode(token))
}

pub fn decode_media_token(token: &str) -> AppResult<DecodedMediaToken> {
    let bytes = URL_SAFE_NO_PAD
        .decode(token)
        .map_err(|_| AppError::InvalidInput("invalid media token".into()))?;
    serde_json::from_slice(&bytes).map_err(|_| AppError::InvalidInput("invalid media token".into()))
}

pub fn sanitize_html(html: &str, base_url: Option<&str>, feed_id: &str) -> String {
    let base = base_url.and_then(|value| Url::parse(value).ok());
    let feed_id = feed_id.to_owned();
    let mut builder = Builder::default();
    let tags: HashSet<&str> = [
        "a",
        "abbr",
        "article",
        "aside",
        "audio",
        "b",
        "blockquote",
        "br",
        "caption",
        "cite",
        "code",
        "col",
        "colgroup",
        "dd",
        "del",
        "details",
        "dfn",
        "div",
        "dl",
        "dt",
        "em",
        "figcaption",
        "figure",
        "footer",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "header",
        "hr",
        "i",
        "img",
        "li",
        "main",
        "mark",
        "ol",
        "p",
        "picture",
        "pre",
        "q",
        "s",
        "section",
        "small",
        "source",
        "span",
        "strong",
        "sub",
        "summary",
        "sup",
        "table",
        "tbody",
        "td",
        "tfoot",
        "th",
        "thead",
        "time",
        "tr",
        "u",
        "ul",
        "video",
    ]
    .into_iter()
    .collect();
    let schemes: HashSet<&str> = ["http", "https", "mailto"].into_iter().collect();
    let generic_attributes: HashSet<&str> = ["title", "lang"].into_iter().collect();
    let image_attributes: HashSet<&str> = ["src", "alt", "width", "height", "loading"]
        .into_iter()
        .collect();
    let video_attributes: HashSet<&str> = ["src", "poster", "controls", "width", "height"]
        .into_iter()
        .collect();
    let audio_attributes: HashSet<&str> = ["src", "controls"].into_iter().collect();
    let source_attributes: HashSet<&str> = ["src", "type"].into_iter().collect();
    builder
        .tags(tags)
        .url_schemes(schemes)
        .add_generic_attributes(generic_attributes)
        .add_tag_attributes("img", image_attributes)
        .add_tag_attributes("video", video_attributes)
        .add_tag_attributes("audio", audio_attributes)
        .add_tag_attributes("source", source_attributes)
        .attribute_filter(move |tag, attr, value| {
            if attr.starts_with("on") || attr == "style" || attr == "srcset" {
                return None;
            }
            if matches!(attr, "src" | "poster")
                && matches!(tag, "img" | "video" | "audio" | "source")
            {
                let resolved = base
                    .as_ref()
                    .and_then(|root| root.join(value).ok())
                    .or_else(|| Url::parse(value).ok());
                return resolved
                    .filter(|url| matches!(url.scheme(), "http" | "https"))
                    .map(|url| Cow::Owned(media_url(url.as_str(), &feed_id)));
            }
            if tag == "a" && attr == "href" {
                let resolved = base
                    .as_ref()
                    .and_then(|root| root.join(value).ok())
                    .or_else(|| Url::parse(value).ok());
                return resolved
                    .filter(|url| matches!(url.scheme(), "http" | "https" | "mailto"))
                    .map(|url| Cow::Owned(url.to_string()));
            }
            Some(Cow::Borrowed(value))
        });
    builder.clean(html).to_string()
}

pub fn plain_text(html: &str) -> String {
    scraper::Html::parse_fragment(html)
        .root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn article_key(guid: Option<&str>, url: Option<&str>, title: &str, body: &str) -> String {
    if let Some(guid) = guid.filter(|value| !value.trim().is_empty()) {
        return format!("guid:{}", guid.trim());
    }
    if let Some(url) = url.filter(|value| !value.trim().is_empty()) {
        return format!("url:{}", url.trim());
    }
    let mut hash = Sha256::new();
    hash.update(title.as_bytes());
    hash.update([0]);
    hash.update(body.as_bytes());
    format!("hash:{}", hex::encode(hash.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_private_targets() {
        assert!(validate_remote_url("http://127.0.0.1/secret").is_err());
        assert!(validate_remote_url("http://2130706433/secret").is_err());
        assert!(validate_remote_url("http://100.64.0.1/secret").is_err());
        assert!(validate_remote_url("http://[::ffff:127.0.0.1]/secret").is_err());
        assert!(validate_remote_url("https://example.com/feed").is_ok());
        assert!(validate_remote_url("https://1.1.1.1/feed").is_ok());
    }

    #[test]
    fn removes_scripts_and_rewrites_media() {
        let html = sanitize_html(
            "<script>alert(1)</script><img onerror='x' src='/a.jpg'><iframe src='https://x'></iframe>",
            Some("https://example.com/post"),
            "feed",
        );
        assert!(!html.contains("script"));
        assert!(!html.contains("iframe"));
        assert!(!html.contains("onerror"));
        assert!(html.contains("reader-media://localhost/"));
    }
}
