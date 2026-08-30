import { assertEquals } from "@std/assert";
import { normalizeTheme } from "./theme.ts";

Deno.test("normalizeTheme accepts supported values", () => {
  assertEquals(normalizeTheme("system"), "system");
  assertEquals(normalizeTheme("light"), "light");
  assertEquals(normalizeTheme("dark"), "dark");
});

Deno.test("normalizeTheme falls back to system", () => {
  assertEquals(normalizeTheme(null), "system");
  assertEquals(normalizeTheme("sepia"), "system");
});
