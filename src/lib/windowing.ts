export const ARTICLE_ROW_HEIGHT = 84;
export const WINDOW_OVERSCAN = 5;

export function windowRange(
  itemCount: number,
  scrollTop: number,
  viewportHeight: number,
) {
  const start = Math.max(
    0,
    Math.floor(scrollTop / ARTICLE_ROW_HEIGHT) - WINDOW_OVERSCAN,
  );
  const visible = Math.ceil(viewportHeight / ARTICLE_ROW_HEIGHT) +
    WINDOW_OVERSCAN * 2;
  const end = Math.min(itemCount, start + visible);
  return {
    start,
    end,
    before: start * ARTICLE_ROW_HEIGHT,
    after: Math.max(0, (itemCount - end) * ARTICLE_ROW_HEIGHT),
  };
}
