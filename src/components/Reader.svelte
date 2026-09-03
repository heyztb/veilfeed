<script lang="ts">
  import ExternalLinkDialog from "./ExternalLinkDialog.svelte";
  import Icon from "./Icon.svelte";
  import SourceIcon from "./SourceIcon.svelte";
  import type { ArticleDetail, FeedSummary } from "../lib/types";
  let { article, feed, vimNavigationEnabled, loading, onstar, onfull }: { article?: ArticleDetail; feed?: FeedSummary; vimNavigationEnabled: boolean; loading: boolean; onstar: () => void; onfull: () => void } = $props();
  let fontScale = $state(1);
  let pendingExternalUrl = $state<string>();
  let hoveredUrl = $state<string>();
  let retrievingFull = $state(false);
  const date = (value?: string) => value ? new Intl.DateTimeFormat(undefined,{dateStyle:"medium",timeStyle:"short"}).format(new Date(value)) : "";
  function contentClick(event: MouseEvent) {
    const link = (event.target as HTMLElement).closest("a") as HTMLAnchorElement | null;
    if (!link) return; event.preventDefault();
    requestExternal(link.href);
  }
  function contentLink(target: EventTarget | null) {
    return target instanceof Element
      ? target.closest<HTMLAnchorElement>("a[href]")
      : null;
  }
  function contentMouseover(event: MouseEvent) {
    hoveredUrl = contentLink(event.target)?.href;
  }
  function contentMouseout(event: MouseEvent) {
    const link = contentLink(event.target);
    if (link && link !== contentLink(event.relatedTarget)) hoveredUrl = undefined;
  }
  function contentFocusin(event: FocusEvent) {
    hoveredUrl = contentLink(event.target)?.href;
  }
  function contentFocusout(event: FocusEvent) {
    const link = contentLink(event.target);
    if (link && link !== contentLink(event.relatedTarget)) hoveredUrl = undefined;
  }
  function requestExternal(url: string) {
    pendingExternalUrl = url;
  }
  function cancelExternal() {
    pendingExternalUrl = undefined;
  }
  let reader: HTMLElement;
  export function focusPane() { reader.focus(); }
  export function scrollVim(key: "j" | "k") {
    reader.scrollBy({ top: key === "j" ? 64 : -64, behavior: "auto" });
  }
  async function retrieveFull() {
    if (retrievingFull) return;
    retrievingFull = true;
    try { await onfull(); }
    finally { retrievingFull = false; }
  }
  function interceptLinks(node: HTMLElement) {
    node.addEventListener("click", contentClick);
    node.addEventListener("mouseover", contentMouseover);
    node.addEventListener("mouseout", contentMouseout);
    node.addEventListener("focusin", contentFocusin);
    node.addEventListener("focusout", contentFocusout);
    return {
      destroy: () => {
        node.removeEventListener("click", contentClick);
        node.removeEventListener("mouseover", contentMouseover);
        node.removeEventListener("mouseout", contentMouseout);
        node.removeEventListener("focusin", contentFocusin);
        node.removeEventListener("focusout", contentFocusout);
        hoveredUrl = undefined;
      },
    };
  }
</script>

