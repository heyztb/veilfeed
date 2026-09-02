const storageKey = "veilfeed.vimNavigation";

export function normalizeVimNavigation(value: string | null): boolean {
  return value === "true";
}

export function vimNavigationEnabled(): boolean {
  return normalizeVimNavigation(
    globalThis.localStorage?.getItem(storageKey) ?? null,
  );
}

export function setVimNavigationEnabled(enabled: boolean): void {
  globalThis.localStorage?.setItem(storageKey, String(enabled));
}
