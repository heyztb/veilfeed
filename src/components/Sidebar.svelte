<script lang="ts">
  import Icon from "./Icon.svelte";
  import SourceIcon from "./SourceIcon.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import { adjacentIndex } from "../lib/keyboardNavigation";
  import type { Scope, SidebarData } from "../lib/types";
  let { data, selection, vimNavigationEnabled, onselect, onadd, oncreatefolder, onrenamefolder, ondeletefolder, onrefresh, onsettings, onmove, onmanage, onmarkread, refreshing }: {
    data: SidebarData; selection: { scope: Scope; id?: string }; onselect: (scope: Scope, id?: string) => void;
    vimNavigationEnabled: boolean; onadd: () => void; oncreatefolder: (feedId?: string) => void; onrenamefolder: (folderId: string) => void; ondeletefolder: (folderId: string) => void; onrefresh: (feedId?: string) => void; onsettings: () => void; onmove: (feedId:string,folderId?:string)=>void; onmanage:(feedId:string)=>void; onmarkread:(scope:Scope,id?:string)=>void; refreshing: boolean;
  } = $props();
  const active = (scope: Scope, id?: string) => selection.scope === scope && selection.id === id;
  type MenuTarget = { x: number; y: number; trigger: HTMLElement; scope: Scope; id?: string; label: string; canMarkRead: boolean; manageable?: boolean };
  let menu = $state<MenuTarget>();
  let draggingFeedId = $state("");
  let dropFolderId = $state<string | undefined>();
  let collapsed = $state<Record<string, boolean>>({});
  let sidebarElement: HTMLElement;
  let unfiledFeeds = $derived(data.feeds.filter((feed) => !feed.folderId));
  let unfiledUnreadCount = $derived(unfiledFeeds.reduce((total, feed) => total + feed.unreadCount, 0));
  function subscriptionButtons() {
    return Array.from(sidebarElement.querySelectorAll<HTMLButtonElement>("[data-vim-subscription]"));
  }
  function selectedButton(buttons: HTMLButtonElement[]) {
    return buttons.find((button) => button.dataset.scope === selection.scope && button.dataset.scopeId === selection.id);
  }
  export function focusCurrent() {
    const buttons = subscriptionButtons();
    (selectedButton(buttons) ?? buttons[0])?.focus();
  }
  export function navigate(key: "j" | "k") {
    const buttons = subscriptionButtons();
    const focused = document.activeElement instanceof HTMLButtonElement ? document.activeElement : undefined;
    const current = focused && buttons.includes(focused) ? buttons.indexOf(focused) : buttons.indexOf(selectedButton(buttons)!);
    const index = adjacentIndex(buttons.length, current, key);
    if (index === undefined) return;
    const button = buttons[index];
    button.focus();
    onselect(button.dataset.scope as Scope, button.dataset.scopeId);
  }
  let menuItems = $derived.by(() => {
    const target = menu;
    if (!target) return [];
    return [
      ...(target.scope === "feed" ? [{ label: "Refresh feed", icon: "refresh", disabled: refreshing, action: () => onrefresh(target.id!) }] : []),
      { label: "Mark all as read", icon: "check", disabled: !target.canMarkRead, action: () => onmarkread(target.scope, target.id) },
      ...(target.scope === "folder" ? [
        { label: "Rename folder", icon: "settings", separatorBefore: true, action: () => onrenamefolder(target.id!) },
        { label: "Delete folder…", icon: "trash", action: () => ondeletefolder(target.id!) },
      ] : []),
      ...(target.manageable ? [{ label: "Manage subscription", icon: "settings", separatorBefore: true, action: () => onmanage(target.id!) }] : []),
    ];
  });
  function openMenu(event: MouseEvent | KeyboardEvent, target: Omit<MenuTarget, "x" | "y" | "trigger">) {
    if (event instanceof KeyboardEvent && event.key !== "ContextMenu" && !(event.shiftKey && event.key === "F10")) return;
    event.preventDefault();
    const trigger = event.currentTarget as HTMLElement;
    const rect = trigger.getBoundingClientRect();
    menu = {
      x: event instanceof MouseEvent ? event.clientX : rect.left + 12,
      y: event instanceof MouseEvent ? event.clientY : rect.bottom,
      trigger,
      ...target,
    };
  }
  const countLabel = (count: number) => count ? `, ${count} unread` : "";
  const feedLabel = (feed: SidebarData["feeds"][number]) => `${feed.title}${countLabel(feed.unreadCount)}${feed.lastErrorCode ? ", refresh error" : ""}`;
  function startDrag(event: DragEvent, feedId: string) {
    draggingFeedId = feedId;
    event.dataTransfer?.setData("application/x-veilfeed-feed", feedId);
    event.dataTransfer?.setData("text/plain", feedId);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  }
  function endDrag() { draggingFeedId = ""; dropFolderId = undefined; }
  function draggedFeed(event: DragEvent) {
    return event.dataTransfer?.getData("application/x-veilfeed-feed") || event.dataTransfer?.getData("text/plain") || draggingFeedId;
  }
  function allowDrop(event: DragEvent, folderId?: string) {
    event.preventDefault();
    event.stopPropagation();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
    dropFolderId = folderId;
  }
  function drop(event: DragEvent, folderId?: string) {
    event.preventDefault();
    event.stopPropagation();
    const feedId = draggedFeed(event);
    endDrag();
    if (feedId) onmove(feedId, folderId);
  }
  function toggleFolder(event: MouseEvent, id: string) {
    event.stopPropagation();
    collapsed[id] = !collapsed[id];
  }
