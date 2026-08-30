import { assertEquals } from "@std/assert";
import { faviconUrls, readerMediaUrl } from "./favicon.ts";

Deno.test("readerMediaUrl creates the media proxy token", () => {
  const value = readerMediaUrl("https://example.com/favicon.ico", "feed-1");
  const token = value.slice("reader-media://localhost/".length).replaceAll(
    "-",
    "+",
  ).replaceAll("_", "/");
  const padded = token.padEnd(Math.ceil(token.length / 4) * 4, "=");
  assertEquals(
    JSON.parse(
      new TextDecoder().decode(
        Uint8Array.from(atob(padded), (character) => character.charCodeAt(0)),
      ),
    ),
    {
      url: "https://example.com/favicon.ico",
      feed_id: "feed-1",
      cache_only: true,
    },
  );
});

Deno.test("faviconUrls prefers the feed icon and falls back to the website favicon", () => {
  const urls = faviconUrls({
    id: "feed-1",
    iconUrl: "https://cdn.example.com/feed.png",
    siteUrl: "https://example.com/news/",
    feedUrl: "https://example.com/rss.xml",
    lastCheckedAt: "2026-08-24T12:00:00Z",
  });
  assertEquals(urls.length, 2);
});

Deno.test("faviconUrls ignores unsafe source URLs", () => {
  assertEquals(
    faviconUrls({
      id: "feed-1",
      iconUrl: "file:///secret",
      feedUrl: "data:text/plain,no",
    }),
    [],
  );
});
