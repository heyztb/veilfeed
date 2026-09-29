# Veilfeed

Veilfeed is a local, privacy-focused desktop feed reader built with Tauri, Rust,
Svelte, and SQLite. It supports RSS, Atom, JSON Feed, OPML, full-text search,
offline article storage, and per-feed Direct, HTTP, SOCKS5, or Tor routing.

## Development

Requirements: current stable Rust, Deno 2, and the macOS prerequisites for
Tauri 2.

```sh
deno install
deno task check
deno task test
cargo test --locked --workspace
deno task tauri dev
```

JavaScript dependencies use JSR when an appropriate package is published there.
The Svelte, Vite, and Tauri JavaScript packages currently come from npm because
their official distributions are npm packages; Deno remains the package manager
and task runner.

Build the ad-hoc-signed macOS application and DMG with:

```sh
deno task tauri build
```

Public releases are built as separate signed and notarized Apple Silicon and
Intel DMGs through a draft GitHub Release. See [docs/RELEASING.md](docs/RELEASING.md)
for the one-time GitHub setup and release checklist.

## Privacy model

- Rust owns all remote networking. The webview CSP does not permit publisher
  origins.
- Explicit proxy clients disable system proxy discovery and fail closed.
- The built-in Tor daemon (`127.0.0.1:9050`) and Tor Browser
  (`127.0.0.1:9150`) profiles use `socks5h` so proxy-side hostname resolution is
  retained.
- Article HTML is allow-list sanitized before storage. Remote media URLs are
  rewritten to a custom application protocol, fetched with the feed's proxy, and
  stored in a bounded LRU cache.
- EasyList and EasyPrivacy rules remove ads and tracking from article HTML before
  storage. Rule updates use the configured default connection profile and retain
  the last valid bundled or downloaded rules if that route is unavailable.
- Scripted embeds are removed. External links require confirmation and open in
  the system browser by default. Tor Browser can be selected in Settings and
  uses its own Tor connection rather than Veilfeed's proxy.
- Proxy credentials are stored in macOS Keychain. The app-data directory is
  owner-only (0700) and the SQLite database and new media files are owner-only
  (0600). Reading history remains plaintext within that directory; FileVault
  protects it at rest when enabled, and unencrypted backups can still expose it.
- Bundled blocking rules work offline. Automatic EasyList/EasyPrivacy updates
  are opt-in and use the default connection profile; while they are off, the
  bundled rules are active even if lists were downloaded earlier.

The database and media cache live in the platform application-data directory.

Tor profiles isolate new connections by provider hostname using SOCKS authentication
tokens. In Edit subscription, **New Tor circuit and retry** rotates that hostname’s
token and refreshes the saved feed. Other hostnames retain their tokens; feeds sharing
the hostname and connection profile use the new token on future requests. Tokens are
kept in memory and reset when Veilfeed restarts. Cached articles and media remain cached.

Custom Tor profiles can enable **Tor circuit isolation by provider** in Settings.
The Tor SOCKS port must have `IsolateSOCKSAuth` enabled (the standard Tor default).
Isolation uses remote DNS and replaces proxy login credentials, so enable it only
for Tor endpoints. No control port is required. A new circuit does not guarantee a
different exit IP or country. Rotation is manual; HTTP errors do not trigger it automatically.
