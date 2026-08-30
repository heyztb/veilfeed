<script lang="ts">
  import { tick } from "svelte";
  import Icon from "./Icon.svelte";

  let { x, y, label, items, returnFocus, onclose }: {
    x: number;
    y: number;
    label: string;
    items: Array<{
      label: string;
      icon?: string;
      disabled?: boolean;
      separatorBefore?: boolean;
      action: () => void;
    }>;
    returnFocus?: HTMLElement;
    onclose: () => void;
  } = $props();

  let menu: HTMLDivElement;
  let left = $state(0);
  let top = $state(0);

  $effect(() => {
    x;
    y;
    void tick().then(() => {
      if (!menu) return;
      const margin = 8;
      left = Math.max(margin, Math.min(x, window.innerWidth - menu.offsetWidth - margin));
      top = Math.max(margin, Math.min(y, window.innerHeight - menu.offsetHeight - margin));
      menu.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
    });
  });

  function outside(event: PointerEvent) {
    if (!menu?.contains(event.target as Node)) onclose();
  }

  function keydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
      return;
    }
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    const buttons = [...menu.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")];
    const current = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const step = event.key === "ArrowDown" ? 1 : -1;
    buttons[(current + step + buttons.length) % buttons.length]?.focus();
  }

  function choose(item: (typeof items)[number]) {
    if (item.disabled) return;
    onclose();
    item.action();
  }
  $effect(() => () => queueMicrotask(() => returnFocus?.focus()));
</script>

<svelte:window onpointerdown={outside} onkeydown={keydown} onresize={onclose}/>

<div
  class="context-menu"
  bind:this={menu}
  role="menu"
  tabindex="-1"
  aria-label={label}
  style={`left:${left}px;top:${top}px`}
  oncontextmenu={(event) => event.preventDefault()}
>
  {#each items as item}
    {#if item.separatorBefore}<div class="separator" role="separator"></div>{/if}
    <button role="menuitem" disabled={item.disabled} onclick={() => choose(item)}>
      {#if item.icon}<Icon name={item.icon} size={15}/>{/if}
      <span>{item.label}</span>
    </button>
  {/each}
</div>

<style>
  .context-menu{position:fixed;z-index:40;width:max-content;min-width:178px;padding:4px;background:var(--reader);border:1px solid var(--line);border-radius:7px;box-shadow:0 10px 30px #0003;color:var(--text)}
  button{width:100%;height:30px;padding:0 9px;border:0;border-radius:5px;background:none;color:inherit;display:grid;grid-template-columns:17px 1fr;align-items:center;gap:7px;text-align:left;font-size:12px}
  button:hover,button:focus-visible{background:var(--hover);outline:none}
  button:disabled{color:var(--muted)}
  .separator{height:1px;margin:4px;background:var(--line)}
</style>
