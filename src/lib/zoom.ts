export type Size = { width: number; height: number };

export type Point = { x: number; y: number };

/** Where the picture's top-left corner sits in the stage, and how many CSS pixels per picture pixel. */
export type View = { scale: number; x: number; y: number };

/** How far past one picture pixel per CSS pixel zooming may go. */
const MAX_SCALE = 8;

/** The whole picture in view, never enlarged past its own size. */
export function fitScale(picture: Size, stage: Size): number {
  if (picture.width <= 0 || picture.height <= 0) return 1;
  return Math.min(1, stage.width / picture.width, stage.height / picture.height);
}

export function fit(picture: Size, stage: Size): View {
  return clamp({ scale: fitScale(picture, stage), x: 0, y: 0 }, picture, stage);
}

/** One picture pixel per CSS pixel, keeping the picture point under `at` where it was. */
export function actualSize(view: View, picture: Size, stage: Size, at: Point): View {
  return zoomAt(view, 1 / view.scale, at, picture, stage);
}

/** Scales by `factor` around `at`, so the picture point under it stays under it. */
export function zoomAt(view: View, factor: number, at: Point, picture: Size, stage: Size): View {
  const scale = Math.min(MAX_SCALE, Math.max(fitScale(picture, stage), view.scale * factor));
  const ratio = scale / view.scale;
  return clamp({ scale, x: at.x - (at.x - view.x) * ratio, y: at.y - (at.y - view.y) * ratio }, picture, stage);
}

export function pan(view: View, dx: number, dy: number, picture: Size, stage: Size): View {
  return clamp({ ...view, x: view.x + dx, y: view.y + dy }, picture, stage);
}

/** A picture smaller than the stage is centered; a larger one may not leave a gap at an edge. */
export function clamp(view: View, picture: Size, stage: Size): View {
  const axis = (position: number, length: number, room: number) =>
    length <= room ? (room - length) / 2 : Math.min(0, Math.max(room - length, position));
  return {
    scale: view.scale,
    x: axis(view.x, picture.width * view.scale, stage.width),
    y: axis(view.y, picture.height * view.scale, stage.height),
  };
}

export function isFitted(view: View, picture: Size, stage: Size): boolean {
  return Math.abs(view.scale - fitScale(picture, stage)) < 1e-6;
}
