import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import type { RefreshResult } from "./types";

const ENABLED_KEY = "veilfeed.notifications.enabled";

export function notificationsEnabled(): boolean {
  return localStorage.getItem(ENABLED_KEY) === "true";
}

export function disableNotifications(): void {
  localStorage.setItem(ENABLED_KEY, "false");
}

export async function enableNotifications(): Promise<boolean> {
  const granted = (await isPermissionGranted()) ||
    (await requestPermission()) === "granted";
  localStorage.setItem(ENABLED_KEY, String(granted));
  return granted;
}

export function notificationBody(result: RefreshResult): string {
  const feeds = result.newArticleFeeds;
  if (feeds.length === 1) {
    const [{ feedTitle, count }] = feeds;
    return `${count} new ${count === 1 ? "post" : "posts"} from ${feedTitle}`;
  }

  const names = feeds.slice(0, 3).map(({ feedTitle }) => feedTitle).join(", ");
  const remaining = feeds.length - 3;
  const sources = remaining > 0 ? `${names}, and ${remaining} more` : names;
  return `${result.newArticles} new posts from ${feeds.length} subscriptions: ${sources}`;
}

export async function notifyForRefresh(result: RefreshResult): Promise<void> {
  if (!notificationsEnabled() || result.newArticles === 0) return;
  try {
    if (!(await isPermissionGranted())) return;
    sendNotification({
      title: "New in Veilfeed",
      body: notificationBody(result),
    });
  } catch {
    // A notification failure must not turn a successful feed refresh into an error.
  }
}
