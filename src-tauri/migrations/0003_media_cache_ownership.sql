CREATE TABLE media_cache_feeds (
  cache_key TEXT NOT NULL REFERENCES media_cache(cache_key) ON DELETE CASCADE,
  feed_id TEXT NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
  PRIMARY KEY (cache_key, feed_id)
);

CREATE INDEX media_cache_feeds_feed ON media_cache_feeds(feed_id);
