<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { api } from "../lib/api";
  import { getExternalBrowser, setExternalBrowser, type ExternalBrowser } from "../lib/externalBrowser";
  import { disableNotifications, enableNotifications, notificationsEnabled } from "../lib/notifications";
  import { applyReaderFont, getReaderFont, type ReaderFont } from "../lib/readerFont";
  import { applyTheme, getTheme, type Theme } from "../lib/theme";
  import { modal } from "../lib/modal";
  import { automaticUpdateChecksEnabled, lastUpdateCheckAt, setAutomaticUpdateChecksEnabled } from "../lib/releaseUpdates";
  import type { OpmlPreview, ProxyProfile, ReleaseCheck } from "../lib/types";

  let { proxies, defaultProxyProfileId, releaseCheck, onclose, onchanged, onimported, oncheckupdates, onopenrelease, onupdatepreferencechanged }: { proxies: ProxyProfile[]; defaultProxyProfileId: string; releaseCheck?: ReleaseCheck; onclose: () => void; onchanged: () => void | Promise<void>; onimported: () => void | Promise<void>; oncheckupdates: () => Promise<ReleaseCheck>; onopenrelease: (url: string) => void; onupdatepreferencechanged: () => void } = $props();
  const sections = ["connections", "external-links", "notifications", "updates", "appearance", "opml"] as const;
  let section = $state<(typeof sections)[number]>("connections");
  let theme = $state<Theme>(getTheme());
  let readerFont = $state<ReaderFont>(getReaderFont());
  let externalBrowser = $state<ExternalBrowser>(getExternalBrowser());
  let editing = $state<ProxyProfile | undefined>();
  let name = $state(""); let kind = $state("socks5"); let endpoint = $state(""); let username = $state(""); let password = $state(""); let remoteDns = $state(true);
  let notificationEnabled = $state(notificationsEnabled()); let notificationBusy = $state(false); let notificationError = $state("");
  let updateChecksEnabled = $state(automaticUpdateChecksEnabled()); let updateBusy = $state(false); let updateError = $state(""); let checkedRelease = $state<ReleaseCheck>(); let checkedAt = $state(lastUpdateCheckAt()); let currentVersion = $state("");
  let displayedRelease = $derived(checkedRelease ?? releaseCheck);
  let error = $state(""); let busy = $state(false); let defaultProxyId = $state(untrack(() => defaultProxyProfileId)); let defaultProxyBusy = $state(false); let defaultProxyError = $state("");
  let opmlPreview = $state<OpmlPreview>(); let opmlProxyId = $state(untrack(() => defaultProxyProfileId)); let opmlBusy = $state(false); let opmlMessage = $state(""); let opmlError = $state("");

  function edit(p?: ProxyProfile) { editing = p; name = p?.name ?? ""; kind = p?.kind ?? "socks5"; endpoint = p?.endpoint ?? ""; username = p?.username ?? ""; password = ""; remoteDns = p?.remoteDns ?? true; error = ""; }
  function selectTheme(value: Theme) { theme = value; applyTheme(value); }
  function selectReaderFont(value: ReaderFont) { readerFont = value; applyReaderFont(value); }
  function selectExternalBrowser(value: ExternalBrowser) { externalBrowser = value; setExternalBrowser(value); }
  function sectionKeydown(event: KeyboardEvent) {
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const current = sections.indexOf(section);
    const next = event.key === "Home" ? 0 : event.key === "End" ? sections.length - 1 : (current + (event.key === "ArrowDown" ? 1 : -1) + sections.length) % sections.length;
    section = sections[next];
    requestAnimationFrame(() => document.getElementById(`settings-tab-${section}`)?.focus());
  }
  async function toggleNotifications() {
    notificationError = "";
    if (notificationEnabled) { disableNotifications(); notificationEnabled = false; return; }
    notificationBusy = true;
    try {
      notificationEnabled = await enableNotifications();
      if (!notificationEnabled) notificationError = "Notifications weren’t allowed. You can enable them in System Settings.";
    } catch (e) { notificationError = (e as { message?: string }).message ?? String(e); }
    finally { notificationBusy = false; }
  }
  function toggleUpdateChecks() {
    updateChecksEnabled = !updateChecksEnabled;
    setAutomaticUpdateChecksEnabled(updateChecksEnabled);
    onupdatepreferencechanged();
  }
  async function checkUpdates() {
    if (updateBusy) return;
    updateBusy = true; updateError = "";
    try { checkedRelease = await oncheckupdates(); checkedAt = lastUpdateCheckAt(); }
    catch (e) { updateError = (e as { message?: string }).message ?? String(e); checkedAt = lastUpdateCheckAt(); }
    finally { updateBusy = false; }
  }
  async function save() {
    busy = true; error = "";
    try { await api.saveProxy({ id: editing?.id, name, kind, endpoint: kind === "direct" ? null : endpoint, username: username || null, password: password || null, remoteDns }); await onchanged(); edit(); }
    catch (e) { error = (e as { message: string }).message; }
    finally { busy = false; }
  }
  async function selectDefaultProxy(id: string) {
    const previous = defaultProxyId;
    defaultProxyId = id; defaultProxyBusy = true; defaultProxyError = "";
    try { await api.setDefaultProxyProfile(id); await onchanged(); }
    catch (e) { defaultProxyId = previous; defaultProxyError = (e as { message: string }).message; }
    finally { defaultProxyBusy = false; }
  }
  async function pickOpml(event: Event) {
    const file = (event.currentTarget as HTMLInputElement).files?.[0];
    if (!file) return;
    opmlPreview = undefined; opmlMessage = ""; opmlError = "";
    try { opmlPreview = await api.previewOpml(await file.text()); }
    catch (e) { opmlError = (e as { message: string }).message; }
  }
  async function importFeeds() {
    if (!opmlPreview) return;
    opmlBusy = true; opmlMessage = ""; opmlError = "";
    try {
      const result = await api.importOpml(opmlPreview.entries, opmlProxyId);
      opmlMessage = `Imported ${result.imported}; skipped ${result.skipped}.`;
      if (result.errors.length) opmlError = result.errors.join("\n");
      await onimported();
    } catch (e) { opmlError = (e as { message: string }).message; }
    finally { opmlBusy = false; }
  }
  async function exportFeeds() {
    opmlBusy = true; opmlMessage = ""; opmlError = "";
    try {
      const xml = await api.exportOpml();
      const link = document.createElement("a");
      link.href = URL.createObjectURL(new Blob([xml], { type: "text/xml" }));
      link.download = "veilfeed-subscriptions.opml";
      link.click();
      URL.revokeObjectURL(link.href);
    } catch (e) { opmlError = (e as { message: string }).message; }
    finally { opmlBusy = false; }
  }
  onMount(() => { void api.appVersion().then((version) => currentVersion = version).catch(() => {}); });
