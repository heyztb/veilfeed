ALTER TABLE feeds ADD COLUMN block_ads INTEGER NOT NULL DEFAULT 1;

ALTER TABLE articles ADD COLUMN content_filter_revision TEXT;
ALTER TABLE articles ADD COLUMN full_content_filter_revision TEXT;
