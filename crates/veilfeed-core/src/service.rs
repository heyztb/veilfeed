use futures::{StreamExt, stream};
use uuid::Uuid;

use crate::{
    AppService as AppState,
    error::{ApiError, AppError},
    media,
    models::{
        ArticleDetail, ArticlePage, ArticleQuery, ImportOpmlInput, ImportResult, NewArticleFeed,
        OpmlPreview, ProxyProfile, RefreshResult, ReleaseCheck, SaveProxyInput, SidebarData,
        SubscribeInput, SubscribeResult, UpdateFeedInput,
    },
    release,
};

type CommandResult<T> = Result<T, ApiError>;

async fn refresh_favicon(
    state: &AppState,
    feed_id: &str,
    icon_url: Option<&str>,
    site_url: Option<&str>,
    feed_url: &str,
) {
    let mut cached_url = None;
    for candidate in media::favicon_candidates(icon_url, site_url, feed_url) {
        if media::prefetch(state, &candidate, feed_id).await.is_ok() {
            cached_url = Some(candidate);
            break;
        }
    }
    let _ = state
        .db
        .set_feed_icon_url(feed_id, cached_url.as_deref())
        .await;
}

impl AppState {
    pub fn app_version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    pub async fn get_automatic_blocklist_updates(&self) -> CommandResult<bool> {
        let state = self;

        state
            .db
            .automatic_blocklist_updates()
            .await
            .map_err(Into::into)
    }

    pub async fn set_automatic_blocklist_updates(&self, enabled: bool) -> CommandResult<()> {
        let state = self;

        state.db.set_automatic_blocklist_updates(enabled).await?;
        state.ad_filter.set_automatic_updates(enabled).await?;
        if enabled {
            // The background updater owns refresh so downloads never overlap.
            state.blocklist_wake.notify_one();
        }
        Ok(())
    }

    pub async fn check_for_update(&self) -> CommandResult<ReleaseCheck> {
        let state = self;

        let profile = state.db.proxy(None).await.map_err(ApiError::from)?;
        release::check(&state.network, profile.as_ref())
            .await
            .map_err(Into::into)
    }

    pub async fn get_sidebar(&self) -> CommandResult<SidebarData> {
        let state = self;

        state.db.sidebar().await.map_err(Into::into)
    }

    pub async fn list_articles(&self, query: ArticleQuery) -> CommandResult<ArticlePage> {
        let state = self;

        state.db.articles(query).await.map_err(Into::into)
    }

    pub async fn get_article(&self, id: String) -> CommandResult<ArticleDetail> {
        let state = self;

        let mut article = state.db.article(&id).await.map_err(ApiError::from)?;
        if article.block_ads {
            let revision = state.ad_filter.revision();
            let mut filtered_content = None;
            let mut filtered_full_content = None;
            if article.content_filter_revision.as_deref() != Some(revision.as_str()) {
                article.content_html = state
                    .ad_filter
                    .filter_fragment(&article.content_html, article.canonical_url.as_deref());
                article.content_filter_revision = Some(revision.clone());
                filtered_content = Some(article.content_html.clone());
            }
            if article.full_content_html.is_some()
                && article.full_content_filter_revision.as_deref() != Some(revision.as_str())
            {
                let cleaned = state.ad_filter.filter_fragment(
                    article.full_content_html.as_deref().unwrap_or_default(),
                    article.canonical_url.as_deref(),
                );
                article.full_content_html = Some(cleaned.clone());
                article.full_content_filter_revision = Some(revision.clone());
                filtered_full_content = Some(cleaned);
            }
            if filtered_content.is_some() || filtered_full_content.is_some() {
                state
                    .db
                    .persist_filtered_article(
                        &article.id,
                        filtered_content
                            .as_deref()
                            .map(|html| (html, revision.as_str())),
                        filtered_full_content
                            .as_deref()
                            .map(|html| (html, revision.as_str())),
                    )
                    .await
                    .map_err(ApiError::from)?;
            }
        }
        if article.is_unread {
            state
                .db
                .set_article_state(&[id], Some(false), None)
                .await
                .map_err(ApiError::from)?;
        }
        Ok(article)
    }

