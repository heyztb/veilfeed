export const themes = ["system", "light", "dark"] as const;

export type Theme = (typeof themes)[number];

const storageKey = "veilfeed.theme";

export function normalizeTheme(value: string | null): Theme {
  return themes.includes(value as Theme) ? value as Theme : "system";
}

export function getTheme(): Theme {
  return normalizeTheme(globalThis.localStorage?.getItem(storageKey) ?? null);
}

export function applyTheme(theme: Theme): void {
  if (theme === "system") {
    document.documentElement.removeAttribute("data-theme");
  } else {
    document.documentElement.dataset.theme = theme;
  }
  globalThis.localStorage?.setItem(storageKey, theme);
}
