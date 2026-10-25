/** The thumbnail size range, as the cell width the grid aims for, in CSS pixels. */
export const CELL_WIDTH = { min: 100, max: 360, initial: 180 };

/** Whole pixels within the range; also for a saved width from an older range. */
export function clampCellWidth(width: number): number {
  return Math.round(Math.min(CELL_WIDTH.max, Math.max(CELL_WIDTH.min, width)));
}

/** The whole page's zoom steps in percent, for large or high-density screens. */
export const PAGE_ZOOM = { levels: [80, 90, 100, 110, 125, 150, 175, 200], min: 80, max: 200, initial: 100 };

export function clampPageZoom(zoom: number): number {
  return Math.min(PAGE_ZOOM.max, Math.max(PAGE_ZOOM.min, zoom));
}

/** The next step in `direction`, from a saved zoom that may lie between the steps. */
export function stepPageZoom(zoom: number, direction: 1 | -1): number {
  const ahead = PAGE_ZOOM.levels.filter((level) => (direction > 0 ? level > zoom : level < zoom));
  const next = direction > 0 ? ahead[0] : ahead[ahead.length - 1];
  return next ?? (direction > 0 ? PAGE_ZOOM.max : PAGE_ZOOM.min);
}

/** From this cell width on, as it shows on screen, thumbnails use the sample instead of the preview. */
const LARGE_CELL_WIDTH = 240;

/** The page zoom draws a cell larger than its CSS width, so the preview turns soft sooner. */
export function isLargeCell(cellWidth: number, pageZoom: number): boolean {
  return (cellWidth * pageZoom) / 100 >= LARGE_CELL_WIDTH;
}

/** Cells are 3:4 portrait, since most e621 art is taller than wide and gets cropped less so. */
export const CELL_ASPECT = 3 / 4;

export type Grid = {
  columns: number;
  cellWidth: number;
  cellHeight: number;
  gap: number;
  rows: number;
  height: number;
};

/**
 * Equal cells filling `width`: as many columns as fit at about `targetWidth` each, then the
 * cells are widened to close the leftover space.
 */
export function grid(count: number, width: number, targetWidth: number, gap: number): Grid {
  const columns = width > 0 && targetWidth > 0 ? Math.max(1, Math.round((width + gap) / (targetWidth + gap))) : 0;
  const cellWidth = columns > 0 ? (width - gap * (columns - 1)) / columns : 0;
  const cellHeight = cellWidth / CELL_ASPECT;
  const rows = columns > 0 ? Math.ceil(count / columns) : 0;
  return { columns, cellWidth, cellHeight, gap, rows, height: rows > 0 ? rows * (cellHeight + gap) - gap : 0 };
}

export function cellPosition(layout: Grid, index: number): { x: number; y: number } {
  const column = index % layout.columns;
  const row = Math.floor(index / layout.columns);
  return { x: column * (layout.cellWidth + layout.gap), y: row * (layout.cellHeight + layout.gap) };
}

/** Indices of the items in rows that overlap `top` to `bottom`, as `[start, end)`. */
export function visibleRange(layout: Grid, count: number, top: number, bottom: number): [number, number] {
  if (layout.columns === 0) return [0, 0];
  const pitch = layout.cellHeight + layout.gap;
  const firstRow = Math.max(0, Math.floor(top / pitch));
  const lastRow = Math.max(0, Math.ceil(bottom / pitch));
  return [Math.min(count, firstRow * layout.columns), Math.min(count, lastRow * layout.columns)];
}

/** The row the item at `index` is in, for keeping it in view across a resize. */
export function rowOf(layout: Grid, index: number): number {
  return layout.columns > 0 ? Math.floor(index / layout.columns) : 0;
}