    pub async fn set_article_state(
        &self,
        ids: Vec<String>,
        unread: Option<bool>,
        starred: Option<bool>,
    ) -> CommandResult<()> {
        let state = self;

        state
            .db
            .set_article_state(&ids, unread, starred)
            .await
            .map_err(Into::into)
    }

    pub async fn mark_all_read(
        &self,
        feed_id: Option<String>,
        folder_id: Option<String>,
        scope: Option<String>,
    ) -> CommandResult<()> {
        let state = self;

        state
            .db
            .mark_all_read(feed_id.as_deref(), folder_id.as_deref(), scope.as_deref())
            .await
            .map_err(Into::into)
    }

    pub async fn create_folder(
        &self,
        name: String,
        parent_id: Option<String>,
        feed_id: Option<String>,
    ) -> CommandResult<String> {
        let state = self;

        if let Some(feed_id) = feed_id {
            state
                .db
                .create_folder_with_feed(&name, parent_id.as_deref(), &feed_id)
                .await
                .map_err(Into::into)
        } else {
            state
                .db
                .create_folder(&name, parent_id.as_deref())
                .await
                .map_err(Into::into)
        }
    }

    pub async fn update_folder(&self, id: String, name: String) -> CommandResult<()> {
        let state = self;

        state.db.update_folder(&id, &name).await.map_err(Into::into)
    }

    pub async fn delete_folder(&self, id: String) -> CommandResult<()> {
        let state = self;

        state.db.delete_folder(&id).await.map_err(Into::into)
    }

    pub async fn update_feed(&self, input: UpdateFeedInput) -> CommandResult<()> {
        let state = self;

        crate::content::validate_remote_url(&input.feed_url).map_err(ApiError::from)?;
        if input.title.trim().is_empty() || !(5..=10080).contains(&input.refresh_interval_minutes) {
            return Err(
                AppError::InvalidInput("title and refresh interval are invalid".into()).into(),
            );
        }
        state.db.update_feed(&input).await.map_err(Into::into)
    }

    pub async fn move_feed(&self, id: String, folder_id: Option<String>) -> CommandResult<()> {
        let state = self;

        state
            .db
            .move_feed(&id, folder_id.as_deref())
            .await
            .map_err(Into::into)
    }

    pub async fn delete_feed(&self, id: String) -> CommandResult<()> {
        let state = self;

        state
            .db
            .delete_feed(&id, &state.cache_dir)
            .await
            .map_err(Into::into)
    }

    pub async fn list_proxies(&self) -> CommandResult<Vec<ProxyProfile>> {
        let state = self;

        state.db.proxies().await.map_err(Into::into)
    }

    pub async fn get_default_proxy_profile(&self) -> CommandResult<String> {
        let state = self;

        state
            .db
            .default_proxy_profile_id()
            .await
            .map_err(Into::into)
    }

    pub async fn set_default_proxy_profile(&self, id: String) -> CommandResult<()> {
        let state = self;

        state
            .db
            .set_default_proxy_profile_id(&id)
            .await
            .map_err(Into::into)
    }

    pub async fn save_proxy(&self, input: SaveProxyInput) -> CommandResult<String> {
        let state = self;

        if !matches!(input.kind.as_str(), "direct" | "http" | "socks5") {
            return Err(AppError::InvalidInput("unknown proxy kind".into()).into());
        }
        if input.kind != "direct" && input.endpoint.as_deref().is_none_or(str::is_empty) {
            return Err(AppError::InvalidInput("proxy endpoint is required".into()).into());
        }
        if input.tor_isolation {
            crate::network::validate_tor_profile(
                &input.kind,
                input.endpoint.as_deref(),
                input.username.as_deref(),
                input.remote_dns,
            )
            .map_err(ApiError::from)?;
        }
        let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = chrono::Utc::now().to_rfc3339();
        let service = format!("app.veilfeed.reader.proxy.{id}");
        if let (Some(username), Some(password)) =
            (input.username.as_deref(), input.password.as_deref())
        {
            keyring::Entry::new(&service, username)
                .map_err(|e| ApiError::from(AppError::Credential(e.to_string())))?
                .set_password(password)
                .map_err(|e| ApiError::from(AppError::Credential(e.to_string())))?;
        }
        sqlx::query("INSERT INTO proxy_profiles(id,name,kind,endpoint,username,keychain_service,remote_dns,tor_isolation,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,kind=excluded.kind,endpoint=excluded.endpoint,username=excluded.username,keychain_service=excluded.keychain_service,remote_dns=excluded.remote_dns,tor_isolation=excluded.tor_isolation,updated_at=excluded.updated_at")
        .bind(&id).bind(input.name).bind(input.kind).bind(input.endpoint).bind(input.username).bind(service).bind(input.remote_dns).bind(input.tor_isolation).bind(&now).bind(&now).execute(&state.db.pool).await.map_err(|e| ApiError::from(AppError::Database(e)))?;
        Ok(id)
    }

