# Veilfeed

Veilfeed is a private feed reader for your Mac. Follow blogs, news sites, and
podcasts in one place, read them offline, and choose how each subscription
connects to the internet, including over Tor.

## What it does

- **Reads your feeds.** Subscribe to RSS, Atom, and JSON feeds, or bring your
  subscriptions from another reader with OPML import and export.
- **Works offline.** Articles and their images are saved on your Mac, and you
  can search everything you've collected.
- **Keeps your reading private.** Each subscription can connect directly, through
  an HTTP or SOCKS5 proxy, or through Tor. Images load the same way as the feed,
  so publishers don't see your real address when you open an article.
- **Strips ads and trackers** from articles before you read them.
- **Stays out of your way.** Everything stays on your Mac, and links open in your
  browser only after you confirm.

## Install

1. Download the latest Veilfeed DMG from the
   [Releases page](https://github.com/heyztb/veilfeed/releases). Choose
   **Apple Silicon** (aarch64) for M-series Macs or **Intel** (x64) for older Macs.
2. Open the DMG and drag **Veilfeed** into your Applications folder.
3. Launch Veilfeed from Applications.

Veilfeed requires macOS 12 Monterey or later.

## Using Tor

To read a feed over Tor, have either the Tor service or Tor Browser running,
then choose the **Tor** or **Tor Browser** connection when you add or edit the
subscription. If a site blocks your current Tor exit, open the subscription and
choose **New Tor circuit and retry**.
