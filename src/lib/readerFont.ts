export const readerFonts = ["serif", "sans-serif", "opendyslexic"] as const;

export type ReaderFont = (typeof readerFonts)[number];

const storageKey = "veilfeed.readerFont";

export function normalizeReaderFont(value: string | null): ReaderFont {
  return readerFonts.includes(value as ReaderFont) ? value as ReaderFont : "serif";
}

export function getReaderFont(): ReaderFont {
  return normalizeReaderFont(globalThis.localStorage?.getItem(storageKey) ?? null);
}

export function applyReaderFont(font: ReaderFont): void {
  document.documentElement.dataset.readerFont = font;
  globalThis.localStorage?.setItem(storageKey, font);
}