<main class="reader" bind:this={reader} data-pane="reader" aria-label="Article reader" aria-keyshortcuts={vimNavigationEnabled ? "h l j k" : undefined} aria-describedby={vimNavigationEnabled ? "reader-keyboard-hint" : undefined} tabindex="-1" aria-busy={loading} inert={!!pendingExternalUrl}>
  {#if vimNavigationEnabled}<span id="reader-keyboard-hint" class="sr-only">Use h and l to switch panes; j and k to scroll the article.</span>{/if}
  {#if loading}<div class="placeholder" role="status">Loading article…</div>
  {:else if !article}<div class="placeholder"><div class="logo"><Icon name="feed" size={25}/></div><strong>Select an article</strong><span>Choose a story from the list to begin reading.</span></div>
  {:else}
    <header><div class="source">{#if feed}<SourceIcon {feed} size={16}/>{/if}<span>{article.feedTitle}</span></div><h1 id="reader-title">{article.title}</h1><div class="byline">{article.author ?? ""}{article.author && article.publishedAt ? " · " : ""}{date(article.publishedAt)}</div><div class="actions"><button class:active={article.isStarred} aria-pressed={article.isStarred} aria-label={`${article.isStarred ? "Unstar" : "Star"} ${article.title}`} onclick={onstar}><Icon name="star"/> {article.isStarred ? "Starred" : "Star"}</button><button aria-busy={retrievingFull} disabled={retrievingFull} onclick={retrieveFull}>{retrievingFull ? "Retrieving full article…" : "Retrieve full article"}</button>{#if article.canonicalUrl}<button onclick={() => requestExternal(article.canonicalUrl!)}><Icon name="external"/> Browser</button>{/if}<span></span><button aria-label="Decrease article text size" disabled={fontScale <= .8} onclick={() => fontScale=Math.max(.8,fontScale-.1)}>A−</button><output aria-live="polite" aria-label="Article text size">{Math.round(fontScale * 100)}%</output><button aria-label="Increase article text size" disabled={fontScale >= 1.5} onclick={() => fontScale=Math.min(1.5,fontScale+.1)}>A+</button></div></header>
    <article class="content" aria-labelledby="reader-title" style={`--font-scale:${fontScale}`} use:interceptLinks>{@html article.fullContentHtml ?? article.contentHtml}</article>
  {/if}
</main>

{#if hoveredUrl}
  <div class="link-preview" aria-hidden="true">{hoveredUrl}</div>
{/if}

{#if pendingExternalUrl}
  <ExternalLinkDialog url={pendingExternalUrl} onclose={cancelExternal}/>
{/if}

<style>
  .reader{height:100%;min-height:0;overflow:auto;background:var(--reader)}.placeholder{height:100%;display:flex;flex-direction:column;align-items:center;justify-content:center;color:var(--muted);gap:8px;font-size:12px}.placeholder strong{font-size:14px;color:var(--text-soft)}.logo{width:48px;height:48px;border-radius:14px;background:var(--selected-soft);color:var(--accent);display:grid;place-items:center;margin-bottom:5px}header{width:100%;padding:56px clamp(24px,4vw,80px) 20px}.source{display:flex;align-items:center;gap:7px;font-size:11px;color:var(--accent);font-weight:650;text-transform:uppercase;letter-spacing:.06em}h1{font-family:var(--reader-font-family);font-size:clamp(25px,3vw,38px);line-height:1.12;margin:9px 0 13px;color:var(--reader-text);letter-spacing:-.02em}.byline{font-size:12px;color:var(--muted)}.actions{display:flex;align-items:center;gap:6px;border-top:1px solid var(--line);border-bottom:1px solid var(--line);padding:9px 0;margin-top:20px}.actions button{display:flex;align-items:center;gap:5px;border:0;background:none;color:var(--muted);font-size:11px;padding:5px 7px;border-radius:5px}.actions button:hover{background:var(--hover);color:var(--text)}.actions button.active{color:var(--star)}.actions span{flex:1}.actions output{min-width:36px;text-align:center;color:var(--muted);font-size:10px}.content{width:100%;padding:12px clamp(24px,4vw,80px) 80px;font-family:var(--reader-font-family);font-size:calc(17px * var(--font-scale));line-height:1.68;color:var(--reader-text);overflow-wrap:anywhere}.content :global(img),.content :global(video){max-width:100%;height:auto;border-radius:5px}.content :global(pre){overflow:auto;background:var(--code);padding:14px;border-radius:7px;font-size:.78em}.content :global(a){color:var(--accent);text-decoration-thickness:1px;text-underline-offset:2px}.content :global(blockquote){border-left:3px solid var(--line);margin-left:0;padding-left:18px;color:var(--muted)}.content :global(table){display:block;width:100%;max-width:100%;overflow-x:auto;border-collapse:collapse;white-space:normal}.content :global(th),.content :global(td){min-width:max-content;padding:.35em .6em;border:1px solid var(--line);vertical-align:top}.link-preview{position:fixed;left:8px;bottom:8px;z-index:15;max-width:min(720px,calc(100vw - 16px));overflow:hidden;padding:5px 9px;border:1px solid var(--line);border-radius:6px;background:var(--reader);box-shadow:0 3px 12px #0003;color:var(--text-soft);font:11px/1.35 -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;text-overflow:ellipsis;white-space:nowrap;pointer-events:none;user-select:none}
</style>
