import { assertEquals } from "@std/assert";
import { normalizeReaderFont } from "./readerFont.ts";

Deno.test("normalizeReaderFont accepts supported values", () => {
  assertEquals(normalizeReaderFont("serif"), "serif");
  assertEquals(normalizeReaderFont("sans-serif"), "sans-serif");
  assertEquals(normalizeReaderFont("opendyslexic"), "opendyslexic");
});

Deno.test("normalizeReaderFont falls back to serif", () => {
  assertEquals(normalizeReaderFont(null), "serif");
  assertEquals(normalizeReaderFont("monospace"), "serif");
});
