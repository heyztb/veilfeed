<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { getExternalBrowser, type ExternalBrowser } from "../lib/externalBrowser";
  import { modal } from "../lib/modal";

  let { url, onclose }: { url: string; onclose: () => void } = $props();
  let browser = $state<ExternalBrowser>(getExternalBrowser());
  let error = $state("");
  let opening = $state(false);
  let browserName = $derived(browser === "tor" ? "Tor Browser" : "system browser");

  function close() {
    if (!opening) onclose();
  }

  async function confirm() {
    if (opening) return;
    opening = true;
    error = "";
    try {
      await openUrl(url, browser === "tor" ? "Tor Browser" : undefined);
      onclose();
    } catch (cause) {
      console.error(`Failed to open URL in ${browserName}`, cause);
      error = browser === "tor"
        ? "Veilfeed could not open this link in Tor Browser. Make sure it is installed."
        : "Veilfeed could not open this link in your system browser.";
    } finally {
      opening = false;
    }
  }
</script>

<svelte:window onkeydown={(event) => event.key === "Escape" && close()}/>

<div class="external-backdrop" role="presentation">
  <div class="external-dialog" role="dialog" aria-modal="true" aria-labelledby="external-title" aria-describedby="external-url external-description" tabindex="-1" use:modal={{ onClose: close }}>
    <h2 id="external-title">Open in {browser === "tor" ? "Tor Browser" : "system browser"}?</h2>
    <p id="external-url" class="external-url">{url}</p>
    <p id="external-description">{browser === "tor" ? "Tor Browser uses its own Tor connection, separate from Veilfeed's proxy settings." : "Your system browser does not inherit Veilfeed's proxy settings."}</p>
    {#if error}<p class="external-error" role="alert">{error}</p>{/if}
    <div class="external-actions">
      <button onclick={close} disabled={opening}>Cancel</button>
      <button class="primary" onclick={confirm} disabled={opening}>{opening ? "Opening…" : `Open in ${browser === "tor" ? "Tor Browser" : "browser"}`}</button>
    </div>
  </div>
</div>

<style>
  .external-backdrop{position:fixed;inset:0;z-index:1000;display:grid;place-items:center;padding:20px;background:rgba(0,0,0,.45)}.external-dialog{width:min(400px,100%);padding:20px;border:1px solid var(--line);border-radius:12px;background:var(--reader);box-shadow:0 20px 60px rgba(0,0,0,.3);color:var(--reader-text)}.external-dialog h2{margin:0 0 8px;font:650 18px/1.3 -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;letter-spacing:0}.external-dialog p{margin:0;color:var(--muted);font:13px/1.5 system-ui,sans-serif}.external-dialog .external-url{margin:10px 0 12px;padding:8px 10px;border:1px solid var(--line);border-radius:6px;background:var(--code);color:var(--reader-text);font:11px/1.45 ui-monospace,SFMono-Regular,Menlo,monospace;overflow-wrap:anywhere;user-select:text}.external-error{margin-top:12px!important;color:#d9534f!important}.external-actions{display:flex;justify-content:flex-end;gap:8px;margin-top:20px}.external-actions button{border:1px solid var(--line);border-radius:7px;padding:7px 12px;background:var(--hover);color:var(--text);font-size:12px}.external-actions button.primary{border-color:var(--accent);background:var(--accent);color:white}.external-actions button:disabled{opacity:.6}
</style>