    pub async fn test_proxy(&self, id: String) -> CommandResult<()> {
        let state = self;

        let profile = state.db.proxy(Some(&id)).await.map_err(ApiError::from)?;
        state
            .network
            .test_connection(profile.as_ref())
            .await
            .map_err(Into::into)
    }

    pub async fn discover_feed(
        &self,
        url: String,
        proxy_profile_id: Option<String>,
    ) -> CommandResult<Vec<String>> {
        let state = self;

        let profile = state
            .db
            .proxy(proxy_profile_id.as_deref())
            .await
            .map_err(ApiError::from)?;
        state
            .network
            .discover(&url, profile.as_ref())
            .await
            .map_err(Into::into)
    }

    pub async fn subscribe(&self, input: SubscribeInput) -> CommandResult<SubscribeResult> {
        let state = self;

        let profile = state
            .db
            .proxy(input.proxy_profile_id.as_deref())
            .await
            .map_err(ApiError::from)?;
        let feed_id = Uuid::new_v4().to_string();
        let parsed = state
            .network
            .fetch_feed(
                &input.url,
                &feed_id,
                profile.as_ref(),
                None,
                &state.ad_filter,
                true,
            )
            .await
            .map_err(ApiError::from)?
            .ok_or_else(|| {
                ApiError::from(AppError::InvalidFeed("feed returned no content".into()))
            })?;
        let title = parsed.title.clone();
        let (feed_id, article_count) = state
            .db
            .insert_feed(
                &input.url,
                input.folder_id.as_deref(),
                input.proxy_profile_id.as_deref(),
                &parsed,
                Some(&feed_id),
            )
            .await
            .map_err(ApiError::from)?;
        refresh_favicon(
            state,
            &feed_id,
            parsed.icon_url.as_deref(),
            parsed.site_url.as_deref(),
            &input.url,
        )
        .await;
        Ok(SubscribeResult {
            feed_id,
            title,
            article_count,
        })
    }

