PRAGMA foreign_keys = ON;

CREATE TABLE proxy_profiles (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('direct', 'http', 'socks5')),
  endpoint TEXT,
  username TEXT,
  keychain_service TEXT,
  remote_dns INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

CREATE TABLE folders (
  id TEXT PRIMARY KEY,
  parent_id TEXT REFERENCES folders(id),
  name TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

CREATE TABLE feeds (
  id TEXT PRIMARY KEY,
  folder_id TEXT REFERENCES folders(id),
  proxy_profile_id TEXT REFERENCES proxy_profiles(id),
  title TEXT NOT NULL,
  feed_url TEXT NOT NULL COLLATE NOCASE,
  site_url TEXT,
  description TEXT,
  icon_url TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  refresh_interval_minutes INTEGER NOT NULL DEFAULT 60,
  etag TEXT,
  last_modified TEXT,
  last_checked_at TEXT,
  last_success_at TEXT,
  last_error_code TEXT,
  last_error_message TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);
CREATE UNIQUE INDEX feeds_active_url ON feeds(feed_url) WHERE deleted_at IS NULL;

CREATE TABLE articles (
  id TEXT PRIMARY KEY,
  feed_id TEXT NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
  source_key TEXT NOT NULL,
  guid TEXT,
  canonical_url TEXT,
  title TEXT NOT NULL,
  author TEXT,
  published_at TEXT,
  received_at TEXT NOT NULL,
  summary_html TEXT,
  content_html TEXT NOT NULL,
  full_content_html TEXT,
  plain_text TEXT NOT NULL,
  is_unread INTEGER NOT NULL DEFAULT 1,
  is_starred INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  UNIQUE(feed_id, source_key)
);
CREATE INDEX articles_feed_date ON articles(feed_id, published_at DESC, received_at DESC);
CREATE INDEX articles_unread_date ON articles(is_unread, published_at DESC);
CREATE INDEX articles_starred_date ON articles(is_starred, published_at DESC);

CREATE VIRTUAL TABLE article_fts USING fts5(
  article_id UNINDEXED,
  title,
  author,
  feed_title,
  body,
  tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER article_fts_insert AFTER INSERT ON articles BEGIN
  INSERT INTO article_fts(article_id, title, author, feed_title, body)
  VALUES (new.id, new.title, coalesce(new.author, ''),
    coalesce((SELECT title FROM feeds WHERE id = new.feed_id), ''), new.plain_text);
END;
CREATE TRIGGER article_fts_update AFTER UPDATE OF title, author, plain_text ON articles BEGIN
  DELETE FROM article_fts WHERE article_id = old.id;
  INSERT INTO article_fts(article_id, title, author, feed_title, body)
  VALUES (new.id, new.title, coalesce(new.author, ''),
    coalesce((SELECT title FROM feeds WHERE id = new.feed_id), ''), new.plain_text);
END;
CREATE TRIGGER article_fts_delete AFTER DELETE ON articles BEGIN
  DELETE FROM article_fts WHERE article_id = old.id;
END;

CREATE TABLE media_cache (
  cache_key TEXT PRIMARY KEY,
  source_url TEXT NOT NULL,
  proxy_profile_id TEXT REFERENCES proxy_profiles(id),
  mime_type TEXT,
  etag TEXT,
  last_modified TEXT,
  byte_size INTEGER NOT NULL DEFAULT 0,
  file_name TEXT NOT NULL,
  last_accessed_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE INDEX media_cache_lru ON media_cache(last_accessed_at);

CREATE TABLE settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

