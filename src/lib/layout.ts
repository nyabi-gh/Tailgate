/** The thumbnail size range, as the cell width the grid aims for, in CSS pixels. */
export const CELL_WIDTH = { min: 100, max: 360, initial: 180 };

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
