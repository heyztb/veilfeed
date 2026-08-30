use std::{io, net::SocketAddr, sync::Arc, time::Duration};

use crate::{
    ad_filter::AdFilter,
    content::{article_key, is_public_ip, plain_text, sanitize_html, validate_remote_url},
    error::{AppError, AppResult},
    models::ProxyProfile,
};
use chrono::{DateTime, Utc};
use feed_rs::model::{Entry, Feed};
use futures::StreamExt;
use reqwest::{
    Client,
    dns::{Addrs, Name, Resolve, Resolving},
    header,
};
use tokio::sync::Semaphore;

const MAX_DOCUMENT_BYTES: usize = 10 * 1024 * 1024;
const MAX_MEDIA_BYTES: usize = 100 * 1024 * 1024;
const MAX_REDIRECTS: usize = 8;
const ARTICLE_COMPLETENESS_MIN_TEXT: usize = 600;
const ARTICLE_COMPLETENESS_MIN_MISSING: usize = 400;
const ARTICLE_ACCEPT: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";

#[derive(Debug)]
struct PublicDnsResolver;

impl Resolve for PublicDnsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let addresses = tokio::net::lookup_host((host.as_str(), 0))
                .await?
                .collect::<Vec<_>>();
            let addresses = public_addresses(&host, addresses)?;
            Ok(Box::new(addresses.into_iter()) as Addrs)
        })
    }
}

fn public_addresses(host: &str, addresses: Vec<SocketAddr>) -> io::Result<Vec<SocketAddr>> {
    if addresses.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("DNS returned no addresses for {host}"),
        ));
    }
    if let Some(address) = addresses.iter().find(|address| !is_public_ip(address.ip())) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("unsafe DNS target for {host}: {}", address.ip()),
        ));
    }
    Ok(addresses)
}

fn redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        if let Err(error) = validate_redirect(attempt.url().as_str(), attempt.previous().len()) {
            return attempt.error(error);
        }
        attempt.follow()
    })
}

fn validate_redirect(url: &str, previous_hops: usize) -> AppResult<()> {
    if previous_hops >= MAX_REDIRECTS {
        return Err(AppError::UnsafeTarget("too many redirects".into()));
    }
    validate_remote_url(url).map(|_| ())
}

#[derive(Debug, Clone)]
pub struct ParsedArticle {
    pub source_key: String,
    pub guid: Option<String>,
    pub canonical_url: Option<String>,
    pub title: String,
    pub author: Option<String>,
    pub published_at: Option<String>,
    pub summary_html: Option<String>,
    pub content_html: String,
    pub plain_text: String,
}

#[derive(Debug, Clone)]
pub struct ParsedFeed {
    pub title: String,
    pub site_url: Option<String>,
    pub icon_url: Option<String>,
    pub description: Option<String>,
    pub articles: Vec<ParsedArticle>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub filter_revision: Option<String>,
}

pub struct TextDownload {
    pub body: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Clone)]
pub struct Network {
    slots: Arc<Semaphore>,
}

impl Network {
    pub fn new() -> Self {
        Self {
            slots: Arc::new(Semaphore::new(6)),
        }
    }

    fn client(&self, profile: Option<&ProxyProfile>) -> AppResult<Client> {
        let mut builder = Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(45))
            .redirect(redirect_policy())
            .user_agent(concat!(
                "Veilfeed/",
                env!("CARGO_PKG_VERSION"),
                " (+local feed reader)"
            ));

        // Direct requests are resolved to the exact addresses reqwest will use,
        // closing DNS-rebinding races. Proxy profiles retain their configured
        // local/remote DNS behavior; URL and redirect checks still apply.
        if profile.is_none_or(|profile| profile.kind == "direct") {
            builder = builder.dns_resolver(Arc::new(PublicDnsResolver));
        }

