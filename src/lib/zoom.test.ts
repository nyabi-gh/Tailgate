import { describe, expect, it } from "vitest";
import { actualSize, clamp, fit, fitScale, isFitted, pan, zoomAt } from "./zoom";

const stage = { width: 1000, height: 800 };
const tall = { width: 2000, height: 4000 };

describe("fit", () => {
  it("shows the whole picture, centered", () => {
    const view = fit(tall, stage);
    expect(view.scale).toBe(0.2);
    expect(view).toEqual({ scale: 0.2, x: 300, y: 0 });
  });

  it("does not enlarge a small picture", () => {
    expect(fitScale({ width: 300, height: 200 }, stage)).toBe(1);
    expect(fit({ width: 300, height: 200 }, stage)).toEqual({ scale: 1, x: 350, y: 300 });
  });
});

describe("zoomAt", () => {
  it("keeps the picture point under the cursor in place", () => {
    const start = fit(tall, stage);
    const at = { x: 500, y: 400 };
    const pictureX = (at.x - start.x) / start.scale;
    const pictureY = (at.y - start.y) / start.scale;
    const zoomed = zoomAt(start, 2, at, tall, stage);
    expect(zoomed.scale).toBeCloseTo(0.4, 6);
    expect((at.x - zoomed.x) / zoomed.scale).toBeCloseTo(pictureX, 6);
    expect((at.y - zoomed.y) / zoomed.scale).toBeCloseTo(pictureY, 6);
  });

  it("stays between the fitted size and eight times the picture size", () => {
    const start = fit(tall, stage);
    expect(zoomAt(start, 0.1, { x: 0, y: 0 }, tall, stage).scale).toBe(0.2);
    expect(zoomAt(start, 1000, { x: 0, y: 0 }, tall, stage).scale).toBe(8);
  });
});

describe("actualSize", () => {
  it("shows one picture pixel per CSS pixel", () => {
    const view = actualSize(fit(tall, stage), tall, stage, { x: 500, y: 400 });
    expect(view.scale).toBe(1);
    expect(isFitted(view, tall, stage)).toBe(false);
  });
});

describe("pan and clamp", () => {
  it("does not let a zoomed picture leave a gap at an edge", () => {
    const zoomed = actualSize(fit(tall, stage), tall, stage, { x: 500, y: 400 });
    expect(pan(zoomed, 1e6, 1e6, tall, stage)).toMatchObject({ x: 0, y: 0 });
    expect(pan(zoomed, -1e6, -1e6, tall, stage)).toMatchObject({ x: -1000, y: -3200 });
  });

  it("centers an axis where the picture is smaller than the stage", () => {
    expect(clamp({ scale: 0.2, x: -50, y: 0 }, tall, stage).x).toBe(300);
  });
});