</script>

<aside class="sidebar" bind:this={sidebarElement} data-pane="subscriptions" aria-label="Subscriptions" aria-keyshortcuts={vimNavigationEnabled ? "h l j k" : undefined} title={vimNavigationEnabled ? "Subscriptions: h/l switch panes; j/k navigate" : undefined}>
  <div class="brand"><span class="mark"><Icon name="feed" size={18}/></span><strong>Veilfeed</strong></div>
  <nav aria-label="Article scopes">
    <button data-vim-item data-vim-subscription data-scope="all" class:active={active("all")} aria-current={active("all") ? "page" : undefined} aria-label={`All articles${countLabel(data.unreadCount)}`} onclick={() => onselect("all")} oncontextmenu={(event) => openMenu(event,{scope:"all",label:"All articles",canMarkRead:data.unreadCount>0})} onkeydown={(event) => openMenu(event,{scope:"all",label:"All articles",canMarkRead:data.unreadCount>0})}><Icon name="inbox"/><span>All articles</span><b aria-hidden="true">{data.unreadCount}</b></button>
    <button data-vim-item data-vim-subscription data-scope="starred" class:active={active("starred")} aria-current={active("starred") ? "page" : undefined} aria-label={`Starred${data.starredCount ? `, ${data.starredCount} articles` : ""}`} onclick={() => onselect("starred")} oncontextmenu={(event) => openMenu(event,{scope:"starred",label:"Starred",canMarkRead:true})} onkeydown={(event) => openMenu(event,{scope:"starred",label:"Starred",canMarkRead:true})}><Icon name="star"/><span>Starred</span><b aria-hidden="true">{data.starredCount}</b></button>
  </nav>
  <div role="group" aria-label="Subscription actions" class="section-title">
    <span>Subscriptions</span><div class="section-actions"><button title="Create folder" aria-label="Create folder" onclick={() => oncreatefolder()} ondragover={(event) => { if (draggingFeedId) event.preventDefault(); }} ondrop={(event) => { event.preventDefault(); event.stopPropagation(); const feedId = draggedFeed(event); endDrag(); if (feedId) oncreatefolder(feedId); }}><Icon name="folder-plus"/></button><button title="Add subscription" aria-label="Add subscription" onclick={onadd}><Icon name="plus"/></button></div>
  </div>
  <nav class="feeds" aria-label="Subscriptions">
    {#each data.folders as folder}
      <div role="group" aria-label={`${folder.name} folder`} class:drop-target={dropFolderId === folder.id} class="folder-row" ondragenter={(event)=>allowDrop(event,folder.id)} ondragover={(event)=>allowDrop(event,folder.id)} ondragleave={(event)=>{if(!event.currentTarget.contains(event.relatedTarget as Node))dropFolderId=undefined}} ondrop={(event)=>drop(event,folder.id)}>
        <button class="folder-toggle" aria-label={`${collapsed[folder.id] ? "Expand" : "Collapse"} ${folder.name}${countLabel(folder.unreadCount)}`} aria-expanded={!collapsed[folder.id]} onclick={(event) => toggleFolder(event,folder.id)} oncontextmenu={(event) => openMenu(event,{scope:"folder",id:folder.id,label:folder.name,canMarkRead:folder.unreadCount>0})} onkeydown={(event) => openMenu(event,{scope:"folder",id:folder.id,label:folder.name,canMarkRead:folder.unreadCount>0})}><span class="disclosure"><span class:collapsed={collapsed[folder.id]}><Icon name="chevron" size={13}/></span></span><Icon name="folder"/><span>{folder.name}</span><b aria-hidden="true">{folder.unreadCount || ""}</b></button>
      </div>
      {#if !collapsed[folder.id]}
        {#each data.feeds.filter((feed) => feed.folderId === folder.id) as feed}
          <button class="feed nested" data-vim-item data-vim-subscription data-scope="feed" data-scope-id={feed.id} draggable="true" aria-current={active("feed",feed.id) ? "page" : undefined} aria-label={feedLabel(feed)} ondragstart={(event)=>startDrag(event,feed.id)} ondragend={endDrag} oncontextmenu={(event)=>openMenu(event,{scope:"feed",id:feed.id,label:feed.title,canMarkRead:feed.unreadCount>0,manageable:true})} onkeydown={(event)=>openMenu(event,{scope:"feed",id:feed.id,label:feed.title,canMarkRead:feed.unreadCount>0,manageable:true})} class:active={active("feed", feed.id)} class:error={!!feed.lastErrorCode} title={feed.lastErrorMessage ?? feed.feedUrl} onclick={() => onselect("feed", feed.id)}><SourceIcon {feed}/><span>{feed.title}</span><b aria-hidden="true">{feed.unreadCount || ""}</b></button>
        {/each}
      {/if}
    {/each}
    {#if unfiledFeeds.length || draggingFeedId}
      <div role="group" aria-label="Uncategorized subscriptions" class:drop-target={draggingFeedId && dropFolderId === undefined} class="folder-row uncategorized" ondragenter={(event)=>allowDrop(event)} ondragover={(event)=>allowDrop(event)} ondragleave={(event)=>{if(!event.currentTarget.contains(event.relatedTarget as Node))dropFolderId=undefined}} ondrop={(event)=>drop(event)}>
        <button class="folder-toggle" aria-label={`${collapsed.uncategorized ? "Expand" : "Collapse"} Uncategorized${countLabel(unfiledUnreadCount)}`} aria-expanded={!collapsed.uncategorized} onclick={(event)=>toggleFolder(event,"uncategorized")}><span class="disclosure"><span class:collapsed={collapsed.uncategorized}><Icon name="chevron" size={13}/></span></span><Icon name="folder"/><span>Uncategorized</span><b aria-hidden="true">{unfiledUnreadCount || ""}</b></button>
      </div>
      {#if !collapsed.uncategorized}
        {#each unfiledFeeds as feed}
          <button class="feed nested" data-vim-item data-vim-subscription data-scope="feed" data-scope-id={feed.id} draggable="true" aria-current={active("feed",feed.id) ? "page" : undefined} aria-label={feedLabel(feed)} ondragstart={(event)=>startDrag(event,feed.id)} ondragend={endDrag} oncontextmenu={(event)=>openMenu(event,{scope:"feed",id:feed.id,label:feed.title,canMarkRead:feed.unreadCount>0,manageable:true})} onkeydown={(event)=>openMenu(event,{scope:"feed",id:feed.id,label:feed.title,canMarkRead:feed.unreadCount>0,manageable:true})} class:active={active("feed", feed.id)} class:error={!!feed.lastErrorCode} title={feed.lastErrorMessage ?? feed.feedUrl} onclick={() => onselect("feed", feed.id)}><SourceIcon {feed}/><span>{feed.title}</span><b aria-hidden="true">{feed.unreadCount || ""}</b></button>
        {/each}
      {/if}
    {/if}
  </nav>
  <footer><button title="Refresh all feeds" aria-label={refreshing ? "Refreshing feeds" : "Refresh all feeds"} aria-busy={refreshing} disabled={refreshing} onclick={() => onrefresh()} class:spinning={refreshing}><Icon name="refresh"/></button><button title="Settings and OPML" aria-label="Open settings" onclick={onsettings}><Icon name="settings"/></button></footer>
</aside>
{#if menu}<ContextMenu x={menu.x} y={menu.y} label={`${menu.label} actions`} items={menuItems} returnFocus={menu.trigger} onclose={() => menu=undefined}/>{/if}

<style>
  .sidebar{height:100%;min-height:0;background:var(--sidebar);display:flex;flex-direction:column;border-right:1px solid var(--line);overflow:hidden}.brand{height:48px;flex:none;display:flex;align-items:center;gap:9px;padding:0 14px;border-bottom:1px solid var(--line);letter-spacing:.2px}.mark{display:grid;place-items:center;width:27px;height:27px;border-radius:8px;background:var(--accent);color:#fff}.brand strong{font-size:14px}nav{padding:7px;flex:none}.feeds{flex:1 1 0;min-height:0;overflow:auto;padding-top:0}.section-title{flex:none;padding:13px 9px 5px 14px;display:flex;align-items:center;justify-content:space-between;color:var(--muted);font-size:11px;text-transform:uppercase;letter-spacing:.08em;border-radius:6px}.section-actions{display:flex}.section-title button,footer button{width:28px;height:28px;display:grid;place-items:center;border:0;background:none;color:var(--muted);border-radius:6px}.section-title button:hover{background:var(--hover);color:var(--text)}nav button{width:100%;height:30px;padding:0 8px;border:0;background:none;border-radius:6px;color:var(--text);display:grid;grid-template-columns:18px 1fr auto;align-items:center;gap:6px;text-align:left;font-size:12.5px}nav button:hover{background:var(--hover)}nav button.active{background:var(--selected);color:var(--selected-text)}.folder-row{border-radius:6px}.folder-row.drop-target{background:var(--selected-soft);box-shadow:inset 0 0 0 1px var(--accent);color:var(--accent)}.folder-row.drop-target .folder-toggle{color:var(--accent)}nav .folder-toggle{grid-template-columns:16px 18px minmax(0,1fr) auto;padding-left:3px}.disclosure{width:16px;display:grid;place-items:center;color:var(--muted)}.disclosure span{display:grid;transform:rotate(90deg);transition:transform .12s ease}.disclosure span.collapsed{transform:rotate(0)}.uncategorized{margin-top:2px}nav button span{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}nav button b{font-size:10px;font-weight:600;color:var(--muted)}nav button.active b{color:inherit}.nested{padding-left:30px;cursor:grab;-webkit-user-drag:element;user-select:none}.nested:active{cursor:grabbing}.error :global(.source-icon){box-shadow:inset 0 0 0 1px var(--danger)}.error :global(svg){color:var(--danger)}footer{height:42px;flex:none;border-top:1px solid var(--line);display:flex;align-items:center;justify-content:space-between;padding:0 9px}footer button{border:0;background:none;color:var(--muted)}footer button:hover:not(:disabled){background:var(--hover);color:var(--text)}footer button:disabled{opacity:.8}.spinning :global(svg){animation:spin 1s linear infinite}@keyframes spin{to{transform:rotate(360deg)}}
</style>
