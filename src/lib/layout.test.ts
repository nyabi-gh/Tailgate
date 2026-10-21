import { describe, expect, it } from "vitest";
import { clampAspect, firstRowBelow, justify } from "./layout";

function at<T>(list: T[], index: number): T {
  const item = list.at(index);
  if (item === undefined) throw new Error(`no item at ${index}`);
  return item;
}

const rowWidth = (row: { tiles: { x: number; width: number }[] }) => {
  const last = at(row.tiles, -1);
  return last.x + last.width;
};

describe("justify", () => {
  it("fills every full row to the exact width", () => {
    const aspects = [1, 1.5, 0.7, 2, 1, 1, 0.8, 1.3, 1.6, 0.9];
    const { rows } = justify(aspects, 1000, 200, 3);
    for (const row of rows.slice(0, -1)) expect(rowWidth(row)).toBeCloseTo(1000, 6);
  });

  it("keeps the row heights near the target", () => {
    const { rows } = justify(Array(40).fill(1.2), 1000, 200, 3);
    for (const row of rows.slice(0, -1)) {
      expect(row.height).toBeGreaterThan(150);
      expect(row.height).toBeLessThanOrEqual(200);
    }
  });

  it("leaves the last row at the target height and narrower than the width", () => {
    const { rows } = justify([1, 1, 1, 1, 1, 1], 1000, 200, 3);
    const last = at(rows, -1);
    expect(last.height).toBe(200);
    expect(rowWidth(last)).toBeLessThan(1000);
  });

  it("places every item once, in order, with the gap between them", () => {
    const aspects = Array.from({ length: 23 }, (_, index) => 0.6 + (index % 5) * 0.4);
    const { rows, height } = justify(aspects, 900, 180, 3);
    const indices = rows.flatMap((row) => row.tiles.map((tile) => tile.index));
    expect(indices).toEqual(aspects.map((_, index) => index));
    for (const row of rows) {
      row.tiles.slice(1).forEach((tile, i) => {
        const previous = at(row.tiles, i);
        expect(tile.x).toBeCloseTo(previous.x + previous.width + 3, 6);
      });
    }
    const last = at(rows, -1);
    expect(height).toBeCloseTo(last.y + last.height, 6);
    rows.slice(1).forEach((row, i) => expect(row.y).toBeCloseTo(at(rows, i).y + at(rows, i).height + 3, 6));
  });

  it("clamps extreme aspect ratios", () => {
    expect(clampAspect(20)).toBe(3);
    expect(clampAspect(0.05)).toBe(0.5);
    expect(clampAspect(0)).toBe(1);
    expect(clampAspect(Number.NaN)).toBe(1);
    const { rows } = justify([20], 1000, 200, 3);
    expect(at(at(rows, 0).tiles, 0).width).toBe(600);
  });

  it("gives an empty layout for no items or no room", () => {
    expect(justify([], 1000, 200, 3)).toEqual({ rows: [], height: 0 });
    expect(justify([1, 1], 0, 200, 3)).toEqual({ rows: [], height: 0 });
  });
});

describe("firstRowBelow", () => {
  const { rows } = justify(Array(30).fill(1), 1000, 200, 3);

  it("finds the first row still visible at a scroll position", () => {
    expect(firstRowBelow(rows, 0)).toBe(0);
    const third = at(rows, 2);
    expect(firstRowBelow(rows, third.y + 1)).toBe(2);
    expect(firstRowBelow(rows, third.y + third.height + 1)).toBe(3);
    expect(firstRowBelow(rows, 1e9)).toBe(rows.length);
  });
});
