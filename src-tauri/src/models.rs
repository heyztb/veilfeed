use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ProxyProfile {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub endpoint: Option<String>,
    pub username: Option<String>,
    pub remote_dns: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProxyInput {
    pub id: Option<String>,
    pub name: String,
    pub kind: String,
    pub endpoint: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub remote_dns: bool,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct FolderSummary {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub unread_count: i64,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct FeedSummary {
    pub id: String,
    pub folder_id: Option<String>,
    pub proxy_profile_id: Option<String>,
    pub title: String,
    pub feed_url: String,
    pub site_url: Option<String>,
    pub icon_url: Option<String>,
    pub block_ads: bool,
    pub refresh_interval_minutes: i64,
    pub unread_count: i64,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub last_checked_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFeedInput {
    pub id: String,
    pub title: String,
    pub feed_url: String,
    pub folder_id: Option<String>,
    pub proxy_profile_id: Option<String>,
    pub refresh_interval_minutes: i64,
    pub block_ads: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SidebarData {
    pub folders: Vec<FolderSummary>,
    pub feeds: Vec<FeedSummary>,
    pub all_count: i64,
    pub unread_count: i64,
    pub starred_count: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleQuery {
    pub scope: Option<String>,
    pub feed_id: Option<String>,
    pub folder_id: Option<String>,
    pub search: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ArticleSummary {
    pub id: String,
    pub feed_id: String,
    pub feed_title: String,
    pub title: String,
    pub author: Option<String>,
    pub published_at: Option<String>,
    pub received_at: String,
    pub is_unread: bool,
    pub is_starred: bool,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ArticleDetail {
    pub id: String,
    pub feed_id: String,
    pub feed_title: String,
    pub title: String,
    pub author: Option<String>,
    pub canonical_url: Option<String>,
    pub published_at: Option<String>,
    pub content_html: String,
    pub full_content_html: Option<String>,
    #[serde(skip)]
    pub content_filter_revision: Option<String>,
    #[serde(skip)]
    pub full_content_filter_revision: Option<String>,
    #[serde(skip)]
    pub block_ads: bool,
    pub is_unread: bool,
    pub is_starred: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticlePage {
    pub items: Vec<ArticleSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeInput {
    pub url: String,
    pub folder_id: Option<String>,
    pub proxy_profile_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeResult {
    pub feed_id: String,
    pub title: String,
    pub article_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpmlEntry {
    pub title: String,
    pub feed_url: String,
    pub site_url: Option<String>,
    pub folder_path: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpmlPreview {
    pub entries: Vec<OpmlEntry>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOpmlInput {
    pub entries: Vec<OpmlEntry>,
    pub proxy_profile_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AvailableRelease {
    pub version: String,
    pub title: String,
    pub release_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseCheck {
    pub current_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available: Option<AvailableRelease>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewArticleFeed {
    pub feed_title: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshResult {
    pub refreshed: usize,
    pub new_articles: usize,
    pub new_article_feeds: Vec<NewArticleFeed>,
    pub errors: Vec<String>,
}
