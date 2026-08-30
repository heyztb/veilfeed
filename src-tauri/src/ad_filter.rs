use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use adblock::{Engine, FilterSet, lists::ParseOptions, request::Request};
use dom_query::{Document, NodeRef, Selection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

use crate::{
    content::decode_media_token,
    error::{AppError, AppResult},
    models::ProxyProfile,
    network::Network,
};

const POLICY_VERSION: &str = "2";
const FALLBACK_EASYLIST: &str = include_str!("../resources/blocklists/easylist.txt");
const FALLBACK_EASYPRIVACY: &str = include_str!("../resources/blocklists/easyprivacy.txt");
const VEILFEED_RULES: &str =
    "[Adblock Plus 2.0]\n! Veilfeed-owned article cleanup rules\n||hubs.li^$document\n";
const EASYLIST_URL: &str = "https://easylist.to/easylist/easylist.txt";
const EASYPRIVACY_URL: &str = "https://easylist.to/easylist/easyprivacy.txt";
const CACHE_FILE: &str = "lists.json";

#[derive(Clone, Default, Deserialize, Serialize)]
struct CachedLists {
    easylist: String,
    easyprivacy: String,
    easylist_etag: Option<String>,
    easylist_last_modified: Option<String>,
    easyprivacy_etag: Option<String>,
    easyprivacy_last_modified: Option<String>,
    checked_at: Option<String>,
}

struct ActiveFilter {
    engine: Engine,
    revision: String,
}

#[derive(Clone)]
pub struct AdFilter {
    active: Arc<RwLock<ActiveFilter>>,
    list_dir: PathBuf,
}

impl AdFilter {
    pub fn load(data_dir: &Path) -> Self {
        let list_dir = data_dir.join("filter-lists");
        let cached = read_cache(&list_dir).filter(|cached| {
            looks_like_filter_list(&cached.easylist, 1000)
                && looks_like_filter_list(&cached.easyprivacy, 1000)
        });
        let (easylist, easyprivacy) = cached
            .map(|cached| (cached.easylist, cached.easyprivacy))
            .unwrap_or_else(|| {
                (
                    FALLBACK_EASYLIST.to_owned(),
                    FALLBACK_EASYPRIVACY.to_owned(),
                )
            });
        let active = build_active(easylist, easyprivacy);
        Self {
            active: Arc::new(RwLock::new(active)),
            list_dir,
        }
    }

    #[cfg(test)]
    fn from_rules(rules: &str) -> Self {
        let directory = std::env::temp_dir().join("veilfeed-ad-filter-tests");
        Self {
            active: Arc::new(RwLock::new(build_active(rules.to_owned(), String::new()))),
            list_dir: directory,
        }
    }

    pub fn revision(&self) -> String {
        self.active
            .read()
            .expect("ad filter lock poisoned")
            .revision
            .clone()
    }

    pub async fn refresh(
        &self,
        network: &Network,
        profile: Option<&ProxyProfile>,
    ) -> AppResult<()> {
        let cached = read_cache(&self.list_dir).unwrap_or_else(|| CachedLists {
            easylist: FALLBACK_EASYLIST.to_owned(),
            easyprivacy: FALLBACK_EASYPRIVACY.to_owned(),
            ..CachedLists::default()
        });
        if cached.checked_at.as_deref().is_some_and(|checked_at| {
            chrono::DateTime::parse_from_rfc3339(checked_at)
                .ok()
                .is_some_and(|checked_at| {
                    chrono::Utc::now().signed_duration_since(checked_at.with_timezone(&chrono::Utc))
                        < chrono::Duration::hours(24)
                })
        }) {
            return Ok(());
        }
        let (easylist_result, easyprivacy_result) = tokio::try_join!(
            network.download_text(
                EASYLIST_URL,
                profile,
                cached.easylist_etag.as_deref(),
                cached.easylist_last_modified.as_deref(),
            ),
            network.download_text(
                EASYPRIVACY_URL,
                profile,
                cached.easyprivacy_etag.as_deref(),
                cached.easyprivacy_last_modified.as_deref(),
            ),
        )?;
        let updated = CachedLists {
            easylist: easylist_result.body.unwrap_or(cached.easylist),
            easyprivacy: easyprivacy_result.body.unwrap_or(cached.easyprivacy),
            easylist_etag: easylist_result.etag,
            easylist_last_modified: easylist_result.last_modified,
            easyprivacy_etag: easyprivacy_result.etag,
            easyprivacy_last_modified: easyprivacy_result.last_modified,
            checked_at: Some(chrono::Utc::now().to_rfc3339()),
        };
        if !looks_like_filter_list(&updated.easylist, 1000)
            || !looks_like_filter_list(&updated.easyprivacy, 1000)
        {
            return Err(AppError::InvalidInput(
                "downloaded blocker lists failed validation".into(),
            ));
        }
        let replacement = build_active(updated.easylist.clone(), updated.easyprivacy.clone());
        write_cache(&self.list_dir, &updated).await?;
        *self.active.write().expect("ad filter lock poisoned") = replacement;
        Ok(())
    }

    pub fn filter_fragment(&self, html: &str, base_url: Option<&str>) -> String {
        self.filter(html, base_url, false)
    }

    pub fn filter_document(&self, html: &str, base_url: &str) -> String {
        self.filter(html, Some(base_url), true)
    }

    fn filter(&self, html: &str, base_url: Option<&str>, document: bool) -> String {
        let doc = if document {
            Document::from(html)
        } else {
            Document::fragment(html)
        };
        let base = base_url.and_then(|value| Url::parse(value).ok());
        let active = self.active.read().expect("ad filter lock poisoned");

        if let Some(page_url) = base.as_ref() {
            apply_cosmetic_rules(&doc, page_url, &active.engine);
        }
        doc.select(
            ".advertisement,.advertising,.ad-container,.ad-wrapper,.sponsored-content,[id^='google_ads']",
        )
        .remove();
        remove_repeated_blocked_promotions(&doc, base.as_ref(), &active.engine);
        filter_media(&doc, base.as_ref(), &active.engine);
        filter_links(&doc, base.as_ref(), &active.engine);

        if document {
            doc.html().to_string()
        } else {
            doc.inner_html().to_string()
        }
    }
}

fn read_cache(list_dir: &Path) -> Option<CachedLists> {
    let value = std::fs::read(list_dir.join(CACHE_FILE)).ok()?;
    serde_json::from_slice(&value).ok()
}

async fn write_cache(list_dir: &Path, cached: &CachedLists) -> AppResult<()> {
    tokio::fs::create_dir_all(list_dir).await?;
    let target = list_dir.join(CACHE_FILE);
    let temporary = list_dir.join("lists.json.tmp");
    let bytes =
        serde_json::to_vec(cached).map_err(|error| AppError::InvalidInput(error.to_string()))?;
    tokio::fs::write(&temporary, bytes).await?;
    tokio::fs::rename(&temporary, &target).await?;
    Ok(())
}

fn build_active(easylist: String, easyprivacy: String) -> ActiveFilter {
    let mut set = FilterSet::new(false);
    set.add_filter_list(easylist.clone(), ParseOptions::default());
    set.add_filter_list(easyprivacy.clone(), ParseOptions::default());
    set.add_filter_list(VEILFEED_RULES.to_owned(), ParseOptions::default());
    let mut hash = Sha256::new();
    hash.update(POLICY_VERSION.as_bytes());
    hash.update([0]);
    hash.update(easylist.as_bytes());
    hash.update([0]);
    hash.update(easyprivacy.as_bytes());
    ActiveFilter {
        engine: Engine::new_with_filter_set(set),
        revision: hex::encode(hash.finalize()),
    }
}

pub fn looks_like_filter_list(value: &str, minimum_rules: usize) -> bool {
    value.trim_start().starts_with('[')
        && value
            .lines()
            .filter(|line| {
                let line = line.trim();
                !line.is_empty() && !line.starts_with('!') && !line.starts_with('[')
            })
            .take(minimum_rules)
            .count()
            >= minimum_rules
}

fn apply_cosmetic_rules(doc: &Document, page_url: &Url, engine: &Engine) {
    let resources = engine.url_cosmetic_resources(page_url.as_str());
    let mut selectors = resources.hide_selectors;
    if !resources.generichide {
        let nodes = doc.select("*[class],*[id]");
        let mut classes = HashSet::new();
        let mut ids = HashSet::new();
        for node in nodes.nodes() {
            if let Some(value) = node.class() {
                classes.extend(value.split_whitespace().map(str::to_owned));
            }
            if let Some(value) = node.id_attr() {
                ids.insert(value.to_string());
            }
        }
        selectors.extend(engine.hidden_class_id_selectors(classes, ids, &resources.exceptions));
    }
    for selector in selectors {
        if let Some(matches) = doc.try_select(&selector) {
            matches.remove();
        }
    }
}

fn resolve_url(value: &str, base: Option<&Url>) -> Option<Url> {
    if let Some(token) = value.strip_prefix("reader-media://localhost/") {
        return decode_media_token(token)
            .ok()
            .and_then(|decoded| Url::parse(&decoded.url).ok());
    }
    base.and_then(|root| root.join(value).ok())
        .or_else(|| Url::parse(value).ok())
}

fn request(
    engine: &Engine,
    target: &Url,
    source: Option<&Url>,
    kind: &str,
) -> Option<(bool, bool)> {
    let request = Request::new(
        target.as_str(),
        source.map(Url::as_str).unwrap_or_default(),
        kind,
        "GET",
    )
    .ok()?;
    let third_party = request.is_third_party;
    Some((
        engine.check_network_request(&request).should_block(),
        third_party,
    ))
}

fn filter_media(doc: &Document, base: Option<&Url>, engine: &Engine) {
    let nodes = doc
        .select("img[src],source[src],video[src],audio[src]")
        .nodes()
        .to_vec();
    for node in nodes {
        let Some(source) = node.attr("src").and_then(|value| resolve_url(&value, base)) else {
            continue;
        };
        let kind = if node.node_name().as_deref() == Some("img") {
            "image"
        } else {
            "media"
        };
        if request(engine, &source, base, kind).is_some_and(|result| result.0) {
            remove_media_node(node);
        }
    }
}

fn remove_media_node(node: NodeRef<'_>) {
    let parent = node.parent();
    node.remove_from_parent();
    if let Some(parent) = parent {
        let name = parent.node_name();
        if matches!(name.as_deref(), Some("a" | "p" | "figure"))
            && parent.text().trim().is_empty()
            && parent.element_children().is_empty()
        {
            parent.remove_from_parent();
        }
    }
}

fn filter_links(doc: &Document, base: Option<&Url>, engine: &Engine) {
    let links = doc.select("a[href]").nodes().to_vec();
    for link in links {
        let Some(raw) = link.attr("href") else {
            continue;
        };
        let Some(mut target) = resolve_url(&raw, base) else {
            continue;
        };
        if !matches!(target.scheme(), "http" | "https") {
            continue;
        }
        let (blocked, third_party) =
            request(engine, &target, base, "document").unwrap_or((false, true));
        let had_tracking = strip_tracking_parameters(&mut target);
        let wraps_media = !link.element_children().is_empty()
            && link.text().trim().is_empty()
            && link.element_children().iter().all(|child| {
                matches!(
                    child.node_name().as_deref(),
                    Some("img" | "picture" | "video")
                )
            });
        if wraps_media && (blocked || (third_party && had_tracking)) {
            remove_linked_media(link);
        } else if blocked {
            link.remove_attr("href");
            link.rename("span");
        } else if had_tracking {
            link.set_attr("href", target.as_str());
        }
    }
}

fn remove_linked_media(link: NodeRef<'_>) {
    let parent = link.parent();
    link.remove_from_parent();
    if let Some(parent) = parent
        && matches!(parent.node_name().as_deref(), Some("p" | "figure"))
        && parent.text().trim().is_empty()
        && parent.element_children().is_empty()
    {
        parent.remove_from_parent();
    }
}

fn remove_repeated_blocked_promotions(doc: &Document, base: Option<&Url>, engine: &Engine) {
    let containers = doc.select("aside,section,div").nodes().to_vec();
    for container in containers.into_iter().rev() {
        let links = Selection::from(container)
            .select("a[href]")
            .nodes()
            .to_vec();
        if links.len() < 2 {
            continue;
        }
        let mut host = None;
        let all_blocked_same_host = links.iter().all(|link| {
            let Some(target) = link.attr("href").and_then(|href| resolve_url(&href, base)) else {
                return false;
            };
            if !request(engine, &target, base, "document").is_some_and(|result| result.0) {
                return false;
            }
            match (host.as_deref(), target.host_str()) {
                (None, Some(value)) => {
                    host = Some(value.to_owned());
                    true
                }
                (Some(expected), Some(value)) => expected.eq_ignore_ascii_case(value),
                _ => false,
            }
        });
        if all_blocked_same_host {
            container.remove_from_parent();
        }
    }
}

fn strip_tracking_parameters(url: &mut Url) -> bool {
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    let retained: Vec<_> = pairs
        .iter()
        .filter(|(key, _)| !is_tracking_parameter(key))
        .cloned()
        .collect();
    if retained.len() == pairs.len() {
        return false;
    }
    url.set_query(None);
    if !retained.is_empty() {
        url.query_pairs_mut().extend_pairs(retained);
    }
    true
}

fn is_tracking_parameter(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.starts_with("utm_")
        || key.starts_with("mc_")
        || matches!(
            key.as_str(),
            "gclid" | "dclid" | "fbclid" | "msclkid" | "twclid" | "igshid" | "sfcid"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter() -> AdFilter {
        AdFilter::from_rules(
            "[Adblock Plus 2.0]\n||ads.example^$image\n||hubs.li^$document\nexample.com##.sponsor\n##.advertisement",
        )
    }

    #[test]
    fn bundled_lists_load_and_block_known_ad_hosts() {
        let directory = tempfile::tempdir().unwrap();
        let filter = AdFilter::load(directory.path());
        let output = filter.filter_fragment(
            r#"<img src="https://securepubads.g.doubleclick.net/banner.jpg"><p>Article</p>"#,
            Some("https://publisher.example/article"),
        );
        assert!(!output.contains("doubleclick"));
        assert!(output.contains("Article"));
        assert_eq!(filter.revision().len(), 64);
    }

    #[test]
    fn removes_marketing_linked_image_but_keeps_editorial_media() {
        let filter = filter();
        let html = r#"<p>Before</p><p><a href="https://wiz.io/report?utm_source=publisher&utm_campaign=test&sfcid=1"><img src="https://example.com/ad.jpg" alt="image"></a></p><figure><img src="https://cdn.example.net/chart.jpg"><figcaption>Chart</figcaption></figure><p>After</p>"#;
        let output = filter.filter_fragment(html, Some("https://example.com/article"));
        assert!(!output.contains("wiz.io"));
        assert!(!output.contains("ad.jpg"));
        assert!(output.contains("Before"));
        assert!(output.contains("chart.jpg"));
        assert!(output.contains("After"));
    }

    #[test]
    fn removes_marketing_linked_legacy_reader_media() {
        let html = r#"<p>Before</p><p><a href="https://wiz.io/lp/ai-security?utm_source=bleepingcomputer&amp;utm_campaign=test&amp;sfcid=1"><img src="reader-media://localhost/eyJ1cmwiOiJodHRwczovL3d3dy5ibGVlcHN0YXRpYy5jb20vYy93L3ctYWktc2VjdXJpdHkuanBnIiwiZmVlZF9pZCI6ImZlZWQifQ" alt="image"></a></p><p>After</p>"#;
        let output = filter().filter_fragment(
            html,
            Some("https://www.bleepingcomputer.com/news/security/article/"),
        );
        assert!(!output.contains("wiz.io"));
        assert!(!output.contains("reader-media"));
        assert!(output.contains("Before"));
        assert!(output.contains("After"));
    }

    #[test]
    fn honors_network_exceptions() {
        let filter = AdFilter::from_rules(
            "[Adblock Plus 2.0]\n||cdn.example^$image\n@@||cdn.example/editorial.jpg$image",
        );
        let output = filter.filter_fragment(
            r#"<img src="https://cdn.example/ad.jpg"><img src="https://cdn.example/editorial.jpg">"#,
            Some("https://publisher.example/article"),
        );
        assert!(!output.contains("ad.jpg"));
        assert!(output.contains("editorial.jpg"));
    }

    #[test]
    fn removes_blocked_media_and_cosmetic_nodes() {
        let filter = filter();
        let html = r#"<div class="sponsor">Sponsored</div><img src="https://ads.example/banner.jpg"><p>Article</p>"#;
        let output = filter.filter_document(html, "https://example.com/article");
        assert!(!output.contains("Sponsored"));
        assert!(!output.contains("banner.jpg"));
        assert!(output.contains("Article"));
    }

    #[test]
    fn strips_tracking_from_text_links_without_removing_text() {
        let output = filter().filter_fragment(
            r#"<a href="https://docs.example/guide?chapter=2&utm_medium=rss&fbclid=x">Guide</a>"#,
            Some("https://example.com/article"),
        );
        assert!(output.contains("Guide"));
        assert!(output.contains("chapter=2"));
        assert!(!output.contains("utm_"));
        assert!(!output.contains("fbclid"));
    }

    #[test]
    fn removes_repeated_blocked_promotion() {
        let output = filter().filter_fragment(
            r#"<div><a href="https://hubs.li/report"><img src="https://example.com/report.jpg"></a><h2><a href="https://hubs.li/report">Get the report</a></h2><p>Promotion</p></div><p>Article</p>"#,
            Some("https://example.com/article"),
        );
        assert!(!output.contains("Promotion"));
        assert!(output.contains("Article"));
    }
}