</script>

<div class="backdrop" role="presentation" onclick={(event) => event.target === event.currentTarget && onclose()}>
  <div class="dialog wide" role="dialog" aria-modal="true" aria-labelledby="settings-title" tabindex="-1" use:modal={{ onClose: onclose }}>
    <header><h2 id="settings-title">Settings</h2><button aria-label="Close settings" onclick={onclose}>×</button></header>
    <div class="settings-body">
      <aside>
        <div role="tablist" aria-label="Settings categories" aria-orientation="vertical" tabindex="-1" onkeydown={sectionKeydown}>
          {#each sections as value}
            <button id={`settings-tab-${value}`} role="tab" aria-selected={section === value} aria-controls={`settings-panel-${value}`} tabindex={section === value ? 0 : -1} class:active={section === value} onclick={() => section = value}>{value === "external-links" ? "External links" : value === "opml" ? "OPML" : value[0].toUpperCase() + value.slice(1)}</button>
          {/each}
        </div>
      </aside>
      <div class="settings-content">
        {#if section === "connections"}
          <div role="tabpanel" id="settings-panel-connections" aria-labelledby="settings-tab-connections" tabindex="0">
            <div class="heading"><div><h3>Connection profiles</h3><p>Feeds fail closed when their assigned connection is unavailable.</p></div><button class="primary" onclick={() => edit(undefined)}>New profile</button></div>
            <label>Default connection for new feeds<select value={defaultProxyId} onchange={(event) => selectDefaultProxy(event.currentTarget.value)} disabled={defaultProxyBusy} aria-busy={defaultProxyBusy}>{#each proxies as proxy}<option value={proxy.id}>{proxy.name}</option>{/each}</select></label>
            <p class="setting-note">Preselected for new subscriptions and imports. Existing feeds keep their assigned connection.</p>
            {#if defaultProxyError}<p class="error" role="alert">{defaultProxyError}</p>{/if}
            <div class="profiles">{#each proxies as proxy}<button onclick={() => edit(proxy)}><b>{proxy.name}</b><span>{proxy.kind === "direct" ? "No proxy" : proxy.endpoint}</span><em>{proxy.remoteDns ? "Remote DNS" : ""}</em></button>{/each}</div>
            {#if editing !== undefined || name !== ""}
              <form onsubmit={(event) => { event.preventDefault(); save(); }}>
                <label>Name<input bind:value={name} required /></label>
                <div class="row"><label>Type<select bind:value={kind}><option value="http">HTTP</option><option value="socks5">SOCKS5 / Tor</option><option value="direct">Direct</option></select></label>{#if kind !== "direct"}<label>Endpoint<input bind:value={endpoint} placeholder="socks5h://127.0.0.1:9050" required /></label>{/if}</div>
                {#if kind !== "direct"}<div class="row"><label>Username<input bind:value={username} /></label><label>Password<input type="password" bind:value={password} placeholder="Unchanged if empty" /></label></div><label class="check"><input type="checkbox" bind:checked={remoteDns} /> Resolve hostnames through the proxy</label>{/if}
                {#if error}<p class="error" role="alert">{error}</p>{/if}<div class="form-actions"><button type="button" class="secondary" onclick={() => edit()}>Cancel</button><button class="primary" disabled={busy} aria-busy={busy}>{busy ? "Saving…" : "Save profile"}</button></div>
              </form>
            {/if}
          </div>
        {:else if section === "external-links"}
          <div role="tabpanel" id="settings-panel-external-links" aria-labelledby="settings-tab-external-links" tabindex="0">
            <div class="heading"><div><h3>External links</h3><p>Choose which browser opens article links outside Veilfeed.</p></div></div>
            <div class="theme-options external-link-options" role="radiogroup" aria-label="External link browser">{#each [["default", "System default", "Use your default browser"], ["tor", "Tor Browser", "Use Tor Browser's own Tor connection"]] as option}<label class="option-card" class:active={externalBrowser === option[0]}><input type="radio" name="external-browser" value={option[0]} checked={externalBrowser === option[0]} onchange={() => selectExternalBrowser(option[0] as ExternalBrowser)}/><span><b>{option[1]}</b><small>{option[2]}</small></span><i aria-hidden="true">{externalBrowser === option[0] ? "✓" : ""}</i></label>{/each}</div>
            <p class="setting-note">Tor Browser must be installed separately. Browser traffic does not use the connection profile assigned to the feed.</p>
          </div>
        {:else if section === "notifications"}
          <div role="tabpanel" id="settings-panel-notifications" aria-labelledby="settings-tab-notifications" tabindex="0">
            <div class="heading"><div><h3>New post notifications</h3><p>Get a native desktop alert when a refresh finds new posts.</p></div></div>
            <div class="setting-card"><div><b>Notify me about new posts</b></div><button class:enabled={notificationEnabled} class="toggle" role="switch" aria-checked={notificationEnabled} aria-label="Notify me about new posts" disabled={notificationBusy} aria-busy={notificationBusy} onclick={toggleNotifications}><span></span></button></div>
            {#if notificationError}<p class="error" role="alert">{notificationError}</p>{/if}<p class="setting-note">Alerts are grouped into one notification per refresh, even when several subscriptions publish at once.</p>
          </div>
        {:else if section === "updates"}
          <div role="tabpanel" id="settings-panel-updates" aria-labelledby="settings-tab-updates" tabindex="0">
            <div class="heading"><div><h3>Application updates</h3><p>Check GitHub for new Veilfeed releases without downloading or installing them.</p></div><button class="secondary" onclick={checkUpdates} disabled={updateBusy} aria-busy={updateBusy}>{updateBusy ? "Checking…" : "Check now"}</button></div>
            {#if currentVersion || displayedRelease?.currentVersion}<p class="setting-note">Current version: {currentVersion || displayedRelease?.currentVersion}</p>{/if}
            <div class="setting-card"><div><b>Check automatically</b><span>At most once every 24 hours when Veilfeed starts.</span></div><button class:enabled={updateChecksEnabled} class="toggle" role="switch" aria-checked={updateChecksEnabled} aria-label="Check automatically for Veilfeed updates" onclick={toggleUpdateChecks}><span></span></button></div>
            <p class="setting-note">Disabled by default. Checks use the default connection profile and send the current Veilfeed version to GitHub in the request’s user agent.</p>
            {#if updateError}<p class="error" role="alert">{updateError}</p>{/if}
            {#if displayedRelease?.available}
              <div class="section-action update-result" role="status"><div><h3>{displayedRelease.available.title}</h3><p>Version {displayedRelease.available.version} is available.</p></div><button class="primary" onclick={() => onopenrelease(displayedRelease!.available!.releaseUrl)}>View release</button></div>
            {:else if displayedRelease}
              <p class="success" role="status">Veilfeed {displayedRelease.currentVersion} is up to date.</p>
            {/if}
            {#if checkedAt}<p class="setting-note">Last checked {new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(checkedAt))}.</p>{/if}
          </div>
        {:else if section === "appearance"}
          <div role="tabpanel" id="settings-panel-appearance" aria-labelledby="settings-tab-appearance" tabindex="0">
            <div class="heading"><div><h3>Appearance</h3><p>Choose how Veilfeed looks. System follows your macOS appearance.</p></div></div>
            <h4>Theme</h4>
            <div class="theme-options" role="radiogroup" aria-label="Theme">{#each [["system", "System", "Follows macOS"], ["light", "Light", "Always use the light theme"], ["dark", "Dark", "Always use the dark theme"]] as option}<label class="option-card" class:active={theme === option[0]}><input type="radio" name="theme" value={option[0]} checked={theme === option[0]} onchange={() => selectTheme(option[0] as Theme)}/><span class="theme-swatch {option[0]}" aria-hidden="true"></span><span><b>{option[1]}</b><small>{option[2]}</small></span><i aria-hidden="true">{theme === option[0] ? "✓" : ""}</i></label>{/each}</div>
            <h4>Reading font</h4>
            <div class="theme-options font-options" role="radiogroup" aria-label="Reading font">{#each [["serif", "Serif", "Classic, book-like typography"], ["sans-serif", "Sans-serif", "Clean, modern typography"], ["opendyslexic", "OpenDyslexic", "Designed for readers with dyslexia"]] as option}<label class="option-card" class:active={readerFont === option[0]}><input type="radio" name="reader-font" value={option[0]} checked={readerFont === option[0]} onchange={() => selectReaderFont(option[0] as ReaderFont)}/><span class="font-sample {option[0]}" aria-hidden="true">Aa</span><span><b>{option[1]}</b><small>{option[2]}</small></span><i aria-hidden="true">{readerFont === option[0] ? "✓" : ""}</i></label>{/each}</div>
          </div>
        {:else}
          <div role="tabpanel" id="settings-panel-opml" aria-labelledby="settings-tab-opml" tabindex="0">
            <div class="heading"><div><h3>Import subscriptions</h3><p>Choose an OPML file exported from another feed reader.</p></div></div>
            <label>OPML file<input type="file" accept=".opml,.xml,text/xml" onchange={pickOpml}/></label>
            {#if opmlPreview}
              <div class="preview" role="status"><b>{opmlPreview.entries.length} feeds ready to import</b>{#if opmlPreview.warnings.length}<span>{opmlPreview.warnings.length} warnings</span>{/if}</div>
              <label>Connection for imported feeds<select bind:value={opmlProxyId}>{#each proxies as proxy}<option value={proxy.id}>{proxy.name}</option>{/each}</select></label>
              <div class="form-actions"><button class="primary" onclick={importFeeds} disabled={opmlBusy} aria-busy={opmlBusy}>{opmlBusy ? "Importing…" : "Import feeds"}</button></div>
            {/if}
            {#if opmlMessage}<p class="success" role="status">{opmlMessage}</p>{/if}
            {#if opmlError}<p class="error pre" role="alert">{opmlError}</p>{/if}
            <hr/>
            <div class="section-action"><div><h3>Export subscriptions</h3><p>Download all folders and feeds as an OPML 2.0 file.</p></div><button class="secondary" onclick={exportFeeds} disabled={opmlBusy} aria-busy={opmlBusy}>{opmlBusy ? "Preparing…" : "Export OPML"}</button></div>
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>
