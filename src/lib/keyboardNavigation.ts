export const panes = ["subscriptions", "articles", "reader"] as const;

export type Pane = (typeof panes)[number];

const standardNavigationKeys = [
  "Tab",
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "Home",
  "End",
  "PageUp",
  "PageDown",
] as const;

export function isPaneNavigationKey(
  key: string,
  vimNavigationEnabled: boolean,
): boolean {
  return standardNavigationKeys.includes(
    key as (typeof standardNavigationKeys)[number],
  ) || (vimNavigationEnabled && ["h", "j", "k", "l"].includes(key));
}

export function adjacentPane(pane: Pane, key: "h" | "l"): Pane {
  const direction = key === "l" ? 1 : -1;
  const index = panes.indexOf(pane);
  return panes[Math.max(0, Math.min(panes.length - 1, index + direction))];
}

export function adjacentIndex(
  length: number,
  current: number,
  key: "j" | "k",
): number | undefined {
  if (length === 0) return undefined;
  const startingIndex = current >= 0 ? current : key === "j" ? -1 : 0;
  const direction = key === "j" ? 1 : -1;
  return Math.max(0, Math.min(length - 1, startingIndex + direction));
}
