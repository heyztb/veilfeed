import { assertEquals } from "@std/assert";
import {
  adjacentIndex,
  adjacentPane,
  isPaneNavigationKey,
  panes,
} from "./keyboardNavigation.ts";

Deno.test("pane navigation is spatial and bounded", () => {
  assertEquals(panes, ["subscriptions", "articles", "reader"]);
  assertEquals(adjacentPane("subscriptions", "h"), "subscriptions");
  assertEquals(adjacentPane("subscriptions", "l"), "articles");
  assertEquals(adjacentPane("articles", "h"), "subscriptions");
  assertEquals(adjacentPane("articles", "l"), "reader");
  assertEquals(adjacentPane("reader", "l"), "reader");
});

Deno.test("pane indicator follows standard navigation and enabled Vim keys", () => {
  assertEquals(isPaneNavigationKey("Tab", false), true);
  assertEquals(isPaneNavigationKey("ArrowDown", false), true);
  assertEquals(isPaneNavigationKey("PageDown", false), true);
  assertEquals(isPaneNavigationKey("j", false), false);
  assertEquals(isPaneNavigationKey("j", true), true);
  assertEquals(isPaneNavigationKey("s", true), false);
});

Deno.test("item navigation moves one place and stops at list boundaries", () => {
  assertEquals(adjacentIndex(4, 1, "j"), 2);
  assertEquals(adjacentIndex(4, 1, "k"), 0);
  assertEquals(adjacentIndex(4, 3, "j"), 3);
  assertEquals(adjacentIndex(4, 0, "k"), 0);
  assertEquals(adjacentIndex(4, -1, "j"), 0);
  assertEquals(adjacentIndex(0, -1, "j"), undefined);
});
