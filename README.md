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
cargo test --manifest-path src-tauri/Cargo.toml
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
- Proxy credentials are stored in macOS Keychain; the SQLite database relies on
  normal filesystem permissions and FileVault.

The database and media cache live in the platform application-data directory.