    pub async fn refresh_feeds(
        &self,
        feed_id: Option<String>,
        due_only: Option<bool>,
        new_tor_circuit: Option<bool>,
    ) -> CommandResult<RefreshResult> {
        let state = self;

        let _guard = state.refresh_guard.try_lock().map_err(|_| ApiError {
            code: "refresh_in_progress".into(),
            message: "A refresh is already in progress".into(),
        })?;
        let contexts = state
            .db
            .feed_refresh_context(feed_id.as_deref(), due_only.unwrap_or(false))
            .await
            .map_err(ApiError::from)?;
        if new_tor_circuit.unwrap_or(false) {
            if feed_id.is_none() || contexts.len() != 1 {
                return Err(AppError::InvalidInput(
                    "Select one feed to request a new Tor circuit".into(),
                )
                .into());
            }
            let (feed, profile, _, _) = &contexts[0];
            let profile = profile.as_ref().ok_or_else(|| {
                AppError::InvalidInput("This feed does not use Tor isolation".into())
            })?;
            state
                .network
                .rotate_tor_circuit(profile, &feed.feed_url)
                .map_err(ApiError::from)?;
        }
        let app_state = state.clone();
        let results = stream::iter(contexts)
            .map(|(feed, profile, etag, modified)| {
                let app_state = app_state.clone();
                async move {
                    let result = app_state
                        .network
                        .fetch_feed(
                            &feed.feed_url,
                            &feed.id,
                            profile.as_ref(),
                            Some((&etag, &modified)),
                            &app_state.ad_filter,
                            feed.block_ads,
                        )
                        .await;
                    match result {
                        Ok(parsed) => {
                            let refreshed_icon = parsed
                                .as_ref()
                                .map(|value| (value.icon_url.as_deref(), value.site_url.as_deref()))
                                .unwrap_or((feed.icon_url.as_deref(), feed.site_url.as_deref()));
                            let applied =
                                app_state.db.apply_refresh(&feed.id, parsed.as_ref()).await;
                            if applied.is_ok() {
                                refresh_favicon(
                                    &app_state,
                                    &feed.id,
                                    refreshed_icon.0,
                                    refreshed_icon.1,
                                    &feed.feed_url,
                                )
                                .await;
                            }
                            applied.map(|count| (feed.title, Ok(count)))
                        }
                        Err(error) => {
                            let api = ApiError::from(error);
                            let _ = app_state
                                .db
                                .record_feed_error(&feed.id, &api.code, &api.message)
                                .await;
                            Ok((feed.title, Err(api.message)))
                        }
                    }
                }
            })
            .buffer_unordered(4)
            .collect::<Vec<Result<(String, Result<usize, String>), AppError>>>()
            .await;
        let mut output = RefreshResult {
            refreshed: 0,
            new_articles: 0,
            new_article_feeds: vec![],
            errors: vec![],
        };
        for result in results {
            match result.map_err(ApiError::from)? {
                (title, Ok(count)) => {
                    output.refreshed += 1;
                    output.new_articles += count;
                    if count > 0 {
                        output.new_article_feeds.push(NewArticleFeed {
                            feed_title: title,
                            count,
                        });
                    }
                }
                (title, Err(error)) => output.errors.push(format!("{title}: {error}")),
            }
        }
        Ok(output)
    }

    pub async fn fetch_full_content(&self, article_id: String) -> CommandResult<String> {
        let state = self;

        let article = state
            .db
            .article(&article_id)
            .await
            .map_err(ApiError::from)?;
        let url = article.canonical_url.ok_or_else(|| {
            ApiError::from(AppError::InvalidInput(
                "article has no canonical URL".into(),
            ))
        })?;
        let proxy_id: Option<String> =
            sqlx::query_scalar("SELECT proxy_profile_id FROM feeds WHERE id=?")
                .bind(&article.feed_id)
                .fetch_optional(&state.db.pool)
                .await
                .map_err(|e| ApiError::from(AppError::Database(e)))?
                .flatten();
        let profile = state
            .db
            .proxy(proxy_id.as_deref())
            .await
            .map_err(ApiError::from)?;
        let html = state
            .network
            .extract_full_text(
                &url,
                &article.feed_id,
                profile.as_ref(),
                &state.ad_filter,
                article.block_ads,
            )
            .await
            .map_err(ApiError::from)?;
        state
            .db
            .set_full_content(
                &article_id,
                &html,
                article
                    .block_ads
                    .then(|| state.ad_filter.revision())
                    .as_deref(),
            )
            .await
            .map_err(ApiError::from)?;
        Ok(html)
    }

    pub fn preview_opml(&self, xml: String) -> CommandResult<OpmlPreview> {
        crate::opml::preview(&xml).map_err(Into::into)
    }

    pub async fn import_opml(&self, input: ImportOpmlInput) -> CommandResult<ImportResult> {
        let state = self;

        let mut result = ImportResult {
            imported: 0,
            skipped: 0,
            errors: vec![],
        };
        for entry in input.entries {
            let folder = state
                .db
                .find_or_create_folder_path(&entry.folder_path)
                .await
                .map_err(ApiError::from)?;
            let subscribe = SubscribeInput {
                url: entry.feed_url,
                folder_id: folder,
                proxy_profile_id: input.proxy_profile_id.clone(),
            };
            match state.subscribe(subscribe).await {
                Ok(_) => result.imported += 1,
                Err(error) if error.message.contains("UNIQUE") => result.skipped += 1,
                Err(error) => result
                    .errors
                    .push(format!("{}: {}", entry.title, error.message)),
            }
        }
        Ok(result)
    }

    pub async fn export_opml(&self) -> CommandResult<String> {
        let state = self;

        let entries = state.db.export_entries().await.map_err(ApiError::from)?;
        crate::opml::export(&entries).map_err(Into::into)
    }
}
