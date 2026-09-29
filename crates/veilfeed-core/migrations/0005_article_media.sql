CREATE TABLE article_media (
  article_id TEXT NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
  token TEXT NOT NULL,
  PRIMARY KEY (article_id, token)
);

CREATE INDEX article_media_token ON article_media(token);

-- Media references must be parsed out of stored HTML, which happens in Rust
-- after migrations run. The flag is cleared once that backfill commits.
INSERT INTO settings(key, value) VALUES('article_media_backfill_pending', 'true');
