use crate::AppState;
use tauri::State;
use veilfeed_core::{error::ApiError, models::*};
type CommandResult<T> = Result<T, ApiError>;

#[tauri::command]
pub fn app_version(state: State<'_, AppState>) -> &'static str {
    state.app_version()
}

#[tauri::command]
pub async fn get_automatic_blocklist_updates(state: State<'_, AppState>) -> CommandResult<bool> {
    state.get_automatic_blocklist_updates().await
}

#[tauri::command]
pub async fn set_automatic_blocklist_updates(
    state: State<'_, AppState>,
    enabled: bool,
) -> CommandResult<()> {
    state.set_automatic_blocklist_updates(enabled).await
}

#[tauri::command]
pub async fn check_for_update(state: State<'_, AppState>) -> CommandResult<ReleaseCheck> {
    state.check_for_update().await
}

#[tauri::command]
pub async fn get_sidebar(state: State<'_, AppState>) -> CommandResult<SidebarData> {
    state.get_sidebar().await
}

#[tauri::command]
pub async fn list_articles(
    state: State<'_, AppState>,
    query: ArticleQuery,
) -> CommandResult<ArticlePage> {
    state.list_articles(query).await
}

#[tauri::command]
pub async fn get_article(state: State<'_, AppState>, id: String) -> CommandResult<ArticleDetail> {
    state.get_article(id).await
}

#[tauri::command]
pub async fn set_article_state(
    state: State<'_, AppState>,
    ids: Vec<String>,
    unread: Option<bool>,
    starred: Option<bool>,
) -> CommandResult<()> {
    state.set_article_state(ids, unread, starred).await
}

#[tauri::command]
pub async fn mark_all_read(
    state: State<'_, AppState>,
    feed_id: Option<String>,
    folder_id: Option<String>,
    scope: Option<String>,
) -> CommandResult<()> {
    state.mark_all_read(feed_id, folder_id, scope).await
}

#[tauri::command]
pub async fn create_folder(
    state: State<'_, AppState>,
    name: String,
    parent_id: Option<String>,
    feed_id: Option<String>,
) -> CommandResult<String> {
    state.create_folder(name, parent_id, feed_id).await
}

#[tauri::command]
pub async fn update_folder(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> CommandResult<()> {
    state.update_folder(id, name).await
}

#[tauri::command]
pub async fn delete_folder(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    state.delete_folder(id).await
}

#[tauri::command]
pub async fn update_feed(state: State<'_, AppState>, input: UpdateFeedInput) -> CommandResult<()> {
    state.update_feed(input).await
}

#[tauri::command]
pub async fn move_feed(
    state: State<'_, AppState>,
    id: String,
    folder_id: Option<String>,
) -> CommandResult<()> {
    state.move_feed(id, folder_id).await
}

#[tauri::command]
pub async fn delete_feed(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    state.delete_feed(id).await
}

#[tauri::command]
pub async fn list_proxies(state: State<'_, AppState>) -> CommandResult<Vec<ProxyProfile>> {
    state.list_proxies().await
}

#[tauri::command]
pub async fn get_default_proxy_profile(state: State<'_, AppState>) -> CommandResult<String> {
    state.get_default_proxy_profile().await
}

#[tauri::command]
pub async fn set_default_proxy_profile(
    state: State<'_, AppState>,
    id: String,
) -> CommandResult<()> {
    state.set_default_proxy_profile(id).await
}

#[tauri::command]
pub async fn save_proxy(
    state: State<'_, AppState>,
    input: SaveProxyInput,
) -> CommandResult<String> {
    state.save_proxy(input).await
}

#[tauri::command]
pub async fn test_proxy(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    state.test_proxy(id).await
}

#[tauri::command]
pub async fn discover_feed(
    state: State<'_, AppState>,
    url: String,
    proxy_profile_id: Option<String>,
) -> CommandResult<Vec<String>> {
    state.discover_feed(url, proxy_profile_id).await
}

#[tauri::command]
pub async fn subscribe(
    state: State<'_, AppState>,
    input: SubscribeInput,
) -> CommandResult<SubscribeResult> {
    state.subscribe(input).await
}

#[tauri::command]
pub async fn refresh_feeds(
    state: State<'_, AppState>,
    feed_id: Option<String>,
    due_only: Option<bool>,
    new_tor_circuit: Option<bool>,
) -> CommandResult<RefreshResult> {
    state
        .refresh_feeds(feed_id, due_only, new_tor_circuit)
        .await
}

#[tauri::command]
pub async fn fetch_full_content(
    state: State<'_, AppState>,
    article_id: String,
) -> CommandResult<String> {
    state.fetch_full_content(article_id).await
}

#[tauri::command]
pub fn preview_opml(state: State<'_, AppState>, xml: String) -> CommandResult<OpmlPreview> {
    state.preview_opml(xml)
}

#[tauri::command]
pub async fn import_opml(
    state: State<'_, AppState>,
    input: ImportOpmlInput,
) -> CommandResult<ImportResult> {
    state.import_opml(input).await
}

#[tauri::command]
pub async fn export_opml(state: State<'_, AppState>) -> CommandResult<String> {
    state.export_opml().await
}
