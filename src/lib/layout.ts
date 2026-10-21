/** The thumbnail size range, as the target height of a row in CSS pixels. */
export const ROW_HEIGHT = { min: 120, max: 400, initial: 220 };

/** One placed item, by its index in the input. */
export type Tile = { index: number; x: number; width: number };

export type Row = { y: number; height: number; tiles: Tile[] };

export type Layout = { rows: Row[]; height: number };

/** Narrower or wider pictures are shown cropped to this range, so none takes over a row. */
const MIN_ASPECT = 0.5;
const MAX_ASPECT = 3;

export function clampAspect(aspect: number): number {
  if (!Number.isFinite(aspect) || aspect <= 0) return 1;
  return Math.min(MAX_ASPECT, Math.max(MIN_ASPECT, aspect));
}

/**
 * Justified rows: pictures keep their aspect ratio, each full row is scaled to exactly `width`,
 * and its height lands near `targetHeight`. The last row is left at the target height rather
 * than stretched, since more items may still arrive.
 */
export function justify(aspects: number[], width: number, targetHeight: number, gap: number): Layout {
  const rows: Row[] = [];
  if (width <= 0 || targetHeight <= 0) return { rows, height: 0 };

  let y = 0;
  let start = 0;
  let aspectSum = 0;
  const place = (end: number, height: number) => {
    const tiles: Tile[] = [];
    let x = 0;
    aspects.slice(start, end).forEach((aspect, offset) => {
      const tileWidth = clampAspect(aspect) * height;
      tiles.push({ index: start + offset, x, width: tileWidth });
      x += tileWidth + gap;
    });
    rows.push({ y, height, tiles });
    y += height + gap;
    start = end;
    aspectSum = 0;
  };

  aspects.forEach((aspect, index) => {
    aspectSum += clampAspect(aspect);
    const gaps = gap * (index - start);
    if (aspectSum * targetHeight + gaps >= width) place(index + 1, (width - gaps) / aspectSum);
  });
  if (start < aspects.length) place(aspects.length, targetHeight);

  return { rows, height: Math.max(0, y - gap) };
}

/** Index of the first row whose bottom is below `top`. */
export function firstRowBelow(rows: Row[], top: number): number {
  let low = 0;
  let high = rows.length;
  while (low < high) {
    const middle = (low + high) >> 1;
    const row = rows[middle];
    if (row !== undefined && row.y + row.height < top) low = middle + 1;
    else high = middle;
  }
  return low;
}
