ALTER TABLE proxy_profiles ADD COLUMN tor_isolation INTEGER NOT NULL DEFAULT 0;
UPDATE proxy_profiles SET tor_isolation=1 WHERE id IN ('tor','tor-browser') AND kind='socks5' AND remote_dns=1 AND username IS NULL AND endpoint IN ('socks5h://127.0.0.1:9050','socks5h://127.0.0.1:9150');
