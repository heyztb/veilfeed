<script lang="ts">
  import { untrack } from "svelte";
  import type { FeedSummary } from "../lib/types";
  import { discardDisplayedFavicon, displayedFaviconState, displayedFaviconUrl, faviconUrls } from "../lib/favicon";
  import Icon from "./Icon.svelte";

  let { feed, size = 16 }: { feed: Pick<FeedSummary, "id" | "title" | "iconUrl" | "siteUrl" | "feedUrl" | "lastCheckedAt">; size?: number } = $props();
  const initialSources = untrack(() => faviconUrls(feed));
  const initial = displayedFaviconState(initialSources);
  let candidate = $state(initial.candidate);
  let sources = $derived(faviconUrls(feed));
  let sourceKey = $derived(sources.join("\n"));
  let requestedSource = $derived(sources[candidate]);
  let src = $state<string | undefined>(initial.url);
  let srcSource = initial.url ? initialSources[initial.candidate] : undefined;
  let displayedFeedId = untrack(() => feed.id);
  let generation = 0;

  $effect(() => {
    sourceKey;
    const feedChanged = displayedFeedId !== feed.id;
    displayedFeedId = feed.id;
    const cached = displayedFaviconState(sources);
    candidate = cached.candidate;
    if (cached.url) {
      src = cached.url;
      srcSource = sources[cached.candidate];
    } else if (feedChanged) {
      src = undefined;
      srcSource = undefined;
    }
  });

  $effect(() => {
    const source = requestedSource;
    const currentGeneration = ++generation;
    if (!source) return;
    const cached = displayedFaviconState([source]);
    if (cached.url) {
      src = cached.url;
      srcSource = source;
      return;
    }
    void displayedFaviconUrl(source).then((resolved) => {
      if (currentGeneration !== generation) return;
      if (resolved) {
        src = resolved;
        srcSource = source;
      }
      else candidate += 1;
    });
  });

  function tryNext() {
    if (srcSource) discardDisplayedFavicon(srcSource);
    src = undefined;
    srcSource = undefined;
    candidate += 1;
  }
</script>

<span class="source-icon" style={`--source-icon-size:${size}px`} aria-hidden="true">
  {#if src}
    <img {src} alt="" loading="lazy" referrerpolicy="no-referrer" onerror={tryNext}/>
  {:else}
    <Icon name="feed" size={Math.max(12, size - 2)}/>
  {/if}
</span>

<style>
  .source-icon{width:var(--source-icon-size);height:var(--source-icon-size);display:inline-grid;place-items:center;flex:none;overflow:hidden;border-radius:4px;color:var(--muted)}
  img{display:block;width:100%;height:100%;object-fit:contain}
</style>
