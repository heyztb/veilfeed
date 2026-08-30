<script lang="ts">
  import Icon from "./Icon.svelte";
  import SourceIcon from "./SourceIcon.svelte";
  import { windowRange, ARTICLE_ROW_HEIGHT } from "../lib/windowing";
  import type { ArticleSummary, FeedSummary } from "../lib/types";
  let { items, feeds, selectedId, search, loading, hasMore, onselect, onsearch, onloadmore, ontoggleStar }: {
    items: ArticleSummary[]; feeds: FeedSummary[]; selectedId?: string; search: string; loading: boolean; hasMore: boolean;
    onselect: (id: string) => void; onsearch: (value: string) => void; onloadmore: () => void; ontoggleStar: (article: ArticleSummary) => void;
  } = $props();
  let viewport: HTMLDivElement; let scrollTop = $state(0); let viewportHeight = $state(700);
  let range = $derived(windowRange(items.length, scrollTop, viewportHeight));
  let feedsById = $derived(new Map(feeds.map((feed) => [feed.id, feed])));
  const date = (value?: string) => value ? new Intl.DateTimeFormat(undefined,{month:"short",day:"numeric"}).format(new Date(value)) : "";
  function scroll() { scrollTop = viewport.scrollTop; viewportHeight = viewport.clientHeight; if (hasMore && viewport.scrollTop + viewport.clientHeight > viewport.scrollHeight - 220) onloadmore(); }
  function rowKeydown(event: KeyboardEvent, id: string) {
    if (!["ArrowDown", "ArrowUp", "j", "k"].includes(event.key)) return;
    event.preventDefault();
    const direction = event.key === "ArrowDown" || event.key === "j" ? 1 : -1;
    const index = items.findIndex((article) => article.id === id);
    const next = items[Math.max(0, Math.min(items.length - 1, index + direction))];
    if (!next) return;
    onselect(next.id);
    requestAnimationFrame(() => viewport.querySelector<HTMLElement>(`[data-article-id="${CSS.escape(next.id)}"]`)?.focus());
  }
</script>

<section class="list-pane" aria-label="Articles">
  <header><label><Icon name="search"/><input value={search} oninput={(event) => onsearch(event.currentTarget.value)} placeholder="Search articles" aria-label="Search articles"/></label></header>
  <div class="viewport" bind:this={viewport} onscroll={scroll} role="list" aria-label="Article results" aria-busy={loading}>
    {#if loading && items.length === 0}<div class="empty" role="status">Loading articles…</div>
    {:else if items.length === 0}<div class="empty" role="status">No articles here yet.</div>
    {:else}
      <div aria-hidden="true" style={`height:${range.before}px`}></div>
      {#each items.slice(range.start, range.end) as article, visibleIndex (article.id)}
        <div class="article-row" class:selected={selectedId === article.id} class:unread={article.isUnread} style={`height:${ARTICLE_ROW_HEIGHT}px`} role="listitem" aria-posinset={range.start + visibleIndex + 1} aria-setsize={items.length}>
          <button class="article-open" data-article-id={article.id} aria-current={selectedId === article.id ? "true" : undefined} onclick={() => onselect(article.id)} onkeydown={(event) => rowKeydown(event, article.id)}>
            <div class="meta"><span class="source">{#if feedsById.get(article.feedId)}<SourceIcon feed={feedsById.get(article.feedId)!} size={14}/>{/if}<span>{article.feedTitle}</span></span><time>{date(article.publishedAt ?? article.receivedAt)}</time></div>
            <div class="title">{article.title}</div>
            <div class="excerpt">{article.excerpt}</div>
            <span class="sr-only">{article.isUnread ? "Unread. " : ""}{article.isStarred ? "Starred." : ""}</span>
          </button>
          <button class:active={article.isStarred} aria-pressed={article.isStarred} aria-label={`${article.isStarred ? "Unstar" : "Star"} ${article.title}`} onclick={() => ontoggleStar(article)}><Icon name="star" size={14}/></button>
        </div>
      {/each}
      <div aria-hidden="true" style={`height:${range.after}px`}></div>
      {#if loading}<div class="loading" role="status">Loading more articles…</div>{/if}
    {/if}
  </div>
</section>

<style>
  .list-pane{height:100%;min-width:0;min-height:0;display:flex;flex-direction:column;overflow:hidden;background:var(--surface);border-right:1px solid var(--line)}header{height:48px;flex:none;padding:8px;display:flex;gap:7px;border-bottom:1px solid var(--line)}label{height:31px;display:flex;align-items:center;gap:6px;flex:1;border:1px solid var(--line);border-radius:7px;padding:0 8px;background:var(--input);color:var(--muted)}input{min-width:0;width:100%;border:0;outline:0;background:none;color:var(--text);font:inherit;font-size:12px}.viewport{min-height:0;overflow:auto;flex:1 1 0}.empty{color:var(--muted);font-size:13px;text-align:center;padding:50px 16px}.article-row{position:relative;box-sizing:border-box;border-bottom:1px solid var(--line);cursor:default}.article-row:hover{background:var(--hover)}.article-row.selected{background:var(--selected-soft)}.article-row.unread .title{font-weight:680;color:var(--text)}.article-row.unread:before{content:"";position:absolute;z-index:1;left:4px;top:35px;width:4px;height:4px;background:var(--accent);border-radius:50%}.article-open{display:block;width:100%;height:100%;padding:7px 31px 8px 13px;border:0;background:none;color:inherit;text-align:left}.meta{height:16px;display:flex;align-items:center;justify-content:space-between;gap:8px;color:var(--muted);font-size:10.5px}.meta>.source{min-width:0;display:flex;align-items:center;gap:5px}.meta>.source>span,.title,.excerpt{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.title{font-size:12.5px;color:var(--text-soft);margin:4px 0}.excerpt{font-size:11px;color:var(--muted);line-height:1.35}.article-row>button:last-child{position:absolute;right:8px;top:31px;width:22px;height:22px;border:0;background:none;color:var(--muted);padding:3px}.article-row>button:last-child.active{color:var(--star);fill:var(--star)}.loading{text-align:center;color:var(--muted);padding:12px;font-size:11px}
</style>
