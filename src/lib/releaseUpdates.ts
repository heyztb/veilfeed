import type { ReleaseCheck } from "./types";

interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

const enabledKey = "veilfeed.updateChecksEnabled";
const lastAttemptKey = "veilfeed.lastUpdateCheckAt";
const dismissedVersionKey = "veilfeed.dismissedReleaseVersion";
export const automaticCheckIntervalMs = 24 * 60 * 60 * 1000;

function storage(): StorageLike | undefined {
  return globalThis.localStorage;
}

export function automaticUpdateChecksEnabled(
  target = storage(),
): boolean {
  return target?.getItem(enabledKey) === "true";
}

export function setAutomaticUpdateChecksEnabled(
  enabled: boolean,
  target = storage(),
): void {
  target?.setItem(enabledKey, String(enabled));
}

export function lastUpdateCheckAt(target = storage()): number | undefined {
  const value = Number(target?.getItem(lastAttemptKey));
  return Number.isFinite(value) && value > 0 ? value : undefined;
}

export function markUpdateCheckAttempt(
  now = Date.now(),
  target = storage(),
): void {
  target?.setItem(lastAttemptKey, String(now));
}

export function automaticUpdateCheckDue(
  now = Date.now(),
  target = storage(),
): boolean {
  if (!automaticUpdateChecksEnabled(target)) return false;
  const previous = lastUpdateCheckAt(target);
  return previous === undefined || now - previous >= automaticCheckIntervalMs;
}

export function dismissedReleaseVersion(
  target = storage(),
): string | undefined {
  return target?.getItem(dismissedVersionKey) ?? undefined;
}

export function dismissRelease(version: string, target = storage()): void {
  target?.setItem(dismissedVersionKey, version);
}

export function shouldShowRelease(
  result: ReleaseCheck | undefined,
  dismissed: string | undefined,
): boolean {
  return !!result?.available && result.available.version !== dismissed;
}
