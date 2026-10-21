import { describe, expect, it } from "vitest";
import { BlacklistMarks } from "./blacklistMarks";

describe("BlacklistMarks", () => {
  it("collects the marks of each page", () => {
    const marks = new BlacklistMarks();
    marks.addPage(1, [1, 2]);
    marks.addPage(1, [150]);
    expect([...marks.ids]).toEqual([1, 2, 150]);
  });

  it("replaces the marks of loaded posts with a refresh", () => {
    const marks = new BlacklistMarks();
    marks.addPage(1, [1, 2]);
    expect(marks.applyRefresh(2, [3])).toBe(true);
    expect([...marks.ids]).toEqual([3]);
  });

  it("keeps a page marked after the refresh that arrived before it", () => {
    const marks = new BlacklistMarks();
    marks.addPage(1, [1]);
    marks.addPage(2, [150]);
    marks.applyRefresh(2, [2]);
    expect([...marks.ids].sort((a, b) => a - b)).toEqual([2, 150]);
  });

  it("drops a page marked before the refresh that arrived after it", () => {
    const marks = new BlacklistMarks();
    marks.addPage(1, [1]);
    marks.applyRefresh(2, [2, 3]);
    expect(marks.addPage(1, [3, 4])).toBe(false);
    expect([...marks.ids]).toEqual([2, 3]);
  });

  it("ignores a refresh older than the marks", () => {
    const marks = new BlacklistMarks();
    marks.addPage(1, [1]);
    marks.applyRefresh(3, [5]);
    expect(marks.applyRefresh(2, [9])).toBe(false);
    expect([...marks.ids]).toEqual([5]);
  });
});
