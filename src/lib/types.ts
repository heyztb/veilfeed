export interface ApiError {
  code: string;
  message: string;
}
export interface ProxyProfile {
  id: string;
  name: string;
  kind: "direct" | "http" | "socks5";
  endpoint?: string;
  username?: string;
  remoteDns: boolean;
}
export interface FolderSummary {
  id: string;
  parentId?: string;
  name: string;
  sortOrder: number;
  unreadCount: number;
}
export interface FeedSummary {
  id: string;
  folderId?: string;
  proxyProfileId?: string;
  title: string;
  feedUrl: string;
  siteUrl?: string;
  iconUrl?: string;
  blockAds: boolean;
  refreshIntervalMinutes: number;
  unreadCount: number;
  lastErrorCode?: string;
  lastErrorMessage?: string;
  lastCheckedAt?: string;
}
export interface SidebarData {
  folders: FolderSummary[];
  feeds: FeedSummary[];
  allCount: number;
  unreadCount: number;
  starredCount: number;
}
export type Scope = "all" | "unread" | "starred" | "feed" | "folder";
export interface ArticleQuery {
  scope?: Scope;
  feedId?: string;
  folderId?: string;
  search?: string;
  cursor?: string;
  limit?: number;
}
export interface ArticleSummary {
  id: string;
  feedId: string;
  feedTitle: string;
  title: string;
  author?: string;
  publishedAt?: string;
  receivedAt: string;
  isUnread: boolean;
  isStarred: boolean;
  excerpt: string;
}
export interface ArticleDetail {
  id: string;
  feedId: string;
  feedTitle: string;
  title: string;
  author?: string;
  canonicalUrl?: string;
  publishedAt?: string;
  contentHtml: string;
  fullContentHtml?: string;
  isUnread: boolean;
  isStarred: boolean;
}
export interface ArticlePage {
  items: ArticleSummary[];
  nextCursor?: string;
}
export interface SubscribeInput {
  url: string;
  folderId?: string;
  proxyProfileId?: string;
}
export interface SubscribeResult {
  feedId: string;
  title: string;
  articleCount: number;
}
export interface OpmlEntry {
  title: string;
  feedUrl: string;
  siteUrl?: string;
  folderPath: string[];
}
export interface OpmlPreview {
  entries: OpmlEntry[];
  warnings: string[];
}
export interface ImportResult {
  imported: number;
  skipped: number;
  errors: string[];
}

export interface AvailableRelease {
  version: string;
  title: string;
  releaseUrl: string;
  publishedAt?: string;
}

export interface ReleaseCheck {
  currentVersion: string;
  available?: AvailableRelease;
}
export interface RefreshResult {
  refreshed: number;
  newArticles: number;
  newArticleFeeds: Array<{ feedTitle: string; count: number }>;
  errors: string[];
}
