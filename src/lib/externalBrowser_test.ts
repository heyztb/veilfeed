import { assertEquals } from "@std/assert";
import { normalizeExternalBrowser } from "./externalBrowser.ts";

Deno.test("normalizeExternalBrowser accepts supported values", () => {
  assertEquals(normalizeExternalBrowser("default"), "default");
  assertEquals(normalizeExternalBrowser("tor"), "tor");
});

Deno.test("normalizeExternalBrowser falls back to the system default", () => {
  assertEquals(normalizeExternalBrowser(null), "default");
  assertEquals(normalizeExternalBrowser("firefox"), "default");
});
