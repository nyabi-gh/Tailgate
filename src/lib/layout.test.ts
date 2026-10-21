import { describe, expect, it } from "vitest";
import { CELL_ASPECT, CELL_WIDTH, cellPosition, clampCellWidth, grid, rowOf, visibleRange } from "./layout";

describe("grid", () => {
  it("fits as many columns as the target width allows and fills the width exactly", () => {
    const layout = grid(10, 1000, 180, 3);
    expect(layout.columns).toBe(5);
    expect(layout.columns * layout.cellWidth + (layout.columns - 1) * 3).toBeCloseTo(1000, 6);
  });

  it("makes every cell 3:4", () => {
    const layout = grid(10, 1000, 180, 3);
    expect(layout.cellWidth / layout.cellHeight).toBeCloseTo(CELL_ASPECT, 6);
  });

  it("keeps at least one column in a narrow view", () => {
    expect(grid(3, 80, 180, 3).columns).toBe(1);
  });

  it("counts rows and total height, the last row included when partly filled", () => {
    const layout = grid(11, 1000, 180, 3);
    expect(layout.rows).toBe(3);
    expect(layout.height).toBeCloseTo(3 * layout.cellHeight + 2 * 3, 6);
  });

  it("gives an empty grid for no items or no room", () => {
    expect(grid(0, 1000, 180, 3).height).toBe(0);
    expect(grid(5, 0, 180, 3)).toMatchObject({ columns: 0, rows: 0, height: 0 });
  });
});

describe("cellPosition", () => {
  it("places items left to right, then row by row, with the gap between", () => {
    const layout = grid(10, 1000, 180, 3);
    expect(cellPosition(layout, 0)).toEqual({ x: 0, y: 0 });
    expect(cellPosition(layout, 1).x).toBeCloseTo(layout.cellWidth + 3, 6);
    expect(cellPosition(layout, 5)).toEqual({ x: 0, y: layout.cellHeight + 3 });
  });
});

describe("visibleRange", () => {
  const layout = grid(100, 1000, 180, 3);
  const pitch = layout.cellHeight + 3;

  it("covers whole rows overlapping the view", () => {
    expect(visibleRange(layout, 100, 0, pitch * 2)).toEqual([0, 10]);
    expect(visibleRange(layout, 100, pitch * 3 + 1, pitch * 4 + 1)).toEqual([15, 25]);
  });

  it("stays within the items", () => {
    expect(visibleRange(layout, 100, -500, pitch)).toEqual([0, 5]);
    expect(visibleRange(layout, 100, pitch * 18, pitch * 40)).toEqual([90, 100]);
  });

  it("finds the row of an item", () => {
    expect(rowOf(layout, 0)).toBe(0);
    expect(rowOf(layout, 7)).toBe(1);
  });
});

describe("clampCellWidth", () => {
  it("keeps widths within the range in whole pixels", () => {
    expect(clampCellWidth(240.4)).toBe(240);
    expect(clampCellWidth(CELL_WIDTH.min - 50)).toBe(CELL_WIDTH.min);
    expect(clampCellWidth(CELL_WIDTH.max + 50)).toBe(CELL_WIDTH.max);
  });
});
