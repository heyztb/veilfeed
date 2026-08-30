export const externalBrowsers = ["default", "tor"] as const;

export type ExternalBrowser = (typeof externalBrowsers)[number];

const storageKey = "veilfeed.externalBrowser";

export function normalizeExternalBrowser(value: string | null): ExternalBrowser {
  return externalBrowsers.includes(value as ExternalBrowser)
    ? value as ExternalBrowser
    : "default";
}

export function getExternalBrowser(): ExternalBrowser {
  return normalizeExternalBrowser(
    globalThis.localStorage?.getItem(storageKey) ?? null,
  );
}

export function setExternalBrowser(browser: ExternalBrowser): void {
  globalThis.localStorage?.setItem(storageKey, browser);
}
