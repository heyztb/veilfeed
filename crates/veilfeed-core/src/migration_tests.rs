use std::borrow::Cow;

use sqlx::{
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

use crate::{AppService, db::Database, models::ArticleQuery};

/// SHA-384 of migrations 0001-0003 as released (`git show HEAD:src-tauri/migrations/<file>.sql |
/// shasum -a 384`). sqlx stores the raw digest bytes and refuses to start when a shipped
/// migration no longer matches what an existing database applied.
const RELEASED_CHECKSUMS: [(i64, &str); 3] = [
    (
        1,
        "611ed97483096530ef1fdf76f44d34d1bb3f7773edf80e1557911b8c98d5d11872a15e5eb1a719166c3e39430faab4ab",
    ),
    (
        2,
        "2ba9b5723941a045afd4896a48d6961b5bf7d56359b2a31115c7fa09c5d59a3bb6216245b0fdaf23715ab6858a79ab30",
    ),
    (
        3,
        "f0a70472033430b9802ec0d7d52d4cd02c81f5fa5d23e66349e2ee59f67bf9e9c02aacc090a9a1a15be5df5af2c2125b",
    ),
];

fn token(reference: &str) -> &str {
    reference.strip_prefix("reader-media://localhost/").unwrap()
}

#[tokio::test]
async fn released_library_upgrades_with_data_isolation_and_media_references() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("veilfeed.db");
    let old = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true)
                .foreign_keys(true)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal),
        )
        .await
        .unwrap();
    let released = Migrator {
        migrations: Cow::Owned(
            sqlx::migrate!()
                .migrations
                .iter()
                .filter(|migration| migration.version < 4)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    released.run(&old).await.unwrap();

    let now = "2026-10-01T12:00:00Z";
    // The seed rows exactly as the released app wrote them, before tor_isolation existed.
    sqlx::query("INSERT INTO proxy_profiles(id,name,kind,remote_dns,created_at,updated_at) VALUES('direct','Direct','direct',0,?,?)")
        .bind(now).bind(now).execute(&old).await.unwrap();
    sqlx::query("INSERT INTO proxy_profiles(id,name,kind,endpoint,remote_dns,created_at,updated_at) VALUES('tor','Tor','socks5','socks5h://127.0.0.1:9050',1,?,?)")
        .bind(now).bind(now).execute(&old).await.unwrap();
    sqlx::query("INSERT INTO proxy_profiles(id,name,kind,endpoint,remote_dns,created_at,updated_at) VALUES('tor-browser','Tor Browser','socks5','socks5h://127.0.0.1:9150',1,?,?)")
        .bind(now).bind(now).execute(&old).await.unwrap();
    sqlx::query("INSERT INTO settings(key,value) VALUES('default_proxy_profile_id','direct'),('media_cache_limit_bytes','524288000'),('refresh_interval_minutes','60')")
        .execute(&old).await.unwrap();
    // A user-defined SOCKS profile pointing at the Tor daemon must not be
    // silently opted into stream isolation.
    sqlx::query("INSERT INTO proxy_profiles(id,name,kind,endpoint,username,remote_dns,created_at,updated_at) VALUES('custom-socks','Work SOCKS','socks5','socks5h://127.0.0.1:9050','reader',1,?,?)")
        .bind(now).bind(now).execute(&old).await.unwrap();

    sqlx::query("INSERT INTO feeds(id,proxy_profile_id,title,feed_url,block_ads,created_at,updated_at) VALUES('existing-feed','tor','Existing','https://example.com/feed',0,?,?)").bind(now).bind(now).execute(&old).await.unwrap();
    let inline = crate::content::media_url("https://example.com/photo.png", "existing-feed");
    let full = crate::content::media_url("https://example.com/full.png", "existing-feed");
    let removed = crate::content::media_url("https://example.com/removed.png", "existing-feed");
    let html = format!("<p>Existing article</p><img src='{inline}'>");
    let full_html = format!("<p>Full article</p><img src=\"{full}\">");
    sqlx::query("INSERT INTO articles(id,feed_id,source_key,title,received_at,content_html,full_content_html,plain_text,is_starred,created_at,updated_at) VALUES('existing-article','existing-feed','guid','Preserved title',?,?,?,'Existing searchable article',1,?,?)")
        .bind(now).bind(&html).bind(&full_html).bind(now).bind(now).execute(&old).await.unwrap();
    sqlx::query("INSERT INTO articles(id,feed_id,source_key,title,received_at,content_html,plain_text,created_at,updated_at,deleted_at) VALUES('deleted-article','existing-feed','deleted','Deleted',?,?,'Deleted',?,?,?)")
        .bind(now).bind(format!("<img src='{removed}'>")).bind(now).bind(now).bind(now).execute(&old).await.unwrap();

    let released_checksums: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT version,checksum FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&old)
            .await
            .unwrap();
    let expected = RELEASED_CHECKSUMS
        .iter()
        .map(|(version, checksum)| (*version, hex::decode(checksum).unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(released_checksums, expected);
    old.close().await;

    let service = AppService::open(dir.path()).await.unwrap();
    assert_eq!(
        service.get_sidebar().await.unwrap().feeds[0]
            .proxy_profile_id
            .as_deref(),
        Some("tor")
    );
    let isolation = service
        .db
        .proxies()
        .await
        .unwrap()
        .into_iter()
        .map(|profile| (profile.id, profile.tor_isolation))
        .collect::<Vec<_>>();
    assert_eq!(
        isolation,
        vec![
            ("direct".to_owned(), false),
            ("tor".to_owned(), true),
            ("tor-browser".to_owned(), true),
            ("custom-socks".to_owned(), false),
        ]
    );
    let page = service
        .list_articles(ArticleQuery {
            scope: Some("starred".into()),
            feed_id: None,
            folder_id: None,
            search: Some("searchable".into()),
            cursor: None,
            limit: None,
        })
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    // The backfill indexes media in content and full content of live articles only.
    assert!(
        service
            .db
            .article_media_authorized("existing-feed", token(&inline))
            .await
            .unwrap()
    );
    assert!(
        service
            .db
            .article_media_authorized("existing-feed", token(&full))
            .await
            .unwrap()
    );
    assert!(
        !service
            .db
            .article_media_authorized("existing-feed", token(&removed))
            .await
            .unwrap()
    );
    let article = service
        .get_article("existing-article".into())
        .await
        .unwrap();
    assert_eq!(article.content_html, html);
    assert_eq!(
        article.full_content_html.as_deref(),
        Some(full_html.as_str())
    );
    assert!(article.is_starred);
    service.db.pool.close().await;

    // Reopening runs no backfill or migration again and keeps released checksums.
    let reopened = Database::open(&path).await.unwrap();
    let article = reopened.article("existing-article").await.unwrap();
    assert!(!article.is_unread);
    assert!(article.is_starred);
    assert_eq!(article.content_html, html);
    assert!(
        reopened
            .article_media_authorized("existing-feed", token(&inline))
            .await
            .unwrap()
    );
    let upgraded: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT version,checksum FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&reopened.pool)
            .await
            .unwrap();
    assert_eq!(upgraded[..3], expected[..]);
    assert_eq!(
        upgraded
            .iter()
            .map(|(version, _)| *version)
            .collect::<Vec<_>>(),
        sqlx::migrate!()
            .iter()
            .map(|migration| migration.version)
            .collect::<Vec<_>>()
    );
}
