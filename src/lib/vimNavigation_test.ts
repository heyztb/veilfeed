import { assertEquals } from "@std/assert";
import { normalizeVimNavigation } from "./vimNavigation.ts";

Deno.test("Vim navigation is enabled only by an explicit true preference", () => {
  assertEquals(normalizeVimNavigation("true"), true);
  assertEquals(normalizeVimNavigation("false"), false);
  assertEquals(normalizeVimNavigation("enabled"), false);
  assertEquals(normalizeVimNavigation(null), false);
});
