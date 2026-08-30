import { assertEquals } from "@std/assert";
import { ARTICLE_ROW_HEIGHT, windowRange } from "./windowing.ts";

Deno.test("window range limits rendered rows", () => {
  assertEquals(
    windowRange(1000, ARTICLE_ROW_HEIGHT * 100, ARTICLE_ROW_HEIGHT * 10),
    {
      start: 95,
      end: 115,
      before: ARTICLE_ROW_HEIGHT * 95,
      after: ARTICLE_ROW_HEIGHT * 885,
    },
  );
});