        if let Some(profile) = profile.filter(|p| p.kind != "direct") {
            let mut endpoint = profile
                .endpoint
                .clone()
                .ok_or_else(|| AppError::InvalidInput("proxy endpoint is required".into()))?;
            if profile.kind == "socks5" {
                endpoint = normalize_socks_dns(&endpoint, profile.remote_dns);
            }
            let mut proxy = reqwest::Proxy::all(&endpoint)
                .map_err(|error| AppError::InvalidInput(error.to_string()))?;
            if let Some(username) = profile.username.as_deref() {
                let service = format!("app.veilfeed.reader.proxy.{}", profile.id);
                let password = keyring::Entry::new(&service, username)
                    .map_err(|error| AppError::Credential(error.to_string()))?
                    .get_password()
                    .map_err(|error| AppError::Credential(error.to_string()))?;
                proxy = proxy.basic_auth(username, &password);
            }
            builder = builder.proxy(proxy);
        }
        builder.build().map_err(AppError::Network)
    }

    async fn bytes_with_limit(
        &self,
        url: &str,
        profile: Option<&ProxyProfile>,
        validators: Option<(&str, &str)>,
        max_bytes: usize,
        article_navigation: bool,
    ) -> AppResult<(Vec<u8>, header::HeaderMap, reqwest::StatusCode)> {
        validate_remote_url(url)?;
        let _permit = self.slots.acquire().await.expect("semaphore remains open");
        let client = self.client(profile)?;
        let mut request = client.get(url);
        if article_navigation {
            request = request.headers(article_navigation_headers());
        }
        if let Some((etag, modified)) = validators {
            if !etag.is_empty() {
                request = request.header(header::IF_NONE_MATCH, etag);
            }
            if !modified.is_empty() {
                request = request.header(header::IF_MODIFIED_SINCE, modified);
            }
        }
        let response = request.send().await.map_err(|error| {
            if profile.is_some_and(|p| p.kind != "direct") {
                AppError::ProxyUnavailable(error.to_string())
            } else {
                AppError::Network(error)
            }
        })?;
        let status = response.status();
        let headers = response.headers().clone();
        if status == reqwest::StatusCode::NOT_MODIFIED {
            return Ok((vec![], headers, status));
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::InvalidFeed(
                "publisher rate limited the request".into(),
            ));
        }
        let response = response.error_for_status()?;
        let mut stream = response.bytes_stream();
        let mut body = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if body.len() + chunk.len() > max_bytes {
                return Err(AppError::ContentTooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        Ok((body, headers, status))
    }

    async fn bytes(
        &self,
        url: &str,
        profile: Option<&ProxyProfile>,
        validators: Option<(&str, &str)>,
    ) -> AppResult<(Vec<u8>, header::HeaderMap, reqwest::StatusCode)> {
        self.bytes_with_limit(url, profile, validators, MAX_DOCUMENT_BYTES, false)
            .await
    }

    async fn article_bytes(
        &self,
        url: &str,
        profile: Option<&ProxyProfile>,
    ) -> AppResult<(Vec<u8>, header::HeaderMap, reqwest::StatusCode)> {
        self.bytes_with_limit(url, profile, None, MAX_DOCUMENT_BYTES, true)
            .await
    }

    pub async fn download_text_limited(
        &self,
        url: &str,
        profile: Option<&ProxyProfile>,
        max_bytes: usize,
    ) -> AppResult<String> {
        let (bytes, _, _) = self
            .bytes_with_limit(url, profile, None, max_bytes, false)
            .await?;
        String::from_utf8(bytes)
            .map_err(|_| AppError::InvalidInput("download is not UTF-8 text".into()))
    }

    pub async fn fetch_feed(
        &self,
        url: &str,
        feed_id: &str,
        profile: Option<&ProxyProfile>,
        validators: Option<(&str, &str)>,
        ad_filter: &AdFilter,
        block_ads: bool,
    ) -> AppResult<Option<ParsedFeed>> {
        let (bytes, headers, status) = self.bytes(url, profile, validators).await?;
        if status == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(None);
        }
        let feed = feed_rs::parser::parse(bytes.as_slice())
            .map_err(|error| AppError::InvalidFeed(error.to_string()))?;
        Ok(Some(convert_feed(
            feed, feed_id, headers, ad_filter, block_ads,
        )))
    }

    pub async fn discover(
        &self,
        input: &str,
        profile: Option<&ProxyProfile>,
    ) -> AppResult<Vec<String>> {
        let url = validate_remote_url(input)?;
        let (bytes, _, _) = self.bytes(url.as_str(), profile, None).await?;
        if feed_rs::parser::parse(bytes.as_slice()).is_ok() {
            return Ok(vec![url.to_string()]);
        }
        let html = String::from_utf8_lossy(&bytes);
        let document = scraper::Html::parse_document(&html);
        let selector = scraper::Selector::parse("link[rel~='alternate']").expect("static selector");
        let mut found = Vec::new();
        for link in document.select(&selector) {
            let value = link.value();
            let kind = value.attr("type").unwrap_or_default();
            if matches!(
                kind,
                "application/rss+xml" | "application/atom+xml" | "application/feed+json"
            ) && let Some(href) = value.attr("href").and_then(|href| url.join(href).ok())
            {
                found.push(href.to_string());
            }
        }
        found.sort();
        found.dedup();
        if found.is_empty() {
            Err(AppError::InvalidFeed(
                "no RSS, Atom, or JSON Feed link was found".into(),
            ))
        } else {
            Ok(found)
        }
    }

    pub async fn test_connection(&self, profile: Option<&ProxyProfile>) -> AppResult<()> {
        let _permit = self.slots.acquire().await.expect("semaphore remains open");
        self.client(profile)?
            .get("https://example.com/")
            .send()
            .await
            .map_err(|error| {
                if profile.is_some_and(|p| p.kind != "direct") {
                    AppError::ProxyUnavailable(error.to_string())
                } else {
                    AppError::Network(error)
                }
            })?
            .error_for_status()?;
        Ok(())
    }

    pub async fn download_text(
        &self,
        url: &str,
        profile: Option<&ProxyProfile>,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> AppResult<TextDownload> {
        let validators = etag
            .or(last_modified)
            .map(|_| (etag.unwrap_or_default(), last_modified.unwrap_or_default()));
        let (bytes, headers, status) = self.bytes(url, profile, validators).await?;
        let body = if status == reqwest::StatusCode::NOT_MODIFIED {
            None
        } else {
            Some(
                String::from_utf8(bytes)
                    .map_err(|_| AppError::InvalidInput("download is not UTF-8 text".into()))?,
            )
        };
        Ok(TextDownload {
            body,
            etag: headers
                .get(header::ETAG)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
                .or_else(|| etag.map(str::to_owned)),
            last_modified: headers
                .get(header::LAST_MODIFIED)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
                .or_else(|| last_modified.map(str::to_owned)),
        })
    }

    pub async fn extract_full_text(
        &self,
        url: &str,
        feed_id: &str,
        profile: Option<&ProxyProfile>,
        ad_filter: &AdFilter,
        block_ads: bool,
    ) -> AppResult<String> {
        let (bytes, _, _) = self.article_bytes(url, profile).await?;
        let html = String::from_utf8_lossy(&bytes);
        let prepared = if block_ads {
            ad_filter.filter_document(html.as_ref(), url)
        } else {
            html.into_owned()
        };
        let mut readability = dom_smoothie::Readability::new(prepared.as_ref(), Some(url), None)
            .map_err(|error| AppError::InvalidFeed(error.to_string()))?;
        let article = readability
            .parse()
            .map_err(|error| AppError::InvalidFeed(error.to_string()))?;
        let content = prefer_complete_article_body(prepared.as_ref(), &article.content);
        let extracted = if block_ads {
            ad_filter.filter_fragment(&content, Some(url))
        } else {
            content
        };
        Ok(sanitize_html(&extracted, Some(url), feed_id))
    }

    pub async fn fetch_media(
        &self,
        url: &str,
        profile: Option<&ProxyProfile>,
    ) -> AppResult<(Vec<u8>, String, Option<String>, Option<String>)> {
        validate_remote_url(url)?;
        let _permit = self.slots.acquire().await.expect("semaphore remains open");
        let response = self
            .client(profile)?
            .get(url)
            .send()
            .await
            .map_err(|error| {
                if profile.is_some_and(|p| p.kind != "direct") {
                    AppError::ProxyUnavailable(error.to_string())
                } else {
                    AppError::Network(error)
                }
            })?
            .error_for_status()?;
        let headers = response.headers().clone();
        let mime = headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("application/octet-stream")
            .split(';')
            .next()
            .unwrap_or("application/octet-stream")
            .to_owned();
        if !(mime.starts_with("image/") || mime.starts_with("audio/") || mime.starts_with("video/"))
        {
            return Err(AppError::InvalidInput(
                "remote resource is not supported media".into(),
            ));
        }
        if headers
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok())
            .is_some_and(|size| size > MAX_MEDIA_BYTES)
        {
            return Err(AppError::ContentTooLarge);
        }
        let etag = headers
            .get(header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let modified = headers
            .get(header::LAST_MODIFIED)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let mut stream = response.bytes_stream();
        let mut body = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if body.len() + chunk.len() > MAX_MEDIA_BYTES {
                return Err(AppError::ContentTooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        Ok((body, mime, etag, modified))
    }
}

fn article_navigation_headers() -> header::HeaderMap {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static(concat!(
            "Mozilla/5.0 (compatible; Veilfeed/",
            env!("CARGO_PKG_VERSION"),
            "; +local feed reader)"
        )),
    );
    headers.insert(
        header::ACCEPT,
        header::HeaderValue::from_static(ARTICLE_ACCEPT),
    );
    headers.insert(
        header::ACCEPT_LANGUAGE,
        header::HeaderValue::from_static("en-US,en;q=0.5"),
    );
    headers
}

fn normalize_socks_dns(endpoint: &str, remote_dns: bool) -> String {
    match remote_dns {
        true if endpoint.starts_with("socks5://") => {
            endpoint.replacen("socks5://", "socks5h://", 1)
        }
        false if endpoint.starts_with("socks5h://") => {
            endpoint.replacen("socks5h://", "socks5://", 1)
        }
        _ => endpoint.to_owned(),
    }
}

fn prefer_complete_article_body(document_html: &str, extracted_html: &str) -> String {
    // Some publishers split a story into repeated body containers around ad slots.
    // Readability can score just one of those containers, so compare its result with
    // common semantic body containers and use the latter only for a clear truncation.
    const BODY_SELECTORS: [&str; 6] = [
        "[itemprop~='articleBody']",
        "[class~='article-body']",
        "[class~='article-content']",
        "[class~='post-content']",
        "[class~='entry-content']",
        "[class~='story-content']",
    ];

    let document = scraper::Html::parse_document(document_html);
    let article_selector = scraper::Selector::parse("article").expect("static selector");
    let roots = {
        let articles = document.select(&article_selector).collect::<Vec<_>>();
        if articles.is_empty() {
            vec![document.root_element()]
        } else {
            articles
        }
    };
    let paragraph_selector = scraper::Selector::parse("p").expect("static selector");
    let extracted_text = paragraph_text_len(extracted_html, &paragraph_selector);
    let mut best: Option<(usize, String)> = None;

    for root in roots {
        for selector in BODY_SELECTORS {
            let selector = scraper::Selector::parse(selector).expect("static selector");
            let body = root
                .select(&selector)
                .map(|element| element.inner_html())
                .collect::<Vec<_>>()
                .join("\n");
            let text_len = paragraph_text_len(&body, &paragraph_selector);
            if text_len > best.as_ref().map_or(0, |(best_len, _)| *best_len) {
                best = Some((text_len, body));
            }
        }
    }

    let Some((body_text, body_html)) = best else {
        return extracted_html.to_owned();
    };
    let clearly_truncated = body_text >= ARTICLE_COMPLETENESS_MIN_TEXT
        && body_text.saturating_sub(extracted_text) >= ARTICLE_COMPLETENESS_MIN_MISSING
        && extracted_text.saturating_mul(10) < body_text.saturating_mul(7);
    if clearly_truncated {
        body_html
    } else {
        extracted_html.to_owned()
    }
}

fn paragraph_text_len(html: &str, paragraph_selector: &scraper::Selector) -> usize {
    scraper::Html::parse_fragment(html)
        .select(paragraph_selector)
        .flat_map(|paragraph| paragraph.text())
        .flat_map(str::split_whitespace)
        .map(str::len)
        .sum()
}

fn convert_feed(
    feed: Feed,
    feed_id: &str,
    headers: header::HeaderMap,
    ad_filter: &AdFilter,
    block_ads: bool,
) -> ParsedFeed {
    let title = feed
        .title
        .map(|value| value.content)
        .unwrap_or_else(|| "Untitled feed".into());
    let site_url = feed
        .links
        .iter()
        .find(|link| link.rel.as_deref().is_none_or(|rel| rel == "alternate"))
        .map(|link| link.href.clone());
    let icon_url = feed
        .icon
        .or(feed.logo)
        .map(|image| image.uri)
        .and_then(|icon| {
            site_url
                .as_deref()
                .and_then(|site| url::Url::parse(site).ok())
                .and_then(|site| site.join(&icon).ok())
                .map(|url| url.to_string())
                .or_else(|| url::Url::parse(&icon).ok().map(|url| url.to_string()))
        });
    let description = feed.description.map(|value| value.content);
    let articles = feed
        .entries
        .into_iter()
        .map(|entry| convert_entry(entry, feed_id, ad_filter, block_ads))
        .collect();
    ParsedFeed {
        title,
        site_url,
        icon_url,
        description,
        articles,
        etag: headers
            .get(header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned),
        last_modified: headers
            .get(header::LAST_MODIFIED)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned),
        filter_revision: block_ads.then(|| ad_filter.revision()),
    }
}

