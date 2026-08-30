import type { FeedSummary } from "./types";

function remoteHttpUrl(value?: string): string | undefined {
  if (!value) return;
  try {
    const url = new URL(value);
    return url.protocol === "http:" || url.protocol === "https:"
      ? url.toString()
      : undefined;
  } catch {
    return;
  }
}

export function readerMediaUrl(source: string, feedId: string): string {
  const payload = new TextEncoder().encode(
    JSON.stringify({ url: source, feed_id: feedId, cache_only: true }),
  );
  let binary = "";
  for (const byte of payload) binary += String.fromCharCode(byte);
  const token = btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(
    /=+$/,
    "",
  );
  return `reader-media://localhost/${token}`;
}

export function faviconUrls(
  feed: Pick<
    FeedSummary,
    "id" | "iconUrl" | "siteUrl" | "feedUrl" | "lastCheckedAt"
  >,
): string[] {
  const candidates: string[] = [];
  const publishedIcon = remoteHttpUrl(feed.iconUrl);
  if (publishedIcon) candidates.push(publishedIcon);

  const site = remoteHttpUrl(feed.siteUrl) ?? remoteHttpUrl(feed.feedUrl);
  if (site) candidates.push(new URL("/favicon.ico", site).toString());

  const refreshKey = encodeURIComponent(feed.lastCheckedAt ?? "pending");
  return [...new Set(candidates)].map((url) =>
    `${readerMediaUrl(url, feed.id)}#${refreshKey}`
  );
}

interface DisplayedFavicon {
  promise: Promise<string | undefined>;
  url?: string;
  settled: boolean;
}

const displayedFavicons = new Map<string, DisplayedFavicon>();
const currentFaviconVersion = new Map<string, string>();

export function displayedFaviconState(sources: string[]): {
  candidate: number;
  url?: string;
} {
  for (const [candidate, source] of sources.entries()) {
    const cached = displayedFavicons.get(source);
    if (cached?.url) return { candidate, url: cached.url };
    if (!cached?.settled) return { candidate };
  }
  return { candidate: sources.length };
}

export function displayedFaviconUrl(
  source: string,
): Promise<string | undefined> {
  const resource = source.split("#", 1)[0];
  const previous = currentFaviconVersion.get(resource);
  let stale: DisplayedFavicon | undefined;
  if (previous && previous !== source) {
    stale = displayedFavicons.get(previous);
    displayedFavicons.delete(previous);
  }
  currentFaviconVersion.set(resource, source);

  let cached = displayedFavicons.get(source);
  if (!cached) {
    cached = { promise: Promise.resolve(undefined), settled: false };
    cached.promise = fetch(source)
      .then((response) => response.ok ? response.blob() : undefined)
      .then((blob) => blob ? URL.createObjectURL(blob) : undefined)
      .catch(() => undefined)
      .then((url) => {
        cached!.url = url;
        cached!.settled = true;
        void stale?.promise.then((staleUrl) => {
          if (staleUrl) URL.revokeObjectURL(staleUrl);
        });
        return url;
      });
    displayedFavicons.set(source, cached);
  }
  return cached.promise;
}

export function discardDisplayedFavicon(source: string): void {
  const cached = displayedFavicons.get(source);
  if (cached?.url) URL.revokeObjectURL(cached.url);
  if (cached) {
    cached.url = undefined;
    cached.settled = true;
  }
}
