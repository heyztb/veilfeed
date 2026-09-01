import { invoke } from "@tauri-apps/api/core";
import type {
  ArticleDetail,
  ArticlePage,
  ArticleQuery,
  ImportResult,
  OpmlPreview,
  ProxyProfile,
  RefreshResult,
  ReleaseCheck,
  SidebarData,
  SubscribeInput,
  SubscribeResult,
} from "./types";

async function call<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    if (typeof error === "object" && error && "message" in error) throw error;
    throw { code: "unknown_error", message: String(error) };
  }
}

export const api = {
  appVersion: () => call<string>("app_version"),
  sidebar: () => call<SidebarData>("get_sidebar"),
  articles: (query: ArticleQuery) =>
    call<ArticlePage>("list_articles", { query }),
  article: (id: string) => call<ArticleDetail>("get_article", { id }),
  setArticleState: (ids: string[], unread?: boolean, starred?: boolean) =>
    call<void>("set_article_state", { ids, unread, starred }),
  markAllRead: (feedId?: string, folderId?: string, scope?: string) =>
    call<void>("mark_all_read", { feedId, folderId, scope }),
  createFolder: (name: string, parentId?: string, feedId?: string) =>
    call<string>("create_folder", { name, parentId, feedId }),
  updateFolder: (id: string, name: string) =>
    call<void>("update_folder", { id, name }),
  deleteFolder: (id: string) => call<void>("delete_folder", { id }),
  updateFeed: (
    id: string,
    title: string,
    feedUrl: string,
    folderId: string | undefined,
    proxyProfileId: string | undefined,
    refreshIntervalMinutes: number,
    blockAds: boolean,
  ) =>
    call<void>("update_feed", {
      input: {
        id,
        title,
        feedUrl,
        folderId,
        proxyProfileId,
        refreshIntervalMinutes,
        blockAds,
      },
    }),
  moveFeed: (id: string, folderId?: string) =>
    call<void>("move_feed", { id, folderId }),
  deleteFeed: (id: string) => call<void>("delete_feed", { id }),
  proxies: () => call<ProxyProfile[]>("list_proxies"),
  defaultProxyProfile: () => call<string>("get_default_proxy_profile"),
  setDefaultProxyProfile: (id: string) =>
    call<void>("set_default_proxy_profile", { id }),
  saveProxy: (input: Record<string, unknown>) =>
    call<string>("save_proxy", { input }),
  testProxy: (id: string) => call<void>("test_proxy", { id }),
  discover: (url: string, proxyProfileId?: string) =>
    call<string[]>("discover_feed", { url, proxyProfileId }),
  subscribe: (input: SubscribeInput) =>
    call<SubscribeResult>("subscribe", { input }),
  refresh: (feedId?: string, dueOnly = false) =>
    call<RefreshResult>("refresh_feeds", { feedId, dueOnly }),
  fullContent: (articleId: string) =>
    call<string>("fetch_full_content", { articleId }),
  previewOpml: (xml: string) => call<OpmlPreview>("preview_opml", { xml }),
  importOpml: (entries: OpmlPreview["entries"], proxyProfileId?: string) =>
    call<ImportResult>("import_opml", { input: { entries, proxyProfileId } }),
  exportOpml: () => call<string>("export_opml"),
  checkForUpdate: () => call<ReleaseCheck>("check_for_update"),
};