fn convert_entry(
    entry: Entry,
    feed_id: &str,
    ad_filter: &AdFilter,
    block_ads: bool,
) -> ParsedArticle {
    let canonical_url = entry
        .links
        .iter()
        .find(|link| link.rel.as_deref().is_none_or(|rel| rel == "alternate"))
        .map(|link| link.href.clone());
    let raw_content = entry
        .content
        .as_ref()
        .and_then(|content| content.body.clone())
        .or_else(|| {
            entry
                .summary
                .as_ref()
                .map(|summary| summary.content.clone())
        })
        .unwrap_or_default();
    let title = entry
        .title
        .map(|value| value.content)
        .unwrap_or_else(|| "Untitled article".into());
    let raw_content = if block_ads {
        ad_filter.filter_fragment(&raw_content, canonical_url.as_deref())
    } else {
        raw_content
    };
    let content_html = sanitize_html(&raw_content, canonical_url.as_deref(), feed_id);
    let text = plain_text(&content_html);
    let guid = (!entry.id.is_empty()).then_some(entry.id);
    let summary_html = entry.summary.map(|summary| {
        let summary = if block_ads {
            ad_filter.filter_fragment(&summary.content, canonical_url.as_deref())
        } else {
            summary.content
        };
        sanitize_html(&summary, canonical_url.as_deref(), feed_id)
    });
    ParsedArticle {
        source_key: article_key(guid.as_deref(), canonical_url.as_deref(), &title, &text),
        guid,
        canonical_url,
        title,
        author: entry.authors.first().map(|person| person.name.clone()),
        published_at: entry
            .published
            .or(entry.updated)
            .map(|date: DateTime<Utc>| date.to_rfc3339()),
        summary_html,
        content_html,
        plain_text: text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn rejects_dns_answers_containing_private_addresses() {
        let addresses = vec![
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)), 0),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 0),
        ];
        assert!(public_addresses("example.test", addresses).is_err());
    }

    #[test]
    fn accepts_public_dns_answers() {
        let addresses = vec![SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)),
            0,
        )];
        assert_eq!(
            public_addresses("example.test", addresses.clone()).unwrap(),
            addresses
        );
    }

    #[tokio::test]
    async fn resolver_rejects_localhost() {
        let result = PublicDnsResolver
            .resolve("localhost".parse().unwrap())
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn redirect_policy_rejects_private_targets() {
        assert!(validate_redirect("http://127.0.0.1/admin", 1).is_err());
        assert!(validate_redirect("https://example.com/next", 1).is_ok());
        assert!(validate_redirect("https://example.com/next", MAX_REDIRECTS).is_err());
    }

    #[test]
    fn socks_dns_scheme_follows_profile_setting() {
        assert_eq!(
            normalize_socks_dns("socks5://127.0.0.1:9050", true),
            "socks5h://127.0.0.1:9050"
        );
        assert_eq!(
            normalize_socks_dns("socks5h://127.0.0.1:9050", false),
            "socks5://127.0.0.1:9050"
        );
    }

    #[test]
    fn full_article_requests_use_compatible_navigation_headers() {
        let headers = article_navigation_headers();

        assert_eq!(headers[header::ACCEPT], ARTICLE_ACCEPT);
        assert_eq!(headers[header::ACCEPT_LANGUAGE], "en-US,en;q=0.5");
        assert_eq!(
            headers[header::USER_AGENT],
            concat!(
                "Mozilla/5.0 (compatible; Veilfeed/",
                env!("CARGO_PKG_VERSION"),
                "; +local feed reader)"
            )
        );
    }

    #[test]
    fn restores_article_body_split_around_an_ad_slot() {
        let first = format!("<p>Opening section {}</p>", "alpha ".repeat(120));
        let second = format!("<p>Closing section {}</p>", "omega ".repeat(120));
        let document = format!(
            "<article><div class='post-content'>{first}</div><div class='ad-wrapper'></div><div class='post-content'>{second}</div></article>"
        );

        let output = prefer_complete_article_body(&document, &second);

        assert!(output.contains("Opening section"));
        assert!(output.contains("Closing section"));
    }

    #[test]
    fn keeps_readability_result_when_it_contains_the_article_body() {
        let body = format!("<p>Complete story {}</p>", "article ".repeat(120));
        let document = format!(
            "<article><div class='post-content'>{body}</div><div class='author'><p>Short author biography.</p></div></article>"
        );

        assert_eq!(prefer_complete_article_body(&document, &body), body);
    }
}
