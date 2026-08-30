import { assertEquals } from "@std/assert";
import { notificationBody } from "./notifications.ts";

Deno.test("formats a notification for one subscription", () => {
  assertEquals(
    notificationBody({
      refreshed: 1,
      newArticles: 1,
      newArticleFeeds: [{ feedTitle: "Example", count: 1 }],
      errors: [],
    }),
    "1 new post from Example",
  );
});

Deno.test("summarizes notifications across subscriptions", () => {
  assertEquals(
    notificationBody({
      refreshed: 4,
      newArticles: 7,
      newArticleFeeds: [
        { feedTitle: "One", count: 1 },
        { feedTitle: "Two", count: 2 },
        { feedTitle: "Three", count: 3 },
        { feedTitle: "Four", count: 1 },
      ],
      errors: [],
    }),
    "7 new posts from 4 subscriptions: One, Two, Three, and 1 more",
  );
});
