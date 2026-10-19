<script lang="ts">
  import { api } from "../lib/api";
  import { modal } from "../lib/modal";
  import type { FeedSummary } from "../lib/types";

  let { feed, onclose, ondeleted }: { feed: FeedSummary; onclose: () => void; ondeleted: () => void } = $props();

  let busy = $state(false);
  let error = $state("");

  async function remove() {
    if (busy) return;
    busy = true;
    error = "";
    try {
      await api.deleteFeed(feed.id);
      ondeleted();
    } catch (value) {
      error = (value as { message: string }).message;
    } finally {
      busy = false;
    }
  }
</script>

<div class="backdrop" role="presentation" onclick={(event) => event.target === event.currentTarget && onclose()}>
  <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="delete-feed-title" aria-describedby="delete-feed-description" tabindex="-1" use:modal={{ onClose: onclose, initialFocus: "[data-keep]" }}>
    <header><h2 id="delete-feed-title">Delete subscription</h2><button aria-label="Close" onclick={onclose}>×</button></header>
    <div class="body">
      <p id="delete-feed-description">Delete <b>{feed.title}</b>? Its locally stored articles will also be removed.</p>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
    </div>
    <footer>
      <span></span><button class="secondary" data-keep onclick={onclose} disabled={busy}>Keep subscription</button><button class="danger solid" onclick={remove} disabled={busy} aria-busy={busy}>{busy ? "Deleting…" : "Delete"}</button>
    </footer>
  </div>
</div>
