<script lang="ts">
  import { api } from "../lib/api";
  import { modal } from "../lib/modal";
  import type { FeedSummary, FolderSummary } from "../lib/types";

  let { folder, feeds, initialFeedId, initialAction, onclose, oncreated, onsaved }: {
    folder?: FolderSummary;
    feeds: FeedSummary[];
    initialFeedId?: string;
    initialAction?: "rename" | "delete";
    onclose: () => void;
    oncreated: () => void;
    onsaved: () => void;
  } = $props();

  let name = $state("");
  let feedId = $state("");
  let busy = $state(false);
  let error = $state("");
  let confirmingDelete = $state(false);
  let keepButton = $state<HTMLButtonElement>();
  let initialized = false;

  $effect(() => {
    if (!initialized) {
      name = folder?.name ?? "";
      feedId = initialFeedId ?? "";
      confirmingDelete = initialAction === "delete";
      initialized = true;
    }
  });
  $effect(() => { if (confirmingDelete) queueMicrotask(() => keepButton?.focus()); });

  async function save() {
    if (!name.trim() || busy) return;
    busy = true;
    error = "";
    try {
      if (folder) {
        await api.updateFolder(folder.id, name.trim());
        onsaved();
      } else {
        await api.createFolder(name.trim(), undefined, feedId || undefined);
        oncreated();
      }
    } catch (value) {
      error = (value as { message: string }).message;
    } finally {
      busy = false;
    }
  }

  async function remove() {
    if (!folder || busy) return;
    busy = true;
    error = "";
    try {
      await api.deleteFolder(folder.id);
      onsaved();
    } catch (value) {
      error = (value as { message: string }).message;
    } finally {
      busy = false;
    }
  }

  function keydown(event: KeyboardEvent) {
    if (event.key === "Enter" && !confirmingDelete) void save();
  }
</script>

<div class="backdrop" role="presentation" onclick={(event) => event.target === event.currentTarget && onclose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="folder-title" tabindex="-1" use:modal={{ onClose: onclose, initialFocus: "input" }}>
    <header><h2 id="folder-title">{folder ? "Edit folder" : "Create folder"}</h2><button aria-label="Close" onclick={onclose}>×</button></header>
    <div class="body">
      <label>Folder name<input bind:value={name} onkeydown={keydown} placeholder="Folder name" /></label>
      {#if !folder}
        <label>Subscription <span class="optional">Optional</span><select bind:value={feedId}><option value="">No subscription yet</option>{#each feeds as feed}<option value={feed.id}>{feed.title}</option>{/each}</select></label>
        <p>You can drag subscriptions into this folder after creating it.</p>
      {/if}
      {#if confirmingDelete}
        <div class="delete-confirm" role="alert" aria-live="assertive"><div><b>Delete this folder?</b><span>Subscriptions inside it will be moved to Uncategorized.</span></div><button class="secondary" bind:this={keepButton} onclick={() => confirmingDelete=false} disabled={busy}>Keep folder</button><button class="danger solid" onclick={remove} disabled={busy} aria-busy={busy}>{busy ? "Deleting…" : "Delete"}</button></div>
      {/if}
      {#if error}<p class="error" role="alert">{error}</p>{/if}
    </div>
    <footer>
      {#if folder}<button class="danger" onclick={() => confirmingDelete=true} disabled={busy || confirmingDelete}>Delete folder</button>{/if}
      <span></span><button class="secondary" onclick={onclose}>Cancel</button><button class="primary" onclick={save} disabled={!name.trim() || busy || confirmingDelete} aria-busy={busy}>{busy ? "Saving…" : folder ? "Rename" : "Create folder"}</button>
    </footer>
  </div>
</div>

<style>
  .optional { float: right; font-size: 10px; }
</style>
